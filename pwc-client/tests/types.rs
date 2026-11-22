//! TASK-011: semantic types, error codes and tolerance of unknown members.

mod common;

use pwc_client::{Error, ErrorCode, Operation, OperationState, PwcError};
use serde::Deserialize;
use serde_json::{json, Value};

// Only parsed, never read.
#[allow(dead_code)]
#[derive(Deserialize)]
struct S(#[serde(with = "pwc_client::types::sats")] u64);
#[allow(dead_code)]
#[derive(Deserialize)]
struct P(#[serde(with = "pwc_client::types::positive_sats")] u64);
#[allow(dead_code)]
#[derive(Deserialize)]
struct G(#[serde(with = "pwc_client::types::signed_sats")] i64);
#[allow(dead_code)]
#[derive(Deserialize)]
struct F(#[serde(with = "pwc_client::types::fee_rate")] u64);

fn parses(ty: &str, v: &Value) -> Option<bool> {
    let v = v.clone();
    Some(match ty {
        "Sats" => serde_json::from_value::<S>(v).is_ok(),
        "PositiveSats" => serde_json::from_value::<P>(v).is_ok(),
        "SignedSats" => serde_json::from_value::<G>(v).is_ok(),
        "FeeRate" => serde_json::from_value::<F>(v).is_ok(),
        _ => return None,
    })
}

#[test]
fn amount_vectors() {
    let path = format!("{}/../pwc-testkit/vectors/types.json", env!("CARGO_MANIFEST_DIR"));
    let file: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut checked = 0;
    for v in file["vectors"].as_array().unwrap() {
        if let Some(ok) = parses(v["type"].as_str().unwrap(), &v["value"]) {
            assert_eq!(ok, v["valid"].as_bool().unwrap(), "{v}");
            checked += 1;
        }
    }
    assert_eq!(checked, 21);
}

#[test]
fn amounts_are_written_as_decimal_strings() {
    let b = common::fixtures::balance();
    let parsed: pwc_client::Balance = serde_json::from_value(b.clone()).unwrap();
    assert_eq!(parsed.confirmed_sats, 1_000_000);
    assert_eq!(parsed.unconfirmed_delta_sats, -301_000);
    assert_eq!(serde_json::to_value(&parsed).unwrap(), b);
}

#[test]
fn unknown_error_code_is_kept() {
    let e: PwcError = serde_json::from_value(json!({"code": "vendor.com.example.quota", "message": "m"})).unwrap();
    assert_eq!(e.code, ErrorCode::Other("vendor.com.example.quota".into()));
    let e: PwcError =
        serde_json::from_value(json!({"code": "RATE_LIMITED", "message": "m", "retryable": true})).unwrap();
    assert_eq!(e.code, ErrorCode::RateLimited);
    assert_eq!(e.retryable, Some(true));
    assert_eq!(serde_json::to_value(&e.code).unwrap(), json!("RATE_LIMITED"));
}

#[test]
fn unknown_members_are_ignored() {
    let mut w = common::fixtures::wallet();
    w["vendor_field"] = json!(1);
    let w: pwc_client::WalletSummary = serde_json::from_value(w).unwrap();
    assert_eq!(w.wallet_id, "w-1");
}

#[test]
fn every_result_fixture_is_valid_and_parses() {
    for (def, value) in common::fixtures::all() {
        common::schema::core(def, &value).unwrap();
        common::fixtures::parse(def, value);
    }
}

#[test]
fn operation_result_accessors() {
    let done: Operation = serde_json::from_value(common::fixtures::operation(
        "psbt.sign",
        "completed",
        Some(common::fixtures::sign_result()),
    ))
    .unwrap();
    assert!(done.is_terminal());
    let r = done.sign_psbt_result().unwrap();
    assert_eq!(r.signed_input_indexes, vec![0]);

    let waiting: Operation =
        serde_json::from_value(common::fixtures::operation("psbt.sign", "awaiting_approval", None)).unwrap();
    assert_eq!(waiting.state, OperationState::AwaitingApproval);
    assert!(!waiting.is_terminal());
    assert!(matches!(waiting.sign_psbt_result(), Err(Error::OperationPending)));

    let rejected: Operation =
        serde_json::from_value(common::fixtures::operation("psbt.sign", "rejected", None)).unwrap();
    match rejected.sign_psbt_result() {
        Err(Error::Pwc(e)) => assert_eq!(e.code, ErrorCode::UserRejected),
        other => panic!("{other:?}"),
    }

    // Completed, but the result is withheld (scope no longer held, or wallet locked: Core §4.8).
    let withheld: Operation =
        serde_json::from_value(common::fixtures::operation("psbt.sign", "completed", None)).unwrap();
    assert!(matches!(withheld.sign_psbt_result(), Err(Error::ResultUnavailable)));

    let grants: Operation = serde_json::from_value(common::fixtures::operation(
        "session.requestPermissions",
        "completed",
        Some(json!({"grants": [common::fixtures::grant()]})),
    ))
    .unwrap();
    assert_eq!(grants.permissions_result().unwrap()[0].grant_id, "g-1");
}
