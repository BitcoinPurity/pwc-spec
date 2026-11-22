//! TASK-012 and TASK-013: endpoints, proofs, handshake, framing, error mapping and events.

mod common;

use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::time::Duration;

use common::fake_provider::{Fake, Options, Reply, CLIENT_ID, PROVIDER_ID};
use common::fixtures;
use pwc_client::local::{client_proof, server_proof};
use pwc_client::*;
use serde_json::{json, Value};

fn unhex(s: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap();
    }
    out
}

/// L-12.
#[test]
fn authentication_vectors() {
    let path = format!("{}/../pwc-testkit/vectors/misc.json", env!("CARGO_MANIFEST_DIR"));
    let file: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let vectors = file["local_auth"]["vectors"].as_array().unwrap();
    assert!(!vectors.is_empty());
    for v in vectors {
        let field = |name: &str| unhex(v[name].as_str().unwrap());
        let (k, sn, cn) = (field("client_secret"), field("server_nonce"), field("client_nonce"));
        let id = v["client_id"].as_str().unwrap();
        assert_eq!(client_proof(&k, &sn, &cn, id), field("client_proof"), "{v}");
        assert_eq!(server_proof(&k, &sn, &cn, id), field("server_proof"), "{v}");
    }
}

#[test]
fn discovery_lists_sockets_of_a_private_directory() {
    let dir = tempfile::tempdir().unwrap();
    let run = dir.path().join("run");
    assert_eq!(discover(&run).unwrap(), vec![]);
    std::fs::create_dir(&run).unwrap();
    std::fs::set_permissions(&run, std::fs::Permissions::from_mode(0o700)).unwrap();
    for name in ["zeta.sock", "alpha.sock", "notes.txt", "Bad_Name.sock"] {
        std::fs::write(run.join(name), b"").unwrap();
    }
    let found = discover(&run).unwrap();
    let ids: Vec<&str> = found.iter().map(|e| e.provider_id.as_str()).collect();
    assert_eq!(ids, ["alpha", "zeta"]);
    assert_eq!(found[0].path, run.join("alpha.sock"));

    std::fs::set_permissions(&run, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(discover(&run), Err(Error::UnsafeEndpoint(_))));
}

