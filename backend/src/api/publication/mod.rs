mod handlers;

use crate::bootstrap::AppState;
use axum::{
    routing::{get, post, put},
    Router,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/publication-handoffs/:handoff_id/publication",
            post(handlers::create_plan),
        )
        .route("/api/publications", get(handlers::list))
        .route("/api/publications/:id", get(handlers::details))
        .route(
            "/api/publications/:id/targets/:platform",
            put(handlers::save_target),
        )
        .route(
            "/api/publication-targets/:id/handoff",
            post(handlers::handoff),
        )
        .route(
            "/api/publication-targets/:id/package",
            post(handlers::generate_package),
        )
        .route(
            "/api/publication-targets/:id/downloads",
            get(handlers::downloads),
        )
        .route("/api/publication-targets/:id/download-audits",post(handlers::audit_download))
        .route("/api/publication-targets/:id/copy-audits",post(handlers::audit_copy))
        .route(
            "/api/publication-packages/:id/download",
            get(handlers::download_package),
        )
        .route(
            "/api/publication-targets/:id/needs-attention",
            post(handlers::needs_attention),
        )
        .route(
            "/api/publication-targets/:id/cancel",
            post(handlers::cancel),
        )
        .route(
            "/api/publication-targets/:id/published",
            post(handlers::published),
        )
        .route(
            "/api/publication-targets/:id/result-corrections",
            post(handlers::correct),
        )
}
