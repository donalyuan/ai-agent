mod dto;
mod error;
mod handlers;

use crate::bootstrap::AppState;
use axum::{routing::get, routing::post, Router};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/tools/tos-staging",
            get(handlers::get_current).put(handlers::save),
        )
        .route("/api/tools/tos-staging/check", post(handlers::check))
}
