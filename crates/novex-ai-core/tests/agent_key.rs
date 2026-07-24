use novex_ai_core::{AgentKey, AgentKeyError};

#[test]
fn agent_key_validates_equality_and_stable_json() {
    let key = AgentKey::new("script.rewrite").unwrap();
    assert_eq!(key, AgentKey::new("script.rewrite").unwrap());
    assert_eq!(key.as_str(), "script.rewrite");
    assert_eq!(serde_json::to_string(&key).unwrap(), r#""script.rewrite""#);
    assert_eq!(
        serde_json::from_str::<AgentKey>(r#""script.rewrite""#).unwrap(),
        key
    );
}

#[test]
fn agent_key_rejects_empty_or_unstable_values() {
    for value in ["", " ", "Script", "script/", "script..rewrite", "-script"] {
        assert_eq!(
            AgentKey::new(value),
            Err(AgentKeyError::Invalid(value.into()))
        );
    }
}
