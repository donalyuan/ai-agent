use crate::domain::script::ScriptStatus;
use crate::repositories::{
    AiModelRepository, AiModelRepositoryError, AiModelStatus, AudioMaterialInspection,
    CreateSoundSubtitleTaskInput, MaterialRepository, MaterialRepositoryError, MaterialStatus,
    MaterialType, PostgresAiModelRepository, PostgresMaterialRepository, PostgresScriptRepository,
    PostgresSoundSubtitleRepository, PostgresTosStagingToolRepository,
    PostgresVoiceCatalogRepository, ScriptRepository, ScriptRepositoryError,
    SoundSubtitleRepositoryError, SoundSubtitleTask, TosStagingToolRepositoryError,
};
use chrono::{DateTime, Utc};
use novex_model::{ApiProtocol, ModelSettings, ModelType, SpeechModelSettings};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub struct SoundTaskIntent {
    pub task_type: String,
    pub model_id: Uuid,
    pub text_content: String,
    pub voice_type: Option<String>,
    pub language: Option<String>,
    pub emotion: Option<String>,
    pub parameters: Value,
    pub generate_subtitle: bool,
    pub subtitle_segments: Vec<String>,
    pub source_audio_material_id: Option<Uuid>,
    pub audio_inspection_id: Option<Uuid>,
    pub source_script_id: Option<Uuid>,
    pub source_script_updated_at: Option<DateTime<Utc>>,
    pub source_script_scene_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SoundTaskPreflight {
    pub task_type: String,
    pub model_id: Uuid,
    pub model_display_name: String,
    pub voice_snapshot: Option<Value>,
    pub resource_usage: Value,
    pub normalized_parameters: Value,
    pub source_script_snapshot: Option<Value>,
    pub confirmation_token: String,
    confirmation_snapshot: Value,
    model_snapshot: Value,
    tos_staging_config_id: Option<Uuid>,
    tos_staging_config_version: Option<i64>,
    normalized_intent: SoundTaskIntent,
}

#[derive(Clone)]
pub struct SoundSubtitleService {
    model_repository: PostgresAiModelRepository,
    material_repository: PostgresMaterialRepository,
    voice_catalog_repository: PostgresVoiceCatalogRepository,
    tos_staging_tool_repository: PostgresTosStagingToolRepository,
    script_repository: PostgresScriptRepository,
    repository: PostgresSoundSubtitleRepository,
}

impl SoundSubtitleService {
    pub fn new(
        model_repository: PostgresAiModelRepository,
        material_repository: PostgresMaterialRepository,
        voice_catalog_repository: PostgresVoiceCatalogRepository,
        tos_staging_tool_repository: PostgresTosStagingToolRepository,
        script_repository: PostgresScriptRepository,
        repository: PostgresSoundSubtitleRepository,
    ) -> Self {
        Self {
            model_repository,
            material_repository,
            voice_catalog_repository,
            tos_staging_tool_repository,
            script_repository,
            repository,
        }
    }

    pub async fn request_audio_inspection(
        &self,
        project_id: Uuid,
        material_id: Uuid,
        idempotency_key: String,
    ) -> Result<(AudioMaterialInspection, bool), SoundSubtitleApplicationError> {
        validate_idempotency_key(&idempotency_key)?;
        let material = self.material_repository.get_material(material_id).await?;
        if material.project_id != project_id {
            return Err(validation(
                "material_project_mismatch",
                "音频素材不属于当前项目",
            ));
        }
        if material.material_type != MaterialType::Audio
            || material.status != MaterialStatus::Active
        {
            return Err(validation(
                "audio_material_unavailable",
                "仅可检查当前项目中的可用音频素材",
            ));
        }
        if !is_managed_asset_url(&material.file_url) {
            return Err(validation(
                "audio_storage_unsupported",
                "音频检查只支持自管素材存储中的文件",
            ));
        }
        self.repository
            .request_audio_inspection(project_id, material_id, idempotency_key.trim())
            .await
            .map_err(Into::into)
    }

    pub async fn get_audio_inspection(
        &self,
        project_id: Uuid,
        material_id: Uuid,
    ) -> Result<AudioMaterialInspection, SoundSubtitleApplicationError> {
        let inspection = self
            .repository
            .latest_audio_inspection(material_id)
            .await?
            .ok_or_else(|| validation("inspection_not_found", "尚未检查该音频素材"))?;
        if inspection.project_id != project_id {
            return Err(validation(
                "material_project_mismatch",
                "音频素材不属于当前项目",
            ));
        }
        Ok(inspection)
    }

    pub async fn preflight(
        &self,
        project_id: Uuid,
        intent: SoundTaskIntent,
    ) -> Result<SoundTaskPreflight, SoundSubtitleApplicationError> {
        match intent.task_type.trim() {
            "tts" | "tts_preview" => self.preflight_tts(project_id, intent).await,
            "asr" => self.preflight_asr(project_id, intent).await,
            _ => Err(validation(
                "task_type_unsupported",
                "仅支持 TTS、主动试听和已有音频 ASR 任务",
            )),
        }
    }

    pub async fn create_task(
        &self,
        project_id: Uuid,
        intent: SoundTaskIntent,
        confirmation_token: String,
        idempotency_key: String,
    ) -> Result<(SoundSubtitleTask, bool), SoundSubtitleApplicationError> {
        validate_idempotency_key(&idempotency_key)?;
        if !is_sha256(&confirmation_token) {
            return Err(validation(
                "confirmation_invalid",
                "确认摘要必须是 64 位 SHA-256",
            ));
        }
        let preflight = self.preflight(project_id, intent).await?;
        if preflight.confirmation_token != confirmation_token.to_ascii_lowercase() {
            return Err(validation(
                "confirmation_stale",
                "模型、音色、文本、参数或资源用量已变化，请重新确认",
            ));
        }
        let intent = preflight.normalized_intent;
        self.repository
            .create_or_reuse_task(CreateSoundSubtitleTaskInput {
                project_id,
                parent_task_id: None,
                task_type: intent.task_type,
                model_id: intent.model_id,
                tos_staging_config_id: preflight.tos_staging_config_id,
                tos_staging_config_version: preflight.tos_staging_config_version,
                audio_inspection_id: intent.audio_inspection_id,
                source_audio_material_id: intent.source_audio_material_id,
                source_script_id: intent.source_script_id,
                source_script_snapshot: preflight.source_script_snapshot,
                text_content: intent.text_content,
                voice_type: intent.voice_type,
                language: intent.language,
                emotion: intent.emotion,
                parameters: preflight.normalized_parameters,
                model_snapshot: preflight.model_snapshot,
                voice_snapshot: preflight.voice_snapshot,
                confirmation_snapshot: preflight.confirmation_snapshot,
                resource_usage: preflight.resource_usage,
                idempotency_key: idempotency_key.trim().to_string(),
            })
            .await
            .map_err(Into::into)
    }

    pub async fn get_task(
        &self,
        project_id: Uuid,
        task_id: Uuid,
    ) -> Result<SoundSubtitleTask, SoundSubtitleApplicationError> {
        self.repository
            .get_task(project_id, task_id)
            .await
            .map_err(Into::into)
    }

    pub async fn list_tasks(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<SoundSubtitleTask>, SoundSubtitleApplicationError> {
        self.repository
            .list_tasks(project_id)
            .await
            .map_err(Into::into)
    }

    pub async fn retry_task(
        &self,
        project_id: Uuid,
        failed_task_id: Uuid,
        intent: SoundTaskIntent,
        confirmation_token: String,
        idempotency_key: String,
    ) -> Result<(SoundSubtitleTask, bool), SoundSubtitleApplicationError> {
        validate_idempotency_key(&idempotency_key)?;
        let failed = self.repository.get_task(project_id, failed_task_id).await?;
        if failed.status != "failed" {
            return Err(validation("retry_not_allowed", "只有失败节点可以人工重试"));
        }
        let preflight = self.preflight(project_id, intent).await?;
        if preflight.confirmation_token != confirmation_token.to_ascii_lowercase() {
            return Err(validation(
                "confirmation_stale",
                "重试资源或模型配置已变化，请重新确认",
            ));
        }
        let intent = preflight.normalized_intent;
        let same_failed_node = failed.task_type == intent.task_type
            && failed.model_id == intent.model_id
            && failed.tos_staging_config_id == preflight.tos_staging_config_id
            && failed.tos_staging_config_version == preflight.tos_staging_config_version
            && failed.text_content == intent.text_content
            && failed.voice_type == intent.voice_type
            && failed.language == intent.language
            && failed.emotion == intent.emotion
            && failed.parameters == preflight.normalized_parameters
            && failed.source_audio_material_id == intent.source_audio_material_id;
        if !same_failed_node {
            return Err(validation(
                "retry_input_changed",
                "重试只能重新执行原失败节点；修改输入请创建新的生成任务",
            ));
        }
        self.repository
            .create_or_reuse_task(CreateSoundSubtitleTaskInput {
                project_id,
                parent_task_id: Some(failed_task_id),
                task_type: intent.task_type,
                model_id: intent.model_id,
                tos_staging_config_id: preflight.tos_staging_config_id,
                tos_staging_config_version: preflight.tos_staging_config_version,
                audio_inspection_id: intent.audio_inspection_id,
                source_audio_material_id: intent.source_audio_material_id,
                source_script_id: failed.source_script_id,
                source_script_snapshot: failed.source_script_snapshot,
                text_content: intent.text_content,
                voice_type: intent.voice_type,
                language: intent.language,
                emotion: intent.emotion,
                parameters: preflight.normalized_parameters,
                model_snapshot: preflight.model_snapshot,
                voice_snapshot: preflight.voice_snapshot,
                confirmation_snapshot: preflight.confirmation_snapshot,
                resource_usage: preflight.resource_usage,
                idempotency_key: idempotency_key.trim().to_string(),
            })
            .await
            .map_err(Into::into)
    }

    pub async fn cancel_task(
        &self,
        project_id: Uuid,
        task_id: Uuid,
    ) -> Result<SoundSubtitleTask, SoundSubtitleApplicationError> {
        self.repository
            .cancel_task(project_id, task_id)
            .await
            .map_err(Into::into)
    }

    async fn preflight_tts(
        &self,
        project_id: Uuid,
        mut intent: SoundTaskIntent,
    ) -> Result<SoundTaskPreflight, SoundSubtitleApplicationError> {
        ensure_project_id(project_id)?;
        let runtime = self
            .model_repository
            .resolve_enabled(intent.model_id, ModelType::Speech)
            .await?;
        let model = self.model_repository.get(intent.model_id).await?;
        if !matches!(
            runtime.snapshot.api_protocol,
            ApiProtocol::VolcengineTtsV3 | ApiProtocol::OpenAiAudioSpeech
        ) {
            return Err(validation(
                "model_protocol_mismatch",
                "所选模型不是 TTS 模型",
            ));
        }
        let settings = speech_settings(&runtime.snapshot.settings)?;
        let source_script_snapshot = self.validate_script_source(project_id, &intent).await?;
        let text = intent.text_content.trim().to_string();
        let character_count = text.chars().count();
        if character_count == 0 {
            return Err(validation("text_empty", "TTS 文本不能为空"));
        }
        if settings
            .max_input_characters
            .is_some_and(|maximum| character_count > maximum as usize)
        {
            return Err(validation("text_too_long", "TTS 文本超过模型字符数上限"));
        }
        if intent.task_type == "tts_preview" && intent.generate_subtitle {
            return Err(validation(
                "preview_subtitle_unsupported",
                "试听不会创建字幕任务",
            ));
        }
        let voice_type = required_trimmed(
            intent.voice_type.as_deref(),
            "voice_required",
            "必须选择音色",
        )?;
        let language = required_trimmed(
            intent.language.as_deref(),
            "language_required",
            "必须选择语言或口音",
        )?;
        if intent
            .emotion
            .as_deref()
            .is_some_and(|emotion| !emotion.trim().is_empty())
        {
            return Err(validation(
                "emotion_unsupported",
                "当前 TTS 协议不支持结构化情绪风格",
            ));
        }
        let catalog = self
            .voice_catalog_repository
            .catalog(intent.model_id, true)
            .await?;
        let voice = catalog
            .voices
            .iter()
            .find(|voice| voice.voice_type == voice_type)
            .ok_or_else(|| validation("voice_not_found", "所选音色不在当前模型目录中"))?;
        if !voice.is_available {
            return Err(validation("voice_unavailable", "所选音色已不可用于新生成"));
        }
        if voice.resource_id != settings.resource_id {
            return Err(validation(
                "voice_resource_mismatch",
                "音色与模型资源版本不匹配",
            ));
        }
        if !catalog_value_contains(
            &voice.languages,
            &language,
            &["Language", "language", "Value", "value"],
        ) {
            return Err(validation(
                "voice_language_unsupported",
                "所选音色不支持该语言或口音",
            ));
        }
        if intent.generate_subtitle
            && (!settings.supports_word_timestamps
                || !language_supported(&settings.word_timestamp_languages, &language))
        {
            return Err(validation(
                "timestamps_unsupported",
                "当前语言不支持可信字词时间戳，不能生成同步字幕",
            ));
        }
        let normalized_parameters = normalize_speech_parameters(&intent.parameters, &settings)?;
        let subtitle_segments = normalize_subtitle_segments(
            &intent.subtitle_segments,
            &text,
            intent.generate_subtitle,
        )?;
        let voice_snapshot = json!({
            "voice_type": voice.voice_type,
            "name": voice.name,
            "resource_id": voice.resource_id,
            "catalog_source_model_id": catalog.source_model_id,
            "language": language,
            "emotion": null,
            "catalog_version": voice.catalog_version,
        });
        let resource_usage = json!({
            "character_count": character_count,
            "task_count": if intent.generate_subtitle { 2 } else { 1 },
            "output_count": if intent.generate_subtitle { 2 } else { 1 },
        });
        intent.text_content = text;
        intent.voice_type = Some(voice_type);
        intent.language = Some(language);
        intent.emotion = None;
        intent.parameters = normalized_parameters.clone();
        intent.subtitle_segments = subtitle_segments;
        intent.source_audio_material_id = None;
        intent.audio_inspection_id = None;
        let model_snapshot = model_snapshot_with_version(&runtime.snapshot, model.version)?;
        build_preflight(
            intent,
            runtime.snapshot.display_name,
            model_snapshot,
            Some(voice_snapshot),
            resource_usage,
            normalized_parameters,
            None,
            None,
            source_script_snapshot,
        )
    }

    async fn preflight_asr(
        &self,
        project_id: Uuid,
        mut intent: SoundTaskIntent,
    ) -> Result<SoundTaskPreflight, SoundSubtitleApplicationError> {
        ensure_project_id(project_id)?;
        let runtime = self
            .model_repository
            .resolve_enabled(intent.model_id, ModelType::Speech)
            .await?;
        if runtime.snapshot.api_protocol != ApiProtocol::VolcengineAsrV3 {
            return Err(validation(
                "model_protocol_mismatch",
                "所选模型不是 ASR 模型",
            ));
        }
        let model = self.model_repository.get(intent.model_id).await?;
        if model.status != AiModelStatus::Enabled || model.deleted_at.is_some() {
            return Err(validation("model_unavailable", "所选 ASR 模型已停用或删除"));
        }
        let settings = speech_settings(&runtime.snapshot.settings)?;
        let material_id = intent
            .source_audio_material_id
            .ok_or_else(|| validation("source_audio_required", "ASR 必须选择已有音频素材"))?;
        let inspection_id = intent
            .audio_inspection_id
            .ok_or_else(|| validation("inspection_required", "ASR 创建前必须完成真实音频检查"))?;
        let material = self.material_repository.get_material(material_id).await?;
        if material.project_id != project_id
            || material.material_type != MaterialType::Audio
            || material.status != MaterialStatus::Active
        {
            return Err(validation(
                "audio_material_unavailable",
                "ASR 来源必须是当前项目中的可用音频素材",
            ));
        }
        if !is_managed_asset_url(&material.file_url) {
            return Err(validation(
                "audio_storage_unsupported",
                "ASR 只支持自管素材存储中的音频",
            ));
        }
        let inspection = self.repository.get_audio_inspection(inspection_id).await?;
        let latest = self
            .repository
            .latest_audio_inspection(material_id)
            .await?
            .ok_or_else(|| validation("inspection_not_found", "尚未检查该音频素材"))?;
        if inspection.project_id != project_id || inspection.material_id != material_id {
            return Err(validation(
                "inspection_mismatch",
                "音频检查与来源素材不匹配",
            ));
        }
        if inspection.id != latest.id || inspection.status != "succeeded" {
            return Err(validation(
                "inspection_stale",
                "音频检查未成功或已被更新，请重新检查并确认",
            ));
        }
        let duration_ms = inspection.duration_ms.ok_or_else(|| {
            SoundSubtitleApplicationError::Internal("成功检查缺少 duration_ms".to_string())
        })?;
        let file_size_bytes = inspection.file_size_bytes.ok_or_else(|| {
            SoundSubtitleApplicationError::Internal("成功检查缺少 file_size_bytes".to_string())
        })?;
        let staging = match self.tos_staging_tool_repository.get_enabled_current().await {
            Ok(config) => config,
            Err(TosStagingToolRepositoryError::NotConfigured) => {
                return Err(validation(
                    "tos_staging_not_configured",
                    "系统私有 TOS 工具尚未配置",
                ))
            }
            Err(TosStagingToolRepositoryError::Disabled) => {
                return Err(validation(
                    "tos_staging_disabled",
                    "系统私有 TOS 工具未启用",
                ))
            }
            Err(error) => return Err(SoundSubtitleApplicationError::TosStagingTool(error)),
        };
        let model_duration_ms = settings
            .max_audio_duration_seconds
            .map(i64::from)
            .unwrap_or(i64::MAX)
            * 1000;
        let staging_duration_ms = i64::from(staging.max_audio_duration_seconds) * 1000;
        if duration_ms > model_duration_ms.min(staging_duration_ms) {
            return Err(validation(
                "audio_too_long",
                "音频实际时长超过 ASR 模型上限",
            ));
        }
        if file_size_bytes > staging.max_file_bytes {
            return Err(validation(
                "audio_too_large",
                "音频实际大小超过 TOS 暂存上限",
            ));
        }
        let normalized_parameters = normalize_speech_parameters(&intent.parameters, &settings)?;
        let resource_usage = json!({
            "audio_duration_ms": duration_ms,
            "source_file_size_bytes": file_size_bytes,
            "task_count": 1,
            "output_count": 1,
        });
        intent.text_content.clear();
        intent.voice_type = None;
        intent.language = None;
        intent.emotion = None;
        intent.parameters = normalized_parameters.clone();
        intent.generate_subtitle = true;
        intent.subtitle_segments.clear();
        intent.source_script_id = None;
        intent.source_script_updated_at = None;
        intent.source_script_scene_ids.clear();
        let model_snapshot = model_snapshot_with_version(&runtime.snapshot, model.version)?;
        build_preflight(
            intent,
            runtime.snapshot.display_name,
            model_snapshot,
            None,
            resource_usage,
            normalized_parameters,
            Some(staging.id),
            Some(staging.version),
            None,
        )
    }

    /// 服务端重新读取来源脚本并构造不可变快照，客户端只提供版本和分镜引用。
    async fn validate_script_source(
        &self,
        project_id: Uuid,
        intent: &SoundTaskIntent,
    ) -> Result<Option<Value>, SoundSubtitleApplicationError> {
        let Some(script_id) = intent.source_script_id else {
            if intent.source_script_updated_at.is_some()
                || !intent.source_script_scene_ids.is_empty()
            {
                return Err(validation(
                    "source_script_reference_invalid",
                    "脚本来源引用不完整，请重新导入",
                ));
            }
            return Ok(None);
        };
        let expected_updated_at = intent.source_script_updated_at.as_ref().ok_or_else(|| {
            validation(
                "source_script_reference_invalid",
                "脚本来源缺少版本信息，请重新导入",
            )
        })?;
        if intent.source_script_scene_ids.is_empty() {
            return Err(validation(
                "source_scene_invalid",
                "至少选择一个非空旁白分镜",
            ));
        }
        let mut requested_scene_ids = intent.source_script_scene_ids.clone();
        requested_scene_ids.sort_unstable();
        if requested_scene_ids.windows(2).any(|ids| ids[0] == ids[1]) {
            return Err(validation("source_scene_invalid", "所选脚本分镜不能重复"));
        }
        let script = match self.script_repository.get_script(script_id).await {
            Ok(script) => script,
            Err(ScriptRepositoryError::NotFound(_)) => {
                return Err(validation(
                    "source_script_unavailable",
                    "来源脚本不存在或已不可用，请重新导入",
                ))
            }
            Err(error) => return Err(SoundSubtitleApplicationError::Script(error)),
        };
        if script.project_id != project_id {
            return Err(validation(
                "source_script_project_mismatch",
                "来源脚本不属于当前项目",
            ));
        }
        if !matches!(&script.status, ScriptStatus::Draft | ScriptStatus::Approved) {
            return Err(validation(
                "source_script_unavailable",
                "来源脚本已归档，请重新导入",
            ));
        }
        if &script.updated_at != expected_updated_at {
            return Err(validation(
                "source_script_changed",
                "来源脚本已更新，请重新导入后再确认",
            ));
        }
        let selected_scenes = script
            .scenes
            .iter()
            .filter(|scene| requested_scene_ids.binary_search(&scene.id).is_ok())
            .collect::<Vec<_>>();
        if selected_scenes.len() != requested_scene_ids.len()
            || selected_scenes
                .iter()
                .any(|scene| scene.narration.trim().is_empty())
        {
            return Err(validation(
                "source_scene_invalid",
                "所选分镜不存在、旁白为空或不属于来源脚本",
            ));
        }
        let scene_snapshot = selected_scenes
            .into_iter()
            .map(|scene| {
                json!({
                    "scene_id": scene.id,
                    "sequence": scene.sequence,
                    "narration": scene.narration,
                })
            })
            .collect::<Vec<_>>();
        Ok(Some(json!({
            "script_id": script.id,
            "title": script.title,
            "updated_at": script.updated_at,
            "scenes": scene_snapshot,
        })))
    }
}

#[allow(clippy::too_many_arguments)]
fn build_preflight(
    intent: SoundTaskIntent,
    model_display_name: String,
    model_snapshot: Value,
    voice_snapshot: Option<Value>,
    resource_usage: Value,
    normalized_parameters: Value,
    tos_staging_config_id: Option<Uuid>,
    tos_staging_config_version: Option<i64>,
    source_script_snapshot: Option<Value>,
) -> Result<SoundTaskPreflight, SoundSubtitleApplicationError> {
    let confirmation_payload = json!({
        "task_type": intent.task_type,
        "model_id": intent.model_id,
        "text_content": intent.text_content,
        "voice_type": intent.voice_type,
        "language": intent.language,
        "emotion": intent.emotion,
        "parameters": normalized_parameters,
        "generate_subtitle": intent.generate_subtitle,
        "subtitle_segments": intent.subtitle_segments,
        "source_audio_material_id": intent.source_audio_material_id,
        "audio_inspection_id": intent.audio_inspection_id,
        "source_script_id": intent.source_script_id,
        "source_script_snapshot": source_script_snapshot,
        "model_snapshot": model_snapshot,
        "voice_snapshot": voice_snapshot,
        "resource_usage": resource_usage,
        "tos_staging_config_id": tos_staging_config_id,
        "tos_staging_config_version": tos_staging_config_version,
    });
    let bytes = serde_json::to_vec(&confirmation_payload)
        .map_err(|error| SoundSubtitleApplicationError::Internal(error.to_string()))?;
    let confirmation_token = format!("{:x}", Sha256::digest(bytes));
    let mut confirmation_snapshot = confirmation_payload;
    confirmation_snapshot["confirmation_token"] = json!(confirmation_token);
    Ok(SoundTaskPreflight {
        task_type: intent.task_type.clone(),
        model_id: intent.model_id,
        model_display_name,
        voice_snapshot,
        resource_usage,
        normalized_parameters,
        source_script_snapshot,
        confirmation_token,
        confirmation_snapshot,
        model_snapshot,
        tos_staging_config_id,
        tos_staging_config_version,
        normalized_intent: intent,
    })
}

fn model_snapshot_with_version(
    snapshot: &novex_model::ModelExecutionSnapshot,
    registry_version: i64,
) -> Result<Value, SoundSubtitleApplicationError> {
    let mut value = serde_json::to_value(snapshot)
        .map_err(|error| SoundSubtitleApplicationError::Internal(error.to_string()))?;
    value["registry_version"] = json!(registry_version);
    Ok(value)
}

fn speech_settings(value: &Value) -> Result<SpeechModelSettings, SoundSubtitleApplicationError> {
    match ModelSettings::parse(ModelType::Speech, value.clone())
        .map_err(|error| validation("model_config_invalid", error.to_string()))?
    {
        ModelSettings::Speech(settings) => Ok(settings),
        _ => unreachable!(),
    }
}

fn normalize_speech_parameters(
    value: &Value,
    settings: &SpeechModelSettings,
) -> Result<Value, SoundSubtitleApplicationError> {
    let input = value
        .as_object()
        .ok_or_else(|| validation("parameters_invalid", "语音参数必须是 JSON object"))?;
    let definitions = settings
        .parameters
        .as_object()
        .ok_or_else(|| validation("model_config_invalid", "模型参数能力配置必须是 JSON object"))?;
    let mut output = Map::new();

    let audio_format = input
        .get("audio_format")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
        .or_else(|| settings.default_audio_format.clone())
        .ok_or_else(|| validation("audio_format_required", "必须选择音频格式"))?;
    if !settings
        .supported_audio_formats
        .iter()
        .any(|item| item.eq_ignore_ascii_case(&audio_format))
    {
        return Err(validation(
            "audio_format_unsupported",
            "模型不支持所选音频格式",
        ));
    }
    output.insert("audio_format".to_string(), json!(audio_format));

    if let Some(sample_rate) = input
        .get("sample_rate")
        .and_then(Value::as_u64)
        .or(settings.default_sample_rate.map(u64::from))
    {
        if !settings
            .supported_sample_rates
            .contains(&(sample_rate as u32))
        {
            return Err(validation(
                "sample_rate_unsupported",
                "模型不支持所选采样率",
            ));
        }
        output.insert("sample_rate".to_string(), json!(sample_rate));
    }

    for (name, parameter) in input {
        if matches!(name.as_str(), "audio_format" | "sample_rate") {
            continue;
        }
        let definition = definitions
            .get(name)
            .and_then(Value::as_object)
            .ok_or_else(|| validation("parameter_unsupported", format!("模型不支持参数 {name}")))?;
        validate_parameter(name, parameter, definition)?;
        output.insert(name.clone(), parameter.clone());
    }
    Ok(Value::Object(output))
}

fn validate_parameter(
    name: &str,
    value: &Value,
    definition: &Map<String, Value>,
) -> Result<(), SoundSubtitleApplicationError> {
    if definition.get("type").and_then(Value::as_str) == Some("number") {
        let number = value
            .as_f64()
            .ok_or_else(|| validation("parameter_invalid", format!("参数 {name} 必须是数字")))?;
        let minimum = definition
            .get("minimum")
            .or_else(|| definition.get("min"))
            .and_then(Value::as_f64);
        let maximum = definition
            .get("maximum")
            .or_else(|| definition.get("max"))
            .and_then(Value::as_f64);
        if minimum.is_some_and(|bound| number < bound)
            || maximum.is_some_and(|bound| number > bound)
        {
            return Err(validation(
                "parameter_out_of_range",
                format!("参数 {name} 超出模型允许范围"),
            ));
        }
    }
    if let Some(options) = definition.get("enum").and_then(Value::as_array) {
        if !options.contains(value) {
            return Err(validation(
                "parameter_unsupported",
                format!("参数 {name} 的值不受模型支持"),
            ));
        }
    }
    Ok(())
}

fn normalize_subtitle_segments(
    values: &[String],
    text: &str,
    required: bool,
) -> Result<Vec<String>, SoundSubtitleApplicationError> {
    if !required {
        return Ok(Vec::new());
    }
    let segments = values
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        return Err(validation(
            "subtitle_segments_required",
            "生成字幕前必须确认字幕断句",
        ));
    }
    let normalized_text = normalize_alignment_text(text);
    let normalized_segments = normalize_alignment_text(&segments.join(""));
    if normalized_text != normalized_segments {
        return Err(validation(
            "subtitle_segments_mismatch",
            "字幕断句文本必须与 TTS 文本完全一致",
        ));
    }
    Ok(segments)
}

fn normalize_alignment_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn catalog_value_contains(value: &Value, expected: &str, keys: &[&str]) -> bool {
    value.as_array().is_some_and(|items| {
        items.iter().any(|item| {
            item.as_str()
                .is_some_and(|value| value.eq_ignore_ascii_case(expected))
                || item.as_object().is_some_and(|object| {
                    keys.iter().any(|key| {
                        object
                            .get(*key)
                            .and_then(Value::as_str)
                            .is_some_and(|value| value.eq_ignore_ascii_case(expected))
                    })
                })
        })
    })
}

fn language_supported(languages: &[String], language: &str) -> bool {
    languages
        .iter()
        .any(|item| item == "*" || item.eq_ignore_ascii_case(language))
}

fn required_trimmed(
    value: Option<&str>,
    code: &'static str,
    message: &'static str,
) -> Result<String, SoundSubtitleApplicationError> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| validation(code, message))
}

