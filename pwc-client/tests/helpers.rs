//! TASK-CLIENT-005: credential store, `connect` and `pair`, waiting for an Operation, pagination.

mod common;

use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::fake_provider::{Fake, Reply, PROVIDER_ID};
use common::fixtures as fx;
use pwc_client::*;
use serde_json::json;

#[test]
fn idempotency_keys_are_distinct_ids() {
    let (a, b) = (idempotency_key(), idempotency_key());
    assert_ne!(a, b);
    common::schema::core("Id", &json!(a)).unwrap();
}

#[test]
fn file_store_keeps_credentials_private() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileCredentialStore::new(dir.path().join("creds"));
    assert_eq!(store.load("wallet-a").unwrap(), None);
    store.delete("wallet-a").unwrap();

    let first = Credential {
        provider_id: "wallet-a".into(),
        client_id: "cl-1".into(),
        client_secret: "11".repeat(32),
    };
    store.save(&first).unwrap();
    let file = dir.path().join("creds/wallet-a.json");
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(
        std::fs::metadata(dir.path().join("creds"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(store.load("wallet-a").unwrap(), Some(first.clone()));
    assert_eq!(store.load("wallet-b").unwrap(), None);

    let second = Credential {
        client_id: "cl-2".into(),
        ..first
    };
    store.save(&second).unwrap();
    assert_eq!(store.load("wallet-a").unwrap(), Some(second));
    store.delete("wallet-a").unwrap();
    assert_eq!(store.load("wallet-a").unwrap(), None);

    assert!(store.load("../escape").is_err());
}

#[tokio::test]
async fn connect_pair_and_unpair_use_the_store() {
    let fake = Fake::start(|method, _| match method {
        "local.unpair" => Reply::Ok(json!({"unpaired": true})),
        _ => Reply::Ok(fx::session_info()),
    });
    let store = Arc::new(FileCredentialStore::new(fake.dir.path().join("creds")));
    let endpoint = fake.endpoint();

    assert!(matches!(
        Client::connect(&endpoint, store.clone()).await,
        Err(Error::NotPaired)
    ));
    assert!(fake.requests().is_empty());

    let params = PairParams {
        client_name: "Test App".into(),
        client_description: Some("A test".into()),
        pairing_code: pairing_code(),
    };
    let paired = Client::pair(&endpoint, store.clone(), params).await.unwrap();
    assert_eq!(store.load(PROVIDER_ID).unwrap(), Some(fake.credential()));
    paired.close().await;

    let client = Client::connect(&endpoint, store.clone()).await.unwrap();
    assert_eq!(client.session_id(), "ss-2");
    client.get_session_info().await.unwrap();

    client.unpair().await.unwrap();
    assert_eq!(fake.params("local.unpair"), json!({}));
    assert_eq!(store.load(PROVIDER_ID).unwrap(), None);
}

#[tokio::test]
async fn wait_operation_polls_once_per_second() {
    let polls = Arc::new(AtomicUsize::new(0));
    let fake = Fake::start({
        let polls = polls.clone();
        move |_, _| {
            let state = if polls.fetch_add(1, Ordering::SeqCst) < 2 {
                "awaiting_approval"
            } else {
                "completed"
            };
            Reply::Ok(fx::operation(
                "psbt.sign",
                state,
                (state == "completed").then(fx::sign_result),
            ))
        }
    });
    let client = fake.client().await;
    let start = Instant::now();
    let op = client.wait_operation("op-1", Duration::from_secs(30)).await.unwrap();
    assert_eq!(op.state, OperationState::Completed);
    assert_eq!(polls.load(Ordering::SeqCst), 3);
    assert!(start.elapsed() >= Duration::from_millis(1900), "{:?}", start.elapsed());
    assert_eq!(fake.requests().iter().filter(|r| r.0 == "operation.get").count(), 3);
}

#[tokio::test]
async fn wait_operation_wakes_on_its_event() {
    let done = Arc::new(AtomicUsize::new(0));
    let fake = Fake::start({
        let done = done.clone();
        move |_, _| {
            let state = if done.load(Ordering::SeqCst) == 0 {
                "awaiting_approval"
            } else {
                "rejected"
            };
            Reply::Ok(fx::operation("psbt.sign", state, None))
        }
    });
    let client = fake.client().await;
    let waiting = tokio::spawn({
        let client = client.clone();
        async move { client.wait_operation("op-1", Duration::from_secs(30)).await }
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let start = Instant::now();
    fake.push_event(fx::event("op-other", "completed")); // another Operation's event does not wake it
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(fake.requests().iter().filter(|r| r.0 == "operation.get").count(), 1);
    done.store(1, Ordering::SeqCst);
    fake.push_event(fx::event("op-1", "rejected"));
    let op = waiting.await.unwrap().unwrap();
    assert_eq!(op.state, OperationState::Rejected);
    assert!(start.elapsed() < Duration::from_millis(600), "{:?}", start.elapsed());
}

#[tokio::test]
async fn wait_operation_times_out_and_passes_errors_on() {
    let fake = Fake::start(|_, params| match params["operation_id"].as_str() {
        Some("op-1") => Reply::Ok(fx::operation("psbt.sign", "awaiting_approval", None)),
        _ => Reply::Err("OPERATION_NOT_FOUND"),
    });
    let client = fake.client().await;
    assert_eq!(
        client.wait_operation("op-1", Duration::from_millis(300)).await,
        Err(Error::Timeout)
    );
    match client.wait_operation("op-gone", Duration::from_secs(5)).await {
        Err(Error::Pwc(e)) => assert_eq!(e.code, ErrorCode::OperationNotFound),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn list_all_follows_cursors() {
    let fake = Fake::start(|method, params| {
        let page: u32 = match params.get("cursor").and_then(|c| c.as_str()) {
            None => 0,
            Some(c) => c.strip_prefix("page-").unwrap().parse().unwrap(),
        };
        let (key, item) = match method {
            "address.list" => ("addresses", fx::address(page)),
            "utxo.list" => ("utxos", fx::utxo(page)),
            _ => ("transactions", fx::transaction()),
        };
        let mut result = json!({key: [item]});
        if page < 2 {
            result["next_cursor"] = json!(format!("page-{}", page + 1));
        }
        Reply::Ok(result)
    });
    let client = fake.client().await;

    let utxos = client.list_all_utxos("a-1").await.unwrap();
    assert_eq!(utxos.iter().map(|u| u.vout).collect::<Vec<_>>(), [0, 1, 2]);
    let sent: Vec<_> = fake
        .requests()
        .into_iter()
        .filter(|r| r.0 == "utxo.list")
        .map(|r| r.1)
        .collect();
    assert_eq!(
        sent,
        [
            json!({"account_id": "a-1"}),
            json!({"account_id": "a-1", "cursor": "page-1"}),
            json!({"account_id": "a-1", "cursor": "page-2"})
        ]
    );

    let addresses = client.list_all_addresses("a-1").await.unwrap();
    assert_eq!(addresses[2].label.as_deref(), Some("label 2"));
    assert_eq!(client.list_all_transactions("a-1").await.unwrap().len(), 3);
}

#[tokio::test]
async fn list_all_passes_cursor_expiry_on() {
    let fake = Fake::start(|_, params| match params.get("cursor") {
        None => Reply::Ok(json!({"utxos": [fx::utxo(0)], "next_cursor": "page-1"})),
        Some(_) => Reply::Err("CURSOR_EXPIRED"),
    });
    match fake.client().await.list_all_utxos("a-1").await {
        Err(Error::Pwc(e)) => assert_eq!(e.code, ErrorCode::CursorExpired),
        other => panic!("{other:?}"),
    }
}
