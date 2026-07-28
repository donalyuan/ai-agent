use novex_ai_core::{
    behavior_fingerprint, canonical_json, validate_model_capabilities, DefinitionRegistry,
    DynamicFragment, ExecutorOwner, ModelBehavior, ModelCapabilities, ModelRequirements,
    PromptCompileInput, PromptCompiler, TrustLevel,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn registry_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("agent-definitions")
}

fn load_fixture_registry(name: &str) -> DefinitionRegistry {
    let root = registry_root();
    let bytes = fs::read(root.join("fixtures").join(name)).unwrap();
    let document: Value = serde_json::from_slice(&bytes).unwrap();
    load_registry_document(&document, None).unwrap()
}

fn load_registry_document(
    document: &Value,
    release_digest: Option<&str>,
) -> Result<DefinitionRegistry, String> {
    let root = registry_root();
    let directory = std::env::temp_dir().join(format!(
        "novex-definition-fixture-{}-{}",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(directory.join("templates")).unwrap();
    fs::write(
        directory.join("registry.json"),
        serde_json::to_vec(document).unwrap(),
    )
    .unwrap();
    for prompt in document["prompts"].as_array().unwrap() {
        for field in ["system_template", "user_template"] {
            let relative = prompt[field].as_str().unwrap();
            fs::copy(root.join(relative), directory.join(relative)).unwrap();
        }
    }
    let digest = release_digest
        .map(str::to_string)
        .unwrap_or_else(|| novex_ai_core::sha256_hex(canonical_json(document).as_bytes()));
    fs::write(
        directory.join("release-index.json"),
        serde_json::to_vec(&json!({
            "schema_version": document["schema_version"],
            "registry_digest": digest,
            "releases": []
        }))
        .unwrap(),
    )
    .unwrap();
    let registry = DefinitionRegistry::load(&directory).map_err(|error| error.to_string());
    fs::remove_dir_all(directory).unwrap();
    registry
}

#[test]
fn rust_loader_validates_registry_references_templates_and_owner() {
    let registry = DefinitionRegistry::load(registry_root()).unwrap();
    assert_eq!(registry.agents().len(), 30);
    assert_eq!(registry.prompts().len(), 44);
    assert_eq!(registry.context_policies().len(), 45);
    assert_eq!(registry.tokenizer_profiles().len(), 3);
    assert_eq!(registry.release_evidence().len(), 122);
    assert!(registry
        .active_agent("personal.general")
        .unwrap()
        .nodes
        .values()
        .all(|reference| reference.context_policy.is_some()));
    assert_eq!(
        registry
            .active_agent("personal.general")
            .unwrap()
            .executor_owner,
        ExecutorOwner::Pi
    );
    assert_eq!(registry.digest().len(), 64);
}

#[test]
fn registry_v2_requires_governed_references_and_rejects_invalid_policy_profile_contracts() {
    let root = registry_root();
    let valid: Value =
        serde_json::from_slice(&fs::read(root.join("fixtures/registry-valid-v2.json")).unwrap())
            .unwrap();
    let loaded = load_registry_document(&valid, None).unwrap();
    assert_eq!(loaded.context_policies().len(), 1);
    assert_eq!(loaded.tokenizer_profiles().len(), 1);

    let mut missing = valid.clone();
    missing["agents"][0]["nodes"]["fixture.node"]
        .as_object_mut()
        .unwrap()
        .remove("context_policy");
    assert!(load_registry_document(&missing, None)
        .unwrap_err()
        .contains("missing context policy"));

    let mut wrong_owner = valid.clone();
    wrong_owner["context_policies"][0]["executor_owners"] = json!(["pi"]);
    assert!(load_registry_document(&wrong_owner, None)
        .unwrap_err()
        .contains("incompatible context policy"));

    let mut duplicate = valid.clone();
    let duplicate_profile = duplicate["tokenizer_profiles"][0].clone();
    duplicate["tokenizer_profiles"]
        .as_array_mut()
        .unwrap()
        .push(duplicate_profile);
    assert!(load_registry_document(&duplicate, None)
        .unwrap_err()
        .contains("invalid contract"));

    let mut unknown = valid;
    unknown["context_policies"][0]["unknown"] = json!(true);
    assert!(load_registry_document(&unknown, None)
        .unwrap_err()
        .contains("unknown field"));
}

#[test]
fn production_crew_contract_changes_are_isolated_in_candidate_versions() {
    let registry = DefinitionRegistry::load(registry_root()).unwrap();
    let compiler = PromptCompiler::new(&registry);
    let affected_roles = [
        "producer",
        "screenwriter",
        "character_critic",
        "director",
        "cinematographer",
        "performance_director",
        "sound_director",
        "editor",
        "qc",
    ];

    for role in affected_roles {
        let agent_key = format!("production.{role}");
        let prompt_key = format!("production.{role}.general");
        let node_key = format!("production.{role}.execute");
        let policy_key = format!("production.{role}.execute.baseline");

        let active = registry.active_agent(&agent_key).unwrap();
        assert_eq!(active.version, "2.0.0");
        assert_eq!(active.nodes[&node_key].version, "2.0.0");
        assert_eq!(
            active.nodes[&node_key]
                .context_policy
                .as_ref()
                .unwrap()
                .version,
            "1.0.0"
        );

        let candidate = registry.agent(&agent_key, "3.0.0").unwrap();
        assert_eq!(candidate.status, novex_ai_core::DefinitionStatus::Candidate);
        assert_eq!(candidate.nodes[&node_key].key, prompt_key);
        assert_eq!(candidate.nodes[&node_key].version, "3.0.0");
        assert_eq!(
            candidate.nodes[&node_key].context_policy.as_ref().unwrap(),
            &novex_ai_core::DefinitionReference {
                key: policy_key.clone(),
                version: "3.0.0".into(),
            }
        );
        assert_eq!(
            registry
                .context_policy(&policy_key, "3.0.0")
                .unwrap()
                .status,
            novex_ai_core::DefinitionStatus::Candidate
        );
        assert_eq!(
            registry
                .context_policy(&policy_key, "3.0.0")
                .unwrap()
                .required_sources,
            ["project", "user_instruction"]
        );
        assert_eq!(
            registry
                .context_policy(&policy_key, "2.0.0")
                .unwrap()
                .required_sources,
            ["project", "script_revision_command", "user_instruction"]
        );

        let input = PromptCompileInput {
            schema_version: "1".into(),
            variables: BTreeMap::new(),
            fragments: vec![DynamicFragment {
                id: format!("candidate-contract:{role}"),
                trust: TrustLevel::Reference,
                source: "production_contract_fixture".into(),
                content: Some("受控 Full Crew 输入快照".into()),
                asset: None,
            }],
        };
        assert!(compiler
            .compile(&agent_key, "3.0.0", &node_key, input.clone(), "chat", None,)
            .is_err());
        let dry_run = compiler
            .compile_for_replay(&agent_key, "3.0.0", &node_key, input, "chat", None)
            .unwrap();
        assert_eq!(dry_run.agent_version, "3.0.0");
        assert_eq!(dry_run.prompt_version, "3.0.0");
        assert_eq!(dry_run.prompt_key, prompt_key);
        assert!(dry_run.system.contains("真实"));
        assert_eq!(
            dry_run.output_schema.as_ref().unwrap()["name"],
            format!("production_{role}_output_v3")
        );
        assert_eq!(dry_run.output_schema.as_ref().unwrap()["strict"], true);
        assert_eq!(
            dry_run.output_schema.as_ref().unwrap()["schema"]["additionalProperties"],
            false
        );
    }
}

#[test]
fn canonical_serialization_and_digest_match_the_cross_language_fixture() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../agent-definitions/fixtures/canonical.json"
    ))
    .unwrap();
    let canonical = canonical_json(&fixture["input"]);
    assert_eq!(canonical, fixture["canonical"]);
    assert_eq!(
        novex_ai_core::sha256_hex(canonical.as_bytes()),
        fixture["sha256"]
    );
}

