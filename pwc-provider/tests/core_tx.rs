//! `psbt.create`, reservations and broadcast (Core §8.3).

mod common;

use std::time::Duration;

use base64::Engine;
use bitcoin::Psbt;
use common::{code, Conn, Harness};
use pwc_provider::backend::WalletState;
use pwc_provider::provider::Config;
use pwc_provider::types::Id;
use serde_json::{json, Value};

const SCOPES: &[&str] = &["balances:read", "utxos:read", "psbt:create", "transactions:broadcast"];

/// A regtest address no demo account owns.
fn external() -> String {
    let key = bitcoin::secp256k1::SecretKey::from_slice(&[7u8; 32]).unwrap();
    let pk = bitcoin::CompressedPublicKey(key.public_key(&bitcoin::secp256k1::Secp256k1::new()));
    bitcoin::Address::p2wpkh(&pk, bitcoin::Network::Regtest).to_string()
}

fn create(amount: u64, key: &str) -> Value {
    json!({"account_id": "a-regtest-wpkh", "outputs": [{"address": external(), "amount_sats": amount.to_string()}],
        "fee_policy": {"type": "rate", "sat_per_kvb": "1250"}, "idempotency_key": key})
}

fn decode(r: &Value) -> Psbt {
    Psbt::deserialize(&base64::engine::general_purpose::STANDARD.decode(r["psbt"].as_str().unwrap()).unwrap()).unwrap()
}

async fn created(c: &mut Conn, params: Value) -> Value {
    let op = c.ok("psbt.create", params).await;
    assert_eq!(op["state"], "completed", "{op}");
    op["result"].clone()
}

async fn locked(c: &mut Conn) -> String {
    let b = c.ok("account.getBalance", json!({"account_id": "a-regtest-wpkh"})).await;
    b["locked_sats"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn create_discloses_fee_vsize_and_change() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let r = created(&mut c, create(50_000, "c1")).await;
    assert_eq!((r["fee_sats"].clone(), r["vsize"].clone()), (json!("177"), json!(141)));
    assert_eq!(r["change_output_indexes"], json!([1]));
    assert!(r.get("reservation_id").is_some() && r.get("reserved_until").is_some());
    let psbt = decode(&r);
    assert_eq!(psbt.unsigned_tx.input.len(), 1);
    assert_eq!(psbt.inputs[0].witness_utxo.as_ref().unwrap().value.to_sat(), 1_000_000, "largest first");
    assert_eq!(psbt.unsigned_tx.output[1].value.to_sat(), 1_000_000 - 50_000 - 177);
    assert_eq!(psbt.unsigned_tx.input[0].sequence, bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME);
}

#[tokio::test]
async fn c39_created_psbt_discloses_only_its_transaction() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let psbt = decode(&created(&mut c, create(50_000, "c1")).await);
    assert!(psbt.xpub.is_empty() && psbt.proprietary.is_empty() && psbt.unknown.is_empty());
    for i in &psbt.inputs {
        assert!(i.bip32_derivation.is_empty() && i.non_witness_utxo.is_none() && i.unknown.is_empty());
    }
    for o in &psbt.outputs {
        assert!(o.bip32_derivation.is_empty() && o.unknown.is_empty());
    }
}

#[tokio::test]
async fn c19_addresses_and_fee_policy() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let mut p = create(50_000, "a");
    p["outputs"][0]["address"] = json!("bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4");
    assert_eq!(c.fails("psbt.create", p.clone()).await, "NETWORK_MISMATCH");
    p["outputs"][0]["address"] = json!("not-an-address");
    assert_eq!(c.fails("psbt.create", p).await, "INVALID_ARGUMENT");
    assert_eq!(c.fails("psbt.create", create(5_000_000, "b")).await, "INSUFFICIENT_FUNDS");
    let mut p = create(50_000, "c");
    p["fee_policy"] = json!({"type": "rate", "sat_per_kvb": "500"});
    assert_eq!(c.fails("psbt.create", p).await, "POLICY_VIOLATION");
    let mut p = create(50_000, "d");
    p["fee_policy"] = json!({"type": "rate", "sat_per_kvb": "1.25"});
    assert_eq!(c.fails("psbt.create", p).await, "INVALID_ARGUMENT", "C-22");
    let mut p = create(50_000, "e");
    p["fee_policy"] = json!({"type": "wallet_estimate", "target_blocks": 6});
    assert_eq!(created(&mut c, p).await["fee_sats"], "282", "2000 sat/kvB at 141 vB");
    h.backend.set_fee_estimate("regtest", None);
    let mut p = create(50_000, "f");
    p["fee_policy"] = json!({"type": "wallet_estimate", "target_blocks": 6});
    assert_eq!(c.fails("psbt.create", p).await, "STATE_UNAVAILABLE");
}

