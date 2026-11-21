//! Sessions, grants, Operations, idempotency and the order of checks (Core §6–§9).

mod common;

use std::time::Duration;

use common::{as_requested, code, Harness};
use pwc_provider::backend::WalletState;
use pwc_provider::prompt::Prompt;
use pwc_provider::provider::{Config, RateLimit};
use pwc_provider::types::Timestamp;
use serde_json::{json, Value};

fn ts(t: Timestamp) -> Value {
    serde_json::to_value(t).unwrap()
}

#[tokio::test]
async fn c01_no_grant_enumerates_wallets() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.pair().await;
    let v = c.call("wallet.list", json!({})).await;
    assert_eq!(code(&v), "PERMISSION_REQUIRED");
    assert!(v.get("result").is_none());
}

#[tokio::test]
async fn capabilities_are_identical_before_and_after_a_grant() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.pair().await;
    let before = c.ok("pwc.getCapabilities", json!({})).await;
    h.grant(&mut c, json!({"scopes": ["wallets:read"]})).await;
    assert_eq!(c.ok("pwc.getCapabilities", json!({})).await, before);
    assert_eq!(before["methods"].as_array().unwrap().len(), 24);
}

#[tokio::test]
async fn granted_inventory_and_session_info() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, client_id, _) = h.pair().await;
    let done = h.grant(&mut c, json!({"scopes": ["wallets:read"]})).await;
    let grant = &done["result"]["grants"][0];
    assert_eq!(grant["network"], "regtest");
    assert_eq!(grant["client_id"], json!(client_id));
    let wallets = c.ok("wallet.list", json!({})).await;
    let ids: Vec<&str> = wallets["wallets"].as_array().unwrap().iter().map(|w| w["wallet_id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["w-regtest-main", "w-regtest-watch"]);
    let accounts = c.ok("account.list", json!({"wallet_id": "w-regtest-main"})).await;
    assert_eq!(accounts["accounts"].as_array().unwrap().len(), 2);
    let info = c.ok("session.getInfo", json!({})).await;
    assert_eq!(info["grant_ids"], json!([grant["grant_id"]]));
}

#[tokio::test]
async fn order_of_checks() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.pair().await;
    // 3 before 4
    assert_eq!(c.fails("vendor.example.nothing", json!({"x": 1})).await, "UNSUPPORTED_METHOD");
    // 4 before 6
    assert_eq!(c.fails("account.getBalance", json!({})).await, "INVALID_ARGUMENT");
    assert_eq!(c.fails("wallet.list", json!({"extra": true})).await, "INVALID_ARGUMENT");
    // 6 before 7
    assert_eq!(c.fails("account.getBalance", json!({"account_id": "nope"})).await, "PERMISSION_REQUIRED");
    h.grant(&mut c, json!({"scopes": ["wallets:read"]})).await;
    // 7: outside the grant (mainnet) and nonexistent give the same error
    let a = c.call("account.list", json!({"wallet_id": "w-mainnet-main"})).await;
    let b = c.call("account.list", json!({"wallet_id": "nope"})).await;
    assert_eq!(code(&a), "WALLET_NOT_FOUND");
    assert_eq!(a["error"], b["error"]);
    // 8 after 7
    h.backend.set_wallet_state("w-regtest-main", WalletState::Locked);
    assert_eq!(c.fails("account.list", json!({"wallet_id": "w-regtest-main"})).await, "WALLET_LOCKED");
    assert_eq!(c.fails("account.list", json!({"wallet_id": "w-mainnet-main"})).await, "WALLET_NOT_FOUND");
    let wallets = c.ok("wallet.list", json!({})).await;
    let main = &wallets["wallets"][0];
    assert_eq!((main["state"].clone(), main["account_ids"].clone()), (json!("locked"), json!([])));
    h.backend.set_wallet_state("w-regtest-main", WalletState::Unavailable);
    assert_eq!(c.fails("account.list", json!({"wallet_id": "w-regtest-main"})).await, "STATE_UNAVAILABLE");
}

