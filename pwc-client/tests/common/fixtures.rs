//! Result fixtures. Each is checked against the strict schema by `tests/types.rs` before any test relies on it.

use pwc_client::*;
use serde_json::{json, Value};

pub const TXID: &str = "abababababababababababababababababababababababababababababababab";
pub const T0: &str = "2026-11-21T00:00:00Z";
pub const T1: &str = "2026-11-21T00:10:00Z";
pub const ADDRESS: &str = "bcrt1qw508d6qejxtdg4y5r3zarvary0c5xw7kygt080";
pub const PSBT: &str = "cHNidP8BAAoCAAAAAAAAAAAAAA==";

pub fn capabilities() -> Value {
    json!({
        "pwc_version": "0.2.1",
        "provider": {"name": "Fake Wallet", "version": "1.0.0", "platform": "desktop"},
        "profiles": ["pwc-core-read-v1", "pwc-psbt-signer-v1"],
        "supported_networks": ["regtest"],
        "methods": ["pwc.getCapabilities", "psbt.sign"],
        "features": ["coin_control", "event_push"],
        "scopes": ["wallets:read", "psbt:sign"],
        "script_types": ["p2wpkh", "p2sh-p2wpkh"],
        "message_formats": ["bip322-simple", "legacy"],
        "limits": {"max_page_size": 100, "max_psbt_bytes": 100000, "max_combine_psbts": 5,
                   "max_raw_transaction_bytes": 400000, "max_message_bytes": 65536,
                   "max_concurrent_operations": 8, "operation_retention_seconds": 86400,
                   "high_fee_rate": "500000"}
    })
}

pub fn session_info() -> Value {
    json!({"session_id": "ss-1", "client_id": "cl-test", "grant_ids": ["g-1"], "created_at": T0})
}

pub fn grant() -> Value {
    json!({"grant_id": "g-1", "client_id": "cl-test", "network": "regtest", "scopes": ["wallets:read"],
           "wallet_ids": ["w-1"], "account_ids": [], "created_at": T0, "expires_at": T1, "status": "active",
           "constraints": {"max_amount_sats_per_request": "50000", "valid_until": T1}})
}

pub fn wallet() -> Value {
    json!({"wallet_id": "w-1", "display_name": "Main", "network": "regtest", "wallet_kind": "software",
           "state": "ready", "account_ids": ["a-1"]})
}

pub fn account() -> Value {
    json!({"account_id": "a-1", "wallet_id": "w-1", "display_name": "Default", "network": "regtest",
           "can_sign": true, "script_types": ["p2wpkh"], "methods": ["account.getBalance", "psbt.sign"]})
}

pub fn balance() -> Value {
    json!({"account_id": "a-1", "network": "regtest", "confirmed_sats": "1000000", "pending_self_sats": "99000",
           "pending_external_sats": "0", "pending_outgoing_sats": "400000", "unconfirmed_delta_sats": "-301000",
           "spendable_sats": "600000", "locked_sats": "0", "immature_sats": "0", "updated_at": T0,
           "sync": {"state": "synced", "tip_height": 1000}})
}

pub fn address(n: u32) -> Value {
    json!({"account_id": "a-1", "network": "regtest", "address": ADDRESS, "role": "receive", "state": "unused",
           "label": format!("label {n}")})
}

pub fn utxo(vout: u32) -> Value {
    json!({"txid": TXID, "vout": vout, "account_id": "a-1", "network": "regtest", "value_sats": "500000",
           "confirmations": 3, "block_height": 998, "address": ADDRESS, "script_pub_key": "0014ab",
           "spendable": true, "restrictions": [], "is_change": false})
}

pub fn transaction() -> Value {
    json!({"account_id": "a-1", "network": "regtest", "txid": TXID, "status": "confirmed", "confirmations": 2,
           "block_height": 999, "first_seen_at": T0, "confirmed_at": T1, "net_change_sats": "-301000",
           "fee_sats": "1000"})
}

pub fn create_result() -> Value {
    json!({"psbt": PSBT, "psbt_version": 0, "account_id": "a-1", "network": "regtest", "fee_sats": "313",
           "vsize": 141, "change_output_indexes": [1], "reservation_id": "r-1", "reserved_until": T1})
}

