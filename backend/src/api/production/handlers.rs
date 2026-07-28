//! Production API 处理函数

use super::dto::*;
use crate::bootstrap::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use novex_production_crew::durable::package::GateDecision;
use novex_production_crew::durable::repository::ProductionActor;
use novex_production_crew::orchestrator::fast_lane::{
    execute_fast_lane as crew_fast_lane, FastLaneRequest as CrewFastLaneRequest,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

/// 列表查询参数
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub project_type: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

fn default_page() -> i64 {
    1
}
fn default_page_size() -> i64 {
    20
}

fn idempotency_key(headers: &HeaderMap) -> Result<String, axum::response::Response> {
    headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "idempotency_key_required",
                    "message": "Idempotency-Key 请求头不能为空"
                })),
            )
                .into_response()
        })
}

/// 产物类型查询参数
#[derive(Debug, Deserialize)]
pub struct ArtifactQuery {
    #[serde(default)]
    pub version: Option<i32>,
    #[serde(default)]
    pub character_id: Option<String>,
    #[serde(default)]
    pub shot_id: Option<String>,
}

/// 建议列表查询参数
#[derive(Debug, Deserialize)]
pub struct SuggestionQuery {
    #[serde(default)]
    pub to_role: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

/// POST /api/v1/production/intents
pub async fn create_production_intent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateProductionIntentRequest>,
) -> impl IntoResponse {
    let key = match idempotency_key(&headers) {
        Ok(key) => key,
        Err(response) => return response,
    };
    let service = match state.production_workflow_service() {
        Ok(service) => service,
        Err(error) => return application_service_error(error).into_response(),
    };
    match service
        .create_intent(
            req.project_id,
            req.topic_id,
            req.title,
            req.description,
            req.initial_input,
            key,
        )
        .await
    {
        Ok(intent) => (StatusCode::CREATED, Json(json!({"intent": intent}))).into_response(),
        Err(error) => production_error_response(error).into_response(),
    }
}

/// POST /api/v1/production/intents/:intent_id/runs
pub async fn start_production_run(
    State(state): State<AppState>,
    Path(intent_id): Path<Uuid>,
    headers: HeaderMap,
    Json(_req): Json<StartProductionRunRequest>,
) -> impl IntoResponse {
    let key = match idempotency_key(&headers) {
        Ok(key) => key,
        Err(response) => return response,
    };
    let service = match state.production_workflow_service() {
        Ok(service) => service,
        Err(error) => return application_service_error(error).into_response(),
    };
    match service.start_run(intent_id, key).await {
        Ok(run) => (
            StatusCode::ACCEPTED,
            Json(json!({"status": "accepted", "run": run})),
        )
            .into_response(),
        Err(error) => production_error_response(error).into_response(),
    }
}

/// GET /api/v1/production/intents/:intent_id
pub async fn get_production_intent(
    State(state): State<AppState>,
    Path(intent_id): Path<Uuid>,
) -> impl IntoResponse {
    let service = match state.production_workflow_service() {
        Ok(service) => service,
        Err(error) => return application_service_error(error).into_response(),
    };
    match service.get_intent(intent_id).await {
        Ok(intent) => (StatusCode::OK, Json(json!({"intent": intent}))).into_response(),
        Err(error) => production_error_response(error).into_response(),
    }
}

/// DELETE /api/v1/production/intents/:intent_id
pub async fn delete_production_intent(
    State(state): State<AppState>,
    Path(intent_id): Path<Uuid>,
    headers: HeaderMap,
    Json(_req): Json<EmptyProductionCommandRequest>,
) -> impl IntoResponse {
    let key = match idempotency_key(&headers) {
        Ok(key) => key,
        Err(response) => return response,
    };
    let service = match state.production_workflow_service() {
        Ok(service) => service,
        Err(error) => return application_service_error(error).into_response(),
    };
    match service.delete_intent(intent_id, key).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => production_error_response(error).into_response(),
    }
}