#[test]
fn compiler_keeps_dynamic_content_in_user_and_fails_closed() {
    let registry = DefinitionRegistry::load(registry_root()).unwrap();
    let compiler = PromptCompiler::new(&registry);
    let snapshot = compiler
        .compile(
            "video.script",
            "1.0.0",
            "script.generation_intent",
            PromptCompileInput {
                schema_version: "1".into(),
                variables: BTreeMap::new(),
                fragments: vec![DynamicFragment {
                    id: "message-1".into(),
                    trust: TrustLevel::UserInstruction,
                    source: "agent_message".into(),
                    content: Some("用户动态内容".into()),
                    asset: None,
                }],
            },
            "chat",
            None,
        )
        .unwrap();
    assert!(!snapshot.system.contains("用户动态内容"));
    assert_eq!(snapshot.user, "用户动态内容");
    assert_eq!(snapshot.fragments[0].source, "agent_message");

    let error = compiler
        .compile(
            "video.script",
            "1.0.0",
            "script.unknown",
            PromptCompileInput {
                schema_version: "1".into(),
                variables: BTreeMap::new(),
                fragments: vec![],
            },
            "workspace",
            None,
        )
        .unwrap_err();
    assert!(error.to_string().contains("tool profile") || error.to_string().contains("node"));
}