#[tokio::test]
async fn c05_c37_idempotent_permission_request() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.pair().await;
    let (mut other, _, _) = h.pair().await;
    let params = json!({"scopes": ["wallets:read"], "idempotency_key": "grant-1"});
    let op = c.ok("session.requestPermissions", params.clone()).await;
    assert_eq!(op["state"], "awaiting_approval");
    assert!(op.get("result").is_none() && op.get("expires_at").is_some());
    let again = c.ok("session.requestPermissions", params).await;
    assert_eq!(again["operation_id"], op["operation_id"]);
    let changed = json!({"scopes": ["balances:read"], "idempotency_key": "grant-1"});
    assert_eq!(c.fails("session.requestPermissions", changed).await, "IDEMPOTENCY_CONFLICT");
    let id = op["operation_id"].clone();
    assert_eq!(other.fails("operation.get", json!({"operation_id": id})).await, "OPERATION_NOT_FOUND");
    let backend = h.backend.clone();
    h.answer_permissions(|r| Some(as_requested(&backend, r))).await;
    h.no_prompt().await;
    let done = c.wait(id.as_str().unwrap()).await;
    assert_eq!(done["result"]["grants"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn c18_cross_client_operation_access() {
    let mut h = Harness::start(Config::default()).await;
    let (mut a, _, _) = h.pair().await;
    let (mut b, _, _) = h.pair().await;
    let op = h.grant(&mut a, json!({"scopes": ["wallets:read"]})).await;
    let id = op["operation_id"].clone();
    let foreign = b.call("operation.get", json!({"operation_id": id})).await;
    let missing = b.call("operation.get", json!({"operation_id": "op-none"})).await;
    assert_eq!(code(&foreign), "OPERATION_NOT_FOUND");
    assert_eq!(foreign["error"], missing["error"]);
    assert_eq!(code(&b.call("operation.cancel", json!({"operation_id": id, "idempotency_key": "x"})).await), "OPERATION_NOT_FOUND");
}

#[tokio::test]
async fn c31_terminal_operation_never_changes() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.pair().await;
    let done = h.grant(&mut c, json!({"scopes": ["wallets:read"]})).await;
    let id = done["operation_id"].clone();
    let canceled = c.ok("operation.cancel", json!({"operation_id": id, "idempotency_key": "c1"})).await;
    assert_eq!(canceled, done);
    assert_eq!(c.ok("operation.get", json!({"operation_id": id})).await, done);
}

#[tokio::test]
async fn cancel_withdraws_the_prompt() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.pair().await;
    let op = c.ok("session.requestPermissions", json!({"scopes": ["wallets:read"], "idempotency_key": "k"})).await;
    let Prompt::Permissions { reply, .. } = h.next_prompt().await else { panic!() };
    let id = op["operation_id"].clone();
    let canceled = c.ok("operation.cancel", json!({"operation_id": id, "idempotency_key": "c"})).await;
    assert_eq!(canceled["state"], "canceled");
    assert_eq!(canceled["error"]["code"], "OPERATION_CANCELED");
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(reply.is_closed());
}

#[tokio::test]
async fn c33_grant_expires_at_valid_until() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.pair().await;
    let now = Timestamp::now();
    let params = json!({"scopes": ["wallets:read"], "expires_at": ts(now.after(Duration::from_secs(3600))),
        "constraints": {"valid_until": ts(now.after(Duration::from_millis(400)))}});
    h.grant(&mut c, params).await;
    c.ok("wallet.list", json!({})).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(c.fails("wallet.list", json!({})).await, "PERMISSION_REVOKED");
    let grants = c.ok("session.getPermissions", json!({})).await;
    assert_eq!(grants["grants"][0]["status"], "expired");
    assert_eq!(c.ok("session.getInfo", json!({})).await["grant_ids"], json!([]));
}

