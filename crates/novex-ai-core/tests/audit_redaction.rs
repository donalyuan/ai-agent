use novex_ai_core::{
    redact_audit_value, validate_asset_references, validate_audit_payload, AUDIT_REDACTED,
    MODEL_CALL_SCHEMA_VERSION,
};
use serde_json::{json, Value};

#[test]
fn redacts_shared_model_call_secrets_without_dropping_business_text() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../backend/tests/fixtures/model_call_safety.json"
    ))
    .unwrap();
    let redacted = redact_audit_value(
        &json!({
            "fixture": fixture,
            "schema_secret": {"secret": true, "value": "schema-secret"},
            "message": "Authorization: Bearer raw-header-secret",
            "business_text": "保留业务文本"
        }),
        &[],
    );
    let serialized = serde_json::to_string(&redacted).unwrap();

    assert_eq!(MODEL_CALL_SCHEMA_VERSION, "1");
    assert!(!serialized.contains("NOVEX_CANARY_SECRET_DO_NOT_PERSIST"));
    assert!(!serialized.contains("schema-secret"));
    assert!(!serialized.contains("raw-header-secret"));
    assert!(!serialized.contains("api_key=NOVEX"));
    assert!(serialized.contains(AUDIT_REDACTED));
    assert!(serialized.contains("保留业务文本"));
}

#[test]
fn accepts_only_stable_asset_references_and_rejects_binary_or_signed_urls() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../backend/tests/fixtures/model_call_safety.json"
    ))
    .unwrap();
    validate_asset_references(&fixture["assets"]).unwrap();
    assert!(validate_asset_references(&json!([{
        "asset_id":"bad",
        "version":"1",
        "sha256":"short",
        "mime":"image/png",
        "url":"https://example.invalid/file"
    }]))
    .is_err());
    assert!(validate_audit_payload(&json!("data:image/png;base64,AAAA")).is_err());
    assert!(validate_audit_payload(&json!(
        "https://assets.invalid/a.png?X-Amz-Signature=secret&X-Amz-Expires=60"
    ))
    .is_err());
}
