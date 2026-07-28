//! RoleExecutor 单元测试：无 DB 依赖，mock executor 场景

use novex_production_crew::{
    executor::role_executor::RoleExecutor,
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
        "story_bible": {
            "premise": "一个美妆博主的创业故事",
            "theme": "自我表达",
            "narrative_structure": "线性",
            "world": "当代工作室"
        },
        "character_bibles": [{
            "character_id": "char_001",
            "name": "小红",
            "role": "主角",
            "personality": "细致",
            "motivation": "完成教程",
            "arc": "从犹豫到自信"
        }],
        "script_draft": {
            "title": "美妆日记",
            "hook": "三步完成妆容",
            "scenes": [
                {"sequence": 1, "narration": "步骤说明", "visual_description": "操作特写", "emotion": "专注", "duration_sec": 5, "character_ids": ["char_001"]},
                {"sequence": 2, "narration": "步骤说明", "visual_description": "操作特写", "emotion": "专注", "duration_sec": 5, "character_ids": ["char_001"]},
                {"sequence": 3, "narration": "步骤说明", "visual_description": "操作特写", "emotion": "专注", "duration_sec": 5, "character_ids": ["char_001"]}
            ]
        }
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
        vec![
            ArtifactType::DirectorialTreatment,
            ArtifactType::ShotContract,
        ],
    );
    let output = json!({
        "directorial_treatment": {
            "visual_style": "暖色调写实",
            "pacing": "明快",
            "emotional_arc": "从好奇到确信",
            "color_palette": ["暖白"],
            "reference_works": []
        },
        "shot_contracts": [{
            "shot_id": "shot_001",
            "sequence": 1,
            "scene_id": "00000000-0000-4000-8000-000000000001",
            "shot_type": "close-up",
            "camera_movement": "static",
            "duration_sec": 3,
            "description": "开场镜头",
            "character_ids": []
        }]
    });
    assert!(RoleExecutor::validate_output(&def, &output).is_ok());
}

#[test]
fn validate_output_no_output_artifacts_is_ok() {
    // 纯协作角色仍必须满足完整的 collaboration_suggestions 契约。
    let def = make_def("cinematographer", vec![ArtifactType::ShotContract], vec![]);
    let output = json!({ "collaboration_suggestions": [] });
    assert!(RoleExecutor::validate_output(&def, &output).is_ok());
}