#[tokio::test]
async fn coin_control_and_account_support() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let utxos = c.ok("utxo.list", json!({"account_id": "a-regtest-wpkh"})).await;
    let small = utxos["utxos"].as_array().unwrap().iter().find(|u| u["value_sats"] == "500000").unwrap().clone();
    let mut p = create(50_000, "s");
    p["selected_utxos"] = json!([{"txid": small["txid"], "vout": small["vout"]}]);
    let psbt = decode(&created(&mut c, p).await);
    assert_eq!(psbt.inputs[0].witness_utxo.as_ref().unwrap().value.to_sat(), 500_000);
    let mut p = create(50_000, "t");
    p["selected_utxos"] = json!([{"txid": "ab".repeat(32), "vout": 0}]);
    assert_eq!(c.fails("psbt.create", p).await, "INVALID_ARGUMENT");
    let mut p = create(50_000, "u");
    p["account_id"] = json!("a-regtest-pkh");
    assert_eq!(c.fails("psbt.create", p).await, "UNSUPPORTED_OPERATION");
    let mut p = create(50_000, "w");
    p["account_id"] = json!("a-regtest-watch");
    created(&mut c, p).await;
}

#[tokio::test]
async fn idempotent_create_reserves_once() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let a = c.ok("psbt.create", create(50_000, "same")).await;
    let b = c.ok("psbt.create", create(50_000, "same")).await;
    assert_eq!(a, b);
    assert_eq!(locked(&mut c).await, "1000000");
}

#[tokio::test]
async fn c46_reserved_inputs_are_locked_until_expiry() {
    let config = Config { reservation_ttl: Duration::from_millis(400), ..Config::default() };
    let mut h = Harness::start(config).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let first = decode(&created(&mut c, create(50_000, "a")).await);
    assert_eq!(locked(&mut c).await, "1000000");
    let utxos = c.ok("utxo.list", json!({"account_id": "a-regtest-wpkh"})).await;
    let reserved = utxos["utxos"].as_array().unwrap().iter().find(|u| u["value_sats"] == "1000000").unwrap().clone();
    assert_eq!((reserved["spendable"].clone(), reserved["restrictions"].clone()), (json!(false), json!(["locked"])));
    let second = decode(&created(&mut c, create(50_000, "b")).await);
    assert_ne!(first.unsigned_tx.input[0].previous_output, second.unsigned_tx.input[0].previous_output);
    assert_eq!(c.fails("psbt.create", create(50_000, "c")).await, "INSUFFICIENT_FUNDS", "both coins reserved");
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(locked(&mut c).await, "0");
    created(&mut c, create(50_000, "d")).await;
}

#[tokio::test]
async fn c47_c48_release_reservation() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let (mut other, _, _) = h.client(SCOPES).await;
    let r = created(&mut c, create(50_000, "a")).await;
    let id = r["reservation_id"].clone();
    let foreign = other.call("psbt.releaseReservation", json!({"account_id": "a-regtest-wpkh", "reservation_id": id, "idempotency_key": "x"})).await;
    let unknown = other.call("psbt.releaseReservation", json!({"account_id": "a-regtest-wpkh", "reservation_id": "rs-none", "idempotency_key": "y"})).await;
    assert_eq!(code(&foreign), "RESERVATION_NOT_FOUND");
    assert_eq!(foreign["error"], unknown["error"]);
    let wrong_account = json!({"account_id": "a-regtest-watch", "reservation_id": id, "idempotency_key": "z"});
    assert_eq!(c.fails("psbt.releaseReservation", wrong_account).await, "RESERVATION_NOT_FOUND");
    let p = |k: &str| json!({"account_id": "a-regtest-wpkh", "reservation_id": id, "idempotency_key": k});
    assert_eq!(c.ok("psbt.releaseReservation", p("r1")).await, json!({"released": true}));
    assert_eq!(c.ok("psbt.releaseReservation", p("r2")).await, json!({"released": false}));
    assert_eq!(c.ok("psbt.releaseReservation", p("r1")).await, json!({"released": true}), "exact retry");
    assert_eq!(locked(&mut c).await, "0");
}

