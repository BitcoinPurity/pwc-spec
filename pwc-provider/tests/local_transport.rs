//! LOCAL transport tests (PWC-TRANSPORT-LOCAL-001 §11).

mod common;

use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::time::Duration;

use common::{code, Conn, Harness};
use pwc_provider::local::{self, EndpointError};
use pwc_provider::prompt::Prompt;
use pwc_provider::provider::{Config, RateLimit};
use serde_json::{json, Value};

fn hex(b: &[u8]) -> String {
    bitcoin::hex::DisplayHex::to_lower_hex_string(b)
}

#[tokio::test]
async fn l01_endpoint_directory_and_socket_permissions() {
    let tmp = tempfile::tempdir().unwrap();
    let open_dir = tmp.path().join("open");
    std::fs::create_dir(&open_dir).unwrap();
    std::fs::set_permissions(&open_dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(local::bind(&open_dir, "pwc-provider"), Err(EndpointError::UnsafeDirectory(_))));
    assert!(!open_dir.join("pwc-provider.sock").exists());

    let h = Harness::start(Config::default()).await;
    let dir = std::fs::metadata(h.path.parent().unwrap()).unwrap();
    assert_eq!(dir.mode() & 0o777, 0o700);
    assert_eq!(std::fs::metadata(&h.path).unwrap().mode() & 0o777, 0o600);
}

#[tokio::test]
async fn l25_endpoint_path_too_long() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("d".repeat(120));
    assert!(matches!(local::bind(&dir, "pwc-provider"), Err(EndpointError::PathTooLong(_))));
}

#[tokio::test]
async fn l26_stale_socket_replaced_running_instance_untouched() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("run");
    std::fs::create_dir(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    drop(std::os::unix::net::UnixListener::bind(dir.join("pwc-provider.sock")).unwrap());
    let first = local::bind(&dir, "pwc-provider").expect("stale file replaced");
    assert!(matches!(local::bind(&dir, "pwc-provider"), Err(EndpointError::AlreadyRunning(_))));
    assert!(std::os::unix::net::UnixStream::connect(first.path()).is_ok());
    let path = first.path().to_path_buf();
    drop(first);
    assert!(!path.exists(), "socket file removed when the Provider stops listening");
}

#[tokio::test]
async fn l03_first_request_not_hello() {
    let h = Harness::start(Config::default()).await;
    let mut c = h.connect().await;
    let v = c.call("pwc.getCapabilities", json!({})).await;
    assert_eq!(code(&v), "UNAUTHENTICATED");
    assert!(c.closed().await);
}

#[tokio::test]
async fn l04_core_method_before_authentication() {
    let h = Harness::start(Config::default()).await;
    let mut c = h.connect().await;
    c.hello().await;
    assert_eq!(c.fails("wallet.list", json!({})).await, "UNAUTHENTICATED");
    assert_eq!(c.fails("local.hello", json!({"protocols": ["pwc-local/1"]})).await, "UNSUPPORTED_OPERATION");
}

#[tokio::test]
async fn l05_unknown_protocol() {
    let h = Harness::start(Config::default()).await;
    let mut c = h.connect().await;
    assert_eq!(c.fails("local.hello", json!({"protocols": ["pwc-local/99"]})).await, "UNSUPPORTED_OPERATION");
    assert!(c.closed().await);
}

#[tokio::test]
async fn hello_result_and_fresh_nonces() {
    let h = Harness::start(Config::default()).await;
    let a = h.connect().await.hello().await;
    let b = h.connect().await.hello().await;
    assert_eq!(a["protocol"], "pwc-local/1");
    assert_eq!(a["provider_id"], "pwc-provider");
    assert_eq!(a["delegation"], false);
    assert!(a["max_frame_bytes"].as_u64().unwrap() >= 1_048_576);
    assert_ne!(a["server_nonce"], b["server_nonce"]);
}

#[tokio::test]
async fn malformed_hello_keeps_connection() {
    let h = Harness::start(Config::default()).await;
    let mut c = h.connect().await;
    assert_eq!(c.fails("local.hello", json!({"protocols": "pwc-local/1"})).await, "INVALID_ARGUMENT");
    c.hello().await;
}

#[tokio::test]
async fn handshake_time_limits() {
    let config = Config {
        hello_timeout: Duration::from_millis(200),
        auth_timeout: Duration::from_millis(300),
        ..Config::default()
    };
    let h = Harness::start(config).await;
    let mut c = h.connect().await;
    assert!(c.closed().await, "closed without local.hello");
    let mut c = h.connect().await;
    c.hello().await;
    assert!(c.closed().await, "closed without local.pair or local.authenticate");
}

#[tokio::test]
async fn l06_pairing_approved() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, client_id, _) = h.pair().await;
    assert!(client_id.starts_with("cl-"));
    let info = c.ok("session.getInfo", json!({})).await;
    assert_eq!(info["client_id"], json!(client_id));
    let clients = h.provider.paired_clients();
    assert_eq!(clients.len(), 1);
    assert_eq!(clients[0].client_id.as_str(), client_id);
    assert_eq!(clients[0].claimed_name.as_str(), "Test client");
}

