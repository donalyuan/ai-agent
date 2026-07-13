pub mod dto;
mod handlers;

use crate::bootstrap::AppState;
use axum::{
    routing::{get, post},
    Router,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/agent/conversations",
            post(handlers::create_agent_conversation),
        )
        .route(
            "/api/agent/conversations/:conversation_id/messages",
            get(handlers::list_agent_messages).post(handlers::send_agent_message),
        )
}
