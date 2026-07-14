pub mod dto;
mod handlers;

use crate::application::material_upload::MAX_UPLOAD_BYTES;
use crate::bootstrap::AppState;
use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post, put},
    Router,
};
use tower_http::limit::RequestBodyLimitLayer;

const MULTIPART_BODY_LIMIT: usize = MAX_UPLOAD_BYTES + 1024 * 1024;

pub(crate) fn router() -> Router<AppState> {
    let upload_router = Router::<AppState>::new()
        .route(
            "/api/projects/:project_id/materials/upload",
            post(handlers::upload_material),
        )
        .layer(DefaultBodyLimit::max(MULTIPART_BODY_LIMIT))
        .layer(RequestBodyLimitLayer::new(MULTIPART_BODY_LIMIT));

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
        .merge(upload_router)
}
