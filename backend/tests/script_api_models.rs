use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;
use validator::Validate;
use novex_api::agents::models::{
    GenerateScriptRequest, Scene, Script, ScriptListFilter, ScriptResponse, ScriptStatus,
    ScriptStyle,
};

#[test]
fn generate_script_request_deserializes_and_validates() {
    let project_id = Uuid::new_v4();
    let payload = json!({
        "project_id": project_id,
        "topic": "ChatGPT如何改变程序员工作流",
        "style": "knowledge",
        "scene_count": 6,
        "parent_id": null
    });

    let request: GenerateScriptRequest = serde_json::from_value(payload).unwrap();

    request.validate().unwrap();
    assert_eq!(request.project_id, project_id);
    assert_eq!(request.style_or_default(), ScriptStyle::Knowledge);
    assert_eq!(request.scene_count_or_default(), 6);
}

#[test]
fn generate_script_request_rejects_invalid_topic_and_scene_count() {
    let payload = json!({
        "project_id": Uuid::new_v4(),
        "topic": "太短",
        "scene_count": 9
    });

    let request: GenerateScriptRequest = serde_json::from_value(payload).unwrap();
    let errors = request.validate().unwrap_err();

    assert!(errors.field_errors().contains_key("topic"));
    assert!(errors.field_errors().contains_key("scene_count"));
}

#[test]
fn script_list_filter_provides_api_defaults_and_validates_limit() {
    let default_filter: ScriptListFilter = serde_json::from_value(json!({})).unwrap();
    assert_eq!(default_filter.limit_or_default(), 20);
    assert_eq!(default_filter.offset_or_default(), 0);
    assert_eq!(default_filter.status, None);
    default_filter.validate().unwrap();

    let invalid_filter: ScriptListFilter = serde_json::from_value(json!({"limit": 101})).unwrap();
    assert!(invalid_filter.validate().is_err());
}

#[test]
fn script_response_maps_domain_script_to_api_shape() {
    let now = Utc::now();
    let script_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let scene_id = Uuid::new_v4();
    let script = Script::new(
        script_id,
        project_id,
        "程序员必看：ChatGPT工作流".to_string(),
        "还在手写重复代码？".to_string(),
        json!({"topic": "ChatGPT如何改变程序员工作流"}),
        ScriptStatus::Draft,
        None,
        vec![Scene {
            id: scene_id,
            sequence: 1,
            narration: "传统程序员每天要写大量重复代码。".to_string(),
            visual_description: "程序员盯着屏幕，快速切换多个代码文件。".to_string(),
            emotion: "焦虑".to_string(),
            duration_sec: 8,
        }],
        now,
        now,
    );

    let response = ScriptResponse::from(script);
    let json_value: Value = serde_json::to_value(response).unwrap();

    assert_eq!(json_value["script_id"], script_id.to_string());
    assert_eq!(json_value["project_id"], project_id.to_string());
    assert_eq!(json_value["status"], "draft");
    assert_eq!(json_value["scenes"][0]["scene_id"], scene_id.to_string());
    assert_eq!(json_value["scenes"][0]["sequence"], 1);
}
