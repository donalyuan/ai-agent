pub mod dto;
mod handlers;

use crate::bootstrap::AppState;
use axum::{
    routing::{get, post, put},
    Router,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/scripts/generate", post(handlers::generate_script))
        .route("/api/scripts/:script_id", get(handlers::get_script))
        .route(
            "/api/projects/:project_id/scripts",
            get(handlers::list_scripts),
        )
        .route(
            "/api/scripts/:script_id/status",
            put(handlers::update_script_status),
        )
}
