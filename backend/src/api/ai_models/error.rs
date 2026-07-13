use crate::application::ai_models::AiModelApplicationError;
use crate::bootstrap::AppStateError;
use crate::repositories::AiModelRepositoryError;
use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

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

impl From<AiModelApplicationError> for ModelApiError {
    fn from(error: AiModelApplicationError) -> Self {
        match error {
            AiModelApplicationError::Repository(error) => Self::Repository(error),
            AiModelApplicationError::InvalidConfig(message) => Self::InvalidConfig(message),
        }
    }
}

impl From<AppStateError> for ModelApiError {
    fn from(error: AppStateError) -> Self {
        Self::State(error.to_string())
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
        (
            status,
            Json(json!({ "error": { "code": code, "message": message } })),
        )
            .into_response()
    }
}
