use crate::application::work_generation::{CreateWorkPlanInput, WorkPlanView, WorkRunView};
use crate::domain::work_generation::{AudioMode, DurationStrategy};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize)]
pub struct CreateWorkPlanRequest {
    pub llm_model_id: Uuid,
    pub video_model_id: Uuid,
    pub tts_model_id: Option<Uuid>,
    pub tts_voice_type: Option<String>,
    pub narration_override: Option<String>,
    pub duration_strategy: DurationStrategy,
    pub duration_seconds: Option<u32>,
    pub aspect_ratio: String,
    pub resolution: String,
    pub audio_mode: AudioMode,
    #[serde(default)]
    pub full_prompt: String,
    pub scene_prompts: Option<Vec<String>>,
    pub segment_prompts: Option<Vec<String>>,
    pub narration_seconds: Option<u32>,
    #[serde(default)]
    pub audio_material_ids: Vec<Uuid>,
    #[serde(default = "default_true")]
    pub burn_subtitles: bool,
}

impl CreateWorkPlanRequest {
    pub fn into_input(self, script_id: Uuid) -> CreateWorkPlanInput {
        CreateWorkPlanInput {
            script_id,
            llm_model_id: self.llm_model_id,
            video_model_id: self.video_model_id,
            tts_model_id: self.tts_model_id,
            tts_voice_type: self.tts_voice_type,
            narration_override: self.narration_override,
            duration_strategy: self.duration_strategy,
            duration_seconds: self.duration_seconds,
            aspect_ratio: self.aspect_ratio,
            resolution: self.resolution,
            audio_mode: self.audio_mode,
            full_prompt: self.full_prompt,
            scene_prompts: self.scene_prompts,
            segment_prompts: self.segment_prompts,
            narration_seconds: self.narration_seconds,
            audio_material_ids: self.audio_material_ids,
            burn_subtitles: self.burn_subtitles,
        }
    }
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_defaults_to_burned_subtitles_and_accepts_segment_prompts() {
        let request: CreateWorkPlanRequest = serde_json::from_value(json!({
            "llm_model_id": Uuid::new_v4(),
            "video_model_id": Uuid::new_v4(),
            "tts_model_id": Uuid::new_v4(),
            "narration_override": " 精简旁白 ",
            "duration_strategy": "preset30",
            "aspect_ratio": "16:9",
            "resolution": "1080p",
            "audio_mode": "independent_tts",
            "segment_prompts": ["开场", "结尾"]
        }))
        .unwrap();

        assert!(request.burn_subtitles);
        assert_eq!(request.narration_override.as_deref(), Some(" 精简旁白 "));
        assert_eq!(request.segment_prompts.unwrap(), vec!["开场", "结尾"]);
    }

    #[test]
    fn request_accepts_silent_mode_without_tts() {
        let request: CreateWorkPlanRequest = serde_json::from_value(json!({
            "llm_model_id": Uuid::new_v4(),
            "video_model_id": Uuid::new_v4(),
            "tts_model_id": null,
            "duration_strategy": "preset15",
            "aspect_ratio": "16:9",
            "resolution": "1080p",
            "audio_mode": "silent"
        }))
        .unwrap();

        assert_eq!(request.audio_mode, AudioMode::Silent);
        assert!(request.tts_model_id.is_none());
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct WorkPlanResponse {
    pub work_id: Uuid,
    pub work_title: String,
    pub plan_id: Uuid,
    pub work_version_id: Uuid,
    pub plan_version: i32,
    pub status: String,
    pub input_fingerprint: String,
    pub model_snapshot: Value,
    pub capability_snapshot: Value,
    pub output_snapshot: Value,
    pub prompt_snapshot: Value,
    pub timeline_snapshot: Value,
    pub resource_usage: Value,
    pub warnings: Value,
    pub segments: Value,
    pub can_confirm: bool,
    pub blockers: Vec<String>,
    pub created_at: DateTime<Utc>,
}

impl From<WorkPlanView> for WorkPlanResponse {
    fn from(value: WorkPlanView) -> Self {
        Self {
            work_id: value.work.id,
            work_title: value.work.title,
            plan_id: value.plan.id,
            work_version_id: value.plan.work_version_id,
            plan_version: value.plan.plan_version,
            status: value.plan.status,
            input_fingerprint: value.plan.input_fingerprint,
            model_snapshot: serde_json::json!({"llm_model_id": value.plan.llm_model_id, "video_model_id": value.plan.video_model_id, "tts_model_id": value.plan.tts_model_id}),
            capability_snapshot: value.plan.capability_snapshot,
            output_snapshot: value.plan.output_snapshot,
            prompt_snapshot: value.plan.prompt_snapshot,
            timeline_snapshot: value.plan.timeline_snapshot,
            resource_usage: value.resource_usage,
            warnings: value.plan.warnings,
            segments: value.segments,
            can_confirm: value.can_confirm,
            blockers: value.blockers,
            created_at: Utc::now(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct WorkRunResponse {
    pub run_id: Uuid,
    pub work_id: Uuid,
    pub work_version_id: Uuid,
    pub work_plan_id: Uuid,
    pub status: String,
    pub created: bool,
    pub resource_usage: Value,
}

impl From<WorkRunView> for WorkRunResponse {
    fn from(value: WorkRunView) -> Self {
        Self {
            run_id: value.run.id,
            work_id: value.run.work_id,
            work_version_id: value.run.work_version_id,
            work_plan_id: value.run.work_plan_id,
            status: value.run.status,
            created: value.created,
            resource_usage: value.run.resource_usage,
        }
    }
}
