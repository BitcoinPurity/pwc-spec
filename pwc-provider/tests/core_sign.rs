//! `psbt.sign`, message signing and the end-to-end payment flow (Core §8.3, §8.4).

mod common;

use std::time::Duration;

use base64::Engine;
use bitcoin::Psbt;
use common::{Conn, Harness};
use pwc_provider::prompt::Prompt;
use pwc_provider::provider::Config;
use serde_json::{json, Value};

const SCOPES: &[&str] = &[
    "balances:read", "utxos:read", "transactions:read", "psbt:create", "psbt:sign", "transactions:broadcast", "messages:sign",
];

fn external() -> String {
    let key = bitcoin::secp256k1::SecretKey::from_slice(&[7u8; 32]).unwrap();
    let pk = bitcoin::CompressedPublicKey(key.public_key(&bitcoin::secp256k1::Secp256k1::new()));
    bitcoin::Address::p2wpkh(&pk, bitcoin::Network::Regtest).to_string()
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn decode(s: &Value) -> Psbt {
    Psbt::deserialize(&base64::engine::general_purpose::STANDARD.decode(s.as_str().unwrap()).unwrap()).unwrap()
}

async fn create(c: &mut Conn, account: &str, amount: u64, rate: &str, key: &str) -> Value {
    let p = json!({"account_id": account, "outputs": [{"address": external(), "amount_sats": amount.to_string()}],
        "fee_policy": {"type": "rate", "sat_per_kvb": rate}, "idempotency_key": key});
    let op = c.ok("psbt.create", p).await;
    op["result"].clone()
}

fn sign_params(psbt: &Value, key: &str) -> Value {
    json!({"account_id": "a-regtest-wpkh", "psbt": psbt, "idempotency_key": key})
}

impl Harness {
    async fn decide(&mut self, yes: bool) -> String {
        match self.next_prompt().await {
            Prompt::SignPsbt { request, reply } => {
                let _ = reply.send(yes);
                request.to_string()
            }
            Prompt::SignMessage { request, reply } => {
                let _ = reply.send(yes);
                request.to_string()
            }
            _ => panic!("expected a signing prompt"),
        }
    }
}

/// Signs with approval and returns the completed Operation.
async fn signed(h: &mut Harness, c: &mut Conn, params: Value) -> Value {
    let op = c.ok("psbt.sign", params).await;
    assert_eq!(op["state"], "awaiting_approval");
    h.decide(true).await;
    let done = c.wait(op["operation_id"].as_str().unwrap()).await;
    assert_eq!(done["state"], "completed", "{done}");
    done
}

#[tokio::test]
async fn end_to_end_payment_c09_c15_c24_c54() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let created = create(&mut c, "a-regtest-wpkh", 300_000, "1250", "c").await;
    let done = signed(&mut h, &mut c, sign_params(&created["psbt"], "s")).await;
    let r = &done["result"];
    assert_eq!((r["signed_input_indexes"].clone(), r["signing_status"].clone()), (json!([0]), json!("complete")));
    let psbt = decode(&r["psbt"]);
    assert_eq!(psbt.unsigned_tx, decode(&created["psbt"]).unsigned_tx, "same transaction");
    assert!(psbt.inputs[0].final_script_witness.is_none(), "not finalized");
    assert_eq!(c.ok("account.getBalance", json!({"account_id": "a-regtest-wpkh"})).await["pending_outgoing_sats"], "0", "not broadcast");

    let fin = c.ok("psbt.finalize", json!({"psbt": r["psbt"]})).await;
    assert_eq!(fin["complete"], true);
    let raw = fin["raw_transaction"].clone();
    let b = json!({"account_id": "a-regtest-wpkh", "raw_transaction": raw, "idempotency_key": "b"});
    let op = c.ok("transaction.broadcast", b).await;
    assert_eq!((op["state"].clone(), op["result"]["txid"].clone()), (json!("completed"), fin["txid"].clone()));
    let t = c.ok("transaction.get", json!({"account_id": "a-regtest-wpkh", "txid": fin["txid"]})).await;
    assert_eq!(t["transaction"]["status"], "unconfirmed");
    assert_eq!(t["transaction"]["fee_sats"], created["fee_sats"]);

    let bal = c.ok("account.getBalance", json!({"account_id": "a-regtest-wpkh"})).await;
    let change = 1_000_000 - 300_000 - created["fee_sats"].as_str().unwrap().parse::<u64>().unwrap();
    assert_eq!(bal["pending_self_sats"], json!(change.to_string()));
    assert_eq!(bal["pending_outgoing_sats"], "1000000");
    assert_eq!(bal["locked_sats"], "0", "the reservation ended when the spend entered the projection");

    let again = json!({"account_id": "a-regtest-wpkh", "raw_transaction": raw, "idempotency_key": "b2"});
    let op = c.ok("transaction.broadcast", again).await;
    assert_eq!((op["state"].clone(), op["result"]["txid"].clone()), (json!("completed"), fin["txid"].clone()), "C-54");
    h.backend.mine("regtest", 1);
    let t = c.ok("transaction.get", json!({"account_id": "a-regtest-wpkh", "txid": fin["txid"]})).await;
    assert_eq!(t["transaction"]["confirmations"], 1);
}