/// POST /api/v1/production/intents/:intent_id/archive
pub async fn archive_production_intent(
    State(state): State<AppState>,
    Path(intent_id): Path<Uuid>,
    headers: HeaderMap,
    Json(_req): Json<EmptyProductionCommandRequest>,
) -> impl IntoResponse {
    let key = match idempotency_key(&headers) {
        Ok(key) => key,
        Err(response) => return response,
    };
    let service = match state.production_workflow_service() {
        Ok(service) => service,
        Err(error) => return application_service_error(error).into_response(),
    };
    match service.archive_intent(intent_id, key).await {
        Ok(intent) => (StatusCode::OK, Json(json!({"intent": intent}))).into_response(),
        Err(error) => production_error_response(error).into_response(),
    }
}

/// GET /api/v1/production/runs/:run_id
pub async fn get_production_run(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
) -> impl IntoResponse {
    let service = match state.production_workflow_service() {
        Ok(service) => service,
        Err(error) => return application_service_error(error).into_response(),
    };
    match service.get_run(run_id).await {
        Ok(view) => (StatusCode::OK, Json(json!(view))).into_response(),
        Err(error) => production_error_response(error).into_response(),
    }
}

/// POST /api/v1/production/runs/:run_id/cancel
pub async fn cancel_production_run(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<CancelProductionRunRequest>,
) -> impl IntoResponse {
    let key = match idempotency_key(&headers) {
        Ok(key) => key,
        Err(response) => return response,
    };
    let service = match state.production_cancellation_service() {
        Ok(service) => service,
        Err(error) => return application_service_error(error).into_response(),
    };
    match service
        .cancel(run_id, ProductionActor::local_operator(), &key, &req.reason)
        .await
    {
        Ok(run) => (
            StatusCode::ACCEPTED,
            Json(json!({"status": "accepted", "run": run})),
        )
            .into_response(),
        Err(error) => production_error_response(error).into_response(),
    }
}

/// POST /api/v1/production/runs/:run_id/packages/:digest/approve
pub async fn approve_package(
    State(state): State<AppState>,
    Path((run_id, digest)): Path<(Uuid, String)>,
    headers: HeaderMap,
    Json(req): Json<ApprovePackageRequest>,
) -> impl IntoResponse {
    decide_package(
        state,
        run_id,
        digest,
        GateDecision::Approve,
        req.note,
        Vec::new(),
        headers,
    )
    .await
}

/// POST /api/v1/production/runs/:run_id/packages/:digest/reject
pub async fn reject_package(
    State(state): State<AppState>,
    Path((run_id, digest)): Path<(Uuid, String)>,
    headers: HeaderMap,
    Json(req): Json<RejectPackageRequest>,
) -> impl IntoResponse {
    decide_package(
        state,
        run_id,
        digest,
        GateDecision::Reject,
        Some(req.reason),
        req.affected_owners,
        headers,
    )
    .await
}

async fn decide_package(
    state: AppState,
    run_id: Uuid,
    digest: String,
    decision: GateDecision,
    reason: Option<String>,
    affected_owners: Vec<String>,
    headers: HeaderMap,
) -> axum::response::Response {
    let key = match idempotency_key(&headers) {
        Ok(key) => key,
        Err(response) => return response,
    };
    let service = match state.production_workflow_service() {
        Ok(service) => service,
        Err(error) => return application_service_error(error).into_response(),
    };
    match service
        .decide_package(run_id, digest, decision, reason, affected_owners, key)
        .await
    {
        Ok(decision) => (StatusCode::OK, Json(json!({"decision": decision}))).into_response(),
        Err(error) => production_error_response(error).into_response(),
    }
}

