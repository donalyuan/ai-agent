use novex_api::domain::work_library::{
    analyze_version_diff, ArtifactRole, Work, WorkArtifact, WorkTimeline, WorkVersion,
    WorkVersionStatus,
};
use serde_json::json;
use uuid::Uuid;

fn version(snapshot: serde_json::Value) -> WorkVersion {
    WorkVersion {
        id: Uuid::new_v4(),
        work_id: Uuid::new_v4(),
        version_no: 2,
        status: WorkVersionStatus::Draft,
        source_version_id: Some(Uuid::new_v4()),
        source_manifest_version: "manifest-v2".into(),
        input_snapshot: snapshot,
        model_snapshot: json!({"video_model_id": Uuid::new_v4()}),
        parameter_snapshot: json!({"aspect_ratio":"16:9","resolution":"1080p"}),
        prompt_snapshot: json!({"segments":[{"sequence":1,"duration_seconds":8},{"sequence":2,"duration_seconds":7}]}),
        timeline_snapshot: json!({"subtitle":{"text":"原字幕","style":"default","burn":true}}),
    }
}

#[test]
fn work_library_entities_keep_version_and_artifact_ownership_explicit() {
    let work_id = Uuid::new_v4();
    let version_id = Uuid::new_v4();
    let work = Work {
        id: work_id,
        project_id: Uuid::new_v4(),
        script_id: Uuid::new_v4(),
        title: "演示作品".into(),
        archived: false,
        current_version_id: Some(version_id),
    };
    let artifact = WorkArtifact {
        id: Uuid::new_v4(),
        work_version_id: version_id,
        role: ArtifactRole::FinalVideo,
        material_id: Some(Uuid::new_v4()),
        file_name: "demo.mp4".into(),
        storage_path: "works/demo.mp4".into(),
        mime_type: "video/mp4".into(),
        size_bytes: 1024,
        sha256: "a".repeat(64),
    };
    let timeline = WorkTimeline {
        work_version_id: version_id,
        video: json!([]),
        audio: json!([]),
        subtitles: json!([]),
    };

    assert_eq!(work.current_version_id, Some(version_id));
    assert_eq!(artifact.work_version_id, version_id);
    assert_eq!(artifact.role, ArtifactRole::FinalVideo);
    assert_eq!(timeline.work_version_id, version_id);
}

#[test]
fn only_draft_version_accepts_snapshot_mutation() {
    assert!(WorkVersionStatus::Draft.allows_snapshot_mutation());
    for status in [
        WorkVersionStatus::Confirmed,
        WorkVersionStatus::Running,
        WorkVersionStatus::Completed,
        WorkVersionStatus::Failed,
    ] {
        assert!(!status.allows_snapshot_mutation(), "{status:?} 必须不可变");
    }
}

#[test]
fn single_scene_prompt_change_regenerates_only_that_video_and_downstream_compose() {
    let mut source = version(
        json!({"scenes":[{"id":"scene-1","visual_description":"旧画面"},{"id":"scene-2","visual_description":"保留"}]}),
    );
    source.status = WorkVersionStatus::Completed;
    let mut draft = source.clone();
    draft.id = Uuid::new_v4();
    draft.status = WorkVersionStatus::Draft;
    draft.source_version_id = Some(source.id);
    draft.input_snapshot["scenes"][0]["visual_description"] = json!("新画面");

    let diff = analyze_version_diff(&source, &draft).unwrap();

    assert!(diff
        .affected_nodes
        .contains(&"video_segment:scene-1".into()));
    assert!(diff.affected_nodes.contains(&"mix".into()));
    assert!(diff.affected_nodes.contains(&"compose".into()));
    assert!(!diff
        .affected_nodes
        .contains(&"video_segment:scene-2".into()));
    assert_eq!(diff.resource_usage.video_task_count, 1);
    assert_eq!(diff.resource_usage.video_seconds, 8);
    assert!(diff.reused_nodes.contains(&"video_segment:scene-2".into()));
}

#[test]
fn output_dimension_change_regenerates_all_video_segments() {
    let mut source = version(json!({"scenes":[{"id":"scene-1"},{"id":"scene-2"}]}));
    source.status = WorkVersionStatus::Completed;
    let mut draft = source.clone();
    draft.id = Uuid::new_v4();
    draft.status = WorkVersionStatus::Draft;
    draft.source_version_id = Some(source.id);
    draft.parameter_snapshot["resolution"] = json!("720p");

    let diff = analyze_version_diff(&source, &draft).unwrap();

    assert!(diff
        .affected_nodes
        .contains(&"video_segment:scene-1".into()));
    assert!(diff
        .affected_nodes
        .contains(&"video_segment:scene-2".into()));
    assert_eq!(diff.resource_usage.video_task_count, 2);
    assert_eq!(diff.resource_usage.video_seconds, 15);
}

#[test]
fn subtitle_only_change_never_schedules_video_tts_or_asr() {
    let mut source = version(json!({"scenes":[{"id":"scene-1"},{"id":"scene-2"}]}));
    source.status = WorkVersionStatus::Completed;
    let mut draft = source.clone();
    draft.id = Uuid::new_v4();
    draft.status = WorkVersionStatus::Draft;
    draft.source_version_id = Some(source.id);
    draft.timeline_snapshot["subtitle"]["text"] = json!("新字幕");

    let diff = analyze_version_diff(&source, &draft).unwrap();

    assert_eq!(diff.affected_nodes, vec!["subtitle", "compose"]);
    assert_eq!(diff.resource_usage.video_task_count, 0);
    assert_eq!(diff.resource_usage.tts_characters, 0);
    assert_eq!(diff.resource_usage.asr_seconds, 0);
    let serialized = serde_json::to_value(diff).unwrap();
    assert!(serialized.get("cost").is_none());
    assert!(serialized.get("amount").is_none());
    assert!(serialized.get("currency").is_none());
}
