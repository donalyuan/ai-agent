use crate::application::work_library::{VersionSnapshotPatches, WorkLibraryApplicationError};
use crate::bootstrap::AppState;
use crate::repositories::WorkLibraryRepositoryError;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, Response, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub(super) struct WorkListQuery {
    #[serde(default)]
    archived: bool,
    query: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct DeriveVersionRequest {
    input_snapshot_patch: Option<Value>,
    model_snapshot_patch: Option<Value>,
    parameter_snapshot_patch: Option<Value>,
    prompt_snapshot_patch: Option<Value>,
    timeline_snapshot_patch: Option<Value>,
}

impl From<DeriveVersionRequest> for VersionSnapshotPatches {
    fn from(value: DeriveVersionRequest) -> Self {
        Self {
            input: value.input_snapshot_patch,
            model: value.model_snapshot_patch,
            parameter: value.parameter_snapshot_patch,
            prompt: value.prompt_snapshot_patch,
            timeline: value.timeline_snapshot_patch,
        }
    }
}

pub(super) async fn list_works(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<WorkListQuery>,
) -> Result<Json<Value>, WorkLibraryHttpError> {
    Ok(Json(
        state
            .work_library_service()?
            .list_works(project_id, query.archived, query.query.as_deref())
            .await?,
    ))
}

pub(super) async fn work_details(
    State(state): State<AppState>,
    Path(work_id): Path<Uuid>,
) -> Result<Json<Value>, WorkLibraryHttpError> {
    Ok(Json(state.work_library_service()?.details(work_id).await?))
}

pub(super) async fn derive_version(
    State(state): State<AppState>,
    Path(version_id): Path<Uuid>,
    Json(request): Json<DeriveVersionRequest>,
) -> Result<(StatusCode, Json<crate::repositories::WorkVersionRecord>), WorkLibraryHttpError> {
    let version = state
        .work_library_service()?
        .derive(version_id, request.into())
        .await?;
    Ok((StatusCode::CREATED, Json(version)))
}

pub(super) async fn regenerate_version(
    State(state): State<AppState>,
    Path(version_id): Path<Uuid>,
) -> Result<(StatusCode, Json<crate::repositories::WorkVersionRecord>), WorkLibraryHttpError> {
    Ok((
        StatusCode::CREATED,
        Json(state.work_library_service()?.regenerate(version_id).await?),
    ))
}

pub(super) async fn analyze_diff(
    State(state): State<AppState>,
    Path(version_id): Path<Uuid>,
) -> Result<
    (
        StatusCode,
        Json<crate::repositories::WorkVersionDiffPlanRecord>,
    ),
    WorkLibraryHttpError,
> {
    Ok((
        StatusCode::CREATED,
        Json(
            state
                .work_library_service()?
                .analyze_diff(version_id)
                .await?,
        ),
    ))
}

pub(super) async fn confirm_diff(
    State(state): State<AppState>,
    Path(diff_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<crate::repositories::WorkDiffConfirmation>), WorkLibraryHttpError> {
    let key = idempotency_key(&headers, "差异确认")?;
    let result = state
        .work_library_service()?
        .confirm_diff(diff_id, key)
        .await?;
    Ok((
        if result.created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(result),
    ))
}

pub(super) async fn delete_work(
    State(state): State<AppState>,
    Path(work_id): Path<Uuid>,
) -> Result<StatusCode, WorkLibraryHttpError> {
    state.work_library_service()?.delete_blank(work_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn archive_work(
    State(state): State<AppState>,
    Path(work_id): Path<Uuid>,
) -> Result<Json<Value>, WorkLibraryHttpError> {
    Ok(Json(state.work_library_service()?.archive(work_id).await?))
}

pub(super) async fn restore_work(
    State(state): State<AppState>,
    Path(work_id): Path<Uuid>,
) -> Result<Json<Value>, WorkLibraryHttpError> {
    Ok(Json(state.work_library_service()?.restore(work_id).await?))
}

pub(super) async fn download_manifest(
    State(state): State<AppState>,
    Path(version_id): Path<Uuid>,
) -> Result<Json<Value>, WorkLibraryHttpError> {
    Ok(Json(
        state
            .work_library_service()?
            .download_manifest(version_id)
            .await?,
    ))
}

pub(super) async fn download_artifact(
    State(state): State<AppState>,
    Path(artifact_id): Path<Uuid>,
) -> Result<Response<Body>, WorkLibraryHttpError> {
    let validated = state
        .work_library_service()?
        .validate_artifact(artifact_id)
        .await?;
    let file = tokio::fs::File::open(&validated.absolute_path)
        .await
        .map_err(|_| {
            WorkLibraryHttpError(WorkLibraryApplicationError::ArtifactIntegrity {
                artifact_id,
                reason: "校验后重新打开文件失败".into(),
            })
        })?;
    let mut response = Response::new(Body::from_stream(ReaderStream::new(file)));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&validated.artifact.mime_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&validated.artifact.size_bytes.to_string()).unwrap(),
    );
    let safe_name = validated.artifact.file_name.replace(['\r', '\n', '"'], "_");
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{safe_name}\"")).unwrap(),
    );
    Ok(response)
}