#[tokio::test]
async fn signing_prompt_derives_from_the_psbt() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let created = create(&mut c, "a-regtest-wpkh", 300_000, "1250", "c").await;
    c.ok("psbt.sign", sign_params(&created["psbt"], "s")).await;
    let shown = h.decide(false).await;
    assert!(shown.contains(&external()) && shown.contains("300000") && shown.contains("back to this account"), "{shown}");
}

#[tokio::test]
async fn refused_signing_is_rejected() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let created = create(&mut c, "a-regtest-wpkh", 300_000, "1250", "c").await;
    let op = c.ok("psbt.sign", sign_params(&created["psbt"], "s")).await;
    h.decide(false).await;
    let done = c.wait(op["operation_id"].as_str().unwrap()).await;
    assert_eq!((done["state"].clone(), done["error"]["code"].clone()), (json!("rejected"), json!("USER_REJECTED")));
}

#[tokio::test]
async fn c03_revocation_during_pending_signing() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let created = create(&mut c, "a-regtest-wpkh", 300_000, "1250", "c").await;
    let op = c.ok("psbt.sign", sign_params(&created["psbt"], "s")).await;
    let Prompt::SignPsbt { reply, .. } = h.next_prompt().await else { panic!() };
    for g in h.provider.grants() {
        h.provider.revoke_grant(g["grant_id"].as_str().unwrap());
    }
    tokio::time::timeout(common::WAIT, async {
        while !reply.is_closed() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("prompt withdrawn");
    let done = c.wait(op["operation_id"].as_str().unwrap()).await;
    assert_eq!((done["state"].clone(), done["error"]["code"].clone()), (json!("failed"), json!("PERMISSION_REVOKED")));
}

#[tokio::test]
async fn c04_watch_only_and_c13_version_2() {
    let f: Value = serde_json::from_str(&std::fs::read_to_string(format!("{}/../pwc-testkit/vectors/psbt.json", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap();
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let created = create(&mut c, "a-regtest-watch", 100_000, "1250", "c").await;
    let p = json!({"account_id": "a-regtest-watch", "psbt": created["psbt"], "idempotency_key": "s"});
    assert_eq!(c.fails("psbt.sign", p).await, "UNSUPPORTED_OPERATION");
    assert_eq!(c.fails("psbt.sign", sign_params(&f["psbts"]["version2"], "v2")).await, "UNSUPPORTED_PSBT_VERSION");
    assert_eq!(c.fails("psbt.sign", sign_params(&json!(b64(b"junk")), "j")).await, "PSBT_VALIDATION_FAILED");
}

#[tokio::test]
async fn c05_c06_c59_idempotent_signing() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let first = create(&mut c, "a-regtest-wpkh", 300_000, "1250", "c1").await;
    let second = create(&mut c, "a-regtest-wpkh", 200_000, "1250", "c2").await;
    let done = signed(&mut h, &mut c, sign_params(&first["psbt"], "s")).await;
    let retry = c.ok("psbt.sign", sign_params(&first["psbt"], "s")).await;
    assert_eq!(retry, done);
    h.no_prompt().await;
    assert_eq!(c.fails("psbt.sign", sign_params(&second["psbt"], "s")).await, "IDEMPOTENCY_CONFLICT");
    for g in h.provider.grants() {
        h.provider.revoke_grant(g["grant_id"].as_str().unwrap());
    }
    let after = c.ok("psbt.sign", sign_params(&first["psbt"], "s")).await;
    assert_eq!((after["operation_id"].clone(), after["state"].clone()), (done["operation_id"].clone(), json!("completed")));
    assert!(after.get("result").is_none(), "C-59");
}

#[tokio::test]
async fn c07_inconsistent_input_values() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let mut psbt = decode(&create(&mut c, "a-regtest-wpkh", 300_000, "1250", "c").await["psbt"]);
    psbt.inputs[0].witness_utxo.as_mut().unwrap().value = bitcoin::Amount::from_sat(5_000_000);
    assert_eq!(c.fails("psbt.sign", sign_params(&json!(b64(&psbt.serialize())), "s")).await, "PSBT_VALIDATION_FAILED");
    h.no_prompt().await;
}

#[tokio::test]
async fn c08_high_fee_and_nondefault_sighash() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let high = create(&mut c, "a-regtest-wpkh", 300_000, "600000", "c1").await;
    assert_eq!(c.fails("psbt.sign", sign_params(&high["psbt"], "s1")).await, "POLICY_VIOLATION");
    let mut psbt = decode(&create(&mut c, "a-regtest-wpkh", 100_000, "1250", "c2").await["psbt"]);
    psbt.inputs[0].sighash_type = Some(bitcoin::EcdsaSighashType::Single.into());
    assert_eq!(c.fails("psbt.sign", sign_params(&json!(b64(&psbt.serialize())), "s2")).await, "POLICY_VIOLATION");
    psbt.inputs[0].sighash_type = Some(bitcoin::psbt::PsbtSighashType::from_u32(0x55));
    assert_eq!(c.fails("psbt.sign", sign_params(&json!(b64(&psbt.serialize())), "s3")).await, "UNSUPPORTED_SCRIPT");
    h.no_prompt().await;
}

/// A PSBT spending both demo P2WPKH coins, plus a foreign input when `foreign` is set.
async fn two_input_psbt(c: &mut Conn, foreign: Option<u64>, pay_back: u64) -> Psbt {
    let utxos = c.ok("utxo.list", json!({"account_id": "a-regtest-wpkh"})).await;
    let utxos = utxos["utxos"].as_array().unwrap();
    let mut input = Vec::new();
    let mut prev = Vec::new();
    for u in utxos {
        input.push(bitcoin::TxIn { previous_output: bitcoin::OutPoint { txid: u["txid"].as_str().unwrap().parse().unwrap(), vout: 0 }, ..Default::default() });
        prev.push(bitcoin::TxOut {
            value: bitcoin::Amount::from_sat(u["value_sats"].as_str().unwrap().parse().unwrap()),
            script_pubkey: bitcoin::ScriptBuf::from_hex(u["script_pub_key"].as_str().unwrap()).unwrap(),
        });
    }
    let own_script = prev[0].script_pubkey.clone();
    if let Some(v) = foreign {
        let ext: bitcoin::Address<_> = external().parse().unwrap();
        input.push(bitcoin::TxIn { previous_output: bitcoin::OutPoint { txid: bitcoin::Txid::from_raw_hash(bitcoin::hashes::Hash::all_zeros()), vout: 3 }, ..Default::default() });
        prev.push(bitcoin::TxOut { value: bitcoin::Amount::from_sat(v), script_pubkey: ext.assume_checked().script_pubkey() });
    }
    let tx = bitcoin::Transaction {
        version: bitcoin::transaction::Version::TWO,
        lock_time: bitcoin::absolute::LockTime::ZERO,
        input,
        output: vec![bitcoin::TxOut { value: bitcoin::Amount::from_sat(pay_back), script_pubkey: own_script }],
    };
    let mut psbt = Psbt::from_unsigned_tx(tx).unwrap();
    for (i, p) in prev.into_iter().enumerate() {
        psbt.inputs[i].witness_utxo = Some(p);
    }
    psbt
}

#[tokio::test]
async fn c10_c41_c42_c43_input_selection() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let psbt = two_input_psbt(&mut c, Some(10_000), 1_400_000).await;
    let s = json!(b64(&psbt.serialize()));
    let with = |idx: Value, key: &str| json!({"account_id": "a-regtest-wpkh", "psbt": s, "input_indexes": idx, "idempotency_key": key});
    assert_eq!(c.fails("psbt.sign", with(json!([]), "e")).await, "INVALID_ARGUMENT");
    assert_eq!(c.fails("psbt.sign", with(json!([0, 0]), "d")).await, "INVALID_ARGUMENT");
    assert_eq!(c.fails("psbt.sign", with(json!([5]), "r")).await, "INVALID_ARGUMENT");
    assert_eq!(c.fails("psbt.sign", with(json!([2]), "f")).await, "UNSUPPORTED_OPERATION", "C-42: foreign input");
    let done = signed(&mut h, &mut c, with(json!([0]), "zero")).await;
    let r = &done["result"];
    assert_eq!((r["signed_input_indexes"].clone(), r["signing_status"].clone()), (json!([0]), json!("partial")));
    let after = decode(&r["psbt"]);
    assert_eq!(after.inputs[0].partial_sigs.len(), 1);
    assert!(after.inputs[1].partial_sigs.is_empty(), "C-10: no other input signed");
    let again = signed(&mut h, &mut c, json!({"account_id": "a-regtest-wpkh", "psbt": r["psbt"], "input_indexes": [0], "idempotency_key": "again"})).await;
    assert_eq!(again["result"]["signed_input_indexes"], json!([]), "C-43");
    let all = signed(&mut h, &mut c, json!({"account_id": "a-regtest-wpkh", "psbt": r["psbt"], "idempotency_key": "rest"})).await;
    assert_eq!(all["result"]["signed_input_indexes"], json!([1]));
    assert_eq!(all["result"]["signing_status"], "partial", "the foreign input remains unsigned");
}

