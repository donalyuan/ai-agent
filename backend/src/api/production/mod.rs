//! Production Crew HTTP API 路由

mod dto;
mod handlers;

use crate::bootstrap::AppState;
use axum::{
    routing::{get, post},
    Router,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        // Full Crew 持久化命令 API
        .route(
            "/api/v1/production/intents",
            post(handlers::create_production_intent),
        )
        .route(
            "/api/v1/production/intents/:intent_id/runs",
            post(handlers::start_production_run),
        )
        .route(
            "/api/v1/production/intents/:intent_id",
            get(handlers::get_production_intent).delete(handlers::delete_production_intent),
        )
        .route(
            "/api/v1/production/intents/:intent_id/archive",
            post(handlers::archive_production_intent),
        )
        .route(
            "/api/v1/production/runs/:run_id",
            get(handlers::get_production_run),
        )
        .route(
            "/api/v1/production/runs/:run_id/cancel",
            post(handlers::cancel_production_run),
        )
        .route(
            "/api/v1/production/runs/:run_id/packages/:digest/approve",
            post(handlers::approve_package),
        )
        .route(
            "/api/v1/production/runs/:run_id/packages/:digest/reject",
            post(handlers::reject_package),
        )
        .route(
            "/api/v1/production/runs/:run_id/resume",
            post(handlers::resume_production_run),
        )
        .route(
            "/api/v1/production/runs/:run_id/steps/:step_id/retry",
            post(handlers::retry_production_step),
        )
        // 项目管理
        .route(
            "/api/v1/production/productions",
            get(handlers::list_productions).post(handlers::create_production),
        )
        .route(
            "/api/v1/production/productions/:id",
            get(handlers::get_production).delete(handlers::delete_production),
        )
        // 产物管理
        .route(
            "/api/v1/production/productions/:id/artifacts/:artifact_type",
            get(handlers::get_artifact),
        )
        .route(
            "/api/v1/production/productions/:id/artifacts/:artifact_type/:artifact_id/approve",
            post(handlers::approve_artifact),
        )
        .route(
            "/api/v1/production/productions/:id/artifacts/:artifact_type/all",
            get(handlers::list_artifacts),
        )
        // 协作建议
        .route(
            "/api/v1/production/productions/:id/suggestions",
            get(handlers::list_suggestions).post(handlers::create_suggestion),
        )
        .route(
            "/api/v1/production/productions/:id/suggestions/:suggestion_id/respond",
            post(handlers::respond_to_suggestion),
        )
        // Fast Lane
        .route(
            "/api/v1/production/productions/:id/fast-lane",
            post(handlers::execute_fast_lane),
        )
        .route(
            "/api/v1/production/productions/:id/fast-lane/:job_id",
            get(handlers::get_fast_lane_status),
        )
        // 审计日志
        .route(
            "/api/v1/production/productions/:id/audit-log",
            get(handlers::get_audit_log),
        )
}
