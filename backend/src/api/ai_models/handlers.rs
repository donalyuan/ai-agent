use super::dto::*;
use super::error::ModelApiError;
use crate::bootstrap::AppState;
use crate::repositories::DeleteAiModelOutcome;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use uuid::Uuid;

pub(super) async fn list_ai_models(
    State(state): State<AppState>,
    Query(query): Query<AiModelListQuery>,
) -> Result<Json<AiModelListResponse>, ModelApiError> {
    let models = state.ai_model_service()?.list(query.into_filter()?).await?;
    Ok(Json(AiModelListResponse {
        models: models.into_iter().map(Into::into).collect(),
    }))
}

pub(super) async fn get_ai_model(
    State(state): State<AppState>,
    Path(model_id): Path<Uuid>,
) -> Result<Json<AiModelAdminResponse>, ModelApiError> {
    Ok(Json(state.ai_model_service()?.get(model_id).await?.into()))
}

pub(super) async fn create_ai_model(
    State(state): State<AppState>,
    Json(request): Json<CreateAiModelRequest>,
) -> Result<(StatusCode, Json<AiModelAdminResponse>), ModelApiError> {
    let (input, requested_default) = request.into_input();
    let model = state
        .ai_model_service()?
        .create(input, requested_default)
        .await?;
    Ok((StatusCode::CREATED, Json(model.into())))
}

pub(super) async fn update_ai_model(
    State(state): State<AppState>,
    Path(model_id): Path<Uuid>,
    Json(request): Json<UpdateAiModelRequest>,
) -> Result<Json<AiModelAdminResponse>, ModelApiError> {
    let (input, requested_default) = request.into_input();
    let model = state
        .ai_model_service()?
        .update(model_id, input, requested_default)
        .await?;
    Ok(Json(model.into()))
}

pub(super) async fn set_default_ai_model(
    State(state): State<AppState>,
    Path(model_id): Path<Uuid>,
    Json(request): Json<VersionRequest>,
) -> Result<Json<AiModelAdminResponse>, ModelApiError> {
    Ok(Json(
        state
            .ai_model_service()?
            .set_default(model_id, request.version)
            .await?
            .into(),
    ))
}

pub(super) async fn change_ai_model_status(
    State(state): State<AppState>,
    Path(model_id): Path<Uuid>,
    Json(request): Json<ChangeAiModelStatusRequest>,
) -> Result<Json<AiModelAdminResponse>, ModelApiError> {
    Ok(Json(
        state
            .ai_model_service()?
            .change_status(request.into_input(model_id)?)
            .await?
            .into(),
    ))
}

pub(super) async fn delete_ai_model(
    State(state): State<AppState>,
    Path(model_id): Path<Uuid>,
    Json(request): Json<DeleteAiModelRequest>,
) -> Result<Json<Value>, ModelApiError> {
    let outcome = state
        .ai_model_service()?
        .delete(request.into_input(model_id))
        .await?;
    Ok(Json(match outcome {
        DeleteAiModelOutcome::Physical => {
            json!({ "deletion": "physical", "model_id": model_id })
        }
        DeleteAiModelOutcome::Logical(model) => json!({
            "deletion": "logical",
            "model": AiModelAdminResponse::from(*model)
        }),
    }))
}

pub(super) async fn list_model_options(
    State(state): State<AppState>,
    Query(query): Query<ModelOptionsQuery>,
) -> Result<Json<ModelOptionListResponse>, ModelApiError> {
    let models = state
        .ai_model_service()?
        .list_enabled_options(query.parse_model_type()?)
        .await?
        .into_iter()
        .map(ModelOptionResponse::from)
        .collect();
    Ok(Json(ModelOptionListResponse { models }))
}