pub fn sign_result() -> Value {
    json!({"psbt": PSBT, "psbt_version": 0, "account_id": "a-1", "network": "regtest",
           "signed_input_indexes": [0], "signing_status": "complete"})
}

pub fn finalize_result() -> Value {
    json!({"psbt": PSBT, "psbt_version": 0, "complete": true, "raw_transaction": "0200", "txid": TXID})
}

pub fn message_result() -> Value {
    json!({"signature": "AAAA", "format": "bip322-simple", "address": ADDRESS})
}

/// An Operation in `state`; non-completed terminal states carry the error of Core §9.1.
pub fn operation(method: &str, state: &str, result: Option<Value>) -> Value {
    let mut op = json!({"operation_id": "op-1", "method": method, "state": state, "created_at": T0, "updated_at": T0});
    match state {
        "queued" | "awaiting_approval" | "awaiting_unlock" | "awaiting_signer" => op["expires_at"] = json!(T1),
        "rejected" => op["error"] = json!({"code": "USER_REJECTED", "message": "Refused"}),
        "expired" => op["error"] = json!({"code": "OPERATION_EXPIRED", "message": "Expired"}),
        "canceled" => op["error"] = json!({"code": "OPERATION_CANCELED", "message": "Canceled"}),
        "failed" => op["error"] = json!({"code": "TRANSACTION_REJECTED", "message": "Refused by backend"}),
        _ => {}
    }
    if let Some(r) = result {
        op["result"] = r;
    }
    op
}

pub fn event(operation_id: &str, state: &str) -> Value {
    json!({"event_id": "ev-1", "event_type": "operation.stateChanged", "occurred_at": T0,
           "operation_id": operation_id, "payload": {"state": state}})
}

/// Every fixture with the `$defs` entry of the strict Core schema it must satisfy.
pub fn all() -> Vec<(&'static str, Value)> {
    vec![
        ("Capabilities", capabilities()),
        ("SessionInfo", session_info()),
        ("Grant", grant()),
        ("WalletSummary", wallet()),
        ("AccountSummary", account()),
        ("Balance", balance()),
        ("AddressRecord", address(0)),
        ("Utxo", utxo(0)),
        ("TransactionRecord", transaction()),
        ("CreatePsbtResult", create_result()),
        ("SignPsbtResult", sign_result()),
        ("FinalizePsbtResult", finalize_result()),
        ("message.sign.operationResult", message_result()),
        ("Operation", operation("psbt.sign", "awaiting_approval", None)),
        ("Operation", operation("psbt.sign", "completed", Some(sign_result()))),
        ("Operation", operation("psbt.sign", "rejected", None)),
        ("Operation", operation("transaction.broadcast", "failed", None)),
        ("PwcEvent", event("op-1", "completed")),
    ]
}

/// Parses `value` as the library type for `def` and checks that writing it back gives the same JSON.
pub fn parse(def: &str, value: Value) {
    fn round<T: serde::de::DeserializeOwned + serde::Serialize>(v: Value) {
        let t: T = serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{e}: {v}"));
        assert_eq!(serde_json::to_value(&t).unwrap(), v);
    }
    match def {
        "Capabilities" => round::<Capabilities>(value),
        "SessionInfo" => round::<SessionInfo>(value),
        "Grant" => round::<Grant>(value),
        "WalletSummary" => round::<WalletSummary>(value),
        "AccountSummary" => round::<AccountSummary>(value),
        "Balance" => round::<Balance>(value),
        "AddressRecord" => round::<AddressRecord>(value),
        "Utxo" => round::<Utxo>(value),
        "TransactionRecord" => round::<TransactionRecord>(value),
        "CreatePsbtResult" => round::<CreatePsbtResult>(value),
        "SignPsbtResult" => round::<SignPsbtResult>(value),
        "FinalizePsbtResult" => round::<FinalizePsbtResult>(value),
        "message.sign.operationResult" => round::<SignMessageResult>(value),
        "Operation" => round::<Operation>(value),
        "PwcEvent" => round::<PwcEvent>(value),
        _ => panic!("no type for {def}"),
    }
}
