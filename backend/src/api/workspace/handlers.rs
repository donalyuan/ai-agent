use super::dto::{WorkspaceMenuListResponse, WorkspaceMenuNodeResponse};
use crate::api::error::ScriptApiError;
use crate::bootstrap::AppState;
use axum::{extract::State, Json};

pub(super) async fn list_workspace_menus(
    State(state): State<AppState>,
) -> Result<Json<WorkspaceMenuListResponse>, ScriptApiError> {
    let menus = state.workspace_service()?.list_visible_menu_tree().await?;
    Ok(Json(WorkspaceMenuListResponse {
        menus: menus
            .into_iter()
            .map(WorkspaceMenuNodeResponse::from)
            .collect(),
    }))
}
