//! 作品库领域模型与确定性差异影响规则，不依赖数据库和 HTTP。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Work {
    pub id: Uuid,
    pub project_id: Uuid,
    pub script_id: Uuid,
    pub title: String,
    pub archived: bool,
    pub current_version_id: Option<Uuid>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkVersionStatus {
    Draft,
    Confirmed,
    Running,
    Completed,
    Failed,
}

impl WorkVersionStatus {
    pub fn allows_snapshot_mutation(self) -> bool {
        self == Self::Draft
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkVersion {
    pub id: Uuid,
    pub work_id: Uuid,
    pub version_no: i32,
    pub status: WorkVersionStatus,
    pub source_version_id: Option<Uuid>,
    pub source_manifest_version: String,
    pub input_snapshot: Value,
    pub model_snapshot: Value,
    pub parameter_snapshot: Value,
    pub prompt_snapshot: Value,
    pub timeline_snapshot: Value,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactRole {
    FinalVideo,
    Subtitle,
    Mix,
    AudioTrack,
    ProductionPackage,
    ReusableIntermediate,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkArtifact {
    pub id: Uuid,
    pub work_version_id: Uuid,
    pub role: ArtifactRole,
    pub material_id: Option<Uuid>,
    pub file_name: String,
    pub storage_path: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkTimeline {
    pub work_version_id: Uuid,
    pub video: Value,
    pub audio: Value,
    pub subtitles: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VersionFieldChange {
    pub path: String,
    pub old_value: Value,
    pub new_value: Value,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct DiffResourceUsage {
    pub video_task_count: usize,
    pub video_seconds: u64,
    pub tts_characters: usize,
    pub asr_seconds: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkVersionDiff {
    pub source_version_id: Uuid,
    pub draft_version_id: Uuid,
    pub changes: Vec<VersionFieldChange>,
    pub affected_nodes: Vec<String>,
    pub reused_nodes: Vec<String>,
    pub resource_usage: DiffResourceUsage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkLibraryDomainError {
    DraftRequired,
    SourceVersionMismatch,
    WorkMismatch,
}

impl fmt::Display for WorkLibraryDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DraftRequired => formatter.write_str("影响分析目标必须是 draft 版本"),
            Self::SourceVersionMismatch => formatter.write_str("草稿来源版本与影响分析来源不一致"),
            Self::WorkMismatch => formatter.write_str("来源版本与草稿不属于同一作品"),
        }
    }
}

impl std::error::Error for WorkLibraryDomainError {}

/// 比较全部不可变快照，并把结构化字段确定性映射到作品生成 DAG。
pub fn analyze_version_diff(
    source: &WorkVersion,
    draft: &WorkVersion,
) -> Result<WorkVersionDiff, WorkLibraryDomainError> {
    if !draft.status.allows_snapshot_mutation() {
        return Err(WorkLibraryDomainError::DraftRequired);
    }
    if source.work_id != draft.work_id {
        return Err(WorkLibraryDomainError::WorkMismatch);
    }
    if draft.source_version_id != Some(source.id) {
        return Err(WorkLibraryDomainError::SourceVersionMismatch);
    }

    let mut changes = Vec::new();
    for (root, old, new) in [
        (
            "input_snapshot",
            &source.input_snapshot,
            &draft.input_snapshot,
        ),
        (
            "model_snapshot",
            &source.model_snapshot,
            &draft.model_snapshot,
        ),
        (
            "parameter_snapshot",
            &source.parameter_snapshot,
            &draft.parameter_snapshot,
        ),
        (
            "prompt_snapshot",
            &source.prompt_snapshot,
            &draft.prompt_snapshot,
        ),
        (
            "timeline_snapshot",
            &source.timeline_snapshot,
            &draft.timeline_snapshot,
        ),
    ] {
        collect_changes(root, old, new, &mut changes);
    }

    let scene_ids = scene_ids(draft);
    let mut affected = BTreeSet::new();
    for change in &changes {
        map_change_to_nodes(&change.path, &scene_ids, &mut affected);
    }

    let mut resource_usage = DiffResourceUsage::default();
    for (index, scene_id) in scene_ids.iter().enumerate() {
        if affected.contains(&format!("video_segment:{scene_id}")) {
            resource_usage.video_task_count += 1;
            resource_usage.video_seconds += segment_seconds(draft, index);
        }
    }
    if affected.contains("tts") {
        resource_usage.tts_characters = narration_text(draft).chars().count();
    }
    if affected.contains("asr") {
        resource_usage.asr_seconds = total_video_seconds(draft);
    }

    let reused_nodes = scene_ids
        .iter()
        .map(|id| format!("video_segment:{id}"))
        .filter(|node| !affected.contains(node))
        .collect();

    Ok(WorkVersionDiff {
        source_version_id: source.id,
        draft_version_id: draft.id,
        changes,
        affected_nodes: ordered_nodes(affected, &scene_ids),
        reused_nodes,
        resource_usage,
    })
}

fn collect_changes(path: &str, old: &Value, new: &Value, output: &mut Vec<VersionFieldChange>) {
    match (old, new) {
        (Value::Object(old), Value::Object(new)) => {
            let keys = old.keys().chain(new.keys()).collect::<BTreeSet<_>>();
            for key in keys {
                collect_changes(
                    &format!("{path}.{key}"),
                    old.get(key).unwrap_or(&Value::Null),
                    new.get(key).unwrap_or(&Value::Null),
                    output,
                );
            }
        }
        (Value::Array(old), Value::Array(new)) => {
            for index in 0..old.len().max(new.len()) {
                collect_changes(
                    &format!("{path}.{index}"),
                    old.get(index).unwrap_or(&Value::Null),
                    new.get(index).unwrap_or(&Value::Null),
                    output,
                );
            }
        }
        _ if old != new => output.push(VersionFieldChange {
            path: path.to_string(),
            old_value: old.clone(),
            new_value: new.clone(),
        }),
        _ => {}
    }
}

fn map_change_to_nodes(path: &str, scene_ids: &[String], affected: &mut BTreeSet<String>) {
    if path.starts_with("timeline_snapshot.subtitle") {
        affected.insert("subtitle".into());
        affected.insert("compose".into());
        return;
    }
    if path.contains("voice") || path.contains("narration") || path.contains("audio_mode") {
        affected.extend([
            "tts".into(),
            "subtitle".into(),
            "mix".into(),
            "compose".into(),
        ]);
        return;
    }
    if path.contains("aspect_ratio") || path.contains("resolution") {
        add_all_video_nodes(scene_ids, affected);
        affected.extend(["mix".into(), "compose".into()]);
        return;
    }
    if let Some(index) = scene_index(path) {
        if let Some(scene_id) = scene_ids.get(index) {
            affected.insert(format!("video_segment:{scene_id}"));
            affected.extend(["mix".into(), "compose".into()]);
            return;
        }
    }
    // 模型、全局提示词和未知生产字段均保守影响全部视频节点及下游合成。
    add_all_video_nodes(scene_ids, affected);
    affected.extend(["mix".into(), "compose".into()]);
}

fn add_all_video_nodes(scene_ids: &[String], affected: &mut BTreeSet<String>) {
    affected.extend(scene_ids.iter().map(|id| format!("video_segment:{id}")));
}

fn scene_index(path: &str) -> Option<usize> {
    for marker in ["input_snapshot.scenes.", "prompt_snapshot.segments."] {
        if let Some(rest) = path.strip_prefix(marker) {
            return rest.split('.').next()?.parse().ok();
        }
    }
    None
}

fn scene_ids(version: &WorkVersion) -> Vec<String> {
    version
        .input_snapshot
        .get("scenes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(index, scene)| {
            scene
                .get("id")
                .or_else(|| scene.get("scene_id"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| (index + 1).to_string())
        })
        .collect()
}

fn segment_seconds(version: &WorkVersion, index: usize) -> u64 {
    version
        .prompt_snapshot
        .get("segments")
        .and_then(Value::as_array)
        .and_then(|segments| segments.get(index))
        .and_then(|segment| segment.get("duration_seconds"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

fn total_video_seconds(version: &WorkVersion) -> u64 {
    version
        .prompt_snapshot
        .get("segments")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|segment| segment.get("duration_seconds").and_then(Value::as_u64))
        .sum()
}

fn narration_text(version: &WorkVersion) -> String {
    if let Some(value) = version
        .input_snapshot
        .get("narration_override")
        .and_then(Value::as_str)
    {
        return value.to_string();
    }
    version
        .input_snapshot
        .get("scenes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|scene| scene.get("narration").and_then(Value::as_str))
        .collect()
}

fn ordered_nodes(mut affected: BTreeSet<String>, scene_ids: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for node in ["tts", "asr"] {
        if affected.remove(node) {
            result.push(node.into());
        }
    }
    for scene_id in scene_ids {
        let node = format!("video_segment:{scene_id}");
        if affected.remove(&node) {
            result.push(node);
        }
    }
    for node in ["subtitle", "mix", "compose"] {
        if affected.remove(node) {
            result.push(node.into());
        }
    }
    result.extend(affected);
    result
}
