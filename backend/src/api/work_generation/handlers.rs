use super::dto::{CreateWorkPlanRequest, WorkPlanResponse, WorkRunResponse};
use crate::api::error::{ScriptApiError, ValidJson};
use crate::bootstrap::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;
use crate::repositories::WorkGenerationTaskFilter;

#[derive(Debug, serde::Deserialize)]
pub(super) struct TaskListQuery {
    pub view: Option<String>,
    pub stage: Option<String>,
    pub query: Option<String>,
    #[serde(default)]
    pub include_hidden: bool,
}

pub(super) async fn create_plan(
    State(state): State<AppState>,
    Path(script_id): Path<Uuid>,
    ValidJson(request): ValidJson<CreateWorkPlanRequest>,
) -> Result<Json<WorkPlanResponse>, ScriptApiError> {
    let response = state
        .work_generation_service()?
        .plan(request.into_input(script_id))
        .await?;
    Ok(Json(response.into()))
}

pub(super) async fn confirm_plan(
    State(state): State<AppState>,
    Path(plan_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<WorkRunResponse>), ScriptApiError> {
    let key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ScriptApiError::AssetValidation("作品生成确认必须提供 Idempotency-Key".into())
        })?;
    let result = state
        .work_generation_service()?
        .confirm(plan_id, key.to_string())
        .await?;
    Ok((
        if result.created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(result.into()),
    ))
}

pub(super) async fn list_tasks(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<TaskListQuery>,
) -> Result<Json<crate::application::work_generation::WorkTaskListView>, ScriptApiError> {
    let result = state
        .work_generation_service()?
        .list_tasks(project_id, WorkGenerationTaskFilter {
            status_view: query.view,
            stage: query.stage,
            query: query.query,
            include_hidden: query.include_hidden,
        })
        .await?;
    Ok(Json(result))
}

pub(super) async fn task_details(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<crate::application::work_generation::WorkTaskDetailsView>, ScriptApiError> {
    Ok(Json(state.work_generation_service()?.task_details(run_id).await?))
}

pub(super) async fn cancel_run(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<crate::application::work_generation::WorkTaskDetailsView>, ScriptApiError> {
    Ok(Json(state.work_generation_service()?.cancel_run(run_id).await?))
}

pub(super) async fn dismiss_run(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<crate::application::work_generation::WorkTaskDetailsView>, ScriptApiError> {
    Ok(Json(state.work_generation_service()?.dismiss_run(run_id).await?))
}

pub(super) async fn retry_step(
    State(state): State<AppState>,
    Path(step_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<crate::repositories::WorkGenerationAttemptRecord>, ScriptApiError> {
    let key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ScriptApiError::AssetValidation("节点重试必须提供 Idempotency-Key".into()))?;
    Ok(Json(state.work_generation_service()?.retry_step(step_id, key.to_string()).await?))
}
