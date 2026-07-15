use crate::application::materials::{MaterialUpdateCommand, WorkGenerationSnapshot};
use crate::repositories::{
    redact_sensitive_material_metadata, validate_material_metadata, AudioUsage,
    CreateMaterialInput, Material, MaterialListFilter, MaterialSourceFilter, MaterialStatus,
    MaterialStatusFilter, MaterialType,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MaterialPayloadRequest {
    pub material_type: String,
    pub file_url: String,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    pub file_name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub metadata: Value,
}

impl MaterialPayloadRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        self.normalized_parts().map(|_| ())
    }

    pub fn into_create_input(self, project_id: Uuid) -> Result<CreateMaterialInput, String> {
        let parts = self.normalized_parts()?;
        Ok(CreateMaterialInput {
            project_id,
            material_type: parts.material_type,
            file_url: parts.file_url,
            file_name: parts.file_name,
            thumbnail_url: parts.thumbnail_url,
            tags: parts.tags,
            metadata: parts.metadata,
        })
    }

    pub fn into_update_command(self) -> Result<MaterialUpdateCommand, String> {
        Ok(MaterialUpdateCommand {
            file_name: normalize_material_name(&self.file_name)?,
            tags: normalize_material_tags(&self.tags)?,
        })
    }

    fn normalized_parts(&self) -> Result<NormalizedMaterialPayload, String> {
        let file_name = normalize_material_name(&self.file_name)?;

        let material_type = MaterialType::try_from(self.material_type.trim())
            .map_err(|_| "素材类型无效".to_string())?;
        let file_url = normalize_http_url("素材 URL", &self.file_url)?;
        let thumbnail_url = self
            .thumbnail_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| normalize_http_url("缩略图 URL", value))
            .transpose()?;
        let tags = normalize_material_tags(&self.tags)?;
        if !self.metadata.is_object() {
            return Err("素材 metadata 必须是 JSON 对象".to_string());
        }
        validate_material_metadata(&self.metadata).map_err(|error| error.to_string())?;
        if self.metadata.get("source").and_then(Value::as_str) == Some("work_generation") {
            return Err("作品生成素材必须通过统一生成物登记接口写入".to_string());
        }

        Ok(NormalizedMaterialPayload {
            material_type,
            file_url,
            thumbnail_url,
            file_name,
            tags,
            metadata: self.metadata.clone(),
        })
    }
}

