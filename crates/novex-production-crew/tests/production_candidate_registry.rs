use jsonschema::{Draft, JSONSchema};
use novex_ai_core::{
    canonical_json, sha256_hex, DefinitionRegistry, DefinitionStatus, DynamicFragment,
    PromptCompileInput, PromptCompiler, TrustLevel,
};
use novex_production_crew::{
    durable::{
        media::{
            media_review_readiness, ComposeInput, FinalMediaAsset, MediaEvidenceSnapshot,
            RequiredTakeInventorySnapshot,
        },
        plan::{FullCrewPlanRegistry, ResourceLimits, StepKind},
        script::{map_script_draft, ScriptDraftInput},
    },
    state::artifacts::output_contract::validate_role_output,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn definitions_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("agent-definitions")
}

fn read_json(path: impl AsRef<Path>) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn assert_valid(schema: &Value, instance: &Value, label: &str) {
    let validator = JSONSchema::options()
        .with_draft(Draft::Draft202012)
        .compile(schema)
        .unwrap_or_else(|error| panic!("{label} schema must compile: {error}"));
    if let Err(errors) = validator.validate(instance) {
        let details = errors.map(|error| error.to_string()).collect::<Vec<_>>();
        panic!("{label} must satisfy its schema: {details:?}");
    };
}

#[test]
fn generated_registry_release_index_and_candidate_output_schemas_compile() {
    let root = definitions_root();
    let registry_document = read_json(root.join("registry.json"));
    let release_document = read_json(root.join("release-index.json"));
    assert_valid(
        &read_json(root.join("schemas/registry.schema.json")),
        &registry_document,
        "definition registry",
    );
    assert_valid(
        &read_json(root.join("schemas/release-index.schema.json")),
        &release_document,
        "definition release index",
    );

    let registry = DefinitionRegistry::load(&root).unwrap();
    let candidate_prompts = registry
        .prompts()
        .iter()
        .filter(|prompt| {
            prompt.status == DefinitionStatus::Candidate
                && prompt.prompt_key.starts_with("production.")
                && prompt.version == "3.0.0"
        })
        .collect::<Vec<_>>();
    assert_eq!(candidate_prompts.len(), 9);
    for prompt in candidate_prompts {
        let output = prompt.output_schema.as_ref().unwrap();
        JSONSchema::options()
            .with_draft(Draft::Draft202012)
            .compile(&output["schema"])
            .unwrap_or_else(|error| {
                panic!(
                    "candidate output schema {}@{} must compile: {error}",
                    prompt.prompt_key, prompt.version
                )
            });
    }
}

