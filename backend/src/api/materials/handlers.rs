use super::dto::*;
use crate::api::error::{ScriptApiError, ValidJson};
use crate::bootstrap::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

pub(super) async fn create_material(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    ValidJson(request): ValidJson<MaterialPayloadRequest>,
) -> Result<(StatusCode, Json<MaterialResponse>), ScriptApiError> {
    let input = request
        .into_create_input(project_id)
        .map_err(ScriptApiError::MaterialValidation)?;
    let material = state.material_service()?.create(input).await?;
    Ok((StatusCode::CREATED, Json(MaterialResponse::from(material))))
}

pub(super) async fn list_materials(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<MaterialListQuery>,
) -> Result<Json<MaterialListResponse>, ScriptApiError> {
    let filter = query
        .into_filter()
        .map_err(ScriptApiError::MaterialValidation)?;
    let materials = state.material_service()?.list(project_id, filter).await?;
    Ok(Json(MaterialListResponse {
        materials: materials.into_iter().map(MaterialResponse::from).collect(),
    }))
}

pub(super) async fn get_material(
    State(state): State<AppState>,
    Path(material_id): Path<Uuid>,
) -> Result<Json<MaterialResponse>, ScriptApiError> {
    let material = state.material_service()?.get(material_id).await?;
    Ok(Json(MaterialResponse::from(material)))
}

pub(super) async fn update_material(
    State(state): State<AppState>,
    Path(material_id): Path<Uuid>,
    ValidJson(request): ValidJson<MaterialPayloadRequest>,
) -> Result<Json<MaterialResponse>, ScriptApiError> {
    let command = request
        .into_update_command()
        .map_err(ScriptApiError::MaterialValidation)?;
    let material = state
        .material_service()?
        .update(material_id, command)
        .await?;
    Ok(Json(MaterialResponse::from(material)))
}

pub(super) async fn update_material_status(
    State(state): State<AppState>,
    Path(material_id): Path<Uuid>,
    ValidJson(request): ValidJson<MaterialStatusRequest>,
) -> Result<Json<MaterialResponse>, ScriptApiError> {
    let status = request
        .parse_status()
        .map_err(ScriptApiError::MaterialValidation)?;
    let material = state
        .material_service()?
        .update_status(material_id, status)
        .await?;
    Ok(Json(MaterialResponse::from(material)))
}