#[tokio::test]
async fn refuses_a_directory_open_to_others() {
    let fake = Fake::start(|_, _| Reply::Never);
    std::fs::set_permissions(fake.dir.path(), std::fs::Permissions::from_mode(0o750)).unwrap();
    assert!(matches!(
        Connection::open(&fake.path).await,
        Err(Error::UnsafeEndpoint(_))
    ));
    assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn hello_then_pair() {
    let fake = Fake::start(|_, _| Reply::Ok(fixtures::session_info()));
    let conn = Connection::open(&fake.path).await.unwrap();
    let hello = conn.hello();
    assert_eq!(hello.provider_id, PROVIDER_ID);
    assert_eq!(hello.provider.name, "Fake Wallet");
    assert_eq!(hello.max_frame_bytes, 1_048_576);
    assert!(!hello.delegation);

    let params = PairParams {
        client_name: "Test App".into(),
        client_description: None,
        pairing_code: pairing_code(),
    };
    let (client, credential) = conn.pair(params.clone()).await.unwrap();
    assert_eq!(credential, fake.credential());
    assert_eq!(client.client_id(), CLIENT_ID);
    assert_eq!(client.session_id(), "ss-1");
    assert_eq!(fake.params("local.hello"), json!({"protocols": ["pwc-local/1"]}));
    assert_eq!(
        fake.params("local.pair"),
        json!({"client_name": "Test App", "pairing_code": params.pairing_code})
    );
    client.get_session_info().await.unwrap();
}

#[tokio::test]
async fn authenticate_with_a_stored_credential() {
    let fake = Fake::start(|_, _| Reply::Ok(fixtures::session_info()));
    let client = fake.client().await;
    assert_eq!(client.session_id(), "ss-2");
    client.get_session_info().await.unwrap();

    let conn = Connection::open(&fake.path).await.unwrap();
    let wrong = Credential {
        client_secret: "00".repeat(32),
        ..fake.credential()
    };
    match conn.authenticate(&wrong).await {
        Err(Error::Pwc(e)) => assert_eq!(e.code, ErrorCode::Unauthenticated),
        other => panic!("{:?}", other.map(|_| ())),
    }
}

/// L-13.
#[tokio::test]
async fn wrong_server_proof_closes_without_sending_more() {
    let options = Options {
        wrong_server_proof: true,
        ..Options::default()
    };
    let fake = Fake::with_options(options, |_, _| Reply::Ok(json!({})));
    let conn = Connection::open(&fake.path).await.unwrap();
    assert!(matches!(
        conn.authenticate(&fake.credential()).await,
        Err(Error::ProviderAuthFailed)
    ));
    fake.closed(1).await;
    let methods: Vec<String> = fake.requests().into_iter().map(|r| r.0).collect();
    assert_eq!(methods, ["local.hello", "local.authenticate"]);
}

#[tokio::test]
async fn provider_named_differently_from_its_socket_is_refused() {
    let fake = Fake::start(|_, _| Reply::Never);
    let other = fake.dir.path().join("other.sock");
    std::os::unix::fs::symlink(&fake.path, &other).unwrap();
    assert!(matches!(Connection::open(&other).await, Err(Error::Protocol(_))));
}

#[tokio::test]
async fn errors_are_mapped() {
    let fake = Fake::start(|method, _| match method {
        "wallet.list" => Reply::Err("PERMISSION_REQUIRED"),
        "account.list" => Reply::Rpc(-32600),
        "session.getInfo" => Reply::Ok(json!({"unexpected": true})),
        _ => Reply::Err("vendor.com.example.custom"),
    });
    let client = fake.client().await;
    match client.list_wallets().await {
        Err(Error::Pwc(e)) => {
            assert_eq!(e.code, ErrorCode::PermissionRequired);
            assert_eq!(e.message, "PERMISSION_REQUIRED from the fake");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        client.list_accounts("w-1").await,
        Err(Error::Rpc {
            code: -32600,
            message: "Invalid Request".into()
        })
    );
    assert!(matches!(client.get_session_info().await, Err(Error::Protocol(_))));
    match client.request("vendor.com.example.do", json!({})).await {
        Err(Error::Pwc(e)) => assert_eq!(e.code, ErrorCode::Other("vendor.com.example.custom".into())),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        client.request("wallet.list", json!([])).await,
        Err(Error::Protocol(_))
    ));
}

#[tokio::test]
async fn concurrent_requests_get_their_own_answers() {
    let fake = Fake::start(|_, params| {
        let n = params["n"].as_u64().unwrap();
        Reply::Later(100 - 10 * n, json!({"n": n}))
    });
    let client = fake.client().await;
    let calls = (0..8u64).map(|n| {
        let client = client.clone();
        tokio::spawn(async move { client.request("vendor.com.example.echo", json!({"n": n})).await })
    });
    for (n, call) in calls.enumerate() {
        assert_eq!(call.await.unwrap().unwrap(), json!({"n": n}));
    }
}

#[tokio::test]
async fn events_arriving_during_a_request_are_kept() {
    let fake = Fake::start(|_, _| Reply::Later(100, fixtures::session_info()));
    let client = fake.client().await;
    let events = client.subscribe_events();
    let pending = tokio::spawn({
        let client = client.clone();
        async move { client.get_session_info().await }
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    fake.push_event(fixtures::event("op-1", "processing"));
    fake.push_event(fixtures::event("op-1", "completed"));
    pending.await.unwrap().unwrap();
    let first = events.next().await.unwrap();
    assert_eq!(first.event_type, "operation.stateChanged");
    assert_eq!(first.operation_id.as_deref(), Some("op-1"));
    assert_eq!(first.payload, json!({"state": "processing"}));
    assert_eq!(events.next().await.unwrap().payload, json!({"state": "completed"}));
}

#[tokio::test]
async fn a_slow_reader_is_told_that_events_were_missed() {
    let fake = Fake::start(|_, _| Reply::Ok(fixtures::session_info()));
    let client = fake.client().await;
    let events = client.subscribe_events();
    for _ in 0..300 {
        fake.push_event(fixtures::event("op-1", "processing"));
    }
    fake.push_event(fixtures::event("op-last", "completed"));
    let last = client.subscribe_events();
    fake.push_event(fixtures::event("op-sync", "completed"));
    while last.next().await.unwrap().operation_id.as_deref() != Some("op-sync") {}
    assert_eq!(events.next().await, Err(Error::EventsMissed));
    assert!(events.next().await.is_ok());
}

#[tokio::test]
async fn closed_connection_fails_outstanding_and_later_requests() {
    let fake = Fake::start(|method, _| match method {
        "wallet.list" => Reply::Never,
        _ => Reply::Close,
    });
    let client = fake.client().await;
    let events = client.subscribe_events();
    let waiting = tokio::spawn({
        let client = client.clone();
        async move { client.list_wallets().await }
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(client.get_session_info().await, Err(Error::ConnectionClosed));
    assert_eq!(waiting.await.unwrap(), Err(Error::ConnectionClosed));
    assert_eq!(client.get_session_info().await, Err(Error::ConnectionClosed));
    assert_eq!(events.next().await, Err(Error::ConnectionClosed));
    assert_eq!(client.subscribe_events().next().await, Err(Error::ConnectionClosed));
}

#[tokio::test]
async fn close_ends_the_connection() {
    let fake = Fake::start(|_, _| Reply::Ok(fixtures::session_info()));
    let client = fake.client().await;
    client.close().await;
    fake.closed(1).await;
    assert_eq!(client.get_session_info().await, Err(Error::ConnectionClosed));
}

#[tokio::test]
async fn oversized_request_is_refused_locally() {
    let options = Options {
        max_frame_bytes: Some(2_000_000),
        ..Options::default()
    };
    let fake = Fake::with_options(options, |_, _| {
        Reply::Ok(json!({"psbt": fixtures::PSBT, "psbt_version": 0}))
    });
    let client = fake.client().await;
    let big = "A".repeat(1_500_000);
    client.finalize_psbt(&big).await.unwrap_err(); // answered, but not a FinalizePsbtResult
    assert_eq!(
        client.combine_psbts(vec![big.clone(), big]).await,
        Err(Error::FrameTooLarge)
    );
    assert_eq!(fake.requests().iter().filter(|r| r.0.starts_with("psbt.")).count(), 1);
}

#[test]
fn credential_debug_hides_the_secret() {
    let c = Credential {
        provider_id: "p".into(),
        client_id: "cl-1".into(),
        client_secret: "ab".repeat(32),
    };
    let shown = format!("{c:?}");
    assert!(shown.contains("cl-1") && !shown.contains("abab"), "{shown}");
}

#[tokio::test]
async fn client_is_shareable_across_tasks() {
    fn assert_send_sync<T: Send + Sync>(_: &T) {}
    let fake = Fake::start(|_, _| Reply::Never);
    let client = fake.client().await;
    assert_send_sync(&client);
    assert_send_sync(&client.subscribe_events());
    let _: Arc<dyn CredentialStore> = Arc::new(FileCredentialStore::new(fake.dir.path()));
}
