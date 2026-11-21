//! Read methods: inventory, balances, addresses, UTXOs and history (Core §8.1, §8.2).

mod common;

use common::{code, Conn, Harness};
use pwc_provider::backend::WalletState;
use pwc_provider::provider::Config;
use pwc_provider::types::{Id, ScriptType};
use serde_json::{json, Value};

const READ: &[&str] = &["wallets:read", "balances:read", "addresses:read", "addresses:create", "transactions:read", "utxos:read"];

fn id(s: &str) -> Id {
    Id(s.into())
}

async fn reader(h: &mut Harness) -> Conn {
    h.client(READ).await.0
}

async fn balance(c: &mut Conn, account: &str) -> Value {
    c.ok("account.getBalance", json!({"account_id": account})).await
}

#[tokio::test]
async fn c02_only_approved_account_data() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.pair().await;
    h.grant(&mut c, json!({"scopes": ["wallets:read", "balances:read"], "account_ids": ["a-regtest-wpkh"]})).await;
    let b = balance(&mut c, "a-regtest-wpkh").await;
    assert_eq!((b["confirmed_sats"].clone(), b["spendable_sats"].clone()), (json!("1500000"), json!("1500000")));
    assert_eq!((b["network"].clone(), b["sync"]["tip_height"].clone()), (json!("regtest"), json!(1000)));
    assert_eq!(c.fails("account.getBalance", json!({"account_id": "a-regtest-pkh"})).await, "ACCOUNT_NOT_FOUND");
    let wallets = c.ok("wallet.list", json!({})).await;
    assert_eq!(wallets["wallets"].as_array().unwrap().len(), 1);
    assert_eq!(wallets["wallets"][0]["account_ids"], json!(["a-regtest-wpkh"]));
    let accounts = c.ok("account.list", json!({"wallet_id": "w-regtest-main"})).await;
    assert_eq!(accounts["accounts"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn account_summaries() {
    let mut h = Harness::start(Config::default()).await;
    let mut c = reader(&mut h).await;
    let watch = c.ok("account.list", json!({"wallet_id": "w-regtest-watch"})).await;
    let a = &watch["accounts"][0];
    assert_eq!((a["can_sign"].clone(), a["script_types"].clone()), (json!(false), json!(["p2wpkh"])));
    let methods: Vec<&str> = a["methods"].as_array().unwrap().iter().map(|m| m.as_str().unwrap()).collect();
    assert!(methods.contains(&"psbt.create") && !methods.contains(&"psbt.sign") && !methods.contains(&"message.sign"));
    let wallets = c.ok("wallet.list", json!({})).await;
    assert_eq!(wallets["wallets"][1]["wallet_kind"], "watch_only");
}

#[tokio::test]
async fn c16_reorg_recomputes_confirmations() {
    let mut h = Harness::start(Config::default()).await;
    let mut c = reader(&mut h).await;
    let txid = h.backend.receive(&id("a-regtest-wpkh"), 70_000, 1).to_string();
    let t = c.ok("transaction.get", json!({"account_id": "a-regtest-wpkh", "txid": txid})).await;
    assert_eq!((t["transaction"]["status"].clone(), t["transaction"]["confirmations"].clone()), (json!("confirmed"), json!(1)));
    assert_eq!(t["transaction"]["net_change_sats"], "70000");
    h.backend.reorg("regtest", 1);
    let t = c.ok("transaction.get", json!({"account_id": "a-regtest-wpkh", "txid": txid})).await;
    assert_eq!((t["transaction"]["status"].clone(), t["transaction"]["confirmations"].clone()), (json!("unconfirmed"), json!(0)));
    assert!(t["transaction"].get("block_height").is_none());
    let b = balance(&mut c, "a-regtest-wpkh").await;
    assert_eq!((b["confirmed_sats"].clone(), b["pending_external_sats"].clone()), (json!("1500000"), json!("70000")));
    assert_eq!(b["sync"]["tip_height"], 999);
    h.backend.mine("regtest", 3);
    let t = c.ok("transaction.get", json!({"account_id": "a-regtest-wpkh", "txid": txid})).await;
    assert_eq!(t["transaction"]["confirmations"], 3);
}

#[tokio::test]
async fn c17_unknown_scope_is_never_granted() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.pair().await;
    let done = h.grant(&mut c, json!({"scopes": ["wallets:read", "vendor.com.example.exportLabels"]})).await;
    assert_eq!(done["result"]["grants"][0]["scopes"], json!(["wallets:read"]));
}

#[tokio::test]
async fn c24_pending_external_receipt() {
    let mut h = Harness::start(Config::default()).await;
    let mut c = reader(&mut h).await;
    h.backend.receive(&id("a-regtest-wpkh"), 50_000, 0);
    let b = balance(&mut c, "a-regtest-wpkh").await;
    assert_eq!(b["pending_external_sats"], "50000");
    assert_eq!(b["unconfirmed_delta_sats"], "50000");
    assert_eq!(b["spendable_sats"], "1500000", "the policy does not spend unconfirmed external outputs");
    let utxos = c.ok("utxo.list", json!({"account_id": "a-regtest-wpkh"})).await;
    let pending: Vec<&Value> = utxos["utxos"].as_array().unwrap().iter().filter(|u| u["confirmations"] == 0).collect();
    assert_eq!(pending.len(), 1);
    assert_eq!((pending[0]["spendable"].clone(), pending[0]["restrictions"].clone()), (json!(false), json!(["unconfirmed"])));
}

#[tokio::test]
async fn c25_conflict_and_replacement() {
    let mut h = Harness::start(Config::default()).await;
    let mut c = reader(&mut h).await;
    let a = id("a-regtest-wpkh");
    let first = h.backend.receive(&a, 40_000, 0);
    let second = h.backend.receive_conflicting(&a, &first, 39_000);
    assert_eq!(c.fails("account.getBalance", json!({"account_id": "a-regtest-wpkh"})).await, "STATE_UNAVAILABLE");
    h.backend.replace("regtest", &first, &second);
    let b = balance(&mut c, "a-regtest-wpkh").await;
    assert_eq!(b["pending_external_sats"], "39000");
    let t = c.ok("transaction.get", json!({"account_id": "a-regtest-wpkh", "txid": first.to_string()})).await;
    assert_eq!(t["transaction"]["status"], "replaced");
    assert_eq!(t["transaction"]["replaced_by_txid"], json!(second.to_string()));
}

#[tokio::test]
async fn immature_coinbase() {
    let mut h = Harness::start(Config::default()).await;
    let mut c = reader(&mut h).await;
    h.backend.receive_coinbase(&id("a-regtest-pkh"), 312_500_000, 10);
    let b = balance(&mut c, "a-regtest-pkh").await;
    assert_eq!((b["immature_sats"].clone(), b["spendable_sats"].clone()), (json!("312500000"), json!("200000")));
    let utxos = c.ok("utxo.list", json!({"account_id": "a-regtest-pkh"})).await;
    assert!(utxos["utxos"].as_array().unwrap().iter().any(|u| u["restrictions"] == json!(["immature"])));
}

#[tokio::test]
async fn c34_new_account_of_granted_wallet_is_covered() {
    let mut h = Harness::start(Config::default()).await;
    let mut c = reader(&mut h).await;
    h.backend.add_account(&id("w-regtest-main"), "a-regtest-new", "New", ScriptType::P2wpkh, true);
    assert_eq!(balance(&mut c, "a-regtest-new").await["confirmed_sats"], "0");
}

#[tokio::test]
async fn c35_c58_locked_and_unavailable_wallets() {
    let mut h = Harness::start(Config::default()).await;
    let mut c = reader(&mut h).await;
    for (state, expected) in [(WalletState::Locked, "WALLET_LOCKED"), (WalletState::Unavailable, "STATE_UNAVAILABLE")] {
        h.backend.set_wallet_state("w-regtest-main", state);
        let a = json!({"account_id": "a-regtest-wpkh"});
        for m in ["account.getBalance", "address.getReceiveAddress", "address.list", "utxo.list", "transaction.list"] {
            assert_eq!(c.fails(m, a.clone()).await, expected, "{m}");
        }
        let get = json!({"account_id": "a-regtest-wpkh", "txid": "ab".repeat(32)});
        assert_eq!(c.fails("transaction.get", get).await, expected);
        let create = json!({"account_id": "a-regtest-wpkh", "idempotency_key": "x"});
        assert_eq!(c.fails("address.create", create).await, expected);
        let w = c.ok("wallet.list", json!({})).await;
        assert_eq!(w["wallets"][0]["account_ids"], json!([]));
    }
    h.backend.set_wallet_state("w-regtest-main", WalletState::Syncing);
    balance(&mut c, "a-regtest-wpkh").await;
}

#[tokio::test]
async fn c36_outside_grant_and_nonexistent_are_identical() {
    let mut h = Harness::start(Config::default()).await;
    let mut c = reader(&mut h).await;
    let outside = c.call("account.getBalance", json!({"account_id": "a-mainnet-wpkh"})).await;
    let missing = c.call("account.getBalance", json!({"account_id": "a-nowhere"})).await;
    assert_eq!(code(&outside), "ACCOUNT_NOT_FOUND");
    assert_eq!(outside["error"], missing["error"]);
}

#[tokio::test]
async fn c38_pagination_and_cursor_binding() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(READ).await;
    let (mut other, _, _) = h.client(READ).await;
    let mut seen = Vec::new();
    let mut cursor: Option<Value> = None;
    loop {
        let mut p = json!({"account_id": "a-regtest-wpkh", "page_size": 1});
        if let Some(cur) = &cursor {
            p["cursor"] = cur.clone();
        }
        let page = c.ok("address.list", p).await;
        seen.extend(page["addresses"].as_array().unwrap().iter().map(|a| a["address"].clone()));
        match page.get("next_cursor") {
            Some(n) => cursor = Some(n.clone()),
            None => break,
        }
    }
    assert_eq!(seen.len(), 4);
    let page = c.ok("address.list", json!({"account_id": "a-regtest-wpkh", "page_size": 1})).await;
    let cur = page["next_cursor"].clone();
    let elsewhere = json!({"account_id": "a-regtest-pkh", "cursor": cur});
    assert_eq!(c.fails("address.list", elsewhere).await, "INVALID_ARGUMENT");
    let other_method = json!({"account_id": "a-regtest-wpkh", "cursor": cur});
    assert_eq!(c.fails("transaction.list", other_method.clone()).await, "INVALID_ARGUMENT");
    assert_eq!(other.fails("address.list", other_method).await, "INVALID_ARGUMENT");
    let forged = json!({"account_id": "a-regtest-wpkh", "cursor": "1.00"});
    assert_eq!(c.fails("address.list", forged).await, "INVALID_ARGUMENT");
    let big = c.ok("address.list", json!({"account_id": "a-regtest-wpkh", "page_size": 1000})).await;
    assert_eq!(big["addresses"].as_array().unwrap().len(), 4);
}

