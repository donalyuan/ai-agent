use super::dto::*;
use super::error::SoundApiError;
use crate::application::sound_subtitle::SoundSubtitleApplicationError;
use crate::bootstrap::AppState;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

pub(super) async fn request_audio_inspection(
    State(state): State<AppState>,
    Path((project_id, material_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<AudioInspectionResponse>), SoundApiError> {
    let idempotency_key = idempotency_key(&headers)?;
    let (inspection, created) = state
        .sound_subtitle_service()
        .map_err(|error| SoundApiError(SoundSubtitleApplicationError::Internal(error.to_string())))?
        .request_audio_inspection(project_id, material_id, idempotency_key)
        .await?;
    Ok((
        if created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(inspection.into()),
    ))
}

pub(super) async fn get_audio_inspection(
    State(state): State<AppState>,
    Path((project_id, material_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<AudioInspectionResponse>, SoundApiError> {
    let inspection = state
        .sound_subtitle_service()
        .map_err(|error| SoundApiError(SoundSubtitleApplicationError::Internal(error.to_string())))?
        .get_audio_inspection(project_id, material_id)
        .await?;
    Ok(Json(inspection.into()))
}

pub(super) async fn preflight_task(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(request): Json<SoundTaskRequest>,
) -> Result<Json<SoundTaskPreflightResponse>, SoundApiError> {
    let preflight = state
        .sound_subtitle_service()
        .map_err(|error| SoundApiError(SoundSubtitleApplicationError::Internal(error.to_string())))?
        .preflight(project_id, request.into_intent())
        .await?;
    Ok(Json(preflight.into()))
}

pub(super) async fn create_task(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<SoundTaskRequest>,
) -> Result<(StatusCode, Json<SoundTaskResponse>), SoundApiError> {
    let idempotency_key = idempotency_key(&headers)?;
    let (intent, confirmation_token) = request.split_creation().map_err(|message| {
        SoundApiError(SoundSubtitleApplicationError::Validation {
            code: "confirmation_required".to_string(),
            message,
        })
    })?;
    let (task, created) = state
        .sound_subtitle_service()
        .map_err(|error| SoundApiError(SoundSubtitleApplicationError::Internal(error.to_string())))?
        .create_task(project_id, intent, confirmation_token, idempotency_key)
        .await?;
    Ok((
        if created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(task.into()),
    ))
}

pub(super) async fn get_task(
    State(state): State<AppState>,
    Path((project_id, task_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<SoundTaskResponse>, SoundApiError> {
    let task = state
        .sound_subtitle_service()
        .map_err(|error| SoundApiError(SoundSubtitleApplicationError::Internal(error.to_string())))?
        .get_task(project_id, task_id)
        .await?;
    Ok(Json(task.into()))
}

pub(super) async fn list_tasks(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<SoundTaskListResponse>, SoundApiError> {
    let tasks = state
        .sound_subtitle_service()
        .map_err(|error| SoundApiError(SoundSubtitleApplicationError::Internal(error.to_string())))?
        .list_tasks(project_id)
        .await?;
    Ok(Json(SoundTaskListResponse {
        tasks: tasks.into_iter().map(Into::into).collect(),
    }))
}

pub(super) async fn retry_task(
    State(state): State<AppState>,
    Path((project_id, task_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(request): Json<SoundTaskRequest>,
) -> Result<(StatusCode, Json<SoundTaskResponse>), SoundApiError> {
    let idempotency_key = idempotency_key(&headers)?;
    let (intent, confirmation_token) = request.split_creation().map_err(|message| {
        SoundApiError(SoundSubtitleApplicationError::Validation {
            code: "confirmation_required".to_string(),
            message,
        })
    })?;
    let (task, created) = state
        .sound_subtitle_service()
        .map_err(|error| SoundApiError(SoundSubtitleApplicationError::Internal(error.to_string())))?
        .retry_task(
            project_id,
            task_id,
            intent,
            confirmation_token,
            idempotency_key,
        )
        .await?;
    Ok((
        if created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(task.into()),
    ))
}

pub(super) async fn cancel_task(
    State(state): State<AppState>,
    Path((project_id, task_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<SoundTaskResponse>, SoundApiError> {
    let task = state
        .sound_subtitle_service()
        .map_err(|error| SoundApiError(SoundSubtitleApplicationError::Internal(error.to_string())))?
        .cancel_task(project_id, task_id)
        .await?;
    Ok(Json(task.into()))
}

fn idempotency_key(headers: &HeaderMap) -> Result<String, SoundApiError> {
    headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            SoundApiError(SoundSubtitleApplicationError::Validation {
                code: "idempotency_key_required".to_string(),
                message: "必须提供 Idempotency-Key".to_string(),
            })
        })
}