#[tokio::test]
async fn l07_pairing_refused() {
    let mut h = Harness::start(Config::default()).await;
    let mut c = h.connect().await;
    c.hello().await;
    let id = c.send_request("local.pair", json!({"client_name": "App", "pairing_code": "111111"})).await;
    match h.next_prompt().await {
        Prompt::Pairing { reply, .. } => reply.send(false).unwrap(),
        _ => panic!(),
    }
    assert_eq!(code(&c.response(id).await), "USER_REJECTED");
    assert!(h.provider.paired_clients().is_empty());
    assert_eq!(c.fails("local.pair", json!({"client_name": "App", "pairing_code": "111111"})).await, "RATE_LIMITED");
}

#[tokio::test]
async fn l08_pairing_unanswered() {
    let config = Config { pairing_timeout: Duration::from_millis(200), ..Config::default() };
    let mut h = Harness::start(config).await;
    let mut c = h.connect().await;
    c.hello().await;
    let id = c.send_request("local.pair", json!({"client_name": "App", "pairing_code": "111111"})).await;
    let prompt = h.next_prompt().await;
    assert_eq!(code(&c.response(id).await), "OPERATION_EXPIRED");
    if let Prompt::Pairing { reply, .. } = prompt {
        assert!(reply.is_closed(), "prompt withdrawn");
    }
    assert!(h.provider.paired_clients().is_empty());
}

