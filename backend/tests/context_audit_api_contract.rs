use serde::Deserialize;

#[derive(Deserialize)]
struct Contract {
    schema_version: String,
    source_runtimes: Vec<String>,
    record_types: Vec<String>,
    list_envelope_fields: Vec<String>,
    summary_fields: Vec<String>,
    detail_envelope_fields: Vec<String>,
    snapshot_record_fields: Vec<String>,
}

#[test]
fn context_audit_read_contract_is_shared_with_pi() {
    let contract: Contract = serde_json::from_str(include_str!(
        "../../agent-definitions/fixtures/context-audit-read-api.json"
    ))
    .unwrap();
    assert_eq!(contract.schema_version, "2");
    assert_eq!(contract.source_runtimes, ["rust", "pi"]);
    assert_eq!(contract.record_types, ["snapshot", "compile_attempt"]);
    assert_eq!(
        contract.list_envelope_fields,
        [
            "schema_version",
            "source_runtime",
            "items",
            "total",
            "limit",
            "offset"
        ]
    );
    assert_eq!(
        contract.detail_envelope_fields,
        ["schema_version", "source_runtime", "record_hash", "record"]
    );
    assert!(!contract
        .summary_fields
        .iter()
        .any(|field| field == "decisions"));
    assert!(!contract
        .summary_fields
        .iter()
        .any(|field| field == "logical_input"));
    assert!(contract
        .snapshot_record_fields
        .iter()
        .any(|field| field == "decisions"));
    assert!(contract
        .snapshot_record_fields
        .iter()
        .any(|field| field == "logical_input"));
}
