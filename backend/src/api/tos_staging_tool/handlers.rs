use super::dto::{
    CheckTosStagingToolRequest, SaveTosStagingToolRequest, TosStagingToolAdminResponse,
};
use super::error::TosStagingToolApiError;
use crate::api::error::ValidJson;
use crate::bootstrap::AppState;
use axum::{extract::State, http::StatusCode, Json};

pub(super) async fn get_current(
    State(state): State<AppState>,
) -> Result<Json<TosStagingToolAdminResponse>, TosStagingToolApiError> {
    let repository = state.tos_staging_tool_repository()?;
    let pending_cleanup_count = repository.pending_cleanup_count().await?;
    let response = match repository.get_current().await? {
        Some(config) => TosStagingToolAdminResponse::configured(config, pending_cleanup_count),
        None => TosStagingToolAdminResponse::unconfigured(pending_cleanup_count),
    };
    Ok(Json(response))
}

pub(super) async fn save(
    State(state): State<AppState>,
    ValidJson(request): ValidJson<SaveTosStagingToolRequest>,
) -> Result<Json<TosStagingToolAdminResponse>, TosStagingToolApiError> {
    let repository = state.tos_staging_tool_repository()?;
    let config = repository.save(request.into()).await?;
    let pending_cleanup_count = repository.pending_cleanup_count().await?;
    Ok(Json(TosStagingToolAdminResponse::configured(
        config,
        pending_cleanup_count,
    )))
}

pub(super) async fn check(
    State(state): State<AppState>,
    ValidJson(request): ValidJson<CheckTosStagingToolRequest>,
) -> Result<(StatusCode, Json<TosStagingToolAdminResponse>), TosStagingToolApiError> {
    let repository = state.tos_staging_tool_repository()?;
    let config = repository.queue_connection_check(request.version).await?;
    let pending_cleanup_count = repository.pending_cleanup_count().await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(TosStagingToolAdminResponse::configured(
            config,
            pending_cleanup_count,
        )),
    ))
}
