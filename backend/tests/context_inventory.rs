use serde_json::Value;
use std::collections::BTreeSet;
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
fn production_inventory_covers_every_llm_node_and_has_one_governed_context_path() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let baseline: Value =
        serde_json::from_str(include_str!("fixtures/versioned_prompt_baseline.json"))
            .expect("prompt baseline must be valid JSON");
    let registry: Value =
        serde_json::from_str(include_str!("../../agent-definitions/registry.json"))
            .expect("registry must be valid JSON");
    let baseline_nodes = baseline["rust_nodes"]
        .as_array()
        .unwrap()
        .iter()
        .chain(baseline["pi_nodes"].as_array().unwrap())
        .map(|node| node["node_key"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    let registry_nodes = registry["agents"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|agent| {
            agent["nodes"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        baseline_nodes, registry_nodes,
        "baseline must enumerate every Registry LLM node"
    );

    let mut production_files = Vec::new();
    rust_sources(
        &workspace.join("backend/src/application"),
        &mut production_files,
    );
    rust_sources(&workspace.join("backend/src/agents"), &mut production_files);
    rust_sources(
        &workspace.join("crates/novex-agent/src"),
        &mut production_files,
    );
    for (marker, description) in [
        ("PromptCompileInput", "preassembled prompt input"),
        ("DynamicFragment", "whole prompt fragment"),
        ("truncate_for_prompt", "manual character truncation"),
    ] {
        assert!(
            production_files
                .iter()
                .all(|path| !std::fs::read_to_string(path).unwrap().contains(marker)),
            "production source still contains legacy {description}"
        );
    }

    let executor =
        std::fs::read_to_string(workspace.join("crates/novex-agent/src/audited_model.rs")).unwrap();
    for required in [
        "PromptCompiler::new(&self.registry).prepare",
        "ContextCompiler::compile",
        "PromptCompiler::new(&self.registry)",
        ".prepare_with_context(",
    ] {
        assert!(
            executor.contains(required),
            "Executor is missing {required}"
        );
    }

    let pi_wrapper =
        std::fs::read_to_string(workspace.join("services/agent-runtime/src/novex-harness.ts"))
            .unwrap();
    for marker in [
        "queuedFragments",
        "redactUnknown(this.context",
        "prepareModelCall({",
    ] {
        assert!(
            !pi_wrapper.contains(marker),
            "Pi wrapper still contains legacy marker {marker}"
        );
    }
    for required in [
        "compileContext(request)",
        "prepareModelCallWithContext",
        "before_provider_request",
        "return { messages:",
    ] {
        assert!(
            pi_wrapper.contains(required),
            "Pi wrapper is missing {required}"
        );
    }

    let mut all_sources = Vec::new();
    rust_sources(&workspace.join("backend/src"), &mut all_sources);
    rust_sources(&workspace.join("crates"), &mut all_sources);
    for flag in [
        "USE_LEGACY_PROMPT",
        "ENABLE_LEGACY_LLM",
        "USE_AUDITED_MODEL",
        "ENABLE_VERSIONED_PROMPT",
        "ENABLE_LEGACY_CONTEXT",
    ] {
        assert!(
            all_sources
                .iter()
                .all(|path| !std::fs::read_to_string(path).unwrap().contains(flag)),
            "Rust source contains forbidden dual-track flag {flag}"
        );
    }
}

#[test]
fn context_rollback_keeps_forward_compatible_audit_schema() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let migrations = [
        "20260725030000_governed_context_compilation.sql",
        "20260725120000_postgres_context_audit.sql",
        "20260725121000_context_failure_links.sql",
        "20260725122000_tokenizer_profile_incompatibility.sql",
        "20260725123000_context_evaluation_evidence.sql",
    ];
    let joined = migrations
        .iter()
        .map(|name| {
            std::fs::read_to_string(workspace.join("backend/migrations").join(name)).unwrap()
        })
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();
    for forbidden in [
        "drop table context_snapshots",
        "drop table context_compile_attempts",
        "drop column context_policy_bindings",
        "drop column tokenizer_profile_key",
        "delete from context_snapshots",
        "delete from context_compile_attempts",
    ] {
        assert!(
            !joined.contains(forbidden),
            "Context migration contains destructive rollback token {forbidden}"
        );
    }
    for required in [
        "create table context_snapshots",
        "create table context_compile_attempts",
        "context_policy_bindings",
        "tokenizer_profile_key",
        "context_node_results",
    ] {
        assert!(
            joined.contains(required),
            "forward schema is missing {required}"
        );
    }
}

#[test]
fn shared_contract_fixture_covers_determinism_safety_and_zero_cost() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../agent-definitions/fixtures/context-contract.json"
    ))
    .expect("context contract fixture must be valid JSON");
    let ids = fixture["tokenizer_cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| case["id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        ids,
        BTreeSet::from(["ascii", "chinese", "emoji", "json", "reasoning"])
    );
    assert_eq!(
        fixture["context_cases"]["atomic_tool_group"]["group_id"],
        "tool-call-1"
    );
    assert_eq!(fixture["safety"]["oversized_candidate_bytes"], 1_048_576);
    assert_eq!(fixture["external_effects"]["real_model_calls"], 0);
    assert_eq!(fixture["external_effects"]["tool_calls"], 0);
    assert_eq!(fixture["external_effects"]["video_generation_calls"], 0);
    assert_eq!(fixture["external_effects"]["platform_publication_calls"], 0);
}
