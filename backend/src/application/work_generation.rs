//! 作品生成用例：把主画面清单转成可确认计划，并以幂等键创建一次作品运行。

use crate::application::asset_generation::{
    AssetGenerationApplicationError, AssetGenerationService, SceneVisualManifestBlocker,
};
use crate::domain::work_generation::{
    self, AudioMode, DurationStrategy, OutputSpec, ReferenceImageMode, SceneInput, VideoCapability,
    WorkGenerationError,
};
use crate::repositories::{
    AiModelRepository, PostgresAiModelRepository, PostgresVoiceCatalogRepository,
    PostgresWorkGenerationRepository, WorkGenerationAttemptRecord, WorkGenerationRepository,
    WorkGenerationRunRecord, WorkGenerationTaskCounts, WorkGenerationTaskFilter,
    WorkGenerationTaskRecord, WorkPlanRecord, WorkRecord, WorkRepositoryError,
};
use novex_model::{ApiProtocol, ModelType};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::{fmt, sync::Arc};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CreateWorkPlanInput {
    pub script_id: Uuid,
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
    pub full_prompt: String,
    pub scene_prompts: Option<Vec<String>>,
    pub segment_prompts: Option<Vec<String>>,
    pub narration_seconds: Option<u32>,
    pub audio_material_ids: Vec<Uuid>,
    pub burn_subtitles: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct WorkPlanView {
    pub work: WorkRecord,
    pub plan: WorkPlanRecord,
    pub segments: Value,
    pub resource_usage: Value,
    pub can_confirm: bool,
    pub blockers: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct WorkRunView {
    pub run: WorkGenerationRunRecord,
    pub created: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct WorkTaskListView {
    pub tasks: Vec<WorkGenerationTaskRecord>,
    pub counts: WorkGenerationTaskCounts,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct WorkTaskDetailsView {
    pub task: WorkGenerationTaskRecord,
    pub steps: Vec<crate::repositories::WorkGenerationStepRecord>,
}

#[derive(Clone)]
pub struct WorkGenerationService {
    repository: PostgresWorkGenerationRepository,
    ai_model_repository: PostgresAiModelRepository,
    voice_catalog_repository: PostgresVoiceCatalogRepository,
    asset_generation: Arc<AssetGenerationService>,
}

impl WorkGenerationService {
    pub fn new(
        repository: PostgresWorkGenerationRepository,
        ai_model_repository: PostgresAiModelRepository,
        voice_catalog_repository: PostgresVoiceCatalogRepository,
        asset_generation: AssetGenerationService,
    ) -> Self {
        Self {
            repository,
            ai_model_repository,
            voice_catalog_repository,
            asset_generation: Arc::new(asset_generation),
        }
    }

    pub async fn plan(
        &self,
        input: CreateWorkPlanInput,
    ) -> Result<WorkPlanView, WorkGenerationApplicationError> {
        let narration_override = normalize_narration_override(input.narration_override.clone())
            .map_err(WorkGenerationApplicationError::Validation)?;
        if narration_override.is_some()
            && matches!(
                input.audio_mode,
                AudioMode::SeedanceOriginal | AudioMode::Silent
            )
        {
            return Err(WorkGenerationApplicationError::Validation(
                "当前声音模式不能使用独立 TTS 旁白覆盖".into(),
            ));
        }
        if matches!(input.audio_mode, AudioMode::Silent)
            && (input.tts_model_id.is_some()
                || input
                    .tts_voice_type
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty()))
        {
            return Err(WorkGenerationApplicationError::Validation(
                "静音模式不得选择 TTS 模型或音色".into(),
            ));
        }
        let manifest = match self
            .asset_generation
            .scene_visual_manifest(input.script_id)
            .await
        {
            Ok(manifest) => manifest,
            Err(AssetGenerationApplicationError::ManifestIncomplete {
                script_id,
                blockers,
            }) => {
                return Err(WorkGenerationApplicationError::ManifestIncomplete {
                    script_id,
                    blockers,
                });
            }
            Err(error) => {
                return Err(WorkGenerationApplicationError::Validation(
                    error.to_string(),
                ))
            }
        };
        if !input.audio_material_ids.is_empty() {
            let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM materials WHERE project_id=$1 AND id = ANY($2) AND material_type='audio' AND status='active'")
                .bind(manifest.project_id).bind(&input.audio_material_ids).fetch_one(self.repository_pool()).await.map_err(WorkRepositoryError::from)?;
            if count != input.audio_material_ids.len() as i64 {
                return Err(WorkGenerationApplicationError::Validation(
                    "存在不可用的已有音频素材".into(),
                ));
            }
        }
        self.ai_model_repository
            .resolve_enabled(input.llm_model_id, ModelType::Text)
            .await
            .map_err(|error| WorkGenerationApplicationError::Validation(error.to_string()))?;
        let video_runtime = self
            .ai_model_repository
            .resolve_enabled(input.video_model_id, ModelType::Video)
            .await
            .map_err(|error| WorkGenerationApplicationError::Validation(error.to_string()))?;
        let mut voice_snapshot = json!({});
        let mut selected_voice_languages = None;
        let mut tts_max_input_characters = None;
        if audio_mode_requires_tts(input.audio_mode) {
            let tts_model_id = input.tts_model_id.ok_or_else(|| {
                WorkGenerationApplicationError::Validation("当前声音模式必须选择 TTS 模型".into())
            })?;
            let tts = self
                .ai_model_repository
                .resolve_enabled(tts_model_id, ModelType::Speech)
                .await
                .map_err(|error| WorkGenerationApplicationError::Validation(error.to_string()))?;
            if !matches!(
                tts.snapshot.api_protocol,
                ApiProtocol::VolcengineTtsV3 | ApiProtocol::OpenAiAudioSpeech
            ) {
                return Err(WorkGenerationApplicationError::Validation(
                    "作品 TTS 模型必须使用可执行 TTS 协议".into(),
                ));
            }
            let voice_type = input
                .tts_voice_type
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    WorkGenerationApplicationError::Validation(
                        "当前声音模式必须选择可用音色".into(),
                    )
                })?;
            let catalog = self
                .voice_catalog_repository
                .catalog(tts_model_id, false)
                .await
                .map_err(|error| WorkGenerationApplicationError::Validation(error.to_string()))?;
            let voice = catalog
                .voices
                .iter()
                .find(|voice| voice.voice_type == voice_type && voice.is_available)
                .ok_or_else(|| {
                    WorkGenerationApplicationError::Validation("所选 TTS 音色不可用".into())
                })?;
            selected_voice_languages = Some(voice.languages.clone());
            tts_max_input_characters = tts
                .snapshot
                .settings
                .get("max_input_characters")
                .and_then(Value::as_u64)
                .map(|value| value as usize);
            voice_snapshot = json!({
                "model_id": tts_model_id,
                "source_model_id": catalog.source_model_id,
                "voice_id": voice.id,
                "voice_type": voice.voice_type,
                "resource_id": voice.resource_id,
                "name": voice.name,
                "catalog_version": voice.catalog_version
            });
        }
        let work = self
            .repository
            .find_or_create_work(
                manifest.project_id,
                manifest.script_id,
                &manifest.script_title,
            )
            .await?;
        let capability = capability_from_settings(
            &video_runtime.snapshot.protocol_version,
            &video_runtime.snapshot.upstream_model,
            &video_runtime.snapshot.settings,
        );
        let output = OutputSpec {
            duration_strategy: input.duration_strategy,
            duration_seconds: input.duration_seconds,
            aspect_ratio: input.aspect_ratio.clone(),
            resolution: input.resolution.clone(),
        };
        let target =
            work_generation::validate_output(&output, &capability, input.narration_seconds)?;
        let subtitle_source =
            work_generation::validate_audio_mode(input.audio_mode, capability.audio_supported)?;
        let prompts = input.scene_prompts.unwrap_or_default();
        let scenes = manifest
            .scenes
            .into_iter()
            .enumerate()
            .map(|(index, scene)| SceneInput {
                scene_id: scene.scene_id,
                sequence: scene.sequence,
                image_material_id: scene.material_id,
                image_url: scene.file_url,
                prompt: prompts
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| scene.visual_description.clone()),
                narration: scene.narration,
                duration_seconds: scene.duration_sec.max(1) as u32,
            })
            .collect::<Vec<_>>();
        let mut segments = work_generation::build_segments(&scenes, target, &capability)?;
        work_generation::apply_segment_prompt_overrides(
            &mut segments,
            input.segment_prompts,
            capability.max_prompt_chars,
        )?;
        let effective_tts_text = narration_override.clone().unwrap_or_else(|| {
            scenes
                .iter()
                .map(|scene| scene.narration.trim())
                .collect::<String>()
        });
        if audio_mode_requires_tts(input.audio_mode) {
            let max_characters = tts_max_input_characters.ok_or_else(|| {
                WorkGenerationApplicationError::Validation("当前 TTS 模型缺少文本字符上限".into())
            })?;
            if effective_tts_text.chars().count() > max_characters {
                return Err(WorkGenerationApplicationError::Validation(format!(
                    "旁白共 {} 字，超过当前 TTS 模型上限 {} 字",
                    effective_tts_text.chars().count(),
                    max_characters
                )));
            }
            if contains_chinese(&effective_tts_text)
                && !voice_languages_support_chinese(
                    selected_voice_languages.as_ref().unwrap_or(&Value::Null),
                )
            {
                return Err(WorkGenerationApplicationError::Validation(
                    "所选 TTS 音色不支持中文旁白".into(),
                ));
            }
        }
        let input_snapshot = json!({"script_id": input.script_id, "scenes": scenes, "narration_override": narration_override, "output": output, "audio_mode": input.audio_mode, "voice_snapshot": voice_snapshot, "audio_material_ids": input.audio_material_ids, "burn_subtitles": input.burn_subtitles});
        let fingerprint = fingerprint(&input_snapshot);
        let plan_id = Uuid::new_v4();
        let plan_version = self
            .repository
            .latest_plan(work.id)
            .await?
            .map(|plan| plan.plan_version + 1)
            .unwrap_or(1);
        let usage = json!({"video_task_count": segments.len(), "video_seconds": target, "tts_characters": if audio_mode_requires_tts(input.audio_mode) { effective_tts_text.chars().count() } else { 0 }, "asr_seconds": if matches!(input.audio_mode, AudioMode::SeedanceOriginal) { target } else { 0 }});
        let plan = WorkPlanRecord {
            id: plan_id,
            work_id: work.id,
            work_version_id: Uuid::nil(),
            plan_version,
            status: "ready".into(),
            input_fingerprint: fingerprint,
            llm_model_id: Some(input.llm_model_id),
            video_model_id: Some(input.video_model_id),
            tts_model_id: input.tts_model_id,
            capability_snapshot: serde_json::to_value(&capability).unwrap_or_else(|_| json!({})),
            output_snapshot: serde_json::to_value(&output).unwrap_or_else(|_| json!({})),
            prompt_snapshot: json!({"full_prompt": input.full_prompt, "segments": segments}),
            timeline_snapshot: json!({"duration_seconds": target, "subtitle_source": subtitle_source, "audio_mode": input.audio_mode, "audio_material_ids": input.audio_material_ids, "voice_snapshot": voice_snapshot, "burn_subtitles": input.burn_subtitles, "save_srt": true}),
            resource_usage: usage.clone(),
            warnings: if matches!(input.audio_mode, AudioMode::SeedanceOriginalAndTts) {
                json!(["原声不可分轨，TTS 区间将降低整体原声；可能出现双重人声"])
            } else {
                json!([])
            },
        };
        let saved = self
            .repository
            .save_plan(work.id, &manifest.input_version, input_snapshot, &plan)
            .await?;
        Ok(WorkPlanView {
            work,
            segments: saved
                .prompt_snapshot
                .get("segments")
                .cloned()
                .unwrap_or_else(|| json!([])),
            resource_usage: saved.resource_usage.clone(),
            can_confirm: true,
            blockers: Vec::new(),
            plan: saved,
        })
    }

    pub async fn confirm(
        &self,
        plan_id: Uuid,
        idempotency_key: String,
    ) -> Result<WorkRunView, WorkGenerationApplicationError> {
        if idempotency_key.trim().is_empty() {
            return Err(WorkGenerationApplicationError::Validation(
                "必须提供 Idempotency-Key".into(),
            ));
        }
        let plan = self.find_plan(plan_id).await?;
        if plan.status == "ready" {
            self.validate_plan_is_current(&plan).await?;
        }
        let snapshot = work_generation::WorkGenerationSnapshot {
            work_id: plan.work_id,
            work_version_id: plan.work_version_id,
            plan_id: plan.id,
            plan_version: plan.plan_version,
            model_snapshot: json!({"llm_model_id": plan.llm_model_id, "video_model_id": plan.video_model_id, "tts_model_id": plan.tts_model_id}),
            capability_snapshot: plan.capability_snapshot.clone(),
            voice_snapshot: plan
                .timeline_snapshot
                .get("voice_snapshot")
                .cloned()
                .unwrap_or_else(|| json!({})),
            prompt_snapshot: plan.prompt_snapshot.clone(),
            timeline_snapshot: plan.timeline_snapshot.clone(),
            parameter_snapshot: plan.output_snapshot.clone(),
        };
        let (run, created) = self
            .repository
            .confirm_run(
                plan_id,
                &idempotency_key,
                &snapshot,
                plan.resource_usage.clone(),
            )
            .await?;
        Ok(WorkRunView { run, created })
    }

    pub async fn list_tasks(
        &self,
        project_id: Uuid,
        filter: WorkGenerationTaskFilter,
    ) -> Result<WorkTaskListView, WorkGenerationApplicationError> {
        let include_hidden = filter.include_hidden;
        Ok(WorkTaskListView {
            tasks: self.repository.list_tasks(project_id, filter).await?,
            counts: self
                .repository
                .task_counts(project_id, include_hidden)
                .await?,
        })
    }

    pub async fn task_details(
        &self,
        run_id: Uuid,
    ) -> Result<WorkTaskDetailsView, WorkGenerationApplicationError> {
        let details = self.repository.task_details(run_id).await?;
        Ok(WorkTaskDetailsView {
            task: details.task,
            steps: details.steps,
        })
    }

    pub async fn cancel_run(
        &self,
        run_id: Uuid,
    ) -> Result<WorkTaskDetailsView, WorkGenerationApplicationError> {
        self.repository.cancel_run(run_id).await?;
        self.task_details(run_id).await
    }

    pub async fn dismiss_run(
        &self,
        run_id: Uuid,
    ) -> Result<WorkTaskDetailsView, WorkGenerationApplicationError> {
        self.repository.dismiss_run(run_id).await?;
        self.task_details(run_id).await
    }

    pub async fn retry_step(
        &self,
        step_id: Uuid,
        idempotency_key: String,
    ) -> Result<WorkGenerationAttemptRecord, WorkGenerationApplicationError> {
        if idempotency_key.trim().is_empty() {
            return Err(WorkGenerationApplicationError::Validation(
                "重试必须提供 Idempotency-Key".into(),
            ));
        }
        Ok(self
            .repository
            .retry_step(step_id, &idempotency_key)
            .await?)
    }

    async fn find_plan(
        &self,
        plan_id: Uuid,
    ) -> Result<WorkPlanRecord, WorkGenerationApplicationError> {
        let plan = sqlx::query("SELECT id, work_id, work_version_id, plan_version, status, input_fingerprint, llm_model_id, video_model_id, tts_model_id, capability_snapshot, output_snapshot, prompt_snapshot, timeline_snapshot, resource_usage, warnings FROM work_plans WHERE id=$1").bind(plan_id).fetch_optional(self.repository_pool()).await.map_err(WorkRepositoryError::from)?.ok_or_else(|| WorkRepositoryError::NotFound(plan_id.to_string()))?;
        Ok(WorkPlanRecord {
            id: plan.get("id"),
            work_id: plan.get("work_id"),
            work_version_id: plan.get("work_version_id"),
            plan_version: plan.get("plan_version"),
            status: plan.get("status"),
            input_fingerprint: plan.get("input_fingerprint"),
            llm_model_id: plan.get("llm_model_id"),
            video_model_id: plan.get("video_model_id"),
            tts_model_id: plan.get("tts_model_id"),
            capability_snapshot: plan.get("capability_snapshot"),
            output_snapshot: plan.get("output_snapshot"),
            prompt_snapshot: plan.get("prompt_snapshot"),
            timeline_snapshot: plan.get("timeline_snapshot"),
            resource_usage: plan.get("resource_usage"),
            warnings: plan.get("warnings"),
        })
    }

    async fn validate_plan_is_current(
        &self,
        plan: &WorkPlanRecord,
    ) -> Result<(), WorkGenerationApplicationError> {
        let source = sqlx::query("SELECT w.script_id, v.source_manifest_version FROM works w JOIN work_versions v ON v.work_id=w.id WHERE w.id=$1 AND v.id=$2")
            .bind(plan.work_id).bind(plan.work_version_id).fetch_one(self.repository_pool()).await.map_err(WorkRepositoryError::from)?;
        let manifest = self
            .asset_generation
            .scene_visual_manifest(source.get("script_id"))
            .await
            .map_err(|error| WorkGenerationApplicationError::Validation(error.to_string()))?;
        if manifest.input_version != source.get::<String, _>("source_manifest_version") {
            return Err(WorkGenerationApplicationError::Domain(
                WorkGenerationError::StalePlan,
            ));
        }
        let model_id = plan
            .video_model_id
            .ok_or_else(|| WorkGenerationApplicationError::Validation("计划缺少视频模型".into()))?;
        let runtime = self
            .ai_model_repository
            .resolve_enabled(model_id, ModelType::Video)
            .await
            .map_err(|error| WorkGenerationApplicationError::Validation(error.to_string()))?;
        if plan
            .capability_snapshot
            .get("version")
            .and_then(Value::as_str)
            != Some(runtime.snapshot.protocol_version.as_str())
        {
            return Err(WorkGenerationApplicationError::Domain(
                WorkGenerationError::StalePlan,
            ));
        }
        Ok(())
    }

    fn repository_pool(&self) -> &sqlx::PgPool {
        self.repository.pool()
    }
}