#[tokio::test]
async fn c32_c61_amount_limit_on_signing() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.pair().await;
    h.grant(&mut c, json!({"scopes": SCOPES, "constraints": {"max_amount_sats_per_request": "1000"}})).await;
    let outflow = two_input_psbt(&mut c, None, 1_400_000).await;
    let p = sign_params(&json!(b64(&outflow.serialize())), "over");
    assert_eq!(c.fails("psbt.sign", p).await, "GRANT_CONSTRAINT_VIOLATED");
    h.no_prompt().await;
    let inflow = two_input_psbt(&mut c, Some(400_000), 1_899_000).await;
    signed(&mut h, &mut c, sign_params(&json!(b64(&inflow.serialize())), "in")).await;
}

#[tokio::test]
async fn c63_inputs_from_another_network() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    h.grant(&mut c, json!({"scopes": ["psbt:create"], "network": "signet"})).await;
    let key = bitcoin::secp256k1::SecretKey::from_slice(&[7u8; 32]).unwrap();
    let pk = bitcoin::CompressedPublicKey(key.public_key(&bitcoin::secp256k1::Secp256k1::new()));
    let p = json!({"account_id": "a-signet-wpkh", "outputs": [{"address": bitcoin::Address::p2wpkh(&pk, bitcoin::Network::Signet).to_string(), "amount_sats": "1000"}],
        "fee_policy": {"type": "rate", "sat_per_kvb": "1250"}, "idempotency_key": "sig"});
    let signet = c.ok("psbt.create", p).await;
    assert_eq!(c.fails("psbt.sign", sign_params(&signet["result"]["psbt"], "x")).await, "UNSUPPORTED_OPERATION");
}