#[test]
fn revoked_definitions_are_available_only_to_the_replay_compiler() {
    let mut document: Value = serde_json::from_str(include_str!(
        "../../../agent-definitions/fixtures/registry-valid.json"
    ))
    .unwrap();
    document["agents"][0]["status"] = json!("revoked");
    document["prompts"][0]["status"] = json!("revoked");
    let registry = load_registry_document(&document, None).unwrap();
    let compiler = PromptCompiler::new(&registry);
    let input = PromptCompileInput {
        schema_version: "1".into(),
        variables: BTreeMap::new(),
        fragments: vec![DynamicFragment {
            id: "history-1".into(),
            trust: TrustLevel::UserInstruction,
            source: "model_call".into(),
            content: Some("history".into()),
            asset: None,
        }],
    };
    assert!(compiler
        .compile(
            "fixture.agent",
            "1.0.0",
            "fixture.node",
            input.clone(),
            "chat",
            None,
        )
        .is_err());
    let replay = compiler
        .compile_for_replay(
            "fixture.agent",
            "1.0.0",
            "fixture.node",
            input,
            "chat",
            None,
        )
        .unwrap();
    assert_eq!(replay.user, "history");
    assert_eq!(replay.agent_key, "fixture.agent");
    assert_eq!(replay.prompt_key, "fixture.prompt");

    let mut invalid: Value = serde_json::from_str(include_str!(
        "../../../agent-definitions/fixtures/registry-valid.json"
    ))
    .unwrap();
    invalid["prompts"][0]["status"] = json!("revoked");
    assert!(load_registry_document(&invalid, None)
        .unwrap_err()
        .contains("executable agent references unavailable prompt"));
}

#[test]
fn lifecycle_switch_keeps_definition_digest_and_never_upgrades_bound_versions() {
    let root = registry_root();
    let mut document: Value =
        serde_json::from_slice(&fs::read(root.join("fixtures/registry-valid.json")).unwrap())
            .unwrap();
    let active_v1: novex_ai_core::AgentDefinition =
        serde_json::from_value(document["agents"][0].clone()).unwrap();
    let active_digest = novex_ai_core::definition_digest(&active_v1).unwrap();

    document["agents"][0]["status"] = json!("supported");
    document["prompts"][0]["status"] = json!("supported");
    let mut agent_v2 = document["agents"][0].clone();
    agent_v2["version"] = json!("2.0.0");
    agent_v2["status"] = json!("active");
    agent_v2["nodes"]["fixture.node"]["version"] = json!("2.0.0");
    let mut prompt_v2 = document["prompts"][0].clone();
    prompt_v2["version"] = json!("2.0.0");
    prompt_v2["status"] = json!("active");
    document["agents"].as_array_mut().unwrap().push(agent_v2);
    document["prompts"].as_array_mut().unwrap().push(prompt_v2);
    let registry = load_registry_document(&document, None).unwrap();
    assert_eq!(
        registry.active_agent("fixture.agent").unwrap().version,
        "2.0.0"
    );

    let input = PromptCompileInput {
        schema_version: "1".into(),
        variables: BTreeMap::new(),
        fragments: vec![DynamicFragment {
            id: "bound-v1".into(),
            trust: TrustLevel::UserInstruction,
            source: "session_binding".into(),
            content: Some("keep v1".into()),
            asset: None,
        }],
    };
    let v1 = PromptCompiler::new(&registry)
        .compile(
            "fixture.agent",
            "1.0.0",
            "fixture.node",
            input.clone(),
            "chat",
            None,
        )
        .unwrap();
    assert_eq!(v1.agent_version, "1.0.0");
    assert_eq!(v1.prompt_version, "1.0.0");

    document["agents"][0]["status"] = json!("revoked");
    document["prompts"][0]["status"] = json!("revoked");
    let revoked: novex_ai_core::AgentDefinition =
        serde_json::from_value(document["agents"][0].clone()).unwrap();
    assert_eq!(
        novex_ai_core::definition_digest(&revoked).unwrap(),
        active_digest
    );
    let registry = load_registry_document(&document, None).unwrap();
    assert!(PromptCompiler::new(&registry)
        .compile(
            "fixture.agent",
            "1.0.0",
            "fixture.node",
            input.clone(),
            "chat",
            None
        )
        .is_err());
    assert_eq!(
        PromptCompiler::new(&registry)
            .compile_for_replay(
                "fixture.agent",
                "1.0.0",
                "fixture.node",
                input,
                "chat",
                None
            )
            .unwrap()
            .agent_version,
        "1.0.0"
    );
}