pub(super) async fn production_package(
    State(state): State<AppState>,
    Path(version_id): Path<Uuid>,
) -> Result<Response<Body>, WorkLibraryHttpError> {
    let package = state
        .work_library_service()?
        .production_package(version_id)
        .await?;
    let bytes = serde_json::to_vec_pretty(&package).map_err(|error| {
        WorkLibraryHttpError(WorkLibraryApplicationError::Validation(error.to_string()))
    })?;
    let mut response = Response::new(Body::from(bytes));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "attachment; filename=\"work-{version_id}-package.json\""
        ))
        .unwrap(),
    );
    Ok(response)
}

pub(super) async fn create_handoff(
    State(state): State<AppState>,
    Path(version_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<
    (
        StatusCode,
        Json<crate::repositories::WorkPublicationHandoff>,
    ),
    WorkLibraryHttpError,
> {
    let key = idempotency_key(&headers, "发布草稿交接")?;
    let result = state
        .work_library_service()?
        .create_handoff(version_id, key)
        .await?;
    Ok((
        if result.created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(result),
    ))
}

fn idempotency_key<'a>(
    headers: &'a HeaderMap,
    operation: &str,
) -> Result<&'a str, WorkLibraryHttpError> {
    headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            WorkLibraryHttpError(WorkLibraryApplicationError::Validation(format!(
                "{operation}必须提供 Idempotency-Key"
            )))
        })
}

#[derive(Debug)]
pub(super) struct WorkLibraryHttpError(WorkLibraryApplicationError);

impl From<WorkLibraryApplicationError> for WorkLibraryHttpError {
    fn from(value: WorkLibraryApplicationError) -> Self {
        Self(value)
    }
}

impl From<crate::bootstrap::AppStateError> for WorkLibraryHttpError {
    fn from(value: crate::bootstrap::AppStateError) -> Self {
        Self(WorkLibraryApplicationError::Validation(value.to_string()))
    }
}

impl IntoResponse for WorkLibraryHttpError {
    fn into_response(self) -> axum::response::Response {
        let (status, code, message) = match self.0 {
            WorkLibraryApplicationError::Repository(WorkLibraryRepositoryError::NotFound(
                message,
            )) => (StatusCode::NOT_FOUND, "work_library_not_found", message),
            WorkLibraryApplicationError::Repository(WorkLibraryRepositoryError::StaleDiff) => (
                StatusCode::CONFLICT,
                "work_diff_stale",
                "差异计划已过期，请重新分析".into(),
            ),
            WorkLibraryApplicationError::Repository(WorkLibraryRepositoryError::Conflict(
                message,
            )) => (StatusCode::CONFLICT, "work_library_conflict", message),
            WorkLibraryApplicationError::Repository(WorkLibraryRepositoryError::Database(
                error,
            )) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "work_library_database_error",
                error.to_string(),
            ),
            WorkLibraryApplicationError::Validation(message) => {
                (StatusCode::BAD_REQUEST, "work_library_validation", message)
            }
            WorkLibraryApplicationError::ArtifactIntegrity {
                artifact_id,
                reason,
            } => (
                StatusCode::CONFLICT,
                "work_artifact_integrity_failed",
                format!("产物 {artifact_id}: {reason}"),
            ),
        };
        (status, Json(json!({"error": message, "code": code}))).into_response()
    }
}