fn default_seedance_capability() -> VideoCapability {
    VideoCapability {
        version: "seedance-contract-v1".into(),
        reference_image_mode: ReferenceImageMode::MultiReference,
        min_duration_seconds: 4,
        max_duration_seconds: 15,
        max_reference_images: 9,
        max_prompt_chars: 500,
        aspect_ratios: vec!["16:9".into(), "9:16".into(), "1:1".into()],
        resolutions: vec!["720p".into(), "1080p".into()],
        audio_supported: true,
    }
}
fn capability_from_settings(
    version: &str,
    upstream_model: &str,
    settings: &Value,
) -> VideoCapability {
    let mut capability = default_seedance_capability();
    capability.version = version.to_string();
    if let Some(value) = settings.get("min_duration_seconds").and_then(Value::as_u64) {
        capability.min_duration_seconds = value as u32;
    }
    if let Some(value) = settings.get("max_duration_seconds").and_then(Value::as_u64) {
        capability.max_duration_seconds = value as u32;
    }
    if let Some(value) = settings.get("max_reference_images").and_then(Value::as_u64) {
        capability.max_reference_images = (value as usize).min(9);
    }
    if let Some(value) = settings.get("max_prompt_chars").and_then(Value::as_u64) {
        capability.max_prompt_chars = (value as usize).min(500);
    }
    if let Some(values) = settings.get("aspect_ratios").and_then(Value::as_array) {
        capability.aspect_ratios = values
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
    }
    if let Some(values) = settings.get("resolutions").and_then(Value::as_array) {
        capability.resolutions = values
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
    }
    if let Some(value) = settings
        .get("generate_audio")
        .or_else(|| settings.get("audio_supported"))
        .and_then(Value::as_bool)
    {
        capability.audio_supported = value;
    }
    if upstream_model.starts_with("doubao-seedance-1-5-") {
        capability.reference_image_mode = ReferenceImageMode::FirstLastFrames;
        capability.max_duration_seconds = capability.max_duration_seconds.min(12);
        capability.max_reference_images = capability.max_reference_images.min(2);
    }
    capability
}
fn fingerprint(value: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    format!("{:x}", hasher.finalize())
}

