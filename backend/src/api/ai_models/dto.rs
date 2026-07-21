use super::error::ModelApiError;
use crate::repositories::{
    AiModel, AiModelListFilter, AiModelStatus, ChangeAiModelStatusInput, CreateAiModelInput,
    DeleteAiModelInput, UpdateAiModelInput, VoiceCatalogEntry, VoiceCatalogSnapshot,
    VoiceCatalogSync,
};
use chrono::{DateTime, Utc};
use novex_model::{ApiProtocol, AuthScheme, ModelType};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Default, Deserialize)]
pub(super) struct AiModelListQuery {
    #[serde(default, rename = "type")]
    model_type: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default, rename = "provider")]
    provider_name: Option<String>,
    #[serde(default, rename = "protocol")]
    api_protocol: Option<String>,
    #[serde(default, rename = "q")]
    search: Option<String>,
}

impl AiModelListQuery {
    pub(super) fn into_filter(self) -> Result<AiModelListFilter, ModelApiError> {
        Ok(AiModelListFilter {
            model_type: self
                .model_type
                .map(|value| parse_model_type(&value))
                .transpose()?,
            status: self.status.map(|value| parse_status(&value)).transpose()?,
            provider_name: self.provider_name,
            api_protocol: self
                .api_protocol
                .map(|value| parse_api_protocol(&value))
                .transpose()?,
            search: self.search,
        })
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateAiModelRequest {
    display_name: String,
    model_type: ModelType,
    provider_name: String,
    api_protocol: ApiProtocol,
    #[serde(default)]
    protocol_version: String,
    auth_scheme: AuthScheme,
    request_base_url: String,
    upstream_model: String,
    api_key: String,
    #[serde(default)]
    api_secret: Option<String>,
    #[serde(default)]
    catalog_access_key: Option<String>,
    #[serde(default)]
    catalog_secret_key: Option<String>,
    #[serde(default = "default_voice_catalog_mode")]
    voice_catalog_mode: String,
    #[serde(default)]
    voice_catalog_source_model_id: Option<Uuid>,
    timeout_seconds: i32,
    #[serde(default)]
    reasoning_effort: Option<String>,
    #[serde(default)]
    max_output_tokens: Option<i32>,
    #[serde(default = "empty_object")]
    settings: Value,
    #[serde(default)]
    sort_order: i32,
    #[serde(default)]
    remark: String,
    #[serde(default)]
    is_default: bool,
}

impl CreateAiModelRequest {
    pub(super) fn into_input(self) -> Result<(CreateAiModelInput, bool), ModelApiError> {
        let requested_default = self.is_default;
        let voice_catalog_source_model_id = parse_create_voice_catalog_binding(
            &self.voice_catalog_mode,
            self.voice_catalog_source_model_id,
        )?;
        Ok((
            CreateAiModelInput {
                display_name: self.display_name,
                model_type: self.model_type,
                provider_name: self.provider_name,
                api_protocol: self.api_protocol,
                protocol_version: self.protocol_version,
                auth_scheme: self.auth_scheme,
                request_base_url: self.request_base_url,
                upstream_model: self.upstream_model,
                api_key: self.api_key,
                api_secret: normalize_optional(self.api_secret),
                catalog_access_key: normalize_optional(self.catalog_access_key),
                catalog_secret_key: normalize_optional(self.catalog_secret_key),
                voice_catalog_source_model_id,
                timeout_seconds: self.timeout_seconds,
                reasoning_effort: normalize_optional(self.reasoning_effort),
                max_output_tokens: self.max_output_tokens,
                settings: self.settings,
                sort_order: self.sort_order,
                remark: self.remark,
                status: AiModelStatus::Enabled,
                source: "admin".to_string(),
                source_key: None,
            },
            requested_default,
        ))
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateAiModelRequest {
    version: i64,
    model_type: ModelType,
    display_name: String,
    provider_name: String,
    api_protocol: ApiProtocol,
    #[serde(default)]
    protocol_version: String,
    auth_scheme: AuthScheme,
    request_base_url: String,
    upstream_model: String,
    #[serde(default)]
    api_key: String,
    #[serde(default)]
    api_secret: Option<String>,
    #[serde(default)]
    catalog_access_key: Option<String>,
    #[serde(default)]
    catalog_secret_key: Option<String>,
    #[serde(default)]
    voice_catalog_mode: Option<String>,
    #[serde(default)]
    voice_catalog_source_model_id: Option<Uuid>,
    timeout_seconds: i32,
    #[serde(default)]
    reasoning_effort: Option<String>,
    #[serde(default)]
    max_output_tokens: Option<i32>,
    #[serde(default = "empty_object")]
    settings: Value,
    #[serde(default)]
    sort_order: i32,
    #[serde(default)]
    remark: String,
    #[serde(default)]
    is_default: bool,
    #[serde(default)]
    replacement_model_id: Option<Uuid>,
    #[serde(default)]
    allow_no_default: bool,
}

impl UpdateAiModelRequest {
    pub(super) fn into_input(self) -> Result<(UpdateAiModelInput, bool), ModelApiError> {
        let requested_default = self.is_default;
        let voice_catalog_source_model_id = parse_update_voice_catalog_binding(
            self.voice_catalog_mode.as_deref(),
            self.voice_catalog_source_model_id,
        )?;
        Ok((
            UpdateAiModelInput {
                version: self.version,
                model_type: self.model_type,
                display_name: self.display_name,
                provider_name: self.provider_name,
                api_protocol: self.api_protocol,
                protocol_version: self.protocol_version,
                auth_scheme: self.auth_scheme,
                request_base_url: self.request_base_url,
                upstream_model: self.upstream_model,
                api_key: normalize_optional(Some(self.api_key)),
                api_secret: normalize_optional(self.api_secret),
                catalog_access_key: normalize_optional(self.catalog_access_key),
                catalog_secret_key: normalize_optional(self.catalog_secret_key),
                voice_catalog_source_model_id,
                timeout_seconds: self.timeout_seconds,
                reasoning_effort: normalize_optional(self.reasoning_effort),
                max_output_tokens: self.max_output_tokens,
                settings: self.settings,
                sort_order: self.sort_order,
                remark: self.remark,
                replacement_model_id: self.replacement_model_id,
                allow_no_default: self.allow_no_default,
            },
            requested_default,
        ))
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct VersionRequest {
    pub(super) version: i64,
}

#[derive(Debug, Deserialize)]
pub(super) struct ChangeAiModelStatusRequest {
    version: i64,
    status: String,
    #[serde(default)]
    replacement_model_id: Option<Uuid>,
    #[serde(default)]
    allow_no_default: bool,
}

impl ChangeAiModelStatusRequest {
    pub(super) fn into_input(
        self,
        model_id: Uuid,
    ) -> Result<ChangeAiModelStatusInput, ModelApiError> {
        Ok(ChangeAiModelStatusInput {
            id: model_id,
            version: self.version,
            status: parse_status(&self.status)?,
            replacement_model_id: self.replacement_model_id,
            allow_no_default: self.allow_no_default,
        })
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct DeleteAiModelRequest {
    version: i64,
    #[serde(default)]
    replacement_model_id: Option<Uuid>,
    #[serde(default)]
    allow_no_default: bool,
}

impl DeleteAiModelRequest {
    pub(super) fn into_input(self, model_id: Uuid) -> DeleteAiModelInput {
        DeleteAiModelInput {
            id: model_id,
            version: self.version,
            replacement_model_id: self.replacement_model_id,
            allow_no_default: self.allow_no_default,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct ModelOptionsQuery {
    #[serde(rename = "type")]
    model_type: String,
}

impl ModelOptionsQuery {
    pub(super) fn parse_model_type(&self) -> Result<ModelType, ModelApiError> {
        parse_model_type(&self.model_type)
    }
}

#[derive(Debug, Serialize)]
pub(super) struct AiModelAdminResponse {
    model_id: Uuid,
    display_name: String,
    model_type: ModelType,
    provider_name: String,
    api_protocol: ApiProtocol,
    protocol_version: String,
    auth_scheme: AuthScheme,
    request_base_url: String,
    upstream_model: String,
    api_key_masked: String,
    api_secret_masked: Option<String>,
    catalog_access_key_masked: Option<String>,
    catalog_secret_key_masked: Option<String>,
    api_key_configured: bool,
    api_secret_configured: bool,
    catalog_access_key_configured: bool,
    catalog_secret_key_configured: bool,
    voice_catalog_mode: String,
    voice_catalog_source_model_id: Option<Uuid>,
    voice_catalog_source_display_name: Option<String>,
    timeout_seconds: i32,
    reasoning_effort: Option<String>,
    max_output_tokens: Option<i32>,
    settings: Value,
    sort_order: i32,
    remark: String,
    status: String,
    is_default: bool,
    last_call_status: String,
    last_call_at: Option<DateTime<Utc>>,
    last_error_summary: Option<String>,
    source: String,
    version: i64,
    deleted_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<AiModel> for AiModelAdminResponse {
    fn from(model: AiModel) -> Self {
        let api_key_configured = !model.api_key.is_empty();
        let api_secret_configured = model
            .api_secret
            .as_ref()
            .is_some_and(|value| !value.is_empty());
        let catalog_access_key_configured = model
            .catalog_access_key
            .as_ref()
            .is_some_and(|value| !value.is_empty());
        let catalog_secret_key_configured = model
            .catalog_secret_key
            .as_ref()
            .is_some_and(|value| !value.is_empty());
        let voice_catalog_mode = if model.voice_catalog_source_model_id.is_some() {
            "shared"
        } else {
            "official_sync"
        }
        .to_string();
        Self {
            model_id: model.id,
            display_name: model.display_name,
            model_type: model.model_type,
            provider_name: model.provider_name,
            api_protocol: model.api_protocol,
            protocol_version: model.protocol_version,
            auth_scheme: model.auth_scheme,
            request_base_url: model.request_base_url,
            upstream_model: model.upstream_model,
            api_key_masked: mask_credential(&model.api_key),
            api_secret_masked: model.api_secret.as_deref().map(mask_credential),
            catalog_access_key_masked: model.catalog_access_key.as_deref().map(mask_credential),
            catalog_secret_key_masked: model.catalog_secret_key.as_deref().map(mask_credential),
            api_key_configured,
            api_secret_configured,
            catalog_access_key_configured,
            catalog_secret_key_configured,
            voice_catalog_mode,
            voice_catalog_source_model_id: model.voice_catalog_source_model_id,
            voice_catalog_source_display_name: model.voice_catalog_source_display_name,
            timeout_seconds: model.timeout_seconds,
            reasoning_effort: model.reasoning_effort,
            max_output_tokens: model.max_output_tokens,
            settings: model.settings,
            sort_order: model.sort_order,
            remark: model.remark,
            status: model.status.as_str().to_string(),
            is_default: model.is_default,
            last_call_status: model.last_call_status,
            last_call_at: model.last_call_at,
            last_error_summary: model.last_error_summary,
            source: model.source,
            version: model.version,
            deleted_at: model.deleted_at,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct AiModelListResponse {
    pub(super) models: Vec<AiModelAdminResponse>,
}

#[derive(Debug, Serialize)]
pub(super) struct ModelOptionResponse {
    model_id: Uuid,
    display_name: String,
    model_type: ModelType,
    provider_name: String,
    api_protocol: ApiProtocol,
    upstream_model: String,
    is_default: bool,
    capabilities: Value,
}

impl From<AiModel> for ModelOptionResponse {
    fn from(model: AiModel) -> Self {
        let capabilities = if model.model_type == ModelType::Video {
            model.settings.clone()
        } else {
            json!({})
        };
        Self {
            model_id: model.id,
            display_name: model.display_name,
            model_type: model.model_type,
            provider_name: model.provider_name,
            api_protocol: model.api_protocol,
            upstream_model: model.upstream_model,
            is_default: model.is_default,
            capabilities,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct ModelOptionListResponse {
    pub(super) models: Vec<ModelOptionResponse>,
}

fn empty_object() -> Value {
    json!({})
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn default_voice_catalog_mode() -> String {
    "official_sync".to_string()
}

fn parse_create_voice_catalog_binding(
    mode: &str,
    source_model_id: Option<Uuid>,
) -> Result<Option<Uuid>, ModelApiError> {
    match mode {
        "official_sync" if source_model_id.is_none() => Ok(None),
        "shared" => source_model_id.map(Some).ok_or_else(|| {
            ModelApiError::InvalidConfig("复用已有目录时必须选择来源模型".to_string())
        }),
        "official_sync" => Err(ModelApiError::InvalidConfig(
            "官方同步模式不能指定目录来源模型".to_string(),
        )),
        _ => Err(ModelApiError::InvalidConfig(
            "未知音色目录来源模式".to_string(),
        )),
    }
}

fn parse_update_voice_catalog_binding(
    mode: Option<&str>,
    source_model_id: Option<Uuid>,
) -> Result<Option<Option<Uuid>>, ModelApiError> {
    match mode {
        None if source_model_id.is_none() => Ok(None),
        None => Err(ModelApiError::InvalidConfig(
            "指定音色目录来源时必须同时提交来源模式".to_string(),
        )),
        Some("official_sync") if source_model_id.is_none() => Ok(Some(None)),
        Some("shared") => source_model_id.map(|id| Some(Some(id))).ok_or_else(|| {
            ModelApiError::InvalidConfig("复用已有目录时必须选择来源模型".to_string())
        }),
        Some("official_sync") => Err(ModelApiError::InvalidConfig(
            "官方同步模式不能指定目录来源模型".to_string(),
        )),
        Some(_) => Err(ModelApiError::InvalidConfig(
            "未知音色目录来源模式".to_string(),
        )),
    }
}

fn mask_credential(value: &str) -> String {
    let characters: Vec<char> = value.chars().collect();
    if characters.len() <= 8 {
        return "****".to_string();
    }
    let prefix: String = characters.iter().take(4).collect();
    let suffix: String = characters.iter().rev().take(4).rev().collect();
    format!("{prefix}****{suffix}")
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct VoiceCatalogQuery {
    #[serde(default)]
    pub(super) include_unavailable: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct VoiceCatalogSyncResponse {
    sync_id: Uuid,
    model_id: Uuid,
    trigger_source: String,
    status: String,
    page_limit: i32,
    page_count: i32,
    speaker_count: i32,
    error_summary: Option<String>,
    requested_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<VoiceCatalogSync> for VoiceCatalogSyncResponse {
    fn from(sync: VoiceCatalogSync) -> Self {
        Self {
            sync_id: sync.id,
            model_id: sync.model_id,
            trigger_source: sync.trigger_source,
            status: sync.status,
            page_limit: sync.page_limit,
            page_count: sync.page_count,
            speaker_count: sync.speaker_count,
            error_summary: sync.error_summary,
            requested_at: sync.requested_at,
            started_at: sync.started_at,
            completed_at: sync.completed_at,
            created_at: sync.created_at,
            updated_at: sync.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct VoiceCatalogEntryResponse {
    voice_id: Uuid,
    voice_type: String,
    resource_id: String,
    name: String,
    avatar_url: Option<String>,
    gender: Option<String>,
    age: Option<String>,
    categories: Value,
    normal_labels: Vec<String>,
    special_labels: Vec<String>,
    trial_url: Option<String>,
    short_trial_url: Option<String>,
    languages: Value,
    emotions: Value,
    description: String,
    is_available: bool,
    catalog_version: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<VoiceCatalogEntry> for VoiceCatalogEntryResponse {
    fn from(voice: VoiceCatalogEntry) -> Self {
        Self {
            voice_id: voice.id,
            voice_type: voice.voice_type,
            resource_id: voice.resource_id,
            name: voice.name,
            avatar_url: voice.avatar_url,
            gender: voice.gender,
            age: voice.age,
            categories: voice.categories,
            normal_labels: voice.normal_labels,
            special_labels: voice.special_labels,
            trial_url: voice.trial_url,
            short_trial_url: voice.short_trial_url,
            languages: voice.languages,
            emotions: voice.emotions,
            description: voice.description,
            is_available: voice.is_available,
            catalog_version: voice.catalog_version,
            created_at: voice.created_at,
            updated_at: voice.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct VoiceCatalogResponse {
    model_id: Uuid,
    source_model_id: Uuid,
    model_settings: Value,
    last_sync: Option<VoiceCatalogSyncResponse>,
    voices: Vec<VoiceCatalogEntryResponse>,
}

impl From<VoiceCatalogSnapshot> for VoiceCatalogResponse {
    fn from(catalog: VoiceCatalogSnapshot) -> Self {
        Self {
            model_id: catalog.model_id,
            source_model_id: catalog.source_model_id,
            model_settings: catalog.model_settings,
            last_sync: catalog.last_sync.map(Into::into),
            voices: catalog.voices.into_iter().map(Into::into).collect(),
        }
    }
}

fn parse_model_type(value: &str) -> Result<ModelType, ModelApiError> {
    ModelType::from_str(value).map_err(|_| ModelApiError::InvalidConfig("未知模型类型".to_string()))
}

fn parse_api_protocol(value: &str) -> Result<ApiProtocol, ModelApiError> {
    ApiProtocol::from_str(value)
        .map_err(|_| ModelApiError::InvalidConfig("未知 API 调用协议".to_string()))
}

fn parse_status(value: &str) -> Result<AiModelStatus, ModelApiError> {
    AiModelStatus::try_from(value)
        .map_err(|_| ModelApiError::InvalidConfig("未知模型状态".to_string()))
}
