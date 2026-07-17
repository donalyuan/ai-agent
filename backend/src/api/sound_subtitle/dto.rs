use crate::application::sound_subtitle::{SoundTaskIntent, SoundTaskPreflight};
use crate::repositories::{AudioMaterialInspection, SoundSubtitleTask};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize)]
pub(super) struct SoundTaskRequest {
    pub task_type: String,
    pub model_id: Uuid,
    #[serde(default)]
    pub text_content: String,
    #[serde(default)]
    pub voice_type: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub emotion: Option<String>,
    #[serde(default = "empty_object")]
    pub parameters: Value,
    #[serde(default)]
    pub generate_subtitle: bool,
    #[serde(default)]
    pub subtitle_segments: Vec<String>,
    #[serde(default)]
    pub source_audio_material_id: Option<Uuid>,
    #[serde(default)]
    pub audio_inspection_id: Option<Uuid>,
    #[serde(default)]
    pub source_script_id: Option<Uuid>,
    #[serde(default)]
    pub source_script_updated_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub source_script_scene_ids: Vec<Uuid>,
    #[serde(default)]
    pub confirmation_token: Option<String>,
}

impl SoundTaskRequest {
    pub(super) fn into_intent(self) -> SoundTaskIntent {
        SoundTaskIntent {
            task_type: self.task_type.trim().to_string(),
            model_id: self.model_id,
            text_content: self.text_content,
            voice_type: self.voice_type,
            language: self.language,
            emotion: self.emotion,
            parameters: self.parameters,
            generate_subtitle: self.generate_subtitle,
            subtitle_segments: self.subtitle_segments,
            source_audio_material_id: self.source_audio_material_id,
            audio_inspection_id: self.audio_inspection_id,
            source_script_id: self.source_script_id,
            source_script_updated_at: self.source_script_updated_at,
            source_script_scene_ids: self.source_script_scene_ids,
        }
    }

    pub(super) fn split_creation(self) -> Result<(SoundTaskIntent, String), String> {
        let token = self
            .confirmation_token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| "创建声音任务前必须提交确认摘要".to_string())?;
        Ok((self.into_intent(), token))
    }
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct SoundTaskPreflightResponse {
    pub task_type: String,
    pub model_id: Uuid,
    pub model_display_name: String,
    pub voice_snapshot: Option<Value>,
    pub resource_usage: Value,
    pub normalized_parameters: Value,
    pub source_script_snapshot: Option<Value>,
    pub confirmation_token: String,
}

impl From<SoundTaskPreflight> for SoundTaskPreflightResponse {
    fn from(preflight: SoundTaskPreflight) -> Self {
        Self {
            task_type: preflight.task_type,
            model_id: preflight.model_id,
            model_display_name: preflight.model_display_name,
            voice_snapshot: preflight.voice_snapshot,
            resource_usage: preflight.resource_usage,
            normalized_parameters: preflight.normalized_parameters,
            source_script_snapshot: preflight.source_script_snapshot,
            confirmation_token: preflight.confirmation_token,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct AudioInspectionResponse {
    pub inspection_id: Uuid,
    pub project_id: Uuid,
    pub material_id: Uuid,
    pub status: String,
    pub source_sha256: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub duration_ms: Option<i64>,
    pub container_format: Option<String>,
    pub audio_codec: Option<String>,
    pub sample_rate_hz: Option<i32>,
    pub channel_count: Option<i32>,
    pub error_code: Option<String>,
    pub error_summary: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<AudioMaterialInspection> for AudioInspectionResponse {
    fn from(inspection: AudioMaterialInspection) -> Self {
        Self {
            inspection_id: inspection.id,
            project_id: inspection.project_id,
            material_id: inspection.material_id,
            status: inspection.status,
            source_sha256: inspection.source_sha256,
            file_size_bytes: inspection.file_size_bytes,
            duration_ms: inspection.duration_ms,
            container_format: inspection.container_format,
            audio_codec: inspection.audio_codec,
            sample_rate_hz: inspection.sample_rate_hz,
            channel_count: inspection.channel_count,
            error_code: inspection.error_code,
            error_summary: inspection.error_summary,
            started_at: inspection.started_at,
            completed_at: inspection.completed_at,
            created_at: inspection.created_at,
            updated_at: inspection.updated_at,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct SoundTaskResponse {
    pub task_id: Uuid,
    pub project_id: Uuid,
    pub parent_task_id: Option<Uuid>,
    pub task_type: String,
    pub status: String,
    pub model_id: Uuid,
    pub audio_inspection_id: Option<Uuid>,
    pub source_audio_material_id: Option<Uuid>,
    pub source_script_id: Option<Uuid>,
    pub source_script_snapshot: Option<Value>,
    pub output_audio_material_id: Option<Uuid>,
    pub output_subtitle_material_id: Option<Uuid>,
    pub text_content: String,
    pub voice_type: Option<String>,
    pub language: Option<String>,
    pub emotion: Option<String>,
    pub parameters: Value,
    pub generate_subtitle: bool,
    pub subtitle_segments: Vec<String>,
    pub model_snapshot: Option<Value>,
    pub voice_snapshot: Option<Value>,
    pub resource_usage: Value,
    pub timeline: Option<Value>,
    pub result: Option<Value>,
    pub request_id: Uuid,
    pub upstream_log_id: Option<String>,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub error_code: Option<String>,
    pub error_summary: Option<String>,
    pub error_details: Value,
    pub staging_status: String,
    pub cleanup_attempt_count: i32,
    pub cleanup_error_summary: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<SoundSubtitleTask> for SoundTaskResponse {
    fn from(task: SoundSubtitleTask) -> Self {
        let generate_subtitle = task
            .confirmation_snapshot
            .get("generate_subtitle")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let subtitle_segments = task
            .confirmation_snapshot
            .get("subtitle_segments")
            .and_then(Value::as_array)
            .map(|segments| {
                segments
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        Self {
            task_id: task.id,
            project_id: task.project_id,
            parent_task_id: task.parent_task_id,
            task_type: task.task_type,
            status: task.status,
            model_id: task.model_id,
            audio_inspection_id: task.audio_inspection_id,
            source_audio_material_id: task.source_audio_material_id,
            source_script_id: task.source_script_id,
            source_script_snapshot: task.source_script_snapshot,
            output_audio_material_id: task.output_audio_material_id,
            output_subtitle_material_id: task.output_subtitle_material_id,
            text_content: task.text_content,
            voice_type: task.voice_type,
            language: task.language,
            emotion: task.emotion,
            parameters: task.parameters,
            generate_subtitle,
            subtitle_segments,
            model_snapshot: task.model_snapshot,
            voice_snapshot: task.voice_snapshot,
            resource_usage: task.resource_usage,
            timeline: task.timeline,
            result: task.result,
            request_id: task.request_id,
            upstream_log_id: task.upstream_log_id,
            attempt_count: task.attempt_count,
            max_attempts: task.max_attempts,
            error_code: task.error_code,
            error_summary: task.error_summary,
            error_details: task.error_details,
            staging_status: task.staging_status,
            cleanup_attempt_count: task.cleanup_attempt_count,
            cleanup_error_summary: task.cleanup_error_summary,
            started_at: task.started_at,
            completed_at: task.completed_at,
            created_at: task.created_at,
            updated_at: task.updated_at,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct SoundTaskListResponse {
    pub tasks: Vec<SoundTaskResponse>,
}

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}
