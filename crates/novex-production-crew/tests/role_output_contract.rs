use novex_ai_core::DefinitionRegistry;
use novex_production_crew::state::artifacts::output_contract::{
    validate_role_output, validate_role_output_schema_compatibility,
};
use serde_json::{json, Value};
use std::path::Path;

const SCRIPT_ID: &str = "00000000-0000-4000-8000-000000000001";
const SCENE_ONE: &str = "00000000-0000-4000-8000-000000000002";
const SCENE_TWO: &str = "00000000-0000-4000-8000-000000000003";
const SHOT_ONE: &str = "00000000-0000-4000-8000-000000000005";
const WORK_VERSION_ID: &str = "00000000-0000-4000-8000-000000000007";
const INVENTORY_ID: &str = "00000000-0000-4000-8000-000000000008";
const EVIDENCE_ID: &str = "00000000-0000-4000-8000-000000000009";
const TAKE_ONE: &str = "00000000-0000-4000-8000-00000000000a";

fn valid_fixture(role: &str) -> Value {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../agent-definitions/fixtures/production-crew-candidate-golden-v3.json"
    ))
    .unwrap();
    fixture["role_outputs"][role].clone()
}

#[test]
fn every_full_crew_role_accepts_a_complete_typed_fixture() {
    for role in [
        "producer",
        "screenwriter",
        "director",
        "cinematographer",
        "performance_director",
        "sound_director",
        "editor",
        "qc",
        "character_critic",
    ] {
        validate_role_output(role, &valid_fixture(role))
            .unwrap_or_else(|error| panic!("{role} fixture should be valid: {error}"));
    }
}

#[test]
fn invalid_role_outputs_fail_closed_on_fields_references_order_and_duration() {
    let cases = [
        (
            "producer",
            json!({"creative_brief": {"target_audience": "", "tone": [], "key_messages": [], "constraints": {}, "success_criteria": []}}),
        ),
        (
            "screenwriter",
            json!({"story_bible": {"premise": "x", "theme": "x", "narrative_structure": "x", "world": "x"}, "character_bibles": [{"character_id": "known", "name": "n", "role": "r", "personality": "p", "motivation": "m", "arc": "a"}], "script_draft": {"title": "t", "hook": "h", "scenes": [{"sequence": 2, "narration": "n", "visual_description": "v", "emotion": "e", "duration_sec": 0, "character_ids": ["unknown"]}]}}),
        ),
        (
            "director",
            json!({"directorial_treatment": {"visual_style": "v", "pacing": "p", "emotional_arc": "e", "color_palette": ["c"], "reference_works": []}, "shot_contracts": [{"shot_id": "same", "sequence": 1, "scene_id": "scene-one", "shot_type": "wide", "camera_movement": "static", "duration_sec": 0, "description": "d", "character_ids": []}, {"shot_id": "same", "sequence": 3, "scene_id": SCENE_TWO, "shot_type": "wide", "camera_movement": "static", "duration_sec": 1, "description": "d", "character_ids": []}]}),
        ),
        (
            "cinematographer",
            json!({"collaboration_suggestions": [{"target_artifact_id": "shot-one", "target_artifact_version": 0, "suggestion_type": "revision", "content": "", "priority": "urgent", "blocking": true, "rationale": ""}]}),
        ),
        (
            "performance_director",
            json!({"performance_briefs": [{"character_bible_id": "character", "character_id": "creator", "script_id": SCRIPT_ID, "emotional_arc": [{"sequence": 2, "scene_id": SCENE_ONE, "emotion": "e", "intensity": 11, "notes": "n"}], "body_language": "b", "vocal_direction": "v"}]}),
        ),
        (
            "sound_director",
            json!({"sound_plan": {"script_id": SCRIPT_ID, "music_style": "m", "scene_sound_notes": [{"sequence": 1, "scene_id": "scene", "music_cue": "m", "sfx_notes": [], "dialogue_direction": "d"}]}}),
        ),
        (
            "editor",
            json!({"continuity_ledgers": [{"order": 2, "shot_contract_id": SHOT_ONE, "work_version_id": WORK_VERSION_ID, "inventory_id": INVENTORY_ID, "evidence_snapshot_id": EVIDENCE_ID, "visual_facts": [], "continuity_flags": []}]}),
        ),
        (
            "qc",
            json!({"take_reviews": [{"required_take_id": TAKE_ONE, "work_version_id": WORK_VERSION_ID, "inventory_id": INVENTORY_ID, "evidence_snapshot_id": EVIDENCE_ID, "applicable_shot_contract_ids": [], "review_status": "passed", "quality_assessment": {}, "issues": [], "suggestions": []}]}),
        ),
    ];

    for (role, fixture) in cases {
        assert!(
            validate_role_output(role, &fixture).is_err(),
            "{role} invalid fixture was accepted"
        );
    }
}

#[test]
fn active_v2_schemas_fail_closed_and_v3_candidates_match_the_durable_contract() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("agent-definitions");
    let registry = DefinitionRegistry::load(root).unwrap();
    for role in [
        "producer",
        "screenwriter",
        "character_critic",
        "director",
        "cinematographer",
        "performance_director",
        "sound_director",
        "editor",
        "qc",
    ] {
        let agent_key = format!("production.{role}");
        let node_key = format!("production.{role}.execute");
        let active_reference = &registry.active_agent(&agent_key).unwrap().nodes[&node_key];
        let active_prompt = registry
            .prompts()
            .iter()
            .find(|prompt| {
                prompt.prompt_key == active_reference.key
                    && prompt.version == active_reference.version
            })
            .unwrap();
        assert_eq!(
            validate_role_output_schema_compatibility(role, active_prompt.output_schema.as_ref())
                .unwrap_err()
                .code(),
            "capability_mismatch",
            "role={role}"
        );

        let candidate_reference = &registry.agent(&agent_key, "3.0.0").unwrap().nodes[&node_key];
        let candidate_prompt = registry
            .prompts()
            .iter()
            .find(|prompt| {
                prompt.prompt_key == candidate_reference.key
                    && prompt.version == candidate_reference.version
            })
            .unwrap();
        validate_role_output_schema_compatibility(role, candidate_prompt.output_schema.as_ref())
            .unwrap_or_else(|error| panic!("role={role}: {error}"));
    }
}
