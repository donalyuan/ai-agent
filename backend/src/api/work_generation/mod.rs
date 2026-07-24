mod dto;
mod handlers;

use crate::bootstrap::AppState;
use axum::{
    routing::{get, post},
    Router,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/scripts/:script_id/work-generation/plans",
            post(handlers::create_plan),
        )
        .route(
            "/api/work-generation/plans/:plan_id/confirm",
            post(handlers::confirm_plan),
        )
        .route(
            "/api/projects/:project_id/work-generation/tasks",
            get(handlers::list_tasks),
        )
        .route(
            "/api/work-generation/runs/:run_id",
            get(handlers::task_details),
        )
        .route(
            "/api/work-generation/runs/:run_id/cancel",
            post(handlers::cancel_run),
        )
        .route(
            "/api/work-generation/runs/:run_id/dismiss",
            post(handlers::dismiss_run),
        )
        .route(
            "/api/work-generation/steps/:step_id/retry",
            post(handlers::retry_step),
        )
}
