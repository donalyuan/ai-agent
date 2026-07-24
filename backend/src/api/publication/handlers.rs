use crate::application::publication::PublicationApplicationError;
use crate::bootstrap::AppState;
use crate::repositories::{PublicationRepositoryError, SavePublicationTarget};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Deserialize)]
pub(super) struct SaveTargetRequest {
    expected_revision: Option<i32>,
    #[serde(default)]
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    tags: Value,
    cover_artifact_id: Option<Uuid>,
    planned_at: Option<DateTime<Utc>>,
}
#[derive(Deserialize)]
pub(super) struct PublishedRequest {
    published_url: String,
    published_at: DateTime<Utc>,
}
#[derive(Deserialize)]
pub(super) struct PackageRequest {
    draft_revision: i32,
}

pub(super) async fn create_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<crate::repositories::PublicationPlanRecord>), HttpError> {
    key(&headers)?;
    let result = state.publication_service()?.plan(id).await?;
    Ok((
        if result.created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(result),
    ))
}
pub(super) async fn list(State(state): State<AppState>) -> Result<Json<Value>, HttpError> {
    Ok(Json(state.publication_service()?.list().await?))
}
pub(super) async fn details(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, HttpError> {
    Ok(Json(state.publication_service()?.details(id).await?))
}
pub(super) async fn save_target(
    State(state): State<AppState>,
    Path((id, platform)): Path<(Uuid, String)>,
    headers: HeaderMap,
    Json(body): Json<SaveTargetRequest>,
) -> Result<Json<crate::repositories::PublicationTargetRecord>, HttpError> {
    key(&headers)?;
    Ok(Json(
        state
            .publication_service()?
            .save_target(
                id,
                &platform,
                body.expected_revision,
                key(&headers)?,
                SavePublicationTarget {
                    title: body.title,
                    body: body.body,
                    tags: body.tags,
                    cover_artifact_id: body.cover_artifact_id,
                    planned_at: body.planned_at,
                },
            )
            .await?,
    ))
}
pub(super) async fn handoff(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Value>, HttpError> {
    Ok(Json(
        state
            .publication_service()?
            .handoff(id, key(&headers)?)
            .await?,
    ))
}
pub(super) async fn generate_package(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<PackageRequest>,
) -> Result<
    (
        StatusCode,
        Json<crate::repositories::PublicationPackageRecord>,
    ),
    HttpError,
> {
    let result = state
        .publication_service()?
        .generate_package(id, body.draft_revision, key(&headers)?)
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
pub(super) async fn downloads(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, HttpError> {
    Ok(Json(state.publication_service()?.downloads(id).await?))
}
pub(super) async fn audit_download(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, HttpError> {
    state
        .publication_service()?
        .audit(id, "downloaded", key(&headers)?)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn audit_copy(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, HttpError> {
    state
        .publication_service()?
        .audit(id, "copied", key(&headers)?)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn download_package(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response, HttpError> {
    let (package, path) = state.publication_service()?.package_file(id).await?;
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|e| HttpError(PublicationApplicationError::Validation(e.to_string())))?;
    let mut response = Response::new(Body::from_stream(tokio_util::io::ReaderStream::new(file)));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/zip"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "attachment; filename=\"publication-{}.zip\"",
            package.id
        ))
        .unwrap(),
    );
    Ok(response)
}
pub(super) async fn needs_attention(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<crate::repositories::PublicationTargetRecord>, HttpError> {
    Ok(Json(
        state
            .publication_service()?
            .needs_attention(id, key(&headers)?)
            .await?,
    ))
}
pub(super) async fn cancel(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<crate::repositories::PublicationTargetRecord>, HttpError> {
    Ok(Json(
        state
            .publication_service()?
            .cancel(id, key(&headers)?)
            .await?,
    ))
}
pub(super) async fn published(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<PublishedRequest>,
) -> Result<Json<crate::repositories::PublicationTargetRecord>, HttpError> {
    Ok(Json(
        state
            .publication_service()?
            .publish(id, &body.published_url, body.published_at, key(&headers)?)
            .await?,
    ))
}
pub(super) async fn correct(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<PublishedRequest>,
) -> Result<Json<crate::repositories::PublicationTargetRecord>, HttpError> {
    Ok(Json(
        state
            .publication_service()?
            .correct(id, &body.published_url, body.published_at, key(&headers)?)
            .await?,
    ))
}

fn key(headers: &HeaderMap) -> Result<&str, HttpError> {
    headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            HttpError(PublicationApplicationError::Validation(
                "必须提供 Idempotency-Key".into(),
            ))
        })
}
pub(super) struct HttpError(PublicationApplicationError);
impl From<PublicationApplicationError> for HttpError {
    fn from(v: PublicationApplicationError) -> Self {
        Self(v)
    }
}
impl From<crate::bootstrap::AppStateError> for HttpError {
    fn from(v: crate::bootstrap::AppStateError) -> Self {
        Self(PublicationApplicationError::Validation(v.to_string()))
    }
}
impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self.0 {
            PublicationApplicationError::Validation(v) => {
                (StatusCode::BAD_REQUEST, "publication_validation", v)
            }
            PublicationApplicationError::Repository(PublicationRepositoryError::NotFound(v)) => {
                (StatusCode::NOT_FOUND, "publication_not_found", v)
            }
            PublicationApplicationError::Repository(PublicationRepositoryError::Conflict(v)) => {
                (StatusCode::CONFLICT, "publication_conflict", v)
            }
            PublicationApplicationError::Repository(PublicationRepositoryError::Database(e)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "publication_database_error",
                e.to_string(),
            ),
            PublicationApplicationError::WorkLibrary(e) => (
                StatusCode::CONFLICT,
                "publication_artifact_integrity",
                e.to_string(),
            ),
        };
        (status, Json(json!({"code":code,"error":message}))).into_response()
    }
}