#[test]
fn candidate_golden_covers_role_outputs_history_mapper_fake_media_and_fixed_workflow() {
    let root = definitions_root();
    let golden = read_json(root.join("fixtures/production-crew-candidate-golden-v3.json"));
    let registry = DefinitionRegistry::load(&root).unwrap();
    let compiler = PromptCompiler::new(&registry);
    let compile_input = |id: &str| PromptCompileInput {
        schema_version: "1".into(),
        variables: BTreeMap::new(),
        fragments: vec![DynamicFragment {
            id: id.into(),
            trust: TrustLevel::Reference,
            source: "production_candidate_golden".into(),
            content: Some("固定计划零费用输入".into()),
            asset: None,
        }],
    };

    for (role, contract) in golden["role_contracts"].as_object().unwrap() {
        let agent_key = format!("production.{role}");
        let node_key = format!("production.{role}.execute");
        let definition_version = contract["definition_version"].as_str().unwrap();
        let snapshot = compiler
            .compile_for_replay(
                &agent_key,
                definition_version,
                &node_key,
                compile_input(&format!("golden:{role}")),
                "chat",
                None,
            )
            .unwrap_or_else(|error| panic!("{role} golden prompt must compile: {error}"));
        assert_eq!(snapshot.agent_version, definition_version);
        assert_eq!(
            snapshot.prompt_version,
            contract["prompt_version"].as_str().unwrap()
        );
        let output = &golden["role_outputs"][role];
        validate_role_output(role, output)
            .unwrap_or_else(|error| panic!("{role} typed golden must validate: {error}"));
        let output_schema = snapshot.output_schema.as_ref().unwrap();
        let validator = JSONSchema::options()
            .with_draft(Draft::Draft202012)
            .compile(&output_schema["schema"])
            .unwrap();
        if let Err(errors) = validator.validate(output) {
            let details = errors.map(|error| error.to_string()).collect::<Vec<_>>();
            panic!("{role} output must satisfy its published schema: {details:?}");
        };

        if definition_version == "3.0.0" {
            assert_eq!(
                sha256_hex(snapshot.system.as_bytes()),
                contract["system_sha256"]
            );
            assert_eq!(
                sha256_hex(canonical_json(output_schema).as_bytes()),
                contract["output_schema_sha256"]
            );
        }
    }

    assert_eq!(golden["historical_active_v2"].as_object().unwrap().len(), 9);
    for (role, expected) in golden["historical_active_v2"].as_object().unwrap() {
        let agent_key = format!("production.{role}");
        let node_key = format!("production.{role}.execute");
        let snapshot = compiler
            .compile_for_replay(
                &agent_key,
                "2.0.0",
                &node_key,
                compile_input(&format!("history:{node_key}")),
                "chat",
                None,
            )
            .unwrap();
        assert_eq!(
            sha256_hex(snapshot.system.as_bytes()),
            expected["system_sha256"]
        );
        assert_eq!(
            sha256_hex(canonical_json(snapshot.output_schema.as_ref().unwrap()).as_bytes()),
            expected["output_schema_sha256"]
        );
    }

    let draft: ScriptDraftInput =
        serde_json::from_value(golden["role_outputs"]["screenwriter"]["script_draft"].clone())
            .unwrap();
    let character_ids = golden["script_mapper"]["character_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    let mapped = map_script_draft(&draft, &character_ids).unwrap();
    assert_eq!(mapped.digest, golden["script_mapper"]["expected_digest"]);
    assert_eq!(mapped.scenes.len(), 3);

    let role_bindings = golden["role_contracts"].clone();
    let plan =
        FullCrewPlanRegistry::snapshot_v1(true, role_bindings, ResourceLimits::strict_default())
            .unwrap();
    let step_keys = plan
        .steps
        .iter()
        .map(|step| step.key.as_str())
        .collect::<Vec<_>>();
    let expected_steps = golden["fixed_plan_steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(step_keys, expected_steps);
    let planned_roles = plan
        .steps
        .iter()
        .filter(|step| step.kind == StepKind::Role)
        .map(|step| step.role_key.as_deref().unwrap())
        .collect::<BTreeSet<_>>();
    let golden_roles = golden["role_contracts"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(planned_roles, golden_roles);

    let id = |suffix| Uuid::parse_str(&format!("00000000-0000-4000-8000-{suffix:012x}")).unwrap();
    let scene_one = id(2);
    let scene_two = id(3);
    let shot_one = id(5);
    let shot_two = id(6);
    let inventory = RequiredTakeInventorySnapshot::build(
        id(8),
        id(20),
        id(21),
        1,
        0,
        id(22),
        id(7),
        id(23),
        FinalMediaAsset {
            artifact_id: id(24),
            sha256: sha256_hex(b"fake-final-media"),
            mime_type: golden["fake_media"]["mime_type"].as_str().unwrap().into(),
            duration_ms: golden["fake_media"]["duration_ms"].as_u64().unwrap(),
        },
        sha256_hex(b"fake-work-version"),
        vec![ComposeInput {
            generation_step_id: id(25),
            generation_attempt_id: id(26),
            output_artifact_id: id(27),
            segment_key: "segment-1".into(),
            scene_ids: vec![scene_one, scene_two],
            shot_contracts: vec![(scene_one, vec![shot_one]), (scene_two, vec![shot_two])],
            consumed_by_final_compose: true,
            generation_succeeded: true,
        }],
    )
    .unwrap();
    let evidence = MediaEvidenceSnapshot::build(
        id(9),
        inventory.run_id,
        inventory.source_step_id,
        inventory.source_attempt,
        inventory.revision_epoch,
        inventory.work_version_id,
        inventory.inventory_id,
        inventory.inventory_digest.clone(),
        inventory.final_asset.clone(),
        golden["fake_media"]["vision_capability_version"]
            .as_str()
            .unwrap()
            .into(),
        golden["fake_media"]["audio_capability_version"]
            .as_str()
            .unwrap()
            .into(),
        serde_json::json!({
            "final_media": {"visual": "pass", "audio": "pass"},
            "takes": [{"take_id": inventory.takes[0].take_id, "result": "pass"}]
        }),
    )
    .unwrap();
    media_review_readiness(Some(inventory), Some(evidence)).unwrap();
    assert!(golden["external_effects"]
        .as_object()
        .unwrap()
        .values()
        .all(|value| value == 0));
}