struct NormalizedMaterialPayload {
    material_type: MaterialType,
    file_url: String,
    thumbnail_url: Option<String>,
    file_name: String,
    tags: Vec<String>,
    metadata: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MaterialStatusRequest {
    pub status: String,
}

impl MaterialStatusRequest {
    pub fn parse_status(&self) -> Result<MaterialStatus, String> {
        MaterialStatus::try_from(self.status.trim()).map_err(|_| "素材状态无效".to_string())
    }
}

#[derive(Clone, Debug, Deserialize, Default, PartialEq)]
pub struct MaterialListQuery {
    #[serde(rename = "type")]
    pub material_type: Option<String>,
    pub status: Option<String>,
    pub q: Option<String>,
    pub tag: Option<String>,
    pub audio_usage: Option<String>,
    pub source: Option<String>,
    pub work_id: Option<String>,
    pub work_version_id: Option<String>,
}

impl MaterialListQuery {
    pub fn into_filter(self) -> Result<MaterialListFilter, String> {
        let material_type = self
            .material_type
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != "all")
            .map(MaterialType::try_from)
            .transpose()
            .map_err(|_| "素材类型筛选无效".to_string())?;
        let status = self
            .status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(MaterialStatusFilter::try_from)
            .transpose()
            .map_err(|_| "素材状态筛选无效".to_string())?
            .unwrap_or_default();
        let audio_usage = self
            .audio_usage
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != "all")
            .map(AudioUsage::try_from)
            .transpose()
            .map_err(|_| "音频用途筛选无效".to_string())?;
        let source = self
            .source
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != "all")
            .map(MaterialSourceFilter::try_from)
            .transpose()
            .map_err(|_| "素材来源筛选无效".to_string())?;
        let work_id = parse_optional_uuid(self.work_id.as_deref(), "来源作品 ID")?;
        let work_version_id =
            parse_optional_uuid(self.work_version_id.as_deref(), "来源作品版本 ID")?;

        Ok(MaterialListFilter {
            material_type,
            status,
            q: self.q,
            tag: self.tag,
            audio_usage,
            source,
            work_id,
            work_version_id,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct WorkGenerationSnapshotRequest {
    pub work_id: Uuid,
    pub work_version_id: Uuid,
    pub generation_run_id: Uuid,
    pub generation_step_id: Uuid,
    pub artifact_role: String,
    #[serde(default)]
    pub audio_usage: Option<String>,
    pub model_snapshot: Value,
    pub voice_snapshot: Value,
    pub prompt_snapshot: Value,
    pub timeline_snapshot: Value,
    pub resource_usage: Value,
    #[serde(default)]
    pub request_trace_id: Option<String>,
    #[serde(default)]
    pub alignment_source: Option<String>,
    #[serde(default)]
    pub source_audio_material_id: Option<Uuid>,
}

impl WorkGenerationSnapshotRequest {
    pub fn into_snapshot(self) -> Result<WorkGenerationSnapshot, String> {
        let audio_usage = self
            .audio_usage
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(AudioUsage::try_from)
            .transpose()
            .map_err(|_| "audio_usage 无效".to_string())?;
        let request_trace_id = normalize_optional_text(
            self.request_trace_id,
            200,
            "request_trace_id 不能超过 200 个字符",
        )?;
        let alignment_source = normalize_optional_text(
            self.alignment_source,
            32,
            "alignment_source 不能超过 32 个字符",
        )?;

        Ok(WorkGenerationSnapshot {
            work_id: self.work_id,
            work_version_id: self.work_version_id,
            generation_run_id: self.generation_run_id,
            generation_step_id: self.generation_step_id,
            artifact_role: self.artifact_role,
            audio_usage,
            model_snapshot: self.model_snapshot,
            voice_snapshot: self.voice_snapshot,
            prompt_snapshot: self.prompt_snapshot,
            timeline_snapshot: self.timeline_snapshot,
            resource_usage: self.resource_usage,
            request_trace_id,
            alignment_source,
            source_audio_material_id: self.source_audio_material_id,
        })
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MaterialResponse {
    pub material_id: Uuid,
    pub project_id: Uuid,
    pub material_type: String,
    pub file_url: String,
    pub thumbnail_url: Option<String>,
    pub file_name: String,
    pub tags: Vec<String>,
    pub metadata: Value,
    pub source: Option<String>,
    pub audio_usage: Option<String>,
    pub work_id: Option<Uuid>,
    pub work_version_id: Option<Uuid>,
    pub generation: Option<Value>,
    pub usage_count: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Material> for MaterialResponse {
    fn from(material: Material) -> Self {
        let metadata = redact_sensitive_material_metadata(material.metadata);
        let generation = generation_snapshot_response(material.source.as_deref(), &metadata);
        Self {
            material_id: material.id,
            project_id: material.project_id,
            material_type: material.material_type.as_str().to_string(),
            file_url: material.file_url,
            thumbnail_url: material.thumbnail_url,
            file_name: material.file_name,
            tags: material.tags,
            metadata,
            source: material.source,
            audio_usage: material
                .audio_usage
                .map(|audio_usage| audio_usage.as_str().to_string()),
            work_id: material.work_id,
            work_version_id: material.work_version_id,
            generation,
            usage_count: material.usage_count,
            status: material.status.as_str().to_string(),
            created_at: material.created_at,
            updated_at: material.updated_at,
        }
    }
}

fn generation_snapshot_response(source: Option<&str>, metadata: &Value) -> Option<Value> {
    if source != Some("work_generation") {
        return None;
    }
    let object = metadata.as_object()?;
    let keys = [
        "work_id",
        "work_version_id",
        "generation_run_id",
        "generation_step_id",
        "artifact_role",
        "audio_usage",
        "model_snapshot",
        "voice_snapshot",
        "prompt_snapshot",
        "timeline_snapshot",
        "resource_usage",
        "request_trace_id",
        "alignment_source",
        "source_audio_material_id",
        "duration_sec",
        "subtitle_format",
    ];
    let snapshot = keys
        .into_iter()
        .filter_map(|key| {
            object
                .get(key)
                .cloned()
                .map(|value| (key.to_string(), value))
        })
        .collect();
    Some(Value::Object(snapshot))
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MaterialListResponse {
    pub materials: Vec<MaterialResponse>,
}

fn normalize_http_url(label: &str, value: &str) -> Result<String, String> {
    let normalized = value.trim().to_string();
    let uri = normalized
        .parse::<axum::http::Uri>()
        .map_err(|_| format!("{label}必须是 http 或 https URL"))?;
    let scheme = uri.scheme_str().unwrap_or_default();
    if !matches!(scheme, "http" | "https") || uri.host().is_none() {
        return Err(format!("{label}必须是 http 或 https URL"));
    }
    Ok(normalized)
}

fn parse_optional_uuid(value: Option<&str>, label: &str) -> Result<Option<Uuid>, String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| Uuid::parse_str(value).map_err(|_| format!("{label}无效")))
        .transpose()
}

fn normalize_optional_text(
    value: Option<String>,
    max_characters: usize,
    length_error: &str,
) -> Result<Option<String>, String> {
    let value = value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if value
        .as_ref()
        .is_some_and(|value| value.chars().count() > max_characters)
    {
        return Err(length_error.to_string());
    }
    Ok(value)
}

pub(crate) fn normalize_material_name(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_string();
    if normalized.is_empty() {
        return Err("素材名称不能为空".to_string());
    }
    if normalized.chars().count() > 255 {
        return Err("素材名称不能超过 255 个字符".to_string());
    }
    Ok(normalized)
}

pub(crate) fn normalize_material_tags(values: &[String]) -> Result<Vec<String>, String> {
    if values.len() > 30 {
        return Err("素材标签最多填写 30 个".to_string());
    }

    let mut normalized = Vec::new();
    for value in values {
        let item = value.trim();
        if item.is_empty() {
            continue;
        }
        if item.chars().count() > 40 {
            return Err("素材标签单项不能超过 40 个字符".to_string());
        }
        if !normalized.iter().any(|existing| existing == item) {
            normalized.push(item.to_string());
        }
    }
    Ok(normalized)
}