#[tokio::test]
async fn c57_unknown_transaction() {
    let mut h = Harness::start(Config::default()).await;
    let mut c = reader(&mut h).await;
    let p = json!({"account_id": "a-regtest-wpkh", "txid": "ab".repeat(32)});
    assert_eq!(c.fails("transaction.get", p).await, "TRANSACTION_NOT_FOUND");
    let history = c.ok("transaction.list", json!({"account_id": "a-regtest-wpkh"})).await;
    assert_eq!(history["transactions"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn networks_are_isolated_by_grant() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.pair().await;
    h.grant(&mut c, json!({"scopes": ["addresses:read"], "network": "signet"})).await;
    let a = c.ok("address.getReceiveAddress", json!({"account_id": "a-signet-wpkh"})).await;
    assert!(a["address"].as_str().unwrap().starts_with("tb1q"));
    assert_eq!(a["network"], "signet");
    assert_eq!(c.fails("address.getReceiveAddress", json!({"account_id": "a-regtest-wpkh"})).await, "ACCOUNT_NOT_FOUND");
    let (mut m, _, _) = h.pair().await;
    h.grant(&mut m, json!({"scopes": ["addresses:read"], "network": "mainnet"})).await;
    let a = m.ok("address.getReceiveAddress", json!({"account_id": "a-mainnet-pkh"})).await;
    assert!(a["address"].as_str().unwrap().starts_with('1'));
    let a = m.ok("address.getReceiveAddress", json!({"account_id": "a-mainnet-wpkh"})).await;
    assert!(a["address"].as_str().unwrap().starts_with("bc1q"));
}

#[tokio::test]
async fn receive_addresses_and_address_create() {
    let mut h = Harness::start(Config::default()).await;
    let mut c = reader(&mut h).await;
    let a = json!({"account_id": "a-regtest-wpkh"});
    let r1 = c.ok("address.getReceiveAddress", a.clone()).await;
    assert_eq!(c.ok("address.getReceiveAddress", a.clone()).await, r1);
    assert_eq!((r1["state"].clone(), r1["role"].clone()), (json!("unused"), json!("receive")));
    assert!(r1["address"].as_str().unwrap().starts_with("bcrt1q"));
    let p = json!({"account_id": "a-regtest-wpkh", "label": "Invoice 7", "idempotency_key": "addr-1"});
    let n1 = c.ok("address.create", p.clone()).await;
    assert_eq!(n1["label"], "Invoice 7");
    assert_eq!(c.ok("address.create", p).await, n1);
    let changed = json!({"account_id": "a-regtest-wpkh", "label": "Other", "idempotency_key": "addr-1"});
    assert_eq!(c.fails("address.create", changed).await, "IDEMPOTENCY_CONFLICT");
    let n2 = c.ok("address.create", json!({"account_id": "a-regtest-wpkh", "idempotency_key": "addr-2"})).await;
    assert_ne!(n1["address"], n2["address"]);
    h.backend.set_wallet_state("w-regtest-main", WalletState::Locked);
    let p = json!({"account_id": "a-regtest-wpkh", "label": "Invoice 7", "idempotency_key": "addr-1"});
    assert_eq!(c.fails("address.create", p).await, "WALLET_LOCKED", "retry obeys the locked-wallet rule");
}

#[tokio::test]
async fn offline_backend_marks_balance_stale() {
    let mut h = Harness::start(Config::default()).await;
    let mut c = reader(&mut h).await;
    h.backend.set_offline("regtest", true);
    assert_eq!(balance(&mut c, "a-regtest-wpkh").await["sync"]["state"], "offline");
}