#[tokio::test]
async fn approval_expires() {
    let config = Config { approval_timeout: Duration::from_millis(200), ..Config::default() };
    let mut h = Harness::start(config).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let created = create(&mut c, "a-regtest-wpkh", 300_000, "1250", "c").await;
    let op = c.ok("psbt.sign", sign_params(&created["psbt"], "s")).await;
    let _prompt = h.next_prompt().await;
    assert_eq!(c.wait(op["operation_id"].as_str().unwrap()).await["state"], "expired");
}

async fn address_of(c: &mut Conn, account: &str) -> String {
    let utxos = c.ok("utxo.list", json!({"account_id": account})).await;
    utxos["utxos"][0]["address"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn c45_message_signatures_verify_over_exact_bytes() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let message = "Zahlung an Café – 支付 ✓  \n\t ";
    for (account, format) in [("a-regtest-wpkh", "bip322-simple"), ("a-regtest-pkh", "legacy")] {
        let address = address_of(&mut c, account).await;
        let p = json!({"account_id": account, "address": address, "message": message, "format": format, "idempotency_key": format});
        let op = c.ok("message.sign", p).await;
        let shown = h.decide(true).await;
        assert!(shown.contains(&serde_json::to_string(message).unwrap()), "{shown}");
        let done = c.wait(op["operation_id"].as_str().unwrap()).await;
        let sig = done["result"]["signature"].clone();
        assert_eq!((done["result"]["format"].clone(), done["result"]["address"].clone()), (json!(format), json!(address)));
        let verify = |m: &str| json!({"network": "regtest", "address": address, "message": m, "signature": sig, "format": format});
        assert_eq!(c.ok("message.verify", verify(message)).await, json!({"valid": true}), "{format}");
        assert_eq!(c.ok("message.verify", verify(message.trim_end())).await, json!({"valid": false}), "{format}");
    }
}

#[tokio::test]
async fn message_sign_checks() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.client(SCOPES).await;
    let wpkh = address_of(&mut c, "a-regtest-wpkh").await;
    let p = |account: &str, address: &str, format: &str, key: &str| {
        json!({"account_id": account, "address": address, "message": "hi", "format": format, "idempotency_key": key})
    };
    assert_eq!(c.fails("message.sign", p("a-regtest-wpkh", &wpkh, "legacy", "1")).await, "UNSUPPORTED_OPERATION");
    assert_eq!(c.fails("message.sign", p("a-regtest-wpkh", &wpkh, "bip322-full", "2")).await, "UNSUPPORTED_OPERATION");
    assert_eq!(c.fails("message.sign", p("a-regtest-wpkh", &external(), "bip322-simple", "3")).await, "INVALID_ARGUMENT");
    assert_eq!(c.fails("message.sign", p("a-regtest-pkh", &wpkh, "bip322-simple", "4")).await, "INVALID_ARGUMENT");
    let mainnet = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4";
    assert_eq!(c.fails("message.sign", p("a-regtest-wpkh", mainnet, "bip322-simple", "5")).await, "NETWORK_MISMATCH");
    assert_eq!(c.fails("message.sign", p("a-regtest-watch", &wpkh, "bip322-simple", "6")).await, "UNSUPPORTED_OPERATION");
    let mut big = p("a-regtest-wpkh", &wpkh, "bip322-simple", "7");
    big["message"] = json!("x".repeat(65_537));
    assert_eq!(c.fails("message.sign", big).await, "PAYLOAD_TOO_LARGE");
    h.no_prompt().await;
}

