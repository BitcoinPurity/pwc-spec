mod common;

use common::schema;
use serde_json::json;

#[test]
fn strict_core_schema_resolves_and_judges() {
    let ok = json!({"code": "INVALID_ARGUMENT", "message": "x"});
    assert!(schema::core("PwcError", &ok).is_ok());
    let extra = json!({"code": "INVALID_ARGUMENT", "message": "x", "internal": 1});
    assert!(schema::core("PwcError", &extra).is_err());
}

#[test]
fn local_schema_resolves_cross_file_refs() {
    let frame = json!({"jsonrpc": "2.0", "id": null,
        "error": {"code": 1000, "message": "m", "data": {"code": "PAYLOAD_TOO_LARGE", "message": "m"}}});
    assert!(schema::local("ProviderFrame", &frame).is_ok());
    let bad = json!({"jsonrpc": "2.0", "id": null,
        "error": {"code": 1000, "message": "m", "data": {"code": "NOT_A_CODE", "message": "m"}}});
    assert!(schema::local("ProviderFrame", &bad).is_err());
}
