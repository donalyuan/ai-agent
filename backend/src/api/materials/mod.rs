pub mod dto;
mod handlers;

use crate::bootstrap::AppState;
use axum::{
    routing::{get, put},
    Router,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/projects/:project_id/materials",
            get(handlers::list_materials).post(handlers::create_material),
        )
        .route(
            "/api/materials/:material_id",
            get(handlers::get_material).put(handlers::update_material),
        )
        .route(
            "/api/materials/:material_id/status",
            put(handlers::update_material_status),
        )
}
