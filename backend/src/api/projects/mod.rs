pub mod dto;
mod handlers;

use crate::bootstrap::AppState;
use axum::{
    routing::{get, post, put},
    Router,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/projects",
            get(handlers::list_projects).post(handlers::create_project),
        )
        .route(
            "/api/projects/:project_id/strategy-profile",
            put(handlers::update_project_strategy_profile),
        )
        .route(
            "/api/projects/:project_id/strategy-profile/draft",
            post(handlers::generate_project_strategy_profile_draft),
        )
}
