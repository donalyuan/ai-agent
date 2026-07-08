use chrono::Utc;
use novex_api::agents::models::{
    AccountStrategyProfileRequest, CreateProjectRequest, GenerateScriptRequest, ProjectResponse,
    Scene, Script, ScriptListFilter, ScriptResponse, ScriptStatus, ScriptStyle,
};
use novex_api::repositories::{AccountStrategyProfile, Project};
use serde_json::{json, Value};
use uuid::Uuid;
use validator::Validate;

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
fn generate_script_request_accepts_scene_count_from_three_to_twelve() {
    for scene_count in [3, 12] {
        let request: GenerateScriptRequest = serde_json::from_value(json!({
            "project_id": Uuid::new_v4(),
            "topic": "ChatGPT如何改变程序员工作流",
            "scene_count": scene_count
        }))
        .unwrap();

        request.validate().unwrap();
        assert_eq!(request.scene_count_or_default(), scene_count);
    }
}

#[test]
fn create_project_request_validates_required_name_and_optional_context() {
    let request: CreateProjectRequest = serde_json::from_value(json!({
        "name": "科技博主",
        "positioning": "科技知识账号",
        "description": "面向程序员的知识短视频",
        "strategy_profile": {
            "target_audience": "内容运营负责人",
            "content_pillars": ["AI 工具", "内容生产"],
            "tone_style": "直接清晰",
            "forbidden_topics": ["夸大收益"],
            "reference_accounts": ["参考账号A"],
            "topic_preferences": "优先教程和案例"
        }
    }))
    .unwrap();

    request.validate_for_api().unwrap();
    let profile = request.strategy_profile.unwrap();
    assert_eq!(profile.target_audience, "内容运营负责人");
    assert_eq!(profile.content_pillars, vec!["AI 工具", "内容生产"]);

    let invalid: CreateProjectRequest = serde_json::from_value(json!({
        "name": "",
        "positioning": "科技知识账号",
        "description": "面向程序员的知识短视频"
    }))
    .unwrap();

    assert_eq!(invalid.validate_for_api().unwrap_err(), "项目名称不能为空");
}

#[test]
fn account_strategy_profile_request_normalizes_lists_and_rejects_invalid_fields() {
    let profile = AccountStrategyProfileRequest {
        target_audience: " 内容运营负责人 ".to_string(),
        content_pillars: vec![
            "AI 工具".to_string(),
            "".to_string(),
            "AI 工具".to_string(),
            "内容生产".to_string(),
        ],
        tone_style: " 直接清晰 ".to_string(),
        forbidden_topics: vec!["夸大收益".to_string(), "夸大收益".to_string()],
        reference_accounts: vec!["参考账号A".to_string()],
        topic_preferences: "优先教程和案例".to_string(),
    };

    let normalized = profile.normalize().unwrap();
    assert_eq!(normalized.target_audience, "内容运营负责人");
    assert_eq!(normalized.content_pillars, vec!["AI 工具", "内容生产"]);
    assert_eq!(normalized.tone_style, "直接清晰");
    assert_eq!(normalized.forbidden_topics, vec!["夸大收益"]);

    let invalid = AccountStrategyProfileRequest {
        content_pillars: (0..21).map(|index| format!("支柱{index}")).collect(),
        ..AccountStrategyProfileRequest::default()
    };
    assert_eq!(invalid.normalize().unwrap_err(), "内容支柱最多填写 20 项");
}

#[test]
fn project_response_maps_repository_project_to_api_shape() {
    let now = Utc::now();
    let project_id = Uuid::new_v4();
    let response = ProjectResponse::from(Project {
        id: project_id,
        name: "科技博主".to_string(),
        positioning: "科技知识账号".to_string(),
        description: "面向程序员的知识短视频".to_string(),
        strategy_profile: AccountStrategyProfile {
            target_audience: "内容运营负责人".to_string(),
            content_pillars: vec!["AI 工具".to_string(), "内容生产".to_string()],
            tone_style: "直接清晰".to_string(),
            forbidden_topics: vec!["夸大收益".to_string()],
            reference_accounts: vec!["参考账号A".to_string()],
            topic_preferences: "优先教程和案例".to_string(),
        },
        status: "active".to_string(),
        created_at: now,
        updated_at: now,
    });
    let json_value: Value = serde_json::to_value(response).unwrap();

    assert_eq!(json_value["project_id"], project_id.to_string());
    assert_eq!(json_value["name"], "科技博主");
    assert_eq!(json_value["positioning"], "科技知识账号");
    assert_eq!(json_value["status"], "active");
    assert_eq!(
        json_value["strategy_profile"]["target_audience"],
        "内容运营负责人"
    );
    assert_eq!(
        json_value["strategy_profile"]["content_pillars"],
        json!(["AI 工具", "内容生产"])
    );
}

#[test]
fn generate_script_request_rejects_too_long_topic_and_invalid_scene_count() {
    let payload = json!({
        "project_id": Uuid::new_v4(),
        "topic": "太".repeat(201),
        "scene_count": 13
    });

    let request: GenerateScriptRequest = serde_json::from_value(payload).unwrap();
    let errors = request.validate().unwrap_err();

    assert!(errors.field_errors().contains_key("topic"));
    assert!(errors.field_errors().contains_key("scene_count"));
}

#[test]
fn generate_script_request_allows_empty_topic_for_topic_id_flow() {
    let request: GenerateScriptRequest = serde_json::from_value(json!({
        "project_id": Uuid::new_v4(),
        "topic_id": Uuid::new_v4(),
        "style": "knowledge",
        "scene_count": 5
    }))
    .unwrap();

    request.validate().unwrap();
    assert!(request.topic.is_empty());
    assert!(request.topic_id.is_some());
}

#[test]
fn generate_script_request_rejects_scene_count_outside_three_to_twelve() {
    for scene_count in [2, 13] {
        let request: GenerateScriptRequest = serde_json::from_value(json!({
            "project_id": Uuid::new_v4(),
            "topic": "ChatGPT如何改变程序员工作流",
            "scene_count": scene_count
        }))
        .unwrap();

        let errors = request.validate().unwrap_err();

        assert!(errors.field_errors().contains_key("scene_count"));
    }
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
        None,
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

#[test]
fn script_response_exposes_topic_snapshot_when_script_was_generated_from_topic() {
    let now = Utc::now();
    let script_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let topic_id = Uuid::new_v4();
    let script = Script::new(
        script_id,
        project_id,
        Some(topic_id),
        "AI 工具如何重塑内容团队".to_string(),
        "三个镜头看懂内容团队的 AI 工作流。".to_string(),
        json!({
            "topic": "AI 工具如何重塑内容团队",
            "topic_id": topic_id,
            "topic_snapshot": {
                "topic_id": topic_id,
                "title": "AI 工具如何重塑内容团队",
                "content_type": "knowledge",
                "score": 91,
                "tags": ["AI工具", "内容运营"]
            }
        }),
        ScriptStatus::Draft,
        None,
        vec![],
        now,
        now,
    );

    let response = ScriptResponse::from(script);
    let json_value: Value = serde_json::to_value(response).unwrap();

    assert_eq!(json_value["topic_id"], topic_id.to_string());
    assert_eq!(
        json_value["topic_snapshot"]["topic_id"],
        topic_id.to_string()
    );
    assert_eq!(
        json_value["topic_snapshot"]["title"],
        "AI 工具如何重塑内容团队"
    );
    assert_eq!(json_value["topic_snapshot"]["content_type"], "knowledge");
}
