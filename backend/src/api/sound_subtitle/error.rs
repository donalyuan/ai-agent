use crate::application::sound_subtitle::SoundSubtitleApplicationError;
use crate::repositories::{
    AiModelRepositoryError, MaterialRepositoryError, SoundSubtitleRepositoryError,
    TosStagingToolRepositoryError, VoiceCatalogRepositoryError,
};
use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

#[derive(Debug)]
pub(super) struct SoundApiError(pub SoundSubtitleApplicationError);

impl From<SoundSubtitleApplicationError> for SoundApiError {
    fn from(error: SoundSubtitleApplicationError) -> Self {
        Self(error)
    }
}

impl IntoResponse for SoundApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, code, message) = match self.0 {
            SoundSubtitleApplicationError::Validation { code, message } => {
                let status = if code == "emotion_unsupported" {
                    StatusCode::BAD_REQUEST
                } else if matches!(
                    code.as_str(),
                    "voice_unavailable"
                        | "model_unavailable"
                        | "inspection_stale"
                        | "confirmation_stale"
                        | "retry_not_allowed"
                        | "retry_input_changed"
                        | "source_script_changed"
                        | "source_script_unavailable"
                ) {
                    StatusCode::CONFLICT
                } else {
                    StatusCode::UNPROCESSABLE_ENTITY
                };
                (status, code, message)
            }
            SoundSubtitleApplicationError::Repository(error) => match error {
                SoundSubtitleRepositoryError::InspectionNotFound(_) => (
                    StatusCode::NOT_FOUND,
                    "inspection_not_found".to_string(),
                    "音频检查不存在".to_string(),
                ),
                SoundSubtitleRepositoryError::TaskNotFound(_) => (
                    StatusCode::NOT_FOUND,
                    "task_not_found".to_string(),
                    "声音任务不存在".to_string(),
                ),
                SoundSubtitleRepositoryError::TaskNotCancellable(_) => (
                    StatusCode::CONFLICT,
                    "task_not_cancellable".to_string(),
                    "该任务当前不可取消".to_string(),
                ),
                SoundSubtitleRepositoryError::IdempotencyConflict => (
                    StatusCode::CONFLICT,
                    "idempotency_conflict".to_string(),
                    "Idempotency-Key 已用于不同请求".to_string(),
                ),
                SoundSubtitleRepositoryError::ConcurrencyLimit { .. } => (
                    StatusCode::TOO_MANY_REQUESTS,
                    "concurrency_limit".to_string(),
                    "当前项目已有过多待执行声音任务".to_string(),
                ),
                SoundSubtitleRepositoryError::Storage(_) => internal_error(),
            },
            SoundSubtitleApplicationError::Model(error) => match error {
                AiModelRepositoryError::NotFound(_) => (
                    StatusCode::NOT_FOUND,
                    "model_not_found".to_string(),
                    "语音模型不存在".to_string(),
                ),
                AiModelRepositoryError::Disabled(_) => (
                    StatusCode::CONFLICT,
                    "model_unavailable".to_string(),
                    "语音模型已停用或删除".to_string(),
                ),
                AiModelRepositoryError::TypeMismatch { .. }
                | AiModelRepositoryError::InvalidConfig(_) => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "model_config_invalid".to_string(),
                    "语音模型配置无效".to_string(),
                ),
                _ => internal_error(),
            },
            SoundSubtitleApplicationError::Material(error) => match error {
                MaterialRepositoryError::MaterialNotFound(_) => (
                    StatusCode::NOT_FOUND,
                    "material_not_found".to_string(),
                    "音频素材不存在".to_string(),
                ),
                _ => internal_error(),
            },
            SoundSubtitleApplicationError::VoiceCatalog(error) => match error {
                VoiceCatalogRepositoryError::ModelNotFound(_) => (
                    StatusCode::NOT_FOUND,
                    "model_not_found".to_string(),
                    "语音模型不存在".to_string(),
                ),
                VoiceCatalogRepositoryError::ModelUnavailable(_) => (
                    StatusCode::CONFLICT,
                    "model_unavailable".to_string(),
                    "语音模型不可用".to_string(),
                ),
                VoiceCatalogRepositoryError::InvalidRequest(message) => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "voice_catalog_invalid".to_string(),
                    message,
                ),
                VoiceCatalogRepositoryError::Storage(_) => internal_error(),
            },
            SoundSubtitleApplicationError::TosStagingTool(error) => match error {
                TosStagingToolRepositoryError::NotConfigured => (
                    StatusCode::CONFLICT,
                    "tos_staging_not_configured".to_string(),
                    "系统私有 TOS 工具尚未配置".to_string(),
                ),
                TosStagingToolRepositoryError::Disabled => (
                    StatusCode::CONFLICT,
                    "tos_staging_disabled".to_string(),
                    "系统私有 TOS 工具未启用".to_string(),
                ),
                TosStagingToolRepositoryError::CheckRequired => (
                    StatusCode::CONFLICT,
                    "tos_staging_check_required".to_string(),
                    "系统私有 TOS 尚未通过连接检查".to_string(),
                ),
                _ => internal_error(),
            },
            SoundSubtitleApplicationError::Script(_) => internal_error(),
            SoundSubtitleApplicationError::Internal(_) => internal_error(),
        };
        (
            status,
            Json(json!({"error": {"code": code, "message": message}})),
        )
            .into_response()
    }
}

fn internal_error() -> (StatusCode, String, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "sound_service_error".to_string(),
        "声音与字幕服务暂时不可用".to_string(),
    )
}