#[tokio::test]
async fn c60_message_verify_checks() {
    let mut h = Harness::start(Config::default()).await;
    let (mut c, _, _) = h.pair().await;
    let addr = "bc1q9vza2e8x573nczrlzms0wvx3gsqjx7vavgkx0l";
    let sig = "AkcwRAIgZRfIY3p7/DoVTty6YZbWS71bc5Vct9p9Fia83eRmw2QCICK/ENGfwLtptFluMGs2KsqoNSk89pO7F29zJLUx9a/sASECx/EgAxlkQpQ9hYjgGu6EBCPMVPwVIVJqO4XCsMvViHI=";
    let v = |network: &str, address: &str, signature: &str, format: &str| {
        json!({"network": network, "address": address, "message": "Hello World", "signature": signature, "format": format})
    };
    assert_eq!(c.ok("message.verify", v("mainnet", addr, sig, "bip322-simple")).await, json!({"valid": true}));
    assert_eq!(c.fails("message.verify", v("testnet3", addr, sig, "bip322-simple")).await, "UNSUPPORTED_OPERATION");
    assert_eq!(c.fails("message.verify", v("regtest", addr, sig, "bip322-simple")).await, "NETWORK_MISMATCH");
    assert_eq!(c.fails("message.verify", v("mainnet", addr, sig, "legacy")).await, "UNSUPPORTED_OPERATION");
    assert_eq!(c.fails("message.verify", v("mainnet", "nonsense", sig, "bip322-simple")).await, "INVALID_ARGUMENT");
    assert_eq!(c.fails("message.verify", v("mainnet", addr, "AAEC", "bip322-simple")).await, "INVALID_ARGUMENT");
}
