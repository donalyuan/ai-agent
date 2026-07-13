mod dto;
mod handlers;

use crate::bootstrap::AppState;
use axum::{routing::get, Router};

pub(crate) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/video-workspace/menus",
        get(handlers::list_workspace_menus),
    )
}