#[tokio::test]
async fn c50_revoke_permissions() {
    let mut h = Harness::start(Config::default()).await;
    let (mut a, _, _) = h.pair().await;
    let (mut b, _, _) = h.pair().await;
    let done = h.grant(&mut a, json!({"scopes": ["wallets:read"]})).await;
    let grant_id = done["result"]["grants"][0]["grant_id"].clone();
    let foreign = json!({"grant_id": grant_id, "idempotency_key": "r1"});
    assert_eq!(b.fails("session.revokePermissions", foreign).await, "GRANT_NOT_FOUND");
    let r = a.ok("session.revokePermissions", json!({"grant_id": grant_id, "idempotency_key": "r1"})).await;
    assert_eq!(r, json!({"revoked": true}));
    let retry = a.ok("session.revokePermissions", json!({"grant_id": grant_id, "idempotency_key": "r1"})).await;
    assert_eq!(retry, json!({"revoked": true}), "exact retry answers the original response");
    let r = a.ok("session.revokePermissions", json!({"grant_id": grant_id, "idempotency_key": "r2"})).await;
    assert_eq!(r, json!({"revoked": false}));
    assert_eq!(a.fails("wallet.list", json!({})).await, "PERMISSION_REVOKED");
}

#[tokio::test]
async fn c51_concurrency_limit() {
    let mut config = Config::default();
    config.limits.max_concurrent_operations = 2;
    let mut h = Harness::start(config).await;
    let (mut c, _, _) = h.pair().await;
    let mut open = Vec::new();
    for k in ["a", "b"] {
        c.ok("session.requestPermissions", json!({"scopes": ["wallets:read"], "idempotency_key": k})).await;
        open.push(h.next_prompt().await);
    }
    let third = json!({"scopes": ["wallets:read"], "idempotency_key": "c"});
    assert_eq!(c.fails("session.requestPermissions", third.clone()).await, "RATE_LIMITED");
    h.no_prompt().await;
    assert_eq!(c.fails("session.requestPermissions", third).await, "RATE_LIMITED", "no record was created");
}

#[tokio::test]
async fn c52_grant_never_exceeds_request() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.pair().await;
    let now = Timestamp::now();
    let expiry = now.after(Duration::from_secs(600));
    let params = json!({"scopes": ["wallets:read", "balances:read"], "network": "regtest",
        "wallet_ids": ["w-regtest-main", "w-unknown"], "expires_at": ts(expiry),
        "constraints": {"max_amount_sats_per_request": "1000"}, "idempotency_key": "g"});
    let op = c.ok("session.requestPermissions", params).await;
    let request = h
        .answer_permissions(|r| {
            let mut choice = as_requested(&h_backend(), r);
            choice.scopes.push(pwc_provider::types::Scope("psbt:sign".into()));
            choice.wallet_ids.push(pwc_provider::types::Id("w-regtest-watch".into()));
            choice.wallet_ids.push(pwc_provider::types::Id("w-mainnet-main".into()));
            choice.expires_at = Some(now.after(Duration::from_secs(7200)));
            choice.constraints = Some(pwc_provider::types::GrantConstraints {
                max_amount_sats_per_request: Some(pwc_provider::types::PositiveSats(5000)),
                valid_until: Some(now.after(Duration::from_secs(60))),
            });
            Some(choice)
        })
        .await;
    assert_eq!(request.wallet_ids.len(), 1, "unknown identifiers are ignored silently");
    let done = c.wait(op["operation_id"].as_str().unwrap()).await;
    let g = &done["result"]["grants"][0];
    assert_eq!(g["scopes"], json!(["wallets:read", "balances:read"]));
    assert_eq!(g["wallet_ids"], json!(["w-regtest-main"]));
    assert_eq!(g["expires_at"], ts(expiry));
    assert_eq!(g["constraints"]["max_amount_sats_per_request"], "1000");
    assert_eq!(g["constraints"]["valid_until"], ts(now.after(Duration::from_secs(60))));

    let op = c.ok("session.requestPermissions", json!({"scopes": ["wallets:read"], "network": "signet", "idempotency_key": "n"})).await;
    h.answer_permissions(|r| {
        let mut choice = as_requested(&h_backend(), r);
        choice.network = pwc_provider::types::Network("regtest".into());
        Some(choice)
    })
    .await;
    assert_eq!(c.wait(op["operation_id"].as_str().unwrap()).await["state"], "rejected");
}

fn h_backend() -> pwc_provider::memory::MemoryBackend {
    pwc_provider::memory::MemoryBackend::demo()
}

