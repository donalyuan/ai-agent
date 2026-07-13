use crate::application::materials::MaterialUpdateCommand;
use crate::repositories::{
    CreateMaterialInput, Material, MaterialListFilter, MaterialStatus, MaterialStatusFilter,
    MaterialType,
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
        let parts = self.normalized_parts()?;
        Ok(MaterialUpdateCommand {
            material_type: parts.material_type,
            file_url: parts.file_url,
            file_name: parts.file_name,
            thumbnail_url: parts.thumbnail_url,
            tags: parts.tags,
            metadata: parts.metadata,
        })
    }

    fn normalized_parts(&self) -> Result<NormalizedMaterialPayload, String> {
        let file_name = self.file_name.trim().to_string();
        if file_name.is_empty() {
            return Err("素材名称不能为空".to_string());
        }
        if file_name.chars().count() > 255 {
            return Err("素材名称不能超过 255 个字符".to_string());
        }

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

        Ok(MaterialListFilter {
            material_type,
            status,
            q: self.q,
            tag: self.tag,
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
    pub usage_count: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Material> for MaterialResponse {
    fn from(material: Material) -> Self {
        Self {
            material_id: material.id,
            project_id: material.project_id,
            material_type: material.material_type.as_str().to_string(),
            file_url: material.file_url,
            thumbnail_url: material.thumbnail_url,
            file_name: material.file_name,
            tags: material.tags,
            metadata: material.metadata,
            usage_count: material.usage_count,
            status: material.status.as_str().to_string(),
            created_at: material.created_at,
            updated_at: material.updated_at,
        }
    }
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

fn normalize_material_tags(values: &[String]) -> Result<Vec<String>, String> {
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
