mod handlers;

use crate::bootstrap::AppState;
use axum::{
    routing::{get, post},
    Router,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/projects/:project_id/works", get(handlers::list_works))
        .route(
            "/api/works/:work_id",
            get(handlers::work_details).delete(handlers::delete_work),
        )
        .route("/api/works/:work_id/archive", post(handlers::archive_work))
        .route("/api/works/:work_id/restore", post(handlers::restore_work))
        .route(
            "/api/work-versions/:version_id/derive",
            post(handlers::derive_version),
        )
        .route(
            "/api/work-versions/:version_id/regenerate",
            post(handlers::regenerate_version),
        )
        .route(
            "/api/work-versions/:version_id/diff",
            post(handlers::analyze_diff),
        )
        .route(
            "/api/work-version-diffs/:diff_id/confirm",
            post(handlers::confirm_diff),
        )
        .route(
            "/api/work-versions/:version_id/downloads",
            get(handlers::download_manifest),
        )
        .route(
            "/api/work-artifacts/:artifact_id/download",
            get(handlers::download_artifact),
        )
        .route(
            "/api/work-versions/:version_id/production-package",
            get(handlers::production_package),
        )
        .route(
            "/api/work-versions/:version_id/publication-handoffs",
            post(handlers::create_handoff),
        )
}
