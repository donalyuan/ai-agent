use super::dto::*;
use crate::api::error::{ScriptApiError, ValidJson};
use crate::bootstrap::AppState;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

pub(super) async fn create_asset_generation_plan(
    State(state): State<AppState>,
    Path(script_id): Path<Uuid>,
    ValidJson(request): ValidJson<AssetGenerationPlanRequest>,
) -> Result<Json<AssetGenerationPlanResponse>, ScriptApiError> {
    request
        .validate_for_api()
        .map_err(ScriptApiError::AssetValidation)?;
    let plan = state
        .asset_generation_service()?
        .create_plan(script_id, request.into_options())
        .await?;
    Ok(Json(plan.into()))
}

pub(super) async fn create_asset_generation_tasks(
    State(state): State<AppState>,
    Path(script_id): Path<Uuid>,
    ValidJson(request): ValidJson<AssetGenerationTaskRequest>,
) -> Result<(StatusCode, Json<AssetGenerationTaskListResponse>), ScriptApiError> {
    request
        .validate_for_api()
        .map_err(ScriptApiError::AssetValidation)?;
    let batch = state
        .asset_generation_service()?
        .create_tasks(script_id, request.into_options())
        .await?;
    let status = if batch.reused_all {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((
        status,
        Json(AssetGenerationTaskListResponse {
            script_id: batch.script_id,
            tasks: batch
                .tasks
                .into_iter()
                .map(AssetGenerationTaskResponse::from)
                .collect(),
        }),
    ))
}

pub(super) async fn list_asset_generation_tasks(
    State(state): State<AppState>,
    Path(script_id): Path<Uuid>,
) -> Result<Json<AssetGenerationTaskListResponse>, ScriptApiError> {
    let tasks = state
        .asset_generation_service()?
        .list_tasks(script_id)
        .await?
        .into_iter()
        .map(AssetGenerationTaskResponse::from)
        .collect();
    Ok(Json(AssetGenerationTaskListResponse { script_id, tasks }))
}

pub(super) async fn list_asset_candidates(
    State(state): State<AppState>,
    Path(script_id): Path<Uuid>,
) -> Result<Json<SceneAssetCandidateListResponse>, ScriptApiError> {
    let candidates = state
        .asset_generation_service()?
        .list_candidates(script_id)
        .await?
        .into_iter()
        .map(SceneAssetCandidateResponse::from)
        .collect();
    Ok(Json(SceneAssetCandidateListResponse { candidates }))
}

pub(super) async fn select_asset_candidate(
    State(state): State<AppState>,
    Path((scene_id, candidate_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<SceneAssetCandidateResponse>, ScriptApiError> {
    let candidate = state
        .asset_generation_service()?
        .select_candidate(scene_id, candidate_id)
        .await?;
    Ok(Json(candidate.into()))
}

pub(super) async fn reject_asset_candidate(
    State(state): State<AppState>,
    Path((scene_id, candidate_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<SceneAssetCandidateResponse>, ScriptApiError> {
    let candidate = state
        .asset_generation_service()?
        .reject_candidate(scene_id, candidate_id)
        .await?;
    Ok(Json(candidate.into()))
}

pub(super) async fn create_scene_asset_generation_task(
    State(state): State<AppState>,
    Path(scene_id): Path<Uuid>,
    headers: HeaderMap,
    ValidJson(request): ValidJson<AssetGenerationTaskRequest>,
) -> Result<(StatusCode, Json<AssetGenerationTaskResponse>), ScriptApiError> {
    let request_idempotency_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            ScriptApiError::AssetValidation(
                "单镜头重生必须提供 UUID 格式 Idempotency-Key".to_string(),
            )
        })?;
    let request_idempotency_key =
        Uuid::parse_str(request_idempotency_key.trim()).map_err(|_| {
            ScriptApiError::AssetValidation(
                "单镜头重生必须提供 UUID 格式 Idempotency-Key".to_string(),
            )
        })?;
    request
        .validate_for_api()
        .map_err(ScriptApiError::AssetValidation)?;
    let result = state
        .asset_generation_service()?
        .create_scene_task(scene_id, request_idempotency_key, request.into_options())
        .await?;
    Ok((
        if result.created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(AssetGenerationTaskResponse::from(result.task)),
    ))
}

pub(super) async fn confirm_asset_generation_task(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Result<Json<AssetGenerationTaskResponse>, ScriptApiError> {
    let task = state
        .asset_generation_service()?
        .confirm_task(task_id)
        .await?;
    Ok(Json(AssetGenerationTaskResponse::from(task)))
}

pub(super) async fn dismiss_asset_generation_task(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Result<Json<AssetGenerationTaskResponse>, ScriptApiError> {
    let task = state
        .asset_generation_service()?
        .dismiss_task(task_id)
        .await?;
    Ok(Json(AssetGenerationTaskResponse::from(task)))
}