fn normalize_narration_override(value: Option<String>) -> Result<Option<String>, String> {
    match value {
        Some(value) if value.trim().is_empty() => Err("旁白覆盖不能为空".into()),
        Some(value) => Ok(Some(value.trim().to_string())),
        None => Ok(None),
    }
}

fn audio_mode_requires_tts(mode: AudioMode) -> bool {
    matches!(
        mode,
        AudioMode::IndependentTts | AudioMode::SeedanceOriginalAndTts
    )
}

fn contains_chinese(value: &str) -> bool {
    value
        .chars()
        .any(|character| ('\u{4e00}'..='\u{9fff}').contains(&character))
}

fn voice_languages_support_chinese(languages: &Value) -> bool {
    languages.as_array().is_some_and(|items| {
        items.iter().any(|item| {
            item.get("Language")
                .or_else(|| item.get("language"))
                .and_then(Value::as_str)
                .is_some_and(|language| language.to_ascii_lowercase().starts_with("zh"))
        })
    })
}

#[cfg(test)]
mod narration_override_tests {
    use super::*;

    #[test]
    fn narration_override_is_trimmed_and_cannot_be_blank() {
        assert_eq!(
            normalize_narration_override(Some(" 精简旁白 ".into())).unwrap(),
            Some("精简旁白".into())
        );
        assert!(normalize_narration_override(Some("  ".into())).is_err());
    }

