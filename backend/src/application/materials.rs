//! 素材 CRUD 用例，统一处理项目边界与素材归属。

use super::material_upload::{
    inspect_upload, probe_media, LocalMaterialStorage, UploadValidationError,
};
use crate::repositories::{
    validate_material_metadata, AudioUsage, CreateMaterialInput, Material, MaterialListFilter,
    MaterialRepository, MaterialRepositoryError, MaterialStatus, MaterialType,
    PostgresMaterialRepository, PostgresProjectRepository, ProjectRepository,
    ProjectRepositoryError, UpdateMaterialInput,
};
use serde_json::{json, Map, Value};
use std::fmt;
use std::path::Path;
use uuid::Uuid;

#[derive(Clone)]
/// 在项目边界内执行素材创建、查询、更新和状态变更。
pub struct MaterialService {
    project_repository: PostgresProjectRepository,
    material_repository: PostgresMaterialRepository,
    storage: LocalMaterialStorage,
}

impl MaterialService {
    pub fn new(
        project_repository: PostgresProjectRepository,
        material_repository: PostgresMaterialRepository,
        asset_storage_root: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            project_repository,
            material_repository,
            storage: LocalMaterialStorage::new(asset_storage_root),
        }
    }

    pub async fn create(
        &self,
        input: CreateMaterialInput,
    ) -> Result<Material, MaterialApplicationError> {
        self.ensure_project_exists(input.project_id).await?;
        if input.metadata.get("source").and_then(Value::as_str) == Some("work_generation") {
            return Err(MaterialApplicationError::Validation(
                "作品生成素材必须通过统一生成物登记接口写入".to_string(),
            ));
        }
        self.material_repository
            .create_material(input)
            .await
            .map_err(Into::into)
    }

    pub async fn list(
        &self,
        project_id: Uuid,
        filter: MaterialListFilter,
    ) -> Result<Vec<Material>, MaterialApplicationError> {
        self.ensure_project_exists(project_id).await?;
        self.material_repository
            .list_materials(project_id, filter)
            .await
            .map_err(Into::into)
    }

    pub async fn upload(
        &self,
        command: MaterialUploadCommand,
    ) -> Result<Material, MaterialApplicationError> {
        self.ensure_project_exists(command.project_id).await?;
        let detected = inspect_upload(
            &command.original_file_name,
            command.content_type.as_deref(),
            &command.bytes,
        )?;
        if detected.material_type != MaterialType::Audio && command.audio_usage.is_some() {
            return Err(MaterialApplicationError::Validation(
                "audio_usage 只能用于音频素材".to_string(),
            ));
        }
        let upload_id = Uuid::new_v4();
        let stored = self
            .storage
            .store(
                command.project_id,
                upload_id,
                &detected.extension,
                &command.bytes,
            )
            .await
            .map_err(|error| MaterialApplicationError::UploadStorage(error.to_string()))?;

        let probe = if matches!(
            detected.material_type,
            MaterialType::Video | MaterialType::Audio
        ) {
            match probe_media(&stored.absolute_path).await {
                Ok(probe) => Some(probe),
                Err(error) => {
                    let _ = self.storage.remove(&stored).await;
                    return Err(error.into());
                }
            }
        } else {
            None
        };
        let thumbnail = if detected.material_type == MaterialType::Video {
            let bytes = match generate_video_thumbnail(&stored.absolute_path).await {
                Ok(bytes) => bytes,
                Err(error) => {
                    let _ = self.storage.remove(&stored).await;
                    return Err(error);
                }
            };
            match self
                .storage
                .store_upload_thumbnail(command.project_id, upload_id, &bytes)
                .await
            {
                Ok(thumbnail) => Some(thumbnail),
                Err(error) => {
                    let _ = self.storage.remove(&stored).await;
                    return Err(MaterialApplicationError::UploadStorage(error.to_string()));
                }
            }
        } else {
            None
        };

        let mut metadata = Map::from_iter([
            ("source".to_string(), json!("user_upload")),
            ("storage_provider".to_string(), json!("local")),
            ("mime_type".to_string(), json!(detected.mime_type)),
            ("format".to_string(), json!(detected.format)),
            ("file_size_bytes".to_string(), json!(command.bytes.len())),
        ]);
        if let Some(width) = detected
            .width
            .or_else(|| probe.as_ref().and_then(|value| value.width))
        {
            metadata.insert("width".to_string(), json!(width));
        }
        if let Some(height) = detected
            .height
            .or_else(|| probe.as_ref().and_then(|value| value.height))
        {
            metadata.insert("height".to_string(), json!(height));
        }
        if let Some(duration_sec) = probe.as_ref().and_then(|value| value.duration_sec) {
            metadata.insert("duration_sec".to_string(), json!(duration_sec));
        }
        if detected.material_type == MaterialType::Subtitle {
            metadata.insert("subtitle_format".to_string(), json!(detected.extension));
        }
        match (detected.material_type, command.audio_usage) {
            (MaterialType::Audio, Some(audio_usage)) => {
                metadata.insert("audio_usage".to_string(), json!(audio_usage.as_str()));
            }
            (MaterialType::Audio, None) | (_, None) => {}
            (_, Some(_)) => unreachable!("非音频用途已在落盘前校验"),
        }

        let input = CreateMaterialInput {
            project_id: command.project_id,
            material_type: detected.material_type,
            file_url: stored.public_url.clone(),
            file_name: command.file_name,
            thumbnail_url: thumbnail.as_ref().map(|value| value.public_url.clone()),
            tags: command.tags,
            metadata: Value::Object(metadata),
        };
        match self.material_repository.create_material(input).await {
            Ok(material) => Ok(material),
            Err(error) => {
                let _ = self.storage.remove(&stored).await;
                if let Some(thumbnail) = thumbnail.as_ref() {
                    let _ = self.storage.remove(thumbnail).await;
                }
                Err(error.into())
            }
        }
    }

    pub async fn register_generated(
        &self,
        command: GeneratedMaterialCommand,
    ) -> Result<Material, MaterialApplicationError> {
        self.ensure_project_exists(command.project_id).await?;
        validate_generation_snapshot(&command.generation)?;
        let detected = inspect_upload(
            &command.original_file_name,
            command.content_type.as_deref(),
            &command.bytes,
        )?;
        if detected.material_type == MaterialType::Image {
            return Err(MaterialApplicationError::Validation(
                "作品生成物登记仅支持音频、字幕和视频".to_string(),
            ));
        }
        match (detected.material_type, command.generation.audio_usage) {
            (MaterialType::Audio, None) => {
                return Err(MaterialApplicationError::Validation(
                    "音频生成物必须指定 audio_usage".to_string(),
                ))
            }
            (MaterialType::Audio, Some(_)) | (_, None) => {}
            (_, Some(_)) => {
                return Err(MaterialApplicationError::Validation(
                    "audio_usage 只能用于音频素材".to_string(),
                ))
            }
        }
        if detected.material_type == MaterialType::Subtitle
            && (command.generation.alignment_source.is_none()
                || command.generation.source_audio_material_id.is_none())
        {
            return Err(MaterialApplicationError::Validation(
                "字幕生成物必须指定 alignment_source 和 source_audio_material_id".to_string(),
            ));
        }
        if let Some(source_audio_material_id) = command.generation.source_audio_material_id {
            let source_audio = self
                .material_repository
                .get_material(source_audio_material_id)
                .await?;
            if source_audio.project_id != command.project_id
                || source_audio.material_type != MaterialType::Audio
                || source_audio.status != MaterialStatus::Active
            {
                return Err(MaterialApplicationError::Validation(
                    "来源音频必须是当前项目下可用的 audio 素材".to_string(),
                ));
            }
        }

        let mut metadata = generation_metadata(&command.generation);
        metadata.insert("mime_type".to_string(), json!(detected.mime_type));
        metadata.insert("format".to_string(), json!(detected.format));
        metadata.insert("file_size_bytes".to_string(), json!(command.bytes.len()));
        if let Some(width) = detected.width {
            metadata.insert("width".to_string(), json!(width));
        }
        if let Some(height) = detected.height {
            metadata.insert("height".to_string(), json!(height));
        }
        if detected.material_type == MaterialType::Subtitle {
            metadata.insert("subtitle_format".to_string(), json!(detected.extension));
        }
        validate_material_metadata(&Value::Object(metadata.clone()))?;

        let artifact_id = Uuid::new_v4();
        let stored = self
            .storage
            .store_generated(
                command.project_id,
                artifact_id,
                &detected.extension,
                &command.bytes,
            )
            .await
            .map_err(|error| MaterialApplicationError::UploadStorage(error.to_string()))?;

        if matches!(
            detected.material_type,
            MaterialType::Video | MaterialType::Audio
        ) {
            match probe_media(&stored.absolute_path).await {
                Ok(probe) => {
                    if let Some(duration_sec) = probe.duration_sec {
                        metadata.insert("duration_sec".to_string(), json!(duration_sec));
                    }
                    if let Some(width) = probe.width {
                        metadata.insert("width".to_string(), json!(width));
                    }
                    if let Some(height) = probe.height {
                        metadata.insert("height".to_string(), json!(height));
                    }
                }
                Err(error) => {
                    let _ = self.storage.remove(&stored).await;
                    return Err(error.into());
                }
            }
        }

        let thumbnail = if detected.material_type == MaterialType::Video {
            let bytes = match generate_video_thumbnail(&stored.absolute_path).await {
                Ok(bytes) => bytes,
                Err(error) => {
                    let _ = self.storage.remove(&stored).await;
                    return Err(error);
                }
            };
            match self
                .storage
                .store_generated_thumbnail(command.project_id, artifact_id, &bytes)
                .await
            {
                Ok(thumbnail) => Some(thumbnail),
                Err(error) => {
                    let _ = self.storage.remove(&stored).await;
                    return Err(MaterialApplicationError::UploadStorage(error.to_string()));
                }
            }
        } else {
            None
        };

        let input = CreateMaterialInput {
            project_id: command.project_id,
            material_type: detected.material_type,
            file_url: stored.public_url.clone(),
            file_name: command.file_name,
            thumbnail_url: thumbnail.as_ref().map(|value| value.public_url.clone()),
            tags: command.tags,
            metadata: Value::Object(metadata),
        };
        match self.material_repository.create_material(input).await {
            Ok(material) => Ok(material),
            Err(error) => {
                let _ = self.storage.remove(&stored).await;
                if let Some(thumbnail) = thumbnail.as_ref() {
                    let _ = self.storage.remove(thumbnail).await;
                }
                Err(error.into())
            }
        }
    }

    pub async fn get(&self, material_id: Uuid) -> Result<Material, MaterialApplicationError> {
        self.material_repository
            .get_material(material_id)
            .await
            .map_err(Into::into)
    }

    pub async fn update(
        &self,
        material_id: Uuid,
        command: MaterialUpdateCommand,
    ) -> Result<Material, MaterialApplicationError> {
        let current = self.material_repository.get_material(material_id).await?;
        self.material_repository
            .update_material(
                material_id,
                UpdateMaterialInput {
                    project_id: current.project_id,
                    material_type: current.material_type,
                    file_url: current.file_url,
                    file_name: command.file_name,
                    thumbnail_url: current.thumbnail_url,
                    tags: command.tags,
                    metadata: current.metadata,
                },
            )
            .await
            .map_err(Into::into)
    }

    pub async fn update_status(
        &self,
        material_id: Uuid,
        status: MaterialStatus,
    ) -> Result<Material, MaterialApplicationError> {
        self.material_repository
            .update_material_status(material_id, status)
            .await
            .map_err(Into::into)
    }

    async fn ensure_project_exists(
        &self,
        project_id: Uuid,
    ) -> Result<(), MaterialApplicationError> {
        if self.project_repository.project_exists(project_id).await? {
            Ok(())
        } else {
            Err(MaterialApplicationError::ProjectNotFound(project_id))
        }
    }
}

