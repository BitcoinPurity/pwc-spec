//! PSBT methods (Core §4.7, §8.3).

mod common;

use base64::Engine;
use bitcoin::Psbt;
use common::Harness;
use pwc_provider::provider::Config;
use serde_json::{json, Value};

fn fixtures() -> Value {
    let path = format!("{}/../pwc-testkit/vectors/psbt.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn decode(b64: &str) -> Psbt {
    Psbt::deserialize(&base64::engine::general_purpose::STANDARD.decode(b64).unwrap()).unwrap()
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// PSBT-01 to PSBT-08 of `pwc-testkit/vectors/psbt.json`; PSBTs compare as key-value sets.
#[tokio::test]
async fn psbt_fixture_cases() {
    let f = fixtures();
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.pair().await;
    for case in f["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let inputs: Vec<&str> = case["inputs"].as_array().unwrap().iter().map(|n| f["psbts"][n.as_str().unwrap()].as_str().unwrap()).collect();
        let method = case["method"].as_str().unwrap();
        let params = if method == "psbt.combine" { json!({"psbts": inputs}) } else { json!({"psbt": inputs[0]}) };
        let expect = &case["expect"];
        match c.request(method, params).await {
            Err(e) => assert_eq!(e["code"], expect["error"], "{id}"),
            Ok(r) => {
                assert!(expect.get("error").is_none(), "{id}: unexpected success");
                assert_eq!(r["psbt_version"], 0);
                if let Some(name) = expect["result_equals"].as_str() {
                    assert_eq!(decode(r["psbt"].as_str().unwrap()), decode(f["psbts"][name].as_str().unwrap()), "{id}");
                }
                if let Some(complete) = expect.get("complete") {
                    assert_eq!(&r["complete"], complete, "{id}");
                }
                if expect.get("raw_transaction").is_some() {
                    assert_eq!(r["raw_transaction"], expect["raw_transaction"], "{id}");
                    assert_eq!(r["txid"], expect["txid"], "{id}");
                } else if method == "psbt.finalize" {
                    assert!(r.get("raw_transaction").is_none() && r.get("txid").is_none(), "{id}");
                }
            }
        }
    }
}

#[tokio::test]
async fn combine_and_finalize_are_deterministic() {
    let f = fixtures();
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.pair().await;
    let p = json!({"psbts": [f["psbts"]["partial_k1"], f["psbts"]["partial_k2"]]});
    assert_eq!(c.ok("psbt.combine", p.clone()).await, c.ok("psbt.combine", p).await);
    let partial = json!({"psbt": f["psbts"]["partial_k1"]});
    let r = c.ok("psbt.finalize", partial).await;
    assert_eq!(decode(r["psbt"].as_str().unwrap()), decode(f["psbts"]["partial_k1"].as_str().unwrap()), "unchanged");
}

#[tokio::test]
async fn c30_size_and_count_limits() {
    let f = fixtures();
    let mut config = Config::default();
    config.limits.max_psbt_bytes = 100;
    config.limits.max_combine_psbts = 2;
    let mut h = Harness::start(config).await;
    let (mut c, _, _) = h.pair().await;
    let k1 = f["psbts"]["partial_k1"].clone();
    assert_eq!(c.fails("psbt.finalize", json!({"psbt": k1})).await, "PAYLOAD_TOO_LARGE");
    let small = b64(b"psbt\xff\x00");
    assert_eq!(c.fails("psbt.combine", json!({"psbts": [small, small, small]})).await, "PAYLOAD_TOO_LARGE");
    assert_eq!(c.fails("psbt.combine", json!({"psbts": [small]})).await, "INVALID_ARGUMENT");
}

#[tokio::test]
async fn malformed_psbts() {
    let f = fixtures();
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.pair().await;
    let garbage = b64(b"not a psbt");
    assert_eq!(c.fails("psbt.finalize", json!({"psbt": garbage})).await, "PSBT_VALIDATION_FAILED");
    assert_eq!(c.fails("psbt.combine", json!({"psbts": [garbage, garbage]})).await, "PSBT_VALIDATION_FAILED");
    assert_eq!(c.fails("psbt.finalize", json!({"psbt": "cHNidP9="})).await, "INVALID_ARGUMENT", "non-canonical base64");

    // An input without previous-output data cannot be verified.
    let mut psbt = decode(f["psbts"]["unsigned"].as_str().unwrap());
    psbt.inputs[0].witness_utxo = None;
    assert_eq!(c.fails("psbt.finalize", json!({"psbt": b64(&psbt.serialize())})).await, "PSBT_VALIDATION_FAILED");
}

#[tokio::test]
async fn finalize_rejects_out_of_range_previous_output() {
    use bitcoin::{absolute, transaction, Amount, OutPoint, ScriptBuf, Sequence, Transaction, TxIn, TxOut, Witness};
    let prev = Transaction {
        version: transaction::Version::TWO,
        lock_time: absolute::LockTime::ZERO,
        input: vec![TxIn { previous_output: OutPoint::null(), script_sig: ScriptBuf::new(), sequence: Sequence::MAX, witness: Witness::new() }],
        output: vec![TxOut { value: Amount::from_sat(1000), script_pubkey: ScriptBuf::new_op_return([]) }],
    };
    let spend = Transaction {
        version: transaction::Version::TWO,
        lock_time: absolute::LockTime::ZERO,
        input: vec![TxIn { previous_output: OutPoint { txid: prev.compute_txid(), vout: 5 }, script_sig: ScriptBuf::new(), sequence: Sequence::MAX, witness: Witness::new() }],
        output: vec![TxOut { value: Amount::from_sat(500), script_pubkey: ScriptBuf::new_op_return([]) }],
    };
    let mut psbt = Psbt::from_unsigned_tx(spend).unwrap();
    psbt.inputs[0].non_witness_utxo = Some(prev);
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.pair().await;
    assert_eq!(c.fails("psbt.finalize", json!({"psbt": b64(&psbt.serialize())})).await, "PSBT_VALIDATION_FAILED");
    let other = c.ok("pwc.getCapabilities", json!({})).await;
    assert_eq!(other["pwc_version"], "0.2.1", "the Provider is still serving");
}
