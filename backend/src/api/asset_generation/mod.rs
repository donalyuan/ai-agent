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
            "/api/scripts/:script_id/asset-generation-plan",
            post(handlers::create_asset_generation_plan),
        )
        .route(
            "/api/scripts/:script_id/asset-generation-tasks",
            get(handlers::list_asset_generation_tasks)
                .post(handlers::create_asset_generation_tasks),
        )
        .route(
            "/api/scripts/:script_id/asset-candidates",
            get(handlers::list_asset_candidates),
        )
        .route(
            "/api/scenes/:scene_id/asset-candidates/:candidate_id/select",
            put(handlers::select_asset_candidate),
        )
        .route(
            "/api/scenes/:scene_id/asset-candidates/:candidate_id/reject",
            put(handlers::reject_asset_candidate),
        )
        .route(
            "/api/scenes/:scene_id/asset-generation-tasks",
            post(handlers::create_scene_asset_generation_task),
        )
        .route(
            "/api/asset-generation-tasks/:task_id/confirm",
            post(handlers::confirm_asset_generation_task),
        )
        .route(
            "/api/asset-generation-tasks/:task_id/dismiss",
            post(handlers::dismiss_asset_generation_task),
        )
}