#[tokio::test]
async fn l09_connection_closed_during_pairing() {
    let mut h = Harness::start(Config::default()).await;
    let mut c = h.connect().await;
    c.hello().await;
    c.send_request("local.pair", json!({"client_name": "App", "pairing_code": "111111"})).await;
    let Prompt::Pairing { reply, .. } = h.next_prompt().await else { panic!() };
    drop(c);
    tokio::time::timeout(common::WAIT, async {
        while !reply.is_closed() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("prompt withdrawn");
    assert!(reply.send(true).is_err());
    assert!(h.provider.paired_clients().is_empty());
}

async fn failed_auth(h: &Harness, client_id: &str) -> (Value, bool) {
    let mut c = h.connect().await;
    let hello = c.hello().await;
    let v = c.authenticate(&hello, client_id, &[0u8; 32]).await;
    let closed = c.closed().await;
    (v, closed)
}

#[tokio::test]
async fn l10_wrong_proof_and_unknown_client_identical() {
    let mut h = Harness::start(Config::default()).await;
    let (_c, client_id, _) = h.pair().await;
    let (a, a_closed) = failed_auth(&h, &client_id).await;
    let (b, b_closed) = failed_auth(&h, "cl-unknown").await;
    assert_eq!(code(&a), "UNAUTHENTICATED");
    assert_eq!(a["error"], b["error"]);
    assert!(a_closed && b_closed);
}

#[tokio::test]
async fn l11_replayed_proof_rejected() {
    let mut h = Harness::start(Config::default()).await;
    let (_c, client_id, secret) = h.pair().await;
    let mut c = h.connect().await;
    let hello = c.hello().await;
    let sn: [u8; 32] = bitcoin::hex::FromHex::from_hex(hello["server_nonce"].as_str().unwrap()).unwrap();
    let cn = [9u8; 32];
    let params = json!({"client_id": client_id, "client_nonce": hex(&cn),
        "client_proof": hex(&local::client_proof(&secret, &sn, &cn, &client_id))});
    assert!(c.request("local.authenticate", params.clone()).await.is_ok());
    let mut c = h.connect().await;
    c.hello().await;
    assert_eq!(c.fails("local.authenticate", params).await, "UNAUTHENTICATED");
}

#[tokio::test]
async fn reauthentication_with_stored_credential() {
    let mut h = Harness::start(Config::default()).await;
    let (_c, client_id, secret) = h.pair().await;
    let mut c = h.login(&client_id, &secret).await;
    let hello = json!({"server_nonce": "00".repeat(32)});
    assert_eq!(code(&c.authenticate(&hello, &client_id, &secret).await), "UNSUPPORTED_OPERATION");
}

#[tokio::test]
async fn failed_authentications_rate_limited() {
    let config = Config { auth_failure_limit: RateLimit { max: 2, window: Duration::from_secs(60) }, ..Config::default() };
    let h = Harness::start(config).await;
    assert_eq!(code(&failed_auth(&h, "cl-x").await.0), "UNAUTHENTICATED");
    assert_eq!(code(&failed_auth(&h, "cl-x").await.0), "UNAUTHENTICATED");
    let (v, closed) = failed_auth(&h, "cl-x").await;
    assert_eq!(code(&v), "RATE_LIMITED");
    assert!(closed);
}

#[tokio::test]
async fn pairing_prompts_rate_limited_per_peer() {
    let config = Config { pairing_limit: RateLimit { max: 1, window: Duration::from_secs(60) }, ..Config::default() };
    let mut h = Harness::start(config).await;
    h.pair().await;
    let mut c = h.connect().await;
    c.hello().await;
    assert_eq!(c.fails("local.pair", json!({"client_name": "App", "pairing_code": "111111"})).await, "RATE_LIMITED");
}

#[tokio::test]
async fn delegator_pairing_not_implemented() {
    let mut h = Harness::start(Config::default()).await;
    let mut c = h.connect().await;
    c.hello().await;
    let params = json!({"client_name": "Bridge", "pairing_code": "111111", "delegator": true});
    assert_eq!(c.fails("local.pair", params).await, "UNSUPPORTED_OPERATION");
    h.no_prompt().await;
}

#[tokio::test]
async fn l14_frame_too_long() {
    let h = Harness::start(Config::default()).await;
    let mut c = h.connect().await;
    let hello = c.hello().await;
    let max = hello["max_frame_bytes"].as_u64().unwrap() as usize;
    let mut big = br#"{"jsonrpc":"2.0","id":9,"method":"local.pair","params":{"client_name":""#.to_vec();
    big.extend(std::iter::repeat_n(b'x', max));
    big.extend(b"\"}}\n");
    c.send_raw(&big).await;
    let v = c.recv().await.expect("an error frame");
    assert_eq!(code(&v), "PAYLOAD_TOO_LARGE");
    assert_eq!(v["id"], Value::Null);
    assert!(c.closed().await);
}

#[tokio::test]
async fn l15_l16_invalid_frames() {
    let h = Harness::start(Config::default()).await;
    let mut c = h.connect().await;
    c.hello().await;
    c.send_raw(b"{not json}\n").await;
    assert_eq!(c.recv().await.unwrap()["error"]["code"], -32700);
    assert_eq!(c.fails("wallet.list", json!({})).await, "UNAUTHENTICATED");
    c.send(&json!([{"jsonrpc": "2.0", "id": 11, "method": "wallet.list", "params": {}}])).await;
    let v = c.recv().await.unwrap();
    assert_eq!(v["error"]["code"], -32600);
    assert_eq!(v["id"], Value::Null);
    c.send(&json!({"jsonrpc": "2.0", "id": 12, "method": "wallet.list"})).await;
    let v = c.recv().await.unwrap();
    assert_eq!((v["error"]["code"].clone(), v["id"].clone()), (json!(-32600), json!(12)));
    c.send(&json!({"jsonrpc": "2.0", "id": 1.5, "method": "wallet.list", "params": {}})).await;
    assert_eq!(c.recv().await.unwrap()["id"], Value::Null);
    c.send(&json!({"jsonrpc": "2.0", "method": "wallet.list", "params": {}})).await;
    assert_eq!(c.fails("wallet.list", json!({})).await, "UNAUTHENTICATED", "notification discarded without a reply");
}

#[tokio::test]
async fn l17_duplicate_outstanding_id() {
    let mut h = Harness::start(Config::default()).await;
    let mut c = h.connect().await;
    c.hello().await;
    let pair = json!({"jsonrpc": "2.0", "id": 12, "method": "local.pair",
        "params": {"client_name": "Duplicate id test", "pairing_code": "000000"}});
    c.send(&pair).await;
    c.send(&pair).await;
    let v = c.recv().await.unwrap();
    assert_eq!((v["error"]["code"].clone(), v["id"].clone()), (json!(-32600), json!(12)));
    let Prompt::Pairing { reply, .. } = h.next_prompt().await else { panic!() };
    h.no_prompt().await;
    reply.send(true).unwrap();
    assert!(c.recv().await.unwrap().get("result").is_some());
}

#[tokio::test]
async fn l27_pairing_prompt_marks_claims_and_shows_peer() {
    let mut h = Harness::start(Config::default()).await;
    let mut c = h.connect().await;
    c.hello().await;
    c.send_request("local.pair", json!({"client_name": "Example Trader", "pairing_code": "482913"})).await;
    let Prompt::Pairing { request, .. } = h.next_prompt().await else { panic!() };
    assert_eq!(request.peer.pid, Some(std::process::id() as i32));
    assert!(request.peer.executable.is_some());
    let shown = request.to_string();
    assert!(shown.contains("Example Trader") && shown.contains("unverified") && shown.contains("482913"));
    assert!(shown.contains(&std::process::id().to_string()));
}

#[tokio::test]
async fn l28_no_network_listener() {
    let _h = Harness::start(Config::default()).await;
    let pid = std::process::id();
    let mut inodes = Vec::new();
    for fd in std::fs::read_dir(format!("/proc/{pid}/fd")).unwrap().flatten() {
        if let Ok(target) = std::fs::read_link(fd.path()) {
            if let Some(inode) = target.to_str().and_then(|t| t.strip_prefix("socket:[")) {
                inodes.push(inode.trim_end_matches(']').to_string());
            }
        }
    }
    for table in ["tcp", "tcp6", "udp", "udp6"] {
        let content = std::fs::read_to_string(format!("/proc/{pid}/net/{table}")).unwrap_or_default();
        for line in content.lines().skip(1) {
            let inode = line.split_whitespace().nth(9).unwrap_or("");
            assert!(!inodes.iter().any(|i| i == inode), "{table} socket open: {line}");
        }
    }
    let _ = Conn::connect(&_h.path).await;
}
