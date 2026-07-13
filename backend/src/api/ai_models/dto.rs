use super::error::ModelApiError;
use crate::repositories::{
    AiModel, AiModelListFilter, AiModelStatus, ChangeAiModelStatusInput, CreateAiModelInput,
    DeleteAiModelInput, UpdateAiModelInput,
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
    pub(super) fn into_input(self) -> (CreateAiModelInput, bool) {
        let requested_default = self.is_default;
        (
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
        )
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
    pub(super) fn into_input(self) -> (UpdateAiModelInput, bool) {
        let requested_default = self.is_default;
        (
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
        )
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
    api_key_configured: bool,
    api_secret_configured: bool,
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
            api_key_configured,
            api_secret_configured,
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
}

impl From<AiModel> for ModelOptionResponse {
    fn from(model: AiModel) -> Self {
        Self {
            model_id: model.id,
            display_name: model.display_name,
            model_type: model.model_type,
            provider_name: model.provider_name,
            api_protocol: model.api_protocol,
            upstream_model: model.upstream_model,
            is_default: model.is_default,
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

fn mask_credential(value: &str) -> String {
    let characters: Vec<char> = value.chars().collect();
    if characters.len() <= 8 {
        return "****".to_string();
    }
    let prefix: String = characters.iter().take(4).collect();
    let suffix: String = characters.iter().rev().take(4).rev().collect();
    format!("{prefix}****{suffix}")
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
