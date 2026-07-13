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
            "/api/projects/:project_id/topics",
            get(handlers::list_topics).post(handlers::create_topic),
        )
        .route(
            "/api/projects/:project_id/topic-generation-batches",
            get(handlers::list_topic_generation_batches),
        )
        .route(
            "/api/projects/:project_id/topic-groups",
            get(handlers::list_topic_groups),
        )
        .route(
            "/api/topic-groups/:root_batch_id/reviews",
            post(handlers::create_topic_group_review),
        )
        .route(
            "/api/topic-groups/:root_batch_id/reviews/latest",
            get(handlers::get_latest_topic_group_review),
        )
        .route(
            "/api/topic-generation-batches/:batch_id/quality-evaluation",
            get(handlers::get_latest_topic_quality_evaluation),
        )
        .route(
            "/api/topics/:topic_id",
            put(handlers::update_topic).delete(handlers::delete_topic),
        )
        .route(
            "/api/topics/:topic_id/status",
            put(handlers::update_topic_status),
        )
        .route(
            "/api/topics/:topic_id/prepare-script",
            post(handlers::prepare_script_from_topic),
        )
}