/// POST /api/v1/production/runs/:run_id/resume
pub async fn resume_production_run(
    State(state): State<AppState>,
    Path(run_id): Path<Uuid>,
    headers: HeaderMap,
    Json(_req): Json<EmptyProductionCommandRequest>,
) -> impl IntoResponse {
    let key = match idempotency_key(&headers) {
        Ok(key) => key,
        Err(response) => return response,
    };
    let service = match state.production_workflow_service() {
        Ok(service) => service,
        Err(error) => return application_service_error(error).into_response(),
    };
    match service.resume_run(run_id, key).await {
        Ok(accepted) => (
            StatusCode::ACCEPTED,
            Json(json!({
                "status": "accepted",
                "run_id": accepted.run_id,
                "step_ids": accepted.step_ids,
            })),
        )
            .into_response(),
        Err(error) => production_error_response(error).into_response(),
    }
}

/// POST /api/v1/production/runs/:run_id/steps/:step_id/retry
pub async fn retry_production_step(
    State(state): State<AppState>,
    Path((run_id, step_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(_req): Json<EmptyProductionCommandRequest>,
) -> impl IntoResponse {
    let key = match idempotency_key(&headers) {
        Ok(key) => key,
        Err(response) => return response,
    };
    let service = match state.production_workflow_service() {
        Ok(service) => service,
        Err(error) => return application_service_error(error).into_response(),
    };
    match service.retry_step(run_id, step_id, key).await {
        Ok(accepted) => (
            StatusCode::ACCEPTED,
            Json(json!({
                "status": "accepted",
                "run_id": accepted.run_id,
                "step_id": step_id,
            })),
        )
            .into_response(),
        Err(error) => production_error_response(error).into_response(),
    }
}

/// POST /api/v1/production/productions
pub async fn create_production(
    State(state): State<AppState>,
    Json(req): Json<CreateProductionRequest>,
) -> impl IntoResponse {
    if req.project_type != "fast_lane" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "durable_intent_required",
                "message": "Full Crew 必须通过 /api/v1/production/intents 创建"
            })),
        )
            .into_response();
    }
    let pool = match state.pg_pool.as_ref() {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "数据库连接不可用"})),
            )
                .into_response()
        }
    };
    let user_id = Uuid::nil();

    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo
        .create_project(
            user_id,
            req.title,
            req.description,
            req.project_type,
            req.initial_input,
        )
        .await
    {
        Ok(project) => (
            StatusCode::CREATED,
            Json(json!({
                "id": project.id,
                "title": project.title,
                "project_type": project.project_type,
                "status": project.status,
                "user_id": project.user_id,
                "created_at": project.created_at,
                "updated_at": project.updated_at
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/v1/production/productions
pub async fn list_productions(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    let pool = match state.pg_pool.as_ref() {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "数据库连接不可用"})),
            )
                .into_response()
        }
    };
    let user_id = Uuid::nil();

    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo
        .list_projects(user_id, query.page, query.page_size)
        .await
    {
        Ok((items, total)) => {
            let items: Vec<_> = items
                .iter()
                .map(|p| {
                    json!({
                        "id": p.id,
                        "title": p.title,
                        "project_type": p.project_type,
                        "status": p.status,
                        "created_at": p.created_at,
                        "updated_at": p.updated_at
                    })
                })
                .collect();
            (
                StatusCode::OK,
                Json(json!({
                    "items": items,
                    "total": total,
                    "page": query.page,
                    "page_size": query.page_size
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/v1/production/productions/:id
pub async fn get_production(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let pool = match state.pg_pool.as_ref() {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "数据库连接不可用"})),
            )
                .into_response()
        }
    };
    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo.get_project(id).await {
        Ok(project) => (
            StatusCode::OK,
            Json(json!({
                "id": project.id,
                "title": project.title,
                "project_type": project.project_type,
                "status": project.status,
                "user_id": project.user_id,
                "created_at": project.created_at,
                "updated_at": project.updated_at
            })),
        )
            .into_response(),
        Err(novex_production_crew::error::ProductionError::ProjectNotFound { .. }) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "制作项目不存在"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// DELETE /api/v1/production/productions/:id
pub async fn delete_production(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let pool = match state.pg_pool.as_ref() {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "数据库连接不可用"})),
            )
                .into_response()
        }
    };
    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo.get_project(id).await {
        Ok(project) if project.project_type == "full_crew" => {
            return (
                StatusCode::CONFLICT,
                Json(json!({
                    "error": "durable_intent_lifecycle_required",
                    "message": "Full Crew 必须通过 intents delete/archive 命令治理生命周期"
                })),
            )
                .into_response();
        }
        Ok(_) => {}
        Err(novex_production_crew::error::ProductionError::ProjectNotFound { .. }) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "制作项目不存在"})),
            )
                .into_response();
        }
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": error.to_string()})),
            )
                .into_response();
        }
    }
    match repo.delete_project(id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(novex_production_crew::error::ProductionError::ProjectNotFound { .. }) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "制作项目不存在"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/v1/production/productions/:id/artifacts/:artifact_type
pub async fn get_artifact(
    State(state): State<AppState>,
    Path((id, artifact_type)): Path<(Uuid, String)>,
    Query(query): Query<ArtifactQuery>,
) -> impl IntoResponse {
    let pool = match state.pg_pool.as_ref() {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "数据库连接不可用"})),
            )
                .into_response()
        }
    };
    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo
        .get_artifact_by_type(
            id,
            &artifact_type,
            query.version,
            query.character_id,
            query.shot_id,
        )
        .await
    {
        Ok(Some(artifact)) => (StatusCode::OK, Json(artifact)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "artifact_not_found",
                "message": format!("产物 {} 不存在", artifact_type)
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// POST /api/v1/production/productions/:id/artifacts/:artifact_type/:artifact_id/approve
pub async fn approve_artifact(
    State(state): State<AppState>,
    Path((id, artifact_type, artifact_id)): Path<(Uuid, String, Uuid)>,
) -> impl IntoResponse {
    let pool = match state.pg_pool.as_ref() {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "数据库连接不可用"})),
            )
                .into_response()
        }
    };
    let user_id = Uuid::nil();
    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo
        .approve_artifact(id, &artifact_type, artifact_id, user_id)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "id": artifact_id,
                "status": "approved",
                "approved_by": user_id
            })),
        )
            .into_response(),
        Err(novex_production_crew::error::ProductionError::ArtifactNotFound { .. }) => {
            (StatusCode::NOT_FOUND, Json(json!({"error": "产物不存在"}))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/v1/production/productions/:id/artifacts/:artifact_type/all
pub async fn list_artifacts(
    State(state): State<AppState>,
    Path((id, artifact_type)): Path<(Uuid, String)>,
) -> impl IntoResponse {
    let pool = match state.pg_pool.as_ref() {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "数据库连接不可用"})),
            )
                .into_response()
        }
    };
    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo.list_artifacts_by_type(id, &artifact_type).await {
        Ok(items) => (StatusCode::OK, Json(json!({ "items": items }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// POST /api/v1/production/productions/:id/suggestions
pub async fn create_suggestion(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    let pool = match state.pg_pool.as_ref() {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "数据库连接不可用"})),
            )
                .into_response()
        }
    };
    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo.create_collaboration_suggestion(id, req).await {
        Ok(suggestion) => (StatusCode::CREATED, Json(suggestion)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/v1/production/productions/:id/suggestions
pub async fn list_suggestions(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<SuggestionQuery>,
) -> impl IntoResponse {
    let pool = match state.pg_pool.as_ref() {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "数据库连接不可用"})),
            )
                .into_response()
        }
    };
    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo
        .list_collaboration_suggestions(id, query.to_role, query.status)
        .await
    {
        Ok((items, total)) => (
            StatusCode::OK,
            Json(json!({
                "items": items,
                "total": total
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// POST /api/v1/production/productions/:id/suggestions/:suggestion_id/respond
pub async fn respond_to_suggestion(
    State(state): State<AppState>,
    Path((id, suggestion_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    let pool = match state.pg_pool.as_ref() {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "数据库连接不可用"})),
            )
                .into_response()
        }
    };
    let user_id = Uuid::nil();
    let status = req
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("accepted")
        .to_string();
    let note = req
        .get("response_note")
        .and_then(|s| s.as_str())
        .map(String::from);

    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo
        .respond_to_suggestion(id, suggestion_id, user_id, status, note)
        .await
    {
        Ok(suggestion) => (StatusCode::OK, Json(suggestion)).into_response(),
        Err(novex_production_crew::error::ProductionError::SuggestionNotFound { .. }) => {
            (StatusCode::NOT_FOUND, Json(json!({"error": "建议不存在"}))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// POST /api/v1/production/productions/:id/fast-lane
pub async fn execute_fast_lane(
    Path(id): Path<Uuid>,
    Json(req): Json<FastLaneRequest>,
) -> impl IntoResponse {
    match crew_fast_lane(
        id,
        CrewFastLaneRequest {
            prompt: req.prompt,
            platform: req.platform,
            duration_seconds: req.duration_seconds,
        },
    )
    .await
    {
        Ok(result) => (
            StatusCode::ACCEPTED,
            Json(json!({
                "job_id": result.job_id,
                "status": "queued",
                "estimated_time_seconds": result.estimated_time_seconds
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /api/v1/production/productions/:id/fast-lane/:job_id
pub async fn get_fast_lane_status(Path((_id, job_id)): Path<(Uuid, Uuid)>) -> impl IntoResponse {
    // TODO: 从 Redis 查询 job 状态
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({
            "job_id": job_id,
            "status": "processing"
        })),
    )
        .into_response()
}

/// GET /api/v1/production/productions/:id/audit-log
pub async fn get_audit_log(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let pool = match state.pg_pool.as_ref() {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "数据库连接不可用"})),
            )
                .into_response()
        }
    };
    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo.get_audit_log(id).await {
        Ok(items) => (StatusCode::OK, Json(json!({ "items": items }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// 将 `ProductionError` 映射到标准化的 HTTP 错误响应。
///
/// HTTP 状态码和 error_code 对应关系见 design.md 错误处理映射章节。
pub fn production_error_response(
    error: novex_production_crew::error::ProductionError,
) -> (StatusCode, Json<serde_json::Value>) {
    use novex_production_crew::error::ProductionError;
    let status = match &error {
        ProductionError::ProjectNotFound { .. }
        | ProductionError::ArtifactNotFound { .. }
        | ProductionError::SuggestionNotFound { .. }
        | ProductionError::RoleNotFound { .. } => StatusCode::NOT_FOUND,
        ProductionError::SourceInvalid { .. }
        | ProductionError::CapabilityMismatch { .. }
        | ProductionError::EvidenceBlocker { .. }
        | ProductionError::InvalidArtifactSchema { .. }
        | ProductionError::MissingInputArtifact { .. }
        | ProductionError::InvalidRoleSequence { .. }
        | ProductionError::BudgetExceeded { .. } => StatusCode::UNPROCESSABLE_ENTITY,
        ProductionError::SourceLocked
        | ProductionError::ActiveIntentConflict
        | ProductionError::RunAlreadyExists
        | ProductionError::IdempotencyConflict
        | ProductionError::TransitionConflict { .. }
        | ProductionError::StalePackage
        | ProductionError::AttentionRequired
        | ProductionError::ExternalWait { .. }
        | ProductionError::GateRejected { .. }
        | ProductionError::GateWaitApproval { .. } => StatusCode::CONFLICT,
        ProductionError::ResourceLimit { .. } => StatusCode::TOO_MANY_REQUESTS,
        ProductionError::Unauthorized { .. } => StatusCode::UNAUTHORIZED,
        ProductionError::AgentExecution(_) => StatusCode::BAD_GATEWAY,
        ProductionError::Database(_)
        | ProductionError::Serialization(_)
        | ProductionError::YamlParse(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    let details = error.details().cloned();
    (
        status,
        Json(json!({
            "error": error.code(),
            "message": error.to_string(),
            "details": details,
        })),
    )
}

fn application_service_error(
    error: crate::bootstrap::AppStateError,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "error": "application_service_unavailable",
            "message": error.to_string(),
        })),
    )
}