    #[test]
    fn chinese_text_requires_a_declared_chinese_language() {
        assert!(contains_chinese("中文旁白"));
        assert!(voice_languages_support_chinese(
            &json!([{"Language":"zh-cn"}])
        ));
        assert!(!voice_languages_support_chinese(
            &json!([{"Language":"en"}])
        ));
    }

    #[test]
    fn silent_mode_never_requires_tts() {
        assert!(!audio_mode_requires_tts(AudioMode::Silent));
        assert!(audio_mode_requires_tts(AudioMode::IndependentTts));
    }

    #[test]
    fn seedance_15_capability_is_clamped_to_official_first_last_contract() {
        let capability = capability_from_settings(
            "v1",
            "doubao-seedance-1-5-pro-251215",
            &json!({
                "min_duration_seconds": 4,
                "max_duration_seconds": 15,
                "max_reference_images": 9,
                "max_prompt_chars": 500,
                "aspect_ratios": ["16:9"],
                "resolutions": ["1080p"],
                "generate_audio": true
            }),
        );

        assert_eq!(
            capability.reference_image_mode,
            ReferenceImageMode::FirstLastFrames
        );
        assert_eq!(capability.max_duration_seconds, 12);
        assert_eq!(capability.max_reference_images, 2);
        assert_eq!(capability.resolutions, vec!["1080p"]);
    }
}

#[derive(Debug)]
pub enum WorkGenerationApplicationError {
    Repository(WorkRepositoryError),
    Domain(WorkGenerationError),
    ManifestIncomplete {
        script_id: Uuid,
        blockers: Vec<SceneVisualManifestBlocker>,
    },
    Validation(String),
}
impl From<WorkRepositoryError> for WorkGenerationApplicationError {
    fn from(value: WorkRepositoryError) -> Self {
        Self::Repository(value)
    }
}
impl From<WorkGenerationError> for WorkGenerationApplicationError {
    fn from(value: WorkGenerationError) -> Self {
        Self::Domain(value)
    }
}
impl fmt::Display for WorkGenerationApplicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Repository(e) => write!(f, "{e}"),
            Self::Domain(e) => write!(f, "{e}"),
            Self::ManifestIncomplete { .. } => f.write_str("主画面清单不完整"),
            Self::Validation(e) => f.write_str(e),
        }
    }
}
impl std::error::Error for WorkGenerationApplicationError {}