#[tokio::test]
async fn c49_user_spend_and_revocation_end_reservations() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let r = created(&mut c, create(50_000, "a")).await;
    let input = decode(&r).unsigned_tx.input[0].previous_output;
    h.backend.spend_by_user(&Id("a-regtest-wpkh".into()), &input, 990_000);
    assert_eq!(locked(&mut c).await, "0");
    let p = json!({"account_id": "a-regtest-wpkh", "reservation_id": r["reservation_id"], "idempotency_key": "r"});
    assert_eq!(c.ok("psbt.releaseReservation", p).await, json!({"released": false}));

    created(&mut c, create(50_000, "b")).await;
    assert_eq!(locked(&mut c).await, "500000");
    for g in h.provider.grants() {
        h.provider.revoke_grant(g["grant_id"].as_str().unwrap());
    }
    let (mut reader, _, _) = h.client(&["balances:read"]).await;
    let b = reader.ok("account.getBalance", json!({"account_id": "a-regtest-wpkh"})).await;
    assert_eq!(b["locked_sats"], "0");
}

#[tokio::test]
async fn amount_limit_applies_to_create() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.pair().await;
    h.grant(&mut c, json!({"scopes": SCOPES, "constraints": {"max_amount_sats_per_request": "50177"}})).await;
    created(&mut c, create(50_000, "ok")).await;
    assert_eq!(c.fails("psbt.create", create(50_001, "over")).await, "GRANT_CONSTRAINT_VIOLATED");
}

#[tokio::test]
async fn c64_completed_create_hides_result_while_locked() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let op = c.ok("psbt.create", create(50_000, "a")).await;
    h.backend.set_wallet_state("w-regtest-main", WalletState::Locked);
    let seen = c.ok("operation.get", json!({"operation_id": op["operation_id"]})).await;
    assert_eq!(seen["state"], "completed");
    assert!(seen.get("result").is_none());
    assert!(c.ok("psbt.create", create(50_000, "a")).await.get("result").is_none(), "exact retry obeys the same rule");
    assert_eq!(c.fails("psbt.create", create(50_000, "b")).await, "WALLET_LOCKED");
    h.backend.set_wallet_state("w-regtest-main", WalletState::Ready);
    assert!(c.ok("operation.get", json!({"operation_id": op["operation_id"]})).await.get("result").is_some());
}

#[tokio::test]
async fn c44_broadcast_refused_and_unreachable_backend() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let unsigned = decode(&created(&mut c, create(50_000, "a")).await).unsigned_tx;
    let raw = bitcoin::consensus::encode::serialize_hex(&unsigned);
    let p = json!({"account_id": "a-regtest-wpkh", "raw_transaction": raw, "idempotency_key": "b1"});
    let op = c.ok("transaction.broadcast", p.clone()).await;
    assert_eq!((op["state"].clone(), op["error"]["code"].clone()), (json!("failed"), json!("TRANSACTION_REJECTED")));
    assert_eq!(c.ok("transaction.broadcast", p).await["operation_id"], op["operation_id"]);
    h.backend.set_offline("regtest", true);
    let p = json!({"account_id": "a-regtest-wpkh", "raw_transaction": raw, "idempotency_key": "b2"});
    assert_eq!(c.ok("transaction.broadcast", p).await["error"]["code"], "NETWORK_UNAVAILABLE");
    let p = json!({"account_id": "a-regtest-wpkh", "raw_transaction": "00", "idempotency_key": "b3"});
    assert_eq!(c.fails("transaction.broadcast", p).await, "INVALID_ARGUMENT");
}