#[test]
fn behavior_fingerprint_normalizes_address_and_excludes_credentials() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../agent-definitions/fixtures/fingerprint.json"
    ))
    .unwrap();
    let input: ModelBehavior = serde_json::from_value(fixture["input"].clone()).unwrap();
    let (digest, normalized) = behavior_fingerprint(&input).unwrap();
    assert_eq!(
        serde_json::to_value(normalized).unwrap(),
        fixture["normalized"]
    );
    assert_eq!(digest, fixture["sha256"]);

    let mut credential_rotated = input;
    credential_rotated.settings = json!({
        "temperature": 0,
        "api_key": "rotated",
        "nested": {"authorization": "Bearer secret", "access_token": "secret"}
    });
    assert_eq!(behavior_fingerprint(&credential_rotated).unwrap().0, digest);

    let baseline: ModelBehavior = serde_json::from_value(fixture["input"].clone()).unwrap();
    let mutations = [
        ModelBehavior {
            protocol: "openai_chat_completions".into(),
            ..baseline.clone()
        },
        ModelBehavior {
            request_base_url: "https://example.com/v2".into(),
            ..baseline.clone()
        },
        ModelBehavior {
            upstream_model: "different-model".into(),
            ..baseline.clone()
        },
        ModelBehavior {
            reasoning_effort: Some("high".into()),
            ..baseline.clone()
        },
        ModelBehavior {
            max_output_tokens: baseline.max_output_tokens + 1,
            ..baseline.clone()
        },
        ModelBehavior {
            context_window: baseline.context_window + 1,
            ..baseline.clone()
        },
        ModelBehavior {
            tokenizer_profile_key: "openai.cl100k".into(),
            ..baseline.clone()
        },
        ModelBehavior {
            tokenizer_profile_version: "2.0.0".into(),
            ..baseline.clone()
        },
        ModelBehavior {
            settings: json!({"temperature": 0.5}),
            ..baseline
        },
    ];
    for changed in mutations {
        assert_ne!(behavior_fingerprint(&changed).unwrap().0, digest);
    }
    let unknown = ModelBehavior {
        protocol: "unknown".into(),
        ..serde_json::from_value(fixture["input"].clone()).unwrap()
    };
    assert!(behavior_fingerprint(&unknown).is_err());
}

#[test]
fn model_capability_validation_is_fail_closed() {
    let requirements = ModelRequirements {
        text: true,
        tool_calling: true,
        structured_output: true,
        vision: false,
        reasoning: false,
        min_context_window: 8_192,
    };
    let available = ModelCapabilities {
        text: true,
        tool_calling: true,
        structured_output: true,
        vision: false,
        reasoning: false,
        context_window: 128_000,
    };
    validate_model_capabilities(&requirements, &available).unwrap();
    assert!(validate_model_capabilities(
        &requirements,
        &ModelCapabilities {
            tool_calling: false,
            ..available.clone()
        }
    )
    .is_err());
    assert!(validate_model_capabilities(
        &requirements,
        &ModelCapabilities {
            context_window: 4_096,
            ..available
        }
    )
    .is_err());
}

#[test]
fn serde_contract_rejects_unknown_fields_and_registry_rejects_cross_owner_reference() {
    let root = registry_root();
    for (name, expected) in [
        ("registry-invalid-unknown-field.json", "unknown field"),
        ("registry-invalid-owner.json", "cross-owner"),
    ] {
        let document: Value =
            serde_json::from_slice(&fs::read(root.join("fixtures").join(name)).unwrap()).unwrap();
        let message = load_registry_document(&document, None).unwrap_err();
        assert!(message.contains(expected), "unexpected error: {message}");
    }
}

#[test]
fn schema_fixtures_cover_valid_contract_and_strict_failures() {
    let registry = load_fixture_registry("registry-valid.json");
    assert_eq!(registry.agents().len(), 1);
    assert_eq!(registry.prompts().len(), 1);

    let root = registry_root();
    for (name, expected) in [
        ("registry-invalid-unknown-field.json", "unknown field"),
        ("registry-invalid-owner.json", "cross-owner"),
    ] {
        let bytes = fs::read(root.join("fixtures").join(name)).unwrap();
        let document: Value = serde_json::from_slice(&bytes).unwrap();
        let message = load_registry_document(&document, None).unwrap_err();
        assert!(message.contains(expected), "unexpected error: {message}");
    }
}

