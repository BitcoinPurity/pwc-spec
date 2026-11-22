//! TASK-CLIENT-005: the library against the test kit's mock Provider, `pwc-testkit/mock_local_provider.py`.

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use pwc_client::*;

struct Mock {
    dir: tempfile::TempDir,
    process: Child,
}

impl Drop for Mock {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

fn start(extra: &[&str]) -> Mock {
    let dir = tempfile::tempdir().unwrap();
    let run = dir.path().join("run");
    let script = format!("{}/../pwc-testkit/mock_local_provider.py", env!("CARGO_MANIFEST_DIR"));
    let process = Command::new("python3")
        .arg(script)
        .arg("--dir")
        .arg(&run)
        .args(extra)
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let mock = Mock { dir, process };
    let start = Instant::now();
    while !run.join("mockwallet.sock").exists() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "the mock Provider did not start"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    mock
}

fn pair_params() -> PairParams {
    PairParams {
        client_name: "Example Trader".into(),
        client_description: None,
        pairing_code: pairing_code(),
    }
}

fn permissions(scopes: &[&str], key: &str) -> RequestPermissionsParams {
    RequestPermissionsParams {
        scopes: scopes.iter().map(|s| s.to_string()).collect(),
        network: None,
        wallet_ids: None,
        account_ids: None,
        expires_at: None,
        constraints: None,
        idempotency_key: key.into(),
    }
}

fn code<T: std::fmt::Debug>(result: Result<T, Error>) -> ErrorCode {
    match result {
        Err(Error::Pwc(e)) => e.code,
        other => panic!("expected a PwcError, got {other:?}"),
    }
}

fn endpoint(run: &Path) -> Endpoint {
    let mut found = discover(run).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].provider_id, "mockwallet");
    found.remove(0)
}

/// The flow of PWC-CLIENT-GUIDE-001 §2 to §4, as far as the mock goes: it never approves a permission request.
#[tokio::test]
async fn pair_reconnect_and_call() {
    let mock = start(&[]);
    let endpoint = endpoint(&mock.dir.path().join("run"));
    let store: Arc<dyn CredentialStore> = Arc::new(FileCredentialStore::new(mock.dir.path().join("creds")));

    assert!(matches!(
        Client::connect(&endpoint, store.clone()).await,
        Err(Error::NotPaired)
    ));
    let first = Client::pair(&endpoint, store.clone(), pair_params()).await.unwrap();
    assert_eq!(first.hello().provider.name, "Mock Wallet");
    first.close().await;

    let client = Client::connect(&endpoint, store.clone()).await.unwrap();
    assert_eq!(client.client_id(), first.client_id());
    assert_ne!(client.session_id(), first.session_id());

    let caps = client.get_capabilities().await.unwrap();
    assert_eq!(caps.pwc_version, "0.2.1");
    assert_eq!(caps.limits.high_fee_rate, 100_000);
    let info = client.get_session_info().await.unwrap();
    assert_eq!(info.client_id, client.client_id());
    assert_eq!(client.get_permissions().await.unwrap(), vec![]);

    let key = idempotency_key();
    let op = client
        .request_permissions(permissions(&["wallets:read", "balances:read"], &key))
        .await
        .unwrap();
    assert_eq!(op.state, OperationState::AwaitingApproval);
    assert!(!op.is_terminal());
    // An exact retry returns the original Operation; the same key for another action is a conflict (Core §9.3).
    let retry = client
        .request_permissions(permissions(&["wallets:read", "balances:read"], &key))
        .await
        .unwrap();
    assert_eq!(retry.operation_id, op.operation_id);
    assert_eq!(
        code(client.request_permissions(permissions(&["utxos:read"], &key)).await),
        ErrorCode::IdempotencyConflict
    );

    assert_eq!(client.get_operation(&op.operation_id).await.unwrap(), op);
    assert_eq!(
        client
            .wait_operation(&op.operation_id, Duration::from_millis(200))
            .await,
        Err(Error::Timeout)
    );
    assert_eq!(
        code(client.get_operation("op-unknown").await),
        ErrorCode::OperationNotFound
    );
    assert_eq!(code(client.list_wallets().await), ErrorCode::PermissionRequired);
    assert_eq!(
        code(client.list_utxos("a-1", None, None).await),
        ErrorCode::UnsupportedMethod
    );

    // Requests on one connection may overlap.
    let (a, b) = tokio::join!(client.get_capabilities(), client.get_session_info());
    assert_eq!((a.unwrap(), b.unwrap()), (caps, info));

    client.unpair().await.unwrap();
    assert_eq!(store.load("mockwallet").unwrap(), None);
    let stale = Credential {
        provider_id: "mockwallet".into(),
        client_id: client.client_id(),
        client_secret: "00".repeat(32),
    };
    let conn = Connection::open(&endpoint.path).await.unwrap();
    assert_eq!(
        code(conn.authenticate(&stale).await.map(|_| ())),
        ErrorCode::Unauthenticated
    );
}

#[tokio::test]
async fn refused_pairing_stores_nothing() {
    let mock = start(&["--reject-pairing"]);
    let endpoint = endpoint(&mock.dir.path().join("run"));
    let store: Arc<dyn CredentialStore> = Arc::new(FileCredentialStore::new(mock.dir.path().join("creds")));
    assert_eq!(
        code(Client::pair(&endpoint, store.clone(), pair_params()).await.map(|_| ())),
        ErrorCode::UserRejected
    );
    assert_eq!(store.load("mockwallet").unwrap(), None);
}
