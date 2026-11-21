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
