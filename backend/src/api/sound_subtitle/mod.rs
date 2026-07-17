mod dto;
mod error;
mod handlers;

use crate::bootstrap::AppState;
use axum::{
    routing::{get, post},
    Router,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/projects/:project_id/audio-materials/:material_id/inspection",
            get(handlers::get_audio_inspection).post(handlers::request_audio_inspection),
        )
        .route(
            "/api/projects/:project_id/sound-subtitle/tasks/preflight",
            post(handlers::preflight_task),
        )
        .route(
            "/api/projects/:project_id/sound-subtitle/tasks",
            get(handlers::list_tasks).post(handlers::create_task),
        )
        .route(
            "/api/projects/:project_id/sound-subtitle/tasks/:task_id",
            get(handlers::get_task),
        )
        .route(
            "/api/projects/:project_id/sound-subtitle/tasks/:task_id/retry",
            post(handlers::retry_task),
        )
        .route(
            "/api/projects/:project_id/sound-subtitle/tasks/:task_id/cancel",
            post(handlers::cancel_task),
        )
}