fn ensure_project_id(project_id: Uuid) -> Result<(), SoundSubtitleApplicationError> {
    if project_id.is_nil() {
        Err(validation("project_required", "项目不能为空"))
    } else {
        Ok(())
    }
}

fn validate_idempotency_key(value: &str) -> Result<(), SoundSubtitleApplicationError> {
    let length = value.trim().len();
    if !(1..=200).contains(&length) {
        Err(validation(
            "idempotency_key_invalid",
            "Idempotency-Key 长度必须为 1-200",
        ))
    } else {
        Ok(())
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_managed_asset_url(value: &str) -> bool {
    value.starts_with("/assets/") && !value.split('/').any(|segment| segment == "..")
}

fn validation(
    code: impl Into<String>,
    message: impl Into<String>,
) -> SoundSubtitleApplicationError {
    SoundSubtitleApplicationError::Validation {
        code: code.into(),
        message: message.into(),
    }
}

#[derive(Debug)]
pub enum SoundSubtitleApplicationError {
    Model(AiModelRepositoryError),
    Material(MaterialRepositoryError),
    Repository(SoundSubtitleRepositoryError),
    VoiceCatalog(crate::repositories::VoiceCatalogRepositoryError),
    TosStagingTool(TosStagingToolRepositoryError),
    Script(ScriptRepositoryError),
    Validation { code: String, message: String },
    Internal(String),
}

impl From<AiModelRepositoryError> for SoundSubtitleApplicationError {
    fn from(error: AiModelRepositoryError) -> Self {
        Self::Model(error)
    }
}

impl From<MaterialRepositoryError> for SoundSubtitleApplicationError {
    fn from(error: MaterialRepositoryError) -> Self {
        Self::Material(error)
    }
}

impl From<SoundSubtitleRepositoryError> for SoundSubtitleApplicationError {
    fn from(error: SoundSubtitleRepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl From<crate::repositories::VoiceCatalogRepositoryError> for SoundSubtitleApplicationError {
    fn from(error: crate::repositories::VoiceCatalogRepositoryError) -> Self {
        Self::VoiceCatalog(error)
    }
}

impl From<TosStagingToolRepositoryError> for SoundSubtitleApplicationError {
    fn from(error: TosStagingToolRepositoryError) -> Self {
        Self::TosStagingTool(error)
    }
}

impl From<ScriptRepositoryError> for SoundSubtitleApplicationError {
    fn from(error: ScriptRepositoryError) -> Self {
        Self::Script(error)
    }
}

impl fmt::Display for SoundSubtitleApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(error) => write!(formatter, "{error}"),
            Self::Material(error) => write!(formatter, "{error}"),
            Self::Repository(error) => write!(formatter, "{error}"),
            Self::VoiceCatalog(error) => write!(formatter, "{error}"),
            Self::TosStagingTool(error) => write!(formatter, "{error}"),
            Self::Script(error) => write!(formatter, "{error}"),
            Self::Validation { message, .. } => formatter.write_str(message),
            Self::Internal(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for SoundSubtitleApplicationError {}
