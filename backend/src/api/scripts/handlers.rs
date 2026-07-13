use super::dto::*;
use crate::api::error::{ScriptApiError, ValidJson};
use crate::bootstrap::AppState;
use crate::domain::script::ScriptListFilter;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use uuid::Uuid;

pub(super) async fn generate_script(
    State(state): State<AppState>,
    ValidJson(request): ValidJson<GenerateScriptRequest>,
) -> Result<Json<ScriptResponse>, ScriptApiError> {
    let model_id = request.model_id;
    let script = state
        .script_service()?
        .generate(model_id, request.into_generation_input())
        .await?;
    Ok(Json(ScriptResponse::from(script)))
}

pub(super) async fn get_script(
    State(state): State<AppState>,
    Path(script_id): Path<Uuid>,
) -> Result<Json<ScriptResponse>, ScriptApiError> {
    let script = state.script_service()?.get(script_id).await?;
    Ok(Json(ScriptResponse::from(script)))
}

pub(super) async fn list_scripts(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(filter): Query<ScriptListFilter>,
) -> Result<Json<ScriptListResponse>, ScriptApiError> {
    let result = state.script_service()?.list(project_id, filter).await?;
    Ok(Json(ScriptListResponse {
        scripts: result.scripts.into_iter().map(Into::into).collect(),
        total: result.total,
        limit: result.limit,
        offset: result.offset,
    }))
}

pub(super) async fn update_script_status(
    State(state): State<AppState>,
    Path(script_id): Path<Uuid>,
    ValidJson(request): ValidJson<UpdateScriptStatusRequest>,
) -> Result<Json<UpdateScriptStatusResponse>, ScriptApiError> {
    let script = state
        .script_service()?
        .update_status(script_id, request.status)
        .await?;
    Ok(Json(UpdateScriptStatusResponse::from(script)))
}
