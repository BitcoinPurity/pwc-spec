//! Saving and loading Provider state through the host wallet (TASK-010).

mod common;

use common::Harness;
use pwc_provider::provider::{Config, Provider, StateError};
use serde_json::json;

const SCOPES: &[&str] = &[
    "wallets:read",
    "balances:read",
    "addresses:create",
    "psbt:create",
    "psbt:sign",
];

fn external() -> String {
    let key = bitcoin::secp256k1::SecretKey::from_slice(&[7u8; 32]).unwrap();
    let pk = bitcoin::CompressedPublicKey(key.public_key(&bitcoin::secp256k1::Secp256k1::new()));
    bitcoin::Address::p2wpkh(&pk, bitcoin::Network::Regtest).to_string()
}

#[tokio::test]
async fn state_survives_a_restart() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, client_id, secret) = h.client(SCOPES).await;
    let address_params = json!({"account_id": "a-regtest-wpkh", "idempotency_key": "addr"});
    let address = c.ok("address.create", address_params.clone()).await;
    let create = json!({"account_id": "a-regtest-wpkh", "outputs": [{"address": external(), "amount_sats": "1000"}],
        "fee_policy": {"type": "rate", "sat_per_kvb": "1250"}, "idempotency_key": "create"});
    let created = c.ok("psbt.create", create.clone()).await;
    let sign = json!({"account_id": "a-regtest-wpkh", "psbt": created["result"]["psbt"], "idempotency_key": "sign"});
    let pending = c.ok("psbt.sign", sign.clone()).await;
    let _prompt = h.next_prompt().await;
    let snapshot = h.provider.save_state();
    let paired = h.provider.paired_clients();
    let backend = h.backend.clone();
    drop(c);
    drop(h);

    let (provider, prompts) = Provider::with_state(backend.clone(), Config::default(), &snapshot).unwrap();
    assert_eq!(provider.paired_clients(), paired);
    let mut h = Harness::restart(backend, provider, prompts).await;
    let mut c = h.login(&client_id, &secret).await;
    assert_eq!(
        c.ok("wallet.list", json!({})).await["wallets"]
            .as_array()
            .unwrap()
            .len(),
        2,
        "grant still active"
    );
    assert_eq!(
        c.ok("address.create", address_params).await,
        address,
        "idempotent retry answers the original"
    );
    assert_eq!(c.ok("psbt.create", create).await, created);
    let op = c
        .ok("operation.get", json!({"operation_id": created["operation_id"]}))
        .await;
    assert_eq!(op, created);
    let b = c
        .ok("account.getBalance", json!({"account_id": "a-regtest-wpkh"}))
        .await;
    assert_eq!(b["locked_sats"], "1000000", "the reservation is still in effect");
    let failed = c
        .ok("operation.get", json!({"operation_id": pending["operation_id"]}))
        .await;
    assert_eq!(
        (
            failed["state"].clone(),
            failed["error"]["code"].clone(),
            failed["error"]["retryable"].clone()
        ),
        (json!("failed"), json!("INTERNAL_ERROR"), json!(true))
    );
    assert_eq!(c.ok("psbt.sign", sign).await["operation_id"], pending["operation_id"]);
    h.no_prompt().await;
}

#[test]
fn corrupt_or_unknown_snapshots_are_refused() {
    let backend = std::sync::Arc::new(pwc_provider::memory::MemoryBackend::demo());
    let err = |bytes: &[u8]| {
        Provider::with_state(backend.clone(), Config::default(), bytes)
            .err()
            .unwrap()
    };
    assert!(matches!(err(b"not json"), StateError::Corrupt(_)));
    assert!(matches!(err(br#"{"version": 1}"#), StateError::Corrupt(_)));
    assert!(matches!(err(br#"{"version": 2}"#), StateError::UnsupportedVersion(2)));
}
