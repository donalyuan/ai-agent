use crate::repositories::TosStagingToolRepositoryError;
use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

#[derive(Debug)]
pub(super) enum TosStagingToolApiError {
    State,
    Repository(TosStagingToolRepositoryError),
}

impl From<crate::bootstrap::AppStateError> for TosStagingToolApiError {
    fn from(_error: crate::bootstrap::AppStateError) -> Self {
        Self::State
    }
}

impl From<TosStagingToolRepositoryError> for TosStagingToolApiError {
    fn from(error: TosStagingToolRepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl IntoResponse for TosStagingToolApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, code, message, details) = match self {
            Self::Repository(TosStagingToolRepositoryError::NotConfigured) => (
                StatusCode::CONFLICT,
                "tos_staging_not_configured",
                "系统私有 TOS 工具尚未配置",
                None,
            ),
            Self::Repository(TosStagingToolRepositoryError::Disabled) => (
                StatusCode::CONFLICT,
                "tos_staging_disabled",
                "系统私有 TOS 工具未启用",
                None,
            ),
            Self::Repository(TosStagingToolRepositoryError::CheckRequired) => (
                StatusCode::CONFLICT,
                "tos_staging_check_required",
                "启用系统私有 TOS 前必须完成真实 Bucket 连接检查",
                None,
            ),
            Self::Repository(TosStagingToolRepositoryError::VersionConflict) => (
                StatusCode::CONFLICT,
                "tos_staging_version_conflict",
                "TOS 配置已被其他操作更新，请刷新后重试",
                None,
            ),
            Self::Repository(TosStagingToolRepositoryError::CleanupPending { count }) => (
                StatusCode::CONFLICT,
                "tos_staging_cleanup_pending",
                "存在待清理的 TOS 临时对象，暂不能修改配置",
                Some(json!({ "pending_cleanup_count": count })),
            ),
            Self::Repository(TosStagingToolRepositoryError::InvalidConfig(_)) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_tos_staging_config",
                "系统私有 TOS 配置无效",
                None,
            ),
            Self::Repository(TosStagingToolRepositoryError::Storage(_)) | Self::State => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "服务暂时不可用",
                None,
            ),
        };
        let mut error = json!({ "code": code, "message": message });
        if let Some(details) = details {
            error["details"] = details;
        }
        (status, Json(json!({ "error": error }))).into_response()
    }
}