async fn generate_video_thumbnail(path: &Path) -> Result<Vec<u8>, MaterialApplicationError> {
    let output = tokio::process::Command::new("ffmpeg")
        .args(["-y", "-ss", "0", "-i"])
        .arg(path)
        .args([
            "-frames:v",
            "1",
            "-vf",
            "scale=640:-2",
            "-f",
            "image2pipe",
            "-vcodec",
            "mjpeg",
            "pipe:1",
        ])
        .output()
        .await
        .map_err(|error| {
            MaterialApplicationError::UploadStorage(format!("视频缩略图生成失败: {error}"))
        })?;
    if !output.status.success() || output.stdout.is_empty() {
        return Err(MaterialApplicationError::UploadStorage(
            "视频缩略图生成失败".to_string(),
        ));
    }
    Ok(output.stdout)
}

#[derive(Clone, Debug, PartialEq)]
pub struct MaterialUpdateCommand {
    pub file_name: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MaterialUploadCommand {
    pub project_id: Uuid,
    pub original_file_name: String,
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub tags: Vec<String>,
    pub audio_usage: Option<AudioUsage>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeneratedMaterialCommand {
    pub project_id: Uuid,
    pub original_file_name: String,
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub tags: Vec<String>,
    pub generation: WorkGenerationSnapshot,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkGenerationSnapshot {
    pub work_id: Uuid,
    pub work_version_id: Uuid,
    pub generation_run_id: Uuid,
    pub generation_step_id: Uuid,
    pub artifact_role: String,
    pub audio_usage: Option<AudioUsage>,
    pub model_snapshot: Value,
    pub voice_snapshot: Value,
    pub prompt_snapshot: Value,
    pub timeline_snapshot: Value,
    pub resource_usage: Value,
    pub request_trace_id: Option<String>,
    pub alignment_source: Option<String>,
    pub source_audio_material_id: Option<Uuid>,
}

fn validate_generation_snapshot(
    snapshot: &WorkGenerationSnapshot,
) -> Result<(), MaterialApplicationError> {
    let artifact_role = snapshot.artifact_role.trim();
    if artifact_role.is_empty()
        || artifact_role.len() > 64
        || !artifact_role.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
    {
        return Err(MaterialApplicationError::Validation(
            "artifact_role 必须是 1-64 位小写字母、数字或下划线".to_string(),
        ));
    }
    for (label, value) in [
        ("model_snapshot", &snapshot.model_snapshot),
        ("voice_snapshot", &snapshot.voice_snapshot),
        ("prompt_snapshot", &snapshot.prompt_snapshot),
        ("timeline_snapshot", &snapshot.timeline_snapshot),
        ("resource_usage", &snapshot.resource_usage),
    ] {
        if !value.is_object() {
            return Err(MaterialApplicationError::Validation(format!(
                "{label} 必须是 JSON 对象"
            )));
        }
    }
    if let Some(path) = monetary_field_path(&snapshot.resource_usage, "resource_usage") {
        return Err(MaterialApplicationError::Validation(format!(
            "资源用量快照禁止包含金额字段: {path}"
        )));
    }
    if let Some(alignment_source) = snapshot.alignment_source.as_deref() {
        if !matches!(alignment_source, "tts_timestamp" | "asr") {
            return Err(MaterialApplicationError::Validation(
                "alignment_source 仅支持 tts_timestamp 或 asr".to_string(),
            ));
        }
    }
    let metadata = Value::Object(generation_metadata(snapshot));
    validate_material_metadata(&metadata)?;
    Ok(())
}

fn monetary_field_path(value: &Value, path: &str) -> Option<String> {
    match value {
        Value::Object(object) => object.iter().find_map(|(key, child)| {
            let child_path = format!("{path}.{key}");
            let normalized = key
                .chars()
                .filter(|character| character.is_ascii_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect::<String>();
            if matches!(
                normalized.as_str(),
                "currency" | "amount" | "cost" | "price" | "fee"
            ) || ["amount", "cost", "price", "fee"]
                .iter()
                .any(|suffix| normalized.ends_with(suffix))
            {
                Some(child_path)
            } else {
                monetary_field_path(child, &child_path)
            }
        }),
        Value::Array(values) => values
            .iter()
            .enumerate()
            .find_map(|(index, child)| monetary_field_path(child, &format!("{path}[{index}]"))),
        _ => None,
    }
}

fn generation_metadata(snapshot: &WorkGenerationSnapshot) -> Map<String, Value> {
    let mut metadata = Map::from_iter([
        ("source".to_string(), json!("work_generation")),
        ("storage_provider".to_string(), json!("local")),
        ("work_id".to_string(), json!(snapshot.work_id)),
        (
            "work_version_id".to_string(),
            json!(snapshot.work_version_id),
        ),
        (
            "generation_run_id".to_string(),
            json!(snapshot.generation_run_id),
        ),
        (
            "generation_step_id".to_string(),
            json!(snapshot.generation_step_id),
        ),
        (
            "artifact_role".to_string(),
            json!(snapshot.artifact_role.trim()),
        ),
        (
            "model_snapshot".to_string(),
            snapshot.model_snapshot.clone(),
        ),
        (
            "voice_snapshot".to_string(),
            snapshot.voice_snapshot.clone(),
        ),
        (
            "prompt_snapshot".to_string(),
            snapshot.prompt_snapshot.clone(),
        ),
        (
            "timeline_snapshot".to_string(),
            snapshot.timeline_snapshot.clone(),
        ),
        (
            "resource_usage".to_string(),
            snapshot.resource_usage.clone(),
        ),
    ]);
    if let Some(audio_usage) = snapshot.audio_usage {
        metadata.insert("audio_usage".to_string(), json!(audio_usage.as_str()));
    }
    if let Some(request_trace_id) = snapshot
        .request_trace_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        metadata.insert("request_trace_id".to_string(), json!(request_trace_id));
    }
    if let Some(alignment_source) = snapshot.alignment_source.as_deref() {
        metadata.insert("alignment_source".to_string(), json!(alignment_source));
    }
    if let Some(source_audio_material_id) = snapshot.source_audio_material_id {
        metadata.insert(
            "source_audio_material_id".to_string(),
            json!(source_audio_material_id),
        );
    }
    metadata
}

#[derive(Debug)]
pub enum MaterialApplicationError {
    ProjectRepository(ProjectRepositoryError),
    MaterialRepository(MaterialRepositoryError),
    ProjectNotFound(Uuid),
    UploadValidation(UploadValidationError),
    UploadStorage(String),
    Validation(String),
}

impl From<ProjectRepositoryError> for MaterialApplicationError {
    fn from(error: ProjectRepositoryError) -> Self {
        Self::ProjectRepository(error)
    }
}

impl From<MaterialRepositoryError> for MaterialApplicationError {
    fn from(error: MaterialRepositoryError) -> Self {
        Self::MaterialRepository(error)
    }
}

impl From<UploadValidationError> for MaterialApplicationError {
    fn from(error: UploadValidationError) -> Self {
        Self::UploadValidation(error)
    }
}

impl fmt::Display for MaterialApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectRepository(error) => write!(formatter, "{error}"),
            Self::MaterialRepository(error) => write!(formatter, "{error}"),
            Self::ProjectNotFound(project_id) => {
                write!(formatter, "project not found: {project_id}")
            }
            Self::UploadValidation(error) => write!(formatter, "{error}"),
            Self::UploadStorage(message) => {
                write!(formatter, "material upload storage error: {message}")
            }
            Self::Validation(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for MaterialApplicationError {}
