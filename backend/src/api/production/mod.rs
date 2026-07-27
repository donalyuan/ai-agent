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
        // 项目管理
        .route(
            "/api/v1/production/productions",
            get(handlers::list_productions).post(handlers::create_production),
        )
        .route(
            "/api/v1/production/productions/:id",
            get(handlers::get_production).delete(handlers::delete_production),
        )
        // 角色执行
        .route(
            "/api/v1/production/productions/:id/roles/:role_key/execute",
            post(handlers::execute_role),
        )
        .route(
            "/api/v1/production/productions/:id/execute-flow",
            post(handlers::execute_flow),
        )
        .route(
            "/api/v1/production/productions/:id/flows/:flow_id",
            get(handlers::get_flow_status),
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
