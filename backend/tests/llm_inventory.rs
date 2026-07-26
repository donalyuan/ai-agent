use novex_ai_core::{
    canonical_json, sha256_hex, DefinitionRegistry, DynamicFragment, PromptCompileInput,
    PromptCompiler, TrustLevel,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn rust_sources(directory: &Path, output: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).expect("production source directory must exist") {
        let path = entry.expect("source entry must be readable").path();
        if path.is_dir() {
            rust_sources(&path, output);
        } else if path.extension().and_then(std::ffi::OsStr::to_str) == Some("rs") {
            output.push(path);
        }
    }
}

#[test]
fn every_production_text_model_entrypoint_is_declared_in_the_baseline_inventory() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/versioned_prompt_baseline.json"))
            .expect("baseline inventory must be valid JSON");
    let declared_sources = fixture["rust_nodes"]
        .as_array()
        .expect("rust_nodes must be an array")
        .iter()
        .map(|node| node["source"].as_str().expect("source must be a string"))
        .collect::<BTreeSet<_>>();

    let mut sources = Vec::new();
    rust_sources(&manifest_dir.join("src"), &mut sources);
    let discovered = sources
        .iter()
        .filter_map(|path| {
            let source = std::fs::read_to_string(path).expect("source must be readable");
            source.contains(".generate_script(").then(|| {
                path.strip_prefix(manifest_dir.join("src"))
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
        })
        .collect::<BTreeSet<_>>();

    assert!(
        discovered.is_empty(),
        "backend production code must route every inventory node through AuditedModelExecutor: {discovered:?}"
    );

    for required in [
        "application/projects.rs",
        "agents/llm.rs",
        "application/agents/adapters/script.rs",
        "application/agents/adapters/topic_generation.rs",
        "application/agents/adapters/topic_quality.rs",
        "application/agents/adapters/topic_review.rs",
        "application/agents/adapters/sound.rs",
        "application/agents/adapters/work.rs",
    ] {
        assert!(
            declared_sources.contains(required),
            "missing inventory declaration for {required}"
        );
    }
}

#[test]
fn production_model_calls_are_restricted_to_provider_and_audited_executor() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("backend must be inside workspace");
    let roots = [
        workspace.join("backend/src"),
        workspace.join("crates/novex-agent/src"),
        workspace.join("crates/novex-model/src"),
    ];
    let mut sources = Vec::new();
    for root in roots {
        rust_sources(&root, &mut sources);
    }
    let direct_call_files = sources
        .iter()
        .filter_map(|path| {
            let source = std::fs::read_to_string(path).expect("source must be readable");
            source.contains(".generate_script(").then(|| {
                path.strip_prefix(workspace)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        direct_call_files,
        BTreeSet::from(["crates/novex-agent/src/audited_model.rs".to_string()]),
        "裸模型调用只能存在于底层 provider 与 AuditedModelExecutor"
    );

    let adapter_root = workspace.join("backend/src/application/agents/adapters");
    let mut adapter_sources = Vec::new();
    rust_sources(&adapter_root, &mut adapter_sources);
    for path in adapter_sources {
        let source = std::fs::read_to_string(&path).expect("adapter source must be readable");
        assert!(
            !source.contains("LLMClient")
                && !source.contains("model.client")
                && !source.contains(".generate_script("),
            "生产 Adapter 不得获得裸模型客户端: {}",
            path.display()
        );
        for forbidden_context_path in [
            "PromptCompileInput",
            "DynamicFragment",
            "truncate_for_prompt",
            "generation_prompt",
            "context_blob",
        ] {
            assert!(
                !source.contains(forbidden_context_path),
                "生产 Adapter 不得保留旧 Context 装配入口 {forbidden_context_path}: {}",
                path.display()
            );
        }
    }

    let kernel_source = std::fs::read_to_string(workspace.join("crates/novex-agent/src/lib.rs"))
        .expect("novex-agent kernel source must be readable");
    assert!(
        !kernel_source.contains("pub client: Arc<dyn LLMClient>"),
        "ModelExecutionRef 不得公开裸 LLMClient"
    );

    let backend_root = workspace.join("backend/src");
    let mut backend_sources = Vec::new();
    rust_sources(&backend_root, &mut backend_sources);
    for path in &backend_sources {
        let source = std::fs::read_to_string(path).expect("backend source must be readable");
        assert!(
            !source.contains("LLMPrompt")
                && !source.contains("ScriptPromptBuilder")
                && !source.contains("base_system_prompt"),
            "生产业务层不得保留旧 Prompt/System 构造入口: {}",
            path.display()
        );
        for forbidden_flag in [
            "USE_LEGACY_PROMPT",
            "ENABLE_LEGACY_LLM",
            "USE_AUDITED_MODEL",
            "ENABLE_VERSIONED_PROMPT",
        ] {
            assert!(
                !source.contains(forbidden_flag),
                "不得用 feature flag 保留新旧模型执行双轨: {}",
                path.display()
            );
        }
    }

    let backend_text = backend_sources
        .iter()
        .map(|path| std::fs::read_to_string(path).expect("backend source must be readable"))
        .collect::<Vec<_>>()
        .join("\n");
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/versioned_prompt_baseline.json"))
            .expect("baseline inventory must be valid JSON");
    for node in fixture["rust_nodes"].as_array().unwrap() {
        let node_key = node["node_key"].as_str().unwrap();
        assert!(
            backend_text.contains(node_key),
            "生产节点 {node_key} 必须存在唯一受审计调用入口"
        );
    }
    assert!(backend_text.contains("AuditedModelRequest"));
    assert!(backend_text.contains("FixedModelBinding"));
    assert!(backend_text.contains("context_candidates"));
}

#[test]
fn baseline_fixtures_cover_golden_contract_and_zero_external_effects() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/versioned_prompt_baseline.json"))
            .expect("baseline inventory must be valid JSON");
    assert_eq!(fixture["external_effects"]["real_model_calls"], 0);
    assert_eq!(fixture["external_effects"]["video_generation_calls"], 0);
    assert_eq!(fixture["external_effects"]["platform_publication_calls"], 0);

    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("backend must be inside workspace");
    let registry_root = workspace.join("agent-definitions");
    let registry = DefinitionRegistry::load(&registry_root).expect("registry must be valid");
    let compiler = PromptCompiler::new(&registry);
    let golden_user = fixture["golden_compile_input"]["user"]
        .as_str()
        .expect("golden user must be a string");
    assert_eq!(
        sha256_hex(golden_user.as_bytes()),
        fixture["golden_compile_input"]["user_sha256"]
    );

    let mut golden_rust_nodes = BTreeSet::new();
    for node in fixture["rust_nodes"].as_array().unwrap() {
        assert!(node["node_key"]
            .as_str()
            .is_some_and(|value| !value.is_empty()));
        let node_key = node["node_key"].as_str().unwrap();
        golden_rust_nodes.insert(node_key.to_string());
        let agent_key = node["agent_key"].as_str().unwrap();
        let prompt_key = node["prompt_key"].as_str().unwrap();
        let agent = registry.agent(agent_key, "1.0.0").unwrap();
        assert_eq!(agent.executor_owner, novex_ai_core::ExecutorOwner::Rust);
        assert_eq!(agent.nodes[node_key].key, prompt_key);

        let prompt = registry
            .prompts()
            .iter()
            .find(|prompt| prompt.prompt_key == prompt_key && prompt.version == "1.0.0")
            .expect("inventory prompt must exist");
        let system = std::fs::read_to_string(registry_root.join(&prompt.system_template))
            .unwrap()
            .trim_end_matches(['\r', '\n'])
            .to_string();
        let user_template = std::fs::read_to_string(registry_root.join(&prompt.user_template))
            .unwrap()
            .trim_end_matches(['\r', '\n'])
            .to_string();
        assert_eq!(system, node["system_prompt"]);
        assert_eq!(sha256_hex(system.as_bytes()), node["system_sha256"]);
        assert_eq!(user_template, node["user_prompt_template"]);
        assert_eq!(
            sha256_hex(user_template.as_bytes()),
            node["user_prompt_template_sha256"]
        );
        let output_schema = prompt.output_schema.clone().unwrap_or(Value::Null);
        assert_eq!(
            sha256_hex(canonical_json(&output_schema).as_bytes()),
            node["output_schema_sha256"]
        );

        let mut variables = BTreeMap::new();
        if node_key == "script.complete" {
            variables.insert(
                "scene_count".into(),
                fixture["golden_compile_input"]["scene_count"].clone(),
            );
        }
        let snapshot = compiler
            .compile(
                agent_key,
                "1.0.0",
                node_key,
                PromptCompileInput {
                    schema_version: "1".into(),
                    variables,
                    fragments: vec![DynamicFragment {
                        id: format!("golden:{node_key}"),
                        trust: golden_trust(prompt_key),
                        source: "migration_baseline_fixture".into(),
                        content: Some(golden_user.into()),
                        asset: None,
                    }],
                },
                "chat",
                None,
            )
            .unwrap_or_else(|error| panic!("failed to compile golden node {node_key}: {error}"));
        assert_eq!(snapshot.prompt_key, prompt_key);
        assert_eq!(snapshot.system, system);
        assert_eq!(snapshot.user, golden_user);
        assert_eq!(
            sha256_hex(snapshot.user.as_bytes()),
            fixture["golden_compile_input"]["user_sha256"]
        );
        let compiled_schema = snapshot.output_schema.unwrap_or(Value::Null);
        assert_eq!(
            sha256_hex(canonical_json(&compiled_schema).as_bytes()),
            node["compiled_output_schema_sha256"]
        );
        assert_eq!(
            snapshot.max_output_tokens.map(u64::from),
            node["max_output_tokens"].as_u64()
        );
        assert_eq!(node["initial_call_count"], 1);
        assert!(node.get("max_output_tokens").is_some());
        assert!(node["max_attempts"].as_u64().is_some_and(|value| value > 0));
        assert!(
            node["max_attempts"].as_u64().unwrap() >= node["initial_call_count"].as_u64().unwrap()
        );
        assert!(node["retry_policy"]
            .as_str()
            .is_some_and(|value| !value.is_empty()));
        assert!(node["step_order"].is_array());
    }
    let registry_rust_nodes = registry
        .agents()
        .iter()
        .filter(|agent| agent.executor_owner == novex_ai_core::ExecutorOwner::Rust)
        .flat_map(|agent| agent.nodes.keys().cloned())
        .collect::<BTreeSet<_>>();
    assert_eq!(golden_rust_nodes, registry_rust_nodes);

    let personal = registry.active_agent("personal.general").unwrap();
    assert_eq!(personal.executor_owner, novex_ai_core::ExecutorOwner::Pi);
    assert!(personal.model_requirements.text);
    assert!(personal.model_requirements.tool_calling);
    assert_eq!(personal.tool_profiles, ["chat", "workspace"]);
    assert_eq!(personal.tools, ["read", "write", "edit", "bash"]);
    let golden_pi_nodes = fixture["pi_nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["node_key"].as_str().unwrap().to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(golden_pi_nodes, personal.nodes.keys().cloned().collect());
    assert!(personal.nodes.values().all(|reference| {
        reference.key == "personal.general"
            && reference.version == "2.0.0"
            && reference.context_policy.is_some()
    }));

    let safety: Value = serde_json::from_str(include_str!("fixtures/model_call_safety.json"))
        .expect("safety fixture must be valid JSON");
    assert_eq!(safety["assets"].as_array().unwrap().len(), 3);
    assert!(safety["legacy"]["run_with_partial_audit"]["prompt_snapshot"].is_null());
}

fn golden_trust(prompt_key: &str) -> TrustLevel {
    match prompt_key {
        "project.strategy_draft" | "topic.generate" | "sound.recommend" => TrustLevel::Reference,
        "topic.quality" | "topic.group_review" => TrustLevel::Candidate,
        _ => TrustLevel::UserInstruction,
    }
}
