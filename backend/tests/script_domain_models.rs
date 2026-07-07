use chrono::Utc;
use novex_api::agents::models::{Scene, Script, ScriptStatus};
use serde_json::json;
use uuid::Uuid;

#[test]
fn script_status_round_trips_database_values() {
    assert_eq!(
        ScriptStatus::try_from("draft").unwrap(),
        ScriptStatus::Draft
    );
    assert_eq!(
        ScriptStatus::try_from("approved").unwrap(),
        ScriptStatus::Approved
    );
    assert_eq!(
        ScriptStatus::try_from("archived").unwrap(),
        ScriptStatus::Archived
    );

    assert_eq!(String::from(ScriptStatus::Draft), "draft");
    assert_eq!(String::from(ScriptStatus::Approved), "approved");
    assert_eq!(String::from(ScriptStatus::Archived), "archived");
    assert!(ScriptStatus::try_from("deleted").is_err());
}

#[test]
fn script_aggregate_orders_scenes_by_sequence() {
    let now = Utc::now();
    let scene_two = Scene {
        id: Uuid::new_v4(),
        sequence: 2,
        narration: "第二个分镜旁白".to_string(),
        visual_description: "第二个分镜画面".to_string(),
        emotion: "好奇".to_string(),
        duration_sec: 9,
    };
    let scene_one = Scene {
        id: Uuid::new_v4(),
        sequence: 1,
        narration: "第一个分镜旁白".to_string(),
        visual_description: "第一个分镜画面".to_string(),
        emotion: "焦虑".to_string(),
        duration_sec: 8,
    };

    let script = Script::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        None,
        "程序员必看：ChatGPT工作流".to_string(),
        "还在手写重复代码？".to_string(),
        json!({"topic": "ChatGPT如何改变程序员工作流"}),
        ScriptStatus::Draft,
        None,
        vec![scene_two, scene_one],
        now,
        now,
    );

    let sequences: Vec<i32> = script.scenes.iter().map(|scene| scene.sequence).collect();
    assert_eq!(sequences, vec![1, 2]);
    assert_eq!(script.total_duration_sec(), 17);
}
