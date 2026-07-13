use super::dto::*;
use crate::api::error::{ScriptApiError, ValidJson};
use crate::bootstrap::AppState;
use crate::repositories::{CreateProjectInput, UpdateProjectStrategyProfileInput};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

pub(super) async fn create_project(
    State(state): State<AppState>,
    ValidJson(request): ValidJson<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectResponse>), ScriptApiError> {
    request
        .validate_for_api()
        .map_err(ScriptApiError::ProjectValidation)?;
    let project = state
        .project_service()?
        .create(CreateProjectInput {
            name: request.name.trim().to_string(),
            positioning: request.positioning.trim().to_string(),
            description: request.description.trim().to_string(),
            strategy_profile: request
                .strategy_profile
                .as_ref()
                .map(AccountStrategyProfileRequest::normalize)
                .transpose()
                .map_err(ScriptApiError::ProjectValidation)?
                .unwrap_or_default(),
        })
        .await?;

    Ok((StatusCode::CREATED, Json(ProjectResponse::from(project))))
}

pub(super) async fn list_projects(
    State(state): State<AppState>,
) -> Result<Json<ProjectListResponse>, ScriptApiError> {
    let projects = state.project_service()?.list().await?;
    Ok(Json(ProjectListResponse {
        projects: projects.into_iter().map(ProjectResponse::from).collect(),
    }))
}

pub(super) async fn update_project_strategy_profile(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    ValidJson(request): ValidJson<UpdateProjectStrategyProfileRequest>,
) -> Result<Json<ProjectResponse>, ScriptApiError> {
    request
        .validate_for_api()
        .map_err(ScriptApiError::ProjectValidation)?;
    let strategy_profile = request
        .strategy_profile
        .normalize()
        .map_err(ScriptApiError::ProjectValidation)?;
    let project = state
        .project_service()?
        .update_strategy_profile(
            project_id,
            UpdateProjectStrategyProfileInput {
                name: request.name.trim().to_string(),
                positioning: request.positioning.trim().to_string(),
                description: request.description.trim().to_string(),
                strategy_profile,
            },
        )
        .await?;

    Ok(Json(ProjectResponse::from(project)))
}

pub(super) async fn generate_project_strategy_profile_draft(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    ValidJson(request): ValidJson<StrategyProfileDraftRequest>,
) -> Result<Json<StrategyProfileDraftResponse>, ScriptApiError> {
    request
        .validate_for_api()
        .map_err(ScriptApiError::ProjectValidation)?;
    let output = state
        .project_service()?
        .generate_strategy_profile_draft(project_id, request.model_id, &request.direction_notes)
        .await?;

    Ok(Json(StrategyProfileDraftResponse {
        draft: output.draft,
        draft_summary: output.draft_summary,
    }))
}
