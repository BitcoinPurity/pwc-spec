//! Event delivery (LOCAL §8, Core §8.6).

mod common;

use common::Harness;
use pwc_provider::backend::{BackendEvent, WalletState};
use pwc_provider::provider::Config;
use pwc_provider::types::Id;
use serde_json::json;

fn balance_changed(account: &str) -> BackendEvent {
    BackendEvent::BalanceChanged { account_id: Id(account.into()) }
}

#[tokio::test]
async fn l21_c27_events_only_with_the_scope_and_the_read_permission() {
    let mut h = Harness::start(Config::default()).await;
    let (mut listener, _, _) = h.client(&["events:read", "balances:read"]).await;
    let (mut no_events, _, _) = h.client(&["balances:read"]).await;
    let (mut no_balances, _, _) = h.client(&["events:read", "wallets:read"]).await;
    h.provider.notify(balance_changed("a-regtest-wpkh"));
    let e = listener.next_event().await;
    assert_eq!((e["event_type"].clone(), e["account_id"].clone(), e["payload"].clone()), (json!("account.balanceChanged"), json!("a-regtest-wpkh"), json!({})));
    no_events.no_event().await;
    no_balances.no_event().await;
    h.provider.notify(balance_changed("a-mainnet-wpkh"));
    listener.no_event().await;
}

#[tokio::test]
async fn c53_no_account_events_for_locked_wallets() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(&["events:read", "wallets:read", "balances:read", "transactions:read"]).await;
    h.backend.set_wallet_state("w-regtest-main", WalletState::Locked);
    h.provider.notify(BackendEvent::WalletLocked { wallet_id: Id("w-regtest-main".into()) });
    let e = c.next_event().await;
    assert_eq!((e["event_type"].clone(), e["payload"].clone()), (json!("wallet.locked"), json!({"wallet_id": "w-regtest-main"})));
    h.provider.notify(balance_changed("a-regtest-wpkh"));
    h.provider.notify(BackendEvent::TransactionChanged { account_id: Id("a-regtest-wpkh".into()), txid: "ab".repeat(32), status: "confirmed" });
    h.provider.notify(BackendEvent::WalletSyncChanged { wallet_id: Id("w-regtest-main".into()) });
    c.no_event().await;
    h.backend.set_wallet_state("w-regtest-main", WalletState::Ready);
    h.provider.notify(BackendEvent::WalletSyncChanged { wallet_id: Id("w-regtest-main".into()) });
    let e = c.next_event().await;
    assert_eq!(e["event_type"], "wallet.syncChanged");
    assert_eq!(e["payload"]["sync"]["state"], "synced");
}

#[tokio::test]
async fn operation_and_grant_events() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(&["events:read"]).await;
    let op = c.ok("session.requestPermissions", json!({"scopes": ["wallets:read"], "idempotency_key": "x"})).await;
    let backend = h.backend.clone();
    h.answer_permissions(|r| Some(common::as_requested(&backend, r))).await;
    let e = c.next_event().await;
    assert_eq!((e["event_type"].clone(), e["operation_id"].clone(), e["payload"].clone()), (json!("operation.stateChanged"), op["operation_id"].clone(), json!({"state": "completed"})));
    let grants = c.ok("session.getPermissions", json!({})).await;
    let second = grants["grants"].as_array().unwrap().iter().find(|g| g["scopes"] == json!(["wallets:read"])).unwrap()["grant_id"].clone();
    h.provider.revoke_grant(second.as_str().unwrap());
    let e = c.next_event().await;
    assert_eq!((e["event_type"].clone(), e["payload"].clone()), (json!("grant.revoked"), json!({"grant_id": second})));
}

#[tokio::test]
async fn broadcast_emits_account_events() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(&["events:read", "balances:read", "transactions:read", "transactions:broadcast"]).await;
    let tx = bitcoin::Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input: vec![bitcoin::TxIn::default()],
        output: vec![bitcoin::TxOut { value: bitcoin::Amount::ZERO, script_pubkey: bitcoin::ScriptBuf::new() }],
    };
    let raw = bitcoin::consensus::encode::serialize_hex(&tx);
    let op = c.ok("transaction.broadcast", json!({"account_id": "a-regtest-wpkh", "raw_transaction": raw, "idempotency_key": "b"})).await;
    assert_eq!(op["state"], "failed");
    c.no_event().await;
    let (mut s, _, _) = h.client(&["events:read", "balances:read", "transactions:read", "psbt:create", "psbt:sign", "transactions:broadcast"]).await;
    let key = bitcoin::secp256k1::SecretKey::from_slice(&[7u8; 32]).unwrap();
    let pk = bitcoin::CompressedPublicKey(key.public_key(&bitcoin::secp256k1::Secp256k1::new()));
    let to = bitcoin::Address::p2wpkh(&pk, bitcoin::Network::Regtest).to_string();
    let p = json!({"account_id": "a-regtest-wpkh", "outputs": [{"address": to, "amount_sats": "1000"}], "fee_policy": {"type": "rate", "sat_per_kvb": "1250"}, "idempotency_key": "c"});
    let created = s.ok("psbt.create", p).await;
    let op = s.ok("psbt.sign", json!({"account_id": "a-regtest-wpkh", "psbt": created["result"]["psbt"], "idempotency_key": "s"})).await;
    let pwc_provider::prompt::Prompt::SignPsbt { reply, .. } = h.next_prompt().await else { panic!() };
    reply.send(true).unwrap();
    assert_eq!(s.next_event().await["payload"]["state"], "completed");
    let done = s.ok("operation.get", json!({"operation_id": op["operation_id"]})).await;
    let fin = s.ok("psbt.finalize", json!({"psbt": done["result"]["psbt"]})).await;
    s.ok("transaction.broadcast", json!({"account_id": "a-regtest-wpkh", "raw_transaction": fin["raw_transaction"], "idempotency_key": "b"})).await;
    let kinds: Vec<String> = [s.next_event().await, s.next_event().await].iter().map(|e| e["event_type"].as_str().unwrap().to_string()).collect();
    assert_eq!(kinds, ["account.balanceChanged", "account.transactionChanged"]);
    assert_eq!(c.next_event().await["event_type"], "account.balanceChanged", "other Clients with the read scope are told too");
}

#[tokio::test(flavor = "multi_thread")]
async fn l22_slow_client_is_disconnected() {
    let config = Config { event_buffer: 64, ..Config::default() };
    let mut h = Harness::start(config).await;
    let (mut slow, _, _) = h.client(&["events:read", "balances:read"]).await;
    let (mut fast, _, _) = h.client(&["events:read", "balances:read"]).await;
    let total = 5_000;
    let reader = tokio::spawn(async move {
        let mut n = 0;
        while n < total {
            assert_eq!(fast.next_event().await["event_type"], "account.balanceChanged");
            n += 1;
        }
        n
    });
    for i in 0..total {
        h.provider.notify(balance_changed("a-regtest-wpkh"));
        if i % 50 == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    }
    assert_eq!(reader.await.unwrap(), total, "the fast Client received every event");
    let mut received = 0;
    while slow.recv().await.is_some() {
        received += 1;
    }
    assert!(received < total, "the slow Client was disconnected instead of losing events silently");
}