#[test]
fn registry_rejects_duplicate_versions_and_release_hash_mismatch() {
    let mut document: Value = serde_json::from_str(include_str!(
        "../../../agent-definitions/fixtures/registry-valid.json"
    ))
    .unwrap();
    let duplicate = document["agents"][0].clone();
    document["agents"].as_array_mut().unwrap().push(duplicate);
    assert!(load_registry_document(&document, None)
        .unwrap_err()
        .contains("duplicate agent"));

    let valid: Value = serde_json::from_str(include_str!(
        "../../../agent-definitions/fixtures/registry-valid.json"
    ))
    .unwrap();
    assert!(load_registry_document(&valid, Some(&"0".repeat(64)))
        .unwrap_err()
        .contains("release index does not match"));
}

#[test]
fn compiler_supports_declared_types_and_rejects_invalid_inputs() {
    let registry = load_fixture_registry("registry-valid-variables.json");
    let compiler = PromptCompiler::new(&registry);
    let input = PromptCompileInput {
        schema_version: "1".into(),
        variables: BTreeMap::from([
            ("title".into(), json!("合同测试")),
            ("tags".into(), json!(["alpha", "beta"])),
            ("item_count".into(), json!(2)),
            ("metadata".into(), json!({"z": 2, "a": 1})),
        ]),
        fragments: vec![DynamicFragment {
            id: "reference-1".into(),
            trust: TrustLevel::Reference,
            source: "shared_fixture".into(),
            content: Some("动态参考".into()),
            asset: None,
        }],
    };
    let tools = json!([
        {"name":"read"}, {"name":"write"}, {"name":"edit"}, {"name":"bash"}
    ]);
    let snapshot = compiler
        .compile(
            "fixture.variables",
            "1.0.0",
            "fixture.variables",
            input.clone(),
            "workspace",
            Some(tools.clone()),
        )
        .unwrap();
    assert_eq!(snapshot.system, "固定 System，不允许动态变量。");
    assert!(snapshot.user.contains("标题：合同测试"));
    assert!(snapshot.user.contains("标签：alpha\nbeta"));
    assert!(snapshot.user.contains("元数据：{\"a\":1,\"z\":2}"));
    assert!(snapshot.user.contains("片段：动态参考"));
    assert_eq!(snapshot.fragments[0].trust, TrustLevel::Reference);
    assert_eq!(snapshot.fragments[0].source, "shared_fixture");
    assert_eq!(snapshot.output_schema.as_ref().unwrap()["strict"], true);
    assert_eq!(
        snapshot.output_schema.as_ref().unwrap()["schema"]["properties"]["items"]["minItems"],
        2
    );
    assert_eq!(
        snapshot.output_schema.as_ref().unwrap()["schema"]["properties"]["items"]["maxItems"],
        2
    );
    assert_eq!(snapshot.tool_schema, Some(tools));
    assert_eq!(snapshot.tool_profile, "workspace");

    let mut cases = Vec::new();
    let mut missing = input.clone();
    missing.variables.remove("title");
    cases.push((
        missing,
        Some(json!([{"name":"read"},{"name":"write"},{"name":"edit"},{"name":"bash"}])),
    ));
    let mut unknown = input.clone();
    unknown.variables.insert("unknown".into(), json!(true));
    cases.push((
        unknown,
        Some(json!([{"name":"read"},{"name":"write"},{"name":"edit"},{"name":"bash"}])),
    ));
    let mut wrong_type = input.clone();
    wrong_type.variables.insert("tags".into(), json!(["ok", 2]));
    cases.push((
        wrong_type,
        Some(json!([{"name":"read"},{"name":"write"},{"name":"edit"},{"name":"bash"}])),
    ));
    let mut non_integer = input.clone();
    non_integer
        .variables
        .insert("item_count".into(), json!(2.5));
    cases.push((
        non_integer,
        Some(json!([{"name":"read"},{"name":"write"},{"name":"edit"},{"name":"bash"}])),
    ));
    let mut oversized = input.clone();
    oversized
        .variables
        .insert("title".into(), json!("x".repeat(33)));
    cases.push((
        oversized,
        Some(json!([{"name":"read"},{"name":"write"},{"name":"edit"},{"name":"bash"}])),
    ));
    cases.push((input, Some(json!([{"name":"read"}]))));
    for (invalid, schema) in cases {
        assert!(compiler
            .compile(
                "fixture.variables",
                "1.0.0",
                "fixture.variables",
                invalid,
                "workspace",
                schema,
            )
            .is_err());
    }
    assert!(compiler
        .compile(
            "fixture.variables",
            "9.0.0",
            "fixture.variables",
            PromptCompileInput {
                schema_version: "1".into(),
                variables: BTreeMap::new(),
                fragments: vec![],
            },
            "workspace",
            None,
        )
        .is_err());
}
