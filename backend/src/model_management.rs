use crate::repositories::{
    AiModel, AiModelListFilter, AiModelRepository, AiModelRepositoryError, AiModelStatus,
    ChangeAiModelStatusInput, CreateAiModelInput, DeleteAiModelInput, DeleteAiModelOutcome,
    UpdateAiModelInput,
};
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use novex_model::{ApiProtocol, AuthScheme, ModelType};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Default, Deserialize)]
pub(crate) struct AiModelListQuery {
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

#[derive(Debug, Deserialize)]
pub(crate) struct CreateAiModelRequest {
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

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateAiModelRequest {
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

#[derive(Debug, Deserialize)]
pub(crate) struct VersionRequest {
    version: i64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChangeAiModelStatusRequest {
    version: i64,
    status: String,
    #[serde(default)]
    replacement_model_id: Option<Uuid>,
    #[serde(default)]
    allow_no_default: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DeleteAiModelRequest {
    version: i64,
    #[serde(default)]
    replacement_model_id: Option<Uuid>,
    #[serde(default)]
    allow_no_default: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ModelOptionsQuery {
    #[serde(rename = "type")]
    model_type: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct AiModelAdminResponse {
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
        let api_secret_configured = model.api_secret.as_ref().is_some_and(|value| !value.is_empty());
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
pub(crate) struct AiModelListResponse {
    models: Vec<AiModelAdminResponse>,
}

#[derive(Debug, Serialize)]
struct ModelOptionResponse {
    model_id: Uuid,
    display_name: String,
    model_type: ModelType,
    provider_name: String,
    api_protocol: ApiProtocol,
    upstream_model: String,
    is_default: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModelOptionListResponse {
    models: Vec<ModelOptionResponse>,
}

pub(crate) async fn list_ai_models(
    State(state): State<AppState>,
    Query(query): Query<AiModelListQuery>,
) -> Result<Json<AiModelListResponse>, ModelApiError> {
    let repository = state.ai_model_repository()?;
    let models = repository
        .list(AiModelListFilter {
            model_type: query
                .model_type
                .map(|value| parse_model_type(&value))
                .transpose()?,
            status: query.status.map(|value| parse_status(&value)).transpose()?,
            provider_name: query.provider_name,
            api_protocol: query
                .api_protocol
                .map(|value| parse_api_protocol(&value))
                .transpose()?,
            search: query.search,
        })
        .await?;
    Ok(Json(AiModelListResponse {
        models: models.into_iter().map(Into::into).collect(),
    }))
}

pub(crate) async fn get_ai_model(
    State(state): State<AppState>,
    Path(model_id): Path<Uuid>,
) -> Result<Json<AiModelAdminResponse>, ModelApiError> {
    Ok(Json(
        state.ai_model_repository()?.get(model_id).await?.into(),
    ))
}

pub(crate) async fn create_ai_model(
    State(state): State<AppState>,
    Json(request): Json<CreateAiModelRequest>,
) -> Result<(StatusCode, Json<AiModelAdminResponse>), ModelApiError> {
    let repository = state.ai_model_repository()?;
    let requested_default = request.is_default;
    let mut model = repository
        .create(CreateAiModelInput {
            display_name: request.display_name,
            model_type: request.model_type,
            provider_name: request.provider_name,
            api_protocol: request.api_protocol,
            protocol_version: request.protocol_version,
            auth_scheme: request.auth_scheme,
            request_base_url: request.request_base_url,
            upstream_model: request.upstream_model,
            api_key: request.api_key,
            api_secret: normalize_optional(request.api_secret),
            timeout_seconds: request.timeout_seconds,
            reasoning_effort: normalize_optional(request.reasoning_effort),
            max_output_tokens: request.max_output_tokens,
            settings: request.settings,
            sort_order: request.sort_order,
            remark: request.remark,
            status: AiModelStatus::Enabled,
            source: "admin".to_string(),
            source_key: None,
        })
        .await?;
    if requested_default && !model.is_default {
        model = repository.set_default(model.id, model.version).await?;
    }
    Ok((StatusCode::CREATED, Json(model.into())))
}

pub(crate) async fn update_ai_model(
    State(state): State<AppState>,
    Path(model_id): Path<Uuid>,
    Json(request): Json<UpdateAiModelRequest>,
) -> Result<Json<AiModelAdminResponse>, ModelApiError> {
    let repository = state.ai_model_repository()?;
    let current = repository.get(model_id).await?;
    if current.model_type == request.model_type && current.is_default && !request.is_default {
        return Err(ModelApiError::InvalidConfig(
            "默认模型只能通过选择替代模型、停用或删除流程取消".to_string(),
        ));
    }
    let requested_default = request.is_default;
    let mut model = repository
        .update(
            model_id,
            UpdateAiModelInput {
                version: request.version,
                model_type: request.model_type,
                display_name: request.display_name,
                provider_name: request.provider_name,
                api_protocol: request.api_protocol,
                protocol_version: request.protocol_version,
                auth_scheme: request.auth_scheme,
                request_base_url: request.request_base_url,
                upstream_model: request.upstream_model,
                api_key: normalize_optional(Some(request.api_key)),
                api_secret: normalize_optional(request.api_secret),
                timeout_seconds: request.timeout_seconds,
                reasoning_effort: normalize_optional(request.reasoning_effort),
                max_output_tokens: request.max_output_tokens,
                settings: request.settings,
                sort_order: request.sort_order,
                remark: request.remark,
                replacement_model_id: request.replacement_model_id,
                allow_no_default: request.allow_no_default,
            },
        )
        .await?;
    if requested_default && !model.is_default {
        model = repository.set_default(model.id, model.version).await?;
    }
    Ok(Json(model.into()))
}

pub(crate) async fn set_default_ai_model(
    State(state): State<AppState>,
    Path(model_id): Path<Uuid>,
    Json(request): Json<VersionRequest>,
) -> Result<Json<AiModelAdminResponse>, ModelApiError> {
    Ok(Json(
        state
            .ai_model_repository()?
            .set_default(model_id, request.version)
            .await?
            .into(),
    ))
}

pub(crate) async fn change_ai_model_status(
    State(state): State<AppState>,
    Path(model_id): Path<Uuid>,
    Json(request): Json<ChangeAiModelStatusRequest>,
) -> Result<Json<AiModelAdminResponse>, ModelApiError> {
    Ok(Json(
        state
            .ai_model_repository()?
            .change_status(ChangeAiModelStatusInput {
                id: model_id,
                version: request.version,
                status: parse_status(&request.status)?,
                replacement_model_id: request.replacement_model_id,
                allow_no_default: request.allow_no_default,
            })
            .await?
            .into(),
    ))
}

pub(crate) async fn delete_ai_model(
    State(state): State<AppState>,
    Path(model_id): Path<Uuid>,
    Json(request): Json<DeleteAiModelRequest>,
) -> Result<Json<Value>, ModelApiError> {
    let outcome = state
        .ai_model_repository()?
        .delete(DeleteAiModelInput {
            id: model_id,
            version: request.version,
            replacement_model_id: request.replacement_model_id,
            allow_no_default: request.allow_no_default,
        })
        .await?;
    Ok(Json(match outcome {
        DeleteAiModelOutcome::Physical => json!({ "deletion": "physical", "model_id": model_id }),
        DeleteAiModelOutcome::Logical(model) => json!({
            "deletion": "logical",
            "model": AiModelAdminResponse::from(model)
        }),
    }))
}

pub(crate) async fn list_model_options(
    State(state): State<AppState>,
    Query(query): Query<ModelOptionsQuery>,
) -> Result<Json<ModelOptionListResponse>, ModelApiError> {
    let model_type = parse_model_type(&query.model_type)?;
    let models = state
        .ai_model_repository()?
        .list_enabled_options(model_type)
        .await?
        .into_iter()
        .map(|model| ModelOptionResponse {
            model_id: model.id,
            display_name: model.display_name,
            model_type: model.model_type,
            provider_name: model.provider_name,
            api_protocol: model.api_protocol,
            upstream_model: model.upstream_model,
            is_default: model.is_default,
        })
        .collect();
    Ok(Json(ModelOptionListResponse { models }))
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
    ModelType::from_str(value)
        .map_err(|_| ModelApiError::InvalidConfig("未知模型类型".to_string()))
}

fn parse_api_protocol(value: &str) -> Result<ApiProtocol, ModelApiError> {
    ApiProtocol::from_str(value)
        .map_err(|_| ModelApiError::InvalidConfig("未知 API 调用协议".to_string()))
}

fn parse_status(value: &str) -> Result<AiModelStatus, ModelApiError> {
    AiModelStatus::try_from(value)
        .map_err(|_| ModelApiError::InvalidConfig("未知模型状态".to_string()))
}

#[derive(Debug)]
pub(crate) enum ModelApiError {
    Repository(AiModelRepositoryError),
    InvalidConfig(String),
    State(String),
}

impl From<AiModelRepositoryError> for ModelApiError {
    fn from(error: AiModelRepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl IntoResponse for ModelApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, code, message) = match self {
            Self::Repository(AiModelRepositoryError::NotFound(_)) => {
                (StatusCode::NOT_FOUND, "model_not_found", "模型不存在")
            }
            Self::Repository(AiModelRepositoryError::Disabled(_)) => {
                (StatusCode::CONFLICT, "model_disabled", "模型已停用或删除")
            }
            Self::Repository(AiModelRepositoryError::TypeMismatch { .. }) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "model_type_mismatch",
                "模型类型不匹配",
            ),
            Self::Repository(AiModelRepositoryError::VersionConflict(_)) => (
                StatusCode::CONFLICT,
                "model_version_conflict",
                "模型已被其他操作更新，请刷新后重试",
            ),
            Self::Repository(AiModelRepositoryError::ReplacementRequired(_)) => (
                StatusCode::CONFLICT,
                "replacement_model_required",
                "必须选择同类型启用模型作为新的默认模型",
            ),
            Self::Repository(AiModelRepositoryError::InvalidReplacement(_)) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_model_config",
                "替代模型无效",
            ),
            Self::Repository(AiModelRepositoryError::NoDefaultConfirmation(_)) => (
                StatusCode::CONFLICT,
                "no_default_model_confirmation_required",
                "必须明确确认该类型将没有默认模型",
            ),
            Self::Repository(AiModelRepositoryError::InvalidConfig(message))
            | Self::InvalidConfig(message) => {
                drop(message);
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_model_config",
                    "模型配置无效",
                )
            }
            Self::Repository(AiModelRepositoryError::Storage(message)) | Self::State(message) => {
                drop(message);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "model_storage_error",
                    "模型配置服务暂时不可用",
                )
            }
        };
        (status, Json(json!({ "error": { "code": code, "message": message } }))).into_response()
    }
}
