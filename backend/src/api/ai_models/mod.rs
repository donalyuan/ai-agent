//! AI 模型管理 HTTP 边界；业务规则由 `AiModelService` 执行。

mod dto;
pub(crate) mod error;
mod handlers;

use crate::bootstrap::AppState;
use axum::{
    routing::{get, post, put},
    Router,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/models",
            get(handlers::list_ai_models).post(handlers::create_ai_model),
        )
        .route(
            "/api/admin/models/:model_id",
            get(handlers::get_ai_model)
                .put(handlers::update_ai_model)
                .delete(handlers::delete_ai_model),
        )
        .route(
            "/api/admin/models/:model_id/default",
            post(handlers::set_default_ai_model),
        )
        .route(
            "/api/admin/models/:model_id/status",
            put(handlers::change_ai_model_status),
        )
        .route("/api/model-options", get(handlers::list_model_options))
        .route(
            "/api/admin/models/:model_id/voice-catalog/sync",
            post(handlers::request_admin_voice_catalog_sync),
        )
        .route(
            "/api/speech/models/:model_id/voice-catalog/check",
            post(handlers::request_workspace_voice_catalog_sync),
        )
        .route(
            "/api/speech/models/:model_id/voice-catalog",
            get(handlers::get_voice_catalog),
        )
}
