use std::collections::BTreeSet;

use pwc_provider::types::*;
use serde::de::DeserializeOwned;
use serde_json::Value;

fn vectors(name: &str) -> Value {
    let path = format!("{}/../pwc-testkit/vectors/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn accepts<T: DeserializeOwned>(v: &Value) -> bool {
    serde_json::from_value::<T>(v.clone()).is_ok()
}

#[test]
fn semantic_type_vectors() {
    let mut skipped = BTreeSet::new();
    for v in vectors("types.json")["vectors"].as_array().unwrap() {
        let ty = v["type"].as_str().unwrap();
        let value = &v["value"];
        let got = match ty {
            "Sats" => accepts::<Sats>(value),
            "PositiveSats" => accepts::<PositiveSats>(value),
            "SignedSats" => accepts::<SignedSats>(value),
            "FeeRate" => accepts::<FeeRate>(value),
            "Id" => accepts::<Id>(value),
            "Hex" => accepts::<Hex>(value),
            "Base64" => accepts::<Base64>(value),
            "Hash256" => accepts::<Hash256>(value),
            "Timestamp" => accepts::<Timestamp>(value),
            "Text" => accepts::<Text>(value),
            "UInt32" => accepts::<u32>(value),
            "PsbtVersion" => accepts::<PsbtVersion>(value),
            "Network" => accepts::<Network>(value),
            _ => {
                skipped.insert(ty.to_string());
                continue;
            }
        };
        assert_eq!(got, v["valid"].as_bool().unwrap(), "{ty} {value}");
    }
    // Composite vectors describe Provider outputs or method params; they are covered by the schema checks
    // of the integration tests and by the method parsers.
    let expected: BTreeSet<String> = [
        "Utxo", "TransactionRecord", "FinalizePsbtResult", "Operation", "WalletSummary", "CreatePsbtResult",
        "PwcEvent", "psbt.combine.params", "session.requestPermissions.params",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(skipped, expected);
}

#[test]
fn fee_vectors() {
    for v in vectors("misc.json")["fees"]["vectors"].as_array().unwrap() {
        let rate: u64 = v["sat_per_kvb"].as_str().unwrap().parse().unwrap();
        let vsize = v["vsize"].as_u64().unwrap();
        assert_eq!(min_fee(rate, vsize).to_string(), v["fee_sats"].as_str().unwrap(), "{}", v["id"]);
    }
}

#[test]
fn local_auth_vectors() {
    for v in vectors("misc.json")["local_auth"]["vectors"].as_array().unwrap() {
        let b = |k: &str| -> [u8; 32] { bitcoin::hex::FromHex::from_hex(v[k].as_str().unwrap()).unwrap() };
        let id = v["client_id"].as_str().unwrap();
        let (k, sn, cn) = (b("client_secret"), b("server_nonce"), b("client_nonce"));
        assert_eq!(pwc_provider::local::client_proof(&k, &sn, &cn, id), b("client_proof"));
        assert_eq!(pwc_provider::local::server_proof(&k, &sn, &cn, id), b("server_proof"));
    }
}

#[test]
fn error_codes_serialize_as_registered_tokens() {
    use pwc_provider::error::{ErrorCode, PwcError};
    let e = PwcError::new(ErrorCode::UnsupportedPsbtVersion, "v2");
    assert_eq!(serde_json::to_value(&e).unwrap(), serde_json::json!({"code": "UNSUPPORTED_PSBT_VERSION", "message": "v2"}));
    let registry: Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{}/../pwc-schema/pwc-core-0.2.1.strict.schema.json", env!("CARGO_MANIFEST_DIR")))
            .unwrap(),
    )
    .unwrap();
    let codes = registry["$defs"]["ErrorCode"]["anyOf"][0]["enum"].as_array().unwrap();
    assert_eq!(codes.len(), ErrorCode::ALL.len());
    for c in ErrorCode::ALL {
        assert!(codes.contains(&serde_json::to_value(c).unwrap()), "{c:?}");
    }
}

#[test]
fn balance_vectors() {
    for v in vectors("balance.json")["vectors"].as_array().unwrap() {
        let scenario: pwc_provider::balance::Scenario = serde_json::from_value(v["scenario"].clone()).unwrap();
        let got = serde_json::to_value(pwc_provider::balance::compute(&scenario).components).unwrap();
        assert_eq!(got, v["expected"], "{}", v["id"]);
    }
}

/// BIP-322 test vectors for the P2WPKH address of private key L3VFeEujGtevx9w18HD1fhRbCH67Az2dpCymeRE1SoPK6XQtaN2k.
#[test]
fn bip322_vectors() {
    use pwc_provider::message::*;
    let hex = |b: &[u8]| bitcoin::hex::DisplayHex::to_lower_hex_string(b);
    assert_eq!(hex(&bip322_hash(b"")), "c90c269c4f8fcbe6880f72a721ddfbf1914268a794cbb21cfafee13770ae19f1");
    assert_eq!(hex(&bip322_hash(b"Hello World")), "f0eb03b1a75ac6d9847f55c624a99169b5dccba2a31f5b23bea77ba270de0a7a");
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let address: bitcoin::Address = "bc1q9vza2e8x573nczrlzms0wvx3gsqjx7vavgkx0l".parse::<bitcoin::Address<_>>().unwrap().assume_checked();
    let b64 = |s: &str| base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s).unwrap();
    let empty = b64("AkcwRAIgM2gBAQqvZX15ZiysmKmQpDrG83avLIT492QBzLnQIxYCIBaTpOaD20qRlEylyxFSeEA2ba9YOixpX8z46TSDtS40ASECx/EgAxlkQpQ9hYjgGu6EBCPMVPwVIVJqO4XCsMvViHI=");
    let hello = b64("AkcwRAIgZRfIY3p7/DoVTty6YZbWS71bc5Vct9p9Fia83eRmw2QCICK/ENGfwLtptFluMGs2KsqoNSk89pO7F29zJLUx9a/sASECx/EgAxlkQpQ9hYjgGu6EBCPMVPwVIVJqO4XCsMvViHI=");
    assert_eq!(verify_bip322_simple(&secp, &address, b"", &empty), Ok(true));
    assert_eq!(verify_bip322_simple(&secp, &address, b"Hello World", &hello), Ok(true));
    assert_eq!(verify_bip322_simple(&secp, &address, b"Hello World", &empty), Ok(false));
    assert!(verify_bip322_simple(&secp, &address, b"", b"\x05garbage").is_err());

    let key: bitcoin::PrivateKey = "L3VFeEujGtevx9w18HD1fhRbCH67Az2dpCymeRE1SoPK6XQtaN2k".parse().unwrap();
    let ours = sign_bip322_simple(&secp, &key.inner, "Hello World".as_bytes());
    assert_eq!(verify_bip322_simple(&secp, &address, b"Hello World", &ours), Ok(true));
}

#[test]
fn legacy_message_signature_round_trip() {
    use pwc_provider::message::*;
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let key = bitcoin::secp256k1::SecretKey::from_slice(&[3u8; 32]).unwrap();
    let pk = bitcoin::CompressedPublicKey(key.public_key(&secp));
    let address = bitcoin::Address::p2pkh(pk, bitcoin::Network::Bitcoin);
    let sig = sign_legacy(&secp, &key, "Grüße \t ");
    assert_eq!(sig.len(), 65);
    assert_eq!(verify_legacy(&secp, &address, "Grüße \t ", &sig), Ok(true));
    assert_eq!(verify_legacy(&secp, &address, "Grüße", &sig), Ok(false));
    assert!(verify_legacy(&secp, &address, "x", &[1, 2, 3]).is_err());
}
