//! RoleExecutor 单元测试：无 DB 依赖，mock executor 场景

use novex_production_crew::{
    executor::role_executor::{ArtifactSummary, RoleExecutionStatus, RoleExecutor},
    roles::definition::{Lifecycle, PromptRef, RoleDefinition},
    state::artifacts::ArtifactType,
};
use serde_json::json;

fn make_def(
    role_key: &str,
    inputs: Vec<ArtifactType>,
    outputs: Vec<ArtifactType>,
) -> RoleDefinition {
    RoleDefinition {
        role_key: role_key.to_string(),
        role_name: role_key.to_string(),
        responsibilities: vec![],
        input_artifacts: inputs,
        output_artifacts: outputs,
        allowed_tools: vec![],
        prompt_definition_ref: PromptRef {
            key: format!("production.{}.general", role_key),
            version: "@1".to_string(),
        },
        lifecycle: Lifecycle::Active,
    }
}

// ── check_inputs_ready ──────────────────────────────────────────────────────

#[test]
fn check_inputs_ready_all_present() {
    let def = make_def(
        "director",
        vec![ArtifactType::StoryBible, ArtifactType::ScriptDraft],
        vec![],
    );
    let available = vec![ArtifactType::StoryBible, ArtifactType::ScriptDraft];
    assert!(RoleExecutor::check_inputs_ready(&def, &available).is_ok());
}

#[test]
fn check_inputs_ready_no_inputs_required() {
    let def = make_def("producer", vec![], vec![ArtifactType::CreativeBrief]);
    assert!(RoleExecutor::check_inputs_ready(&def, &[]).is_ok());
}

#[test]
fn check_inputs_ready_missing_returns_error() {
    let def = make_def("director", vec![ArtifactType::ScriptDraft], vec![]);
    let available = vec![ArtifactType::StoryBible]; // ScriptDraft 缺失
    let err = RoleExecutor::check_inputs_ready(&def, &available).unwrap_err();
    assert!(err.to_string().contains("ScriptDraft"));
}

#[test]
fn check_inputs_ready_partial_missing_errors_on_first_missing() {
    let def = make_def(
        "director",
        vec![ArtifactType::StoryBible, ArtifactType::ScriptDraft],
        vec![],
    );
    let available = vec![ArtifactType::StoryBible]; // ScriptDraft 缺失
    assert!(RoleExecutor::check_inputs_ready(&def, &available).is_err());
}

// ── validate_output ─────────────────────────────────────────────────────────

#[test]
fn validate_output_producer_valid() {
    let def = make_def("producer", vec![], vec![ArtifactType::CreativeBrief]);
    let output = json!({
        "creative_brief": {
            "target_audience": "18-25岁女性",
            "tone": ["活泼", "时尚"],
            "key_messages": ["美妆技巧"],
            "constraints": {},
            "success_criteria": ["完播率>60%"]
        }
    });
    assert!(RoleExecutor::validate_output(&def, &output).is_ok());
}

#[test]
fn validate_output_missing_key_fails() {
    let def = make_def("producer", vec![], vec![ArtifactType::CreativeBrief]);
    let output = json!({ "other_field": {} });
    assert!(RoleExecutor::validate_output(&def, &output).is_err());
}

#[test]
fn validate_output_screenwriter_all_artifacts_present() {
    let def = make_def(
        "screenwriter",
        vec![],
        vec![
            ArtifactType::StoryBible,
            ArtifactType::CharacterBible,
            ArtifactType::ScriptDraft,
        ],
    );
    let output = json!({
        "story_bible": { "premise": "一个美妆博主的创业故事" },
        "character_bibles": [{ "character_id": "char_001", "name": "小红" }],
        "script_draft": { "title": "美妆日记", "scenes": [] }
    });
    assert!(RoleExecutor::validate_output(&def, &output).is_ok());
}

#[test]
fn validate_output_screenwriter_missing_story_bible_fails() {
    let def = make_def(
        "screenwriter",
        vec![],
        vec![
            ArtifactType::StoryBible,
            ArtifactType::CharacterBible,
            ArtifactType::ScriptDraft,
        ],
    );
    let output = json!({
        // story_bible 缺失
        "character_bibles": [],
        "script_draft": {}
    });
    assert!(RoleExecutor::validate_output(&def, &output).is_err());
}

#[test]
fn validate_output_director_valid() {
    let def = make_def(
        "director",
        vec![],
        vec![ArtifactType::DirectorialTreatment, ArtifactType::ShotContract],
    );
    let output = json!({
        "directorial_treatment": { "visual_style": "暖色调写实" },
        "shot_contracts": [{ "shot_id": "shot_001", "description": "开场镜头" }]
    });
    assert!(RoleExecutor::validate_output(&def, &output).is_ok());
}

#[test]
fn validate_output_no_output_artifacts_is_ok() {
    // 纯协作角色（cinematographer、character_critic）output_artifacts 为空
    let def = make_def("cinematographer", vec![ArtifactType::ShotContract], vec![]);
    let output = json!({ "collaboration_suggestions": [] });
    // 无 output_artifacts 需要验证，始终通过
    assert!(RoleExecutor::validate_output(&def, &output).is_ok());
}