#[tokio::test]
async fn c56_operation_expires_while_awaiting_approval() {
    let config = Config { approval_timeout: Duration::from_millis(200), ..Config::default() };
    let mut h = Harness::start(config).await;
    let (mut c, _, _) = h.pair().await;
    let op = c.ok("session.requestPermissions", json!({"scopes": ["wallets:read"], "idempotency_key": "k"})).await;
    let Prompt::Permissions { reply, .. } = h.next_prompt().await else { panic!() };
    let done = c.wait(op["operation_id"].as_str().unwrap()).await;
    assert_eq!((done["state"].clone(), done["error"]["code"].clone()), (json!("expired"), json!("OPERATION_EXPIRED")));
    assert!(reply.is_closed());
}

#[tokio::test]
async fn c62_prompt_flooding_rate_limited() {
    let config = Config { prompt_limit: RateLimit { max: 2, window: Duration::from_secs(60) }, ..Config::default() };
    let mut h = Harness::start(config).await;
    let (mut c, _, _) = h.pair().await;
    for k in ["a", "b"] {
        let op = c.ok("session.requestPermissions", json!({"scopes": ["wallets:read"], "idempotency_key": k})).await;
        h.answer_permissions(|_| None).await;
        assert_eq!(c.wait(op["operation_id"].as_str().unwrap()).await["state"], "rejected");
    }
    assert_eq!(c.fails("session.requestPermissions", json!({"scopes": ["wallets:read"], "idempotency_key": "c"})).await, "RATE_LIMITED");
}

#[tokio::test]
async fn l18_l19_error_mapping() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.pair().await;
    let v = c.call("vendor.example.nothing", json!({})).await;
    assert_eq!((v["error"]["code"].clone(), code(&v)), (json!(1000), "UNSUPPORTED_METHOD"));
    let v = c.call("local.nothing", json!({})).await;
    assert_eq!(code(&v), "UNSUPPORTED_METHOD");
    let v = c.call("utxo.list", json!({"account_id": "acct-01"})).await;
    assert_eq!(v["error"]["data"], json!({"code": "PERMISSION_REQUIRED", "message": "Scope utxos:read is required"}));
    assert_eq!(code(&c.call("local.openDelegate", json!({})).await), "UNSUPPORTED_OPERATION");
}

#[tokio::test]
async fn l20_retry_after_disconnect_returns_same_operation() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, client_id, secret) = h.pair().await;
    let params = json!({"scopes": ["wallets:read"], "idempotency_key": "retry"});
    let op = c.ok("session.requestPermissions", params.clone()).await;
    let _prompt = h.next_prompt().await;
    drop(c);
    let mut c = h.login(&client_id, &secret).await;
    let again = c.ok("session.requestPermissions", params).await;
    assert_eq!(again["operation_id"], op["operation_id"]);
    assert_eq!(again["state"], "awaiting_approval");
    h.no_prompt().await;
}

#[tokio::test]
async fn l23_unpair() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, client_id, secret) = h.pair().await;
    h.grant(&mut c, json!({"scopes": ["wallets:read"]})).await;
    let mut second = h.login(&client_id, &secret).await;
    c.ok("session.requestPermissions", json!({"scopes": ["balances:read"], "idempotency_key": "p"})).await;
    let Prompt::Permissions { reply, .. } = h.next_prompt().await else { panic!() };
    assert_eq!(c.ok("local.unpair", json!({})).await, json!({"unpaired": true}));
    assert!(c.closed().await);
    assert!(second.closed().await);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(reply.is_closed(), "pending Operation failed and its prompt withdrawn");
    assert!(h.provider.grants().iter().all(|g| g["status"] == "revoked"));
    assert!(h.provider.paired_clients().is_empty());
    let mut c = h.connect().await;
    let hello = c.hello().await;
    assert_eq!(code(&c.authenticate(&hello, &client_id, &secret).await), "UNAUTHENTICATED");
}

#[tokio::test]
async fn revocation_fails_pending_operations() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.pair().await;
    let done = h.grant(&mut c, json!({"scopes": ["wallets:read"]})).await;
    let grant_id = done["result"]["grants"][0]["grant_id"].as_str().unwrap().to_string();
    assert!(h.provider.revoke_grant(&grant_id));
    assert!(!h.provider.revoke_grant(&grant_id));
    assert_eq!(c.fails("wallet.list", json!({})).await, "PERMISSION_REVOKED");
}
