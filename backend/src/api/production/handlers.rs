//! Production API 处理函数

use super::dto::*;
use crate::bootstrap::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use novex_production_crew::orchestrator::fast_lane::{execute_fast_lane as crew_fast_lane, FastLaneRequest as CrewFastLaneRequest};
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

fn default_page() -> i64 { 1 }
fn default_page_size() -> i64 { 20 }

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

/// POST /api/v1/production/productions
pub async fn create_production(
    State(state): State<AppState>,
    Json(req): Json<CreateProductionRequest>,
) -> impl IntoResponse {
    let pool = match state.pg_pool.as_ref() {
        Some(p) => p.clone(),
        None => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "数据库连接不可用"}))).into_response(),
    };
    let user_id = Uuid::new_v4(); // TODO: 从认证中间件获取真实用户 ID

    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo.create_project(user_id, req.title, req.description, req.project_type, req.initial_input).await {
        Ok(project) => (StatusCode::CREATED, Json(json!({
            "id": project.id,
            "title": project.title,
            "project_type": project.project_type,
            "status": project.status,
            "user_id": project.user_id,
            "created_at": project.created_at,
            "updated_at": project.updated_at
        }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// GET /api/v1/production/productions
pub async fn list_productions(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    let pool = match state.pg_pool.as_ref() {
        Some(p) => p.clone(),
        None => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "数据库连接不可用"}))).into_response(),
    };
    let user_id = Uuid::new_v4(); // TODO: 从认证中间件获取

    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo.list_projects(user_id, query.page, query.page_size).await {
        Ok((items, total)) => {
            let items: Vec<_> = items.iter().map(|p| json!({
                "id": p.id,
                "title": p.title,
                "project_type": p.project_type,
                "status": p.status,
                "created_at": p.created_at,
                "updated_at": p.updated_at
            })).collect();
            (StatusCode::OK, Json(json!({
                "items": items,
                "total": total,
                "page": query.page,
                "page_size": query.page_size
            }))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// GET /api/v1/production/productions/:id
pub async fn get_production(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let pool = match state.pg_pool.as_ref() {
        Some(p) => p.clone(),
        None => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "数据库连接不可用"}))).into_response(),
    };
    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo.get_project(id).await {
        Ok(project) => (StatusCode::OK, Json(json!({
            "id": project.id,
            "title": project.title,
            "project_type": project.project_type,
            "status": project.status,
            "user_id": project.user_id,
            "created_at": project.created_at,
            "updated_at": project.updated_at
        }))).into_response(),
        Err(novex_production_crew::error::ProductionError::ProjectNotFound { .. }) => {
            (StatusCode::NOT_FOUND, Json(json!({"error": "制作项目不存在"}))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// DELETE /api/v1/production/productions/:id
pub async fn delete_production(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let pool = match state.pg_pool.as_ref() {
        Some(p) => p.clone(),
        None => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "数据库连接不可用"}))).into_response(),
    };
    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo.delete_project(id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(novex_production_crew::error::ProductionError::ProjectNotFound { .. }) => {
            (StatusCode::NOT_FOUND, Json(json!({"error": "制作项目不存在"}))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// POST /api/v1/production/productions/:id/roles/:role_key/execute
pub async fn execute_role(
    State(state): State<AppState>,
    Path((id, role_key)): Path<(Uuid, String)>,
    Json(req): Json<ExecuteRoleRequest>,
) -> impl IntoResponse {
    let orchestrator = match state.production_orchestrator() {
        Ok(o) => o,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "internal_error",
                    "message": format!("执行器初始化失败: {}", e)
                })),
            )
                .into_response();
        }
    };

    let request_model_id = req
        .context
        .as_ref()
        .and_then(|c| c.get("preferred_model_id"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<Uuid>().ok());

    match orchestrator
        .execute_role(id, role_key, req.user_input, request_model_id)
        .await
    {
        Ok(result) => (StatusCode::OK, Json(serde_json::to_value(result).unwrap_or_default())).into_response(),
        Err(e) => production_error_response(e).into_response(),
    }
}

/// POST /api/v1/production/productions/:id/execute-flow
pub async fn execute_flow(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
    Json(req): Json<ExecuteFlowRequest>,
) -> impl IntoResponse {
    // TODO: 调用 ProductionOrchestrator.execute_flow()
    let flow_id = Uuid::new_v4();
    (StatusCode::ACCEPTED, Json(json!({
        "flow_id": flow_id,
        "status": "running",
        "completed_roles": [],
        "current_role": req.roles.first(),
        "pending_roles": &req.roles[1..]
    }))).into_response()
}

/// GET /api/v1/production/productions/:id/flows/:flow_id
pub async fn get_flow_status(
    Path((id, flow_id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
    // TODO: 从 Redis 或 DB 查询流程状态
    (StatusCode::NOT_IMPLEMENTED, Json(json!({
        "error": "流程状态查询接口尚在实施中",
        "project_id": id,
        "flow_id": flow_id
    }))).into_response()
}

/// GET /api/v1/production/productions/:id/artifacts/:artifact_type
pub async fn get_artifact(
    State(state): State<AppState>,
    Path((id, artifact_type)): Path<(Uuid, String)>,
    Query(query): Query<ArtifactQuery>,
) -> impl IntoResponse {
    let pool = match state.pg_pool.as_ref() {
        Some(p) => p.clone(),
        None => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "数据库连接不可用"}))).into_response(),
    };
    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo.get_artifact_by_type(id, &artifact_type, query.version, query.character_id, query.shot_id).await {
        Ok(Some(artifact)) => (StatusCode::OK, Json(artifact)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({
            "error": "artifact_not_found",
            "message": format!("产物 {} 不存在", artifact_type)
        }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// POST /api/v1/production/productions/:id/artifacts/:artifact_type/:artifact_id/approve
pub async fn approve_artifact(
    State(state): State<AppState>,
    Path((id, artifact_type, artifact_id)): Path<(Uuid, String, Uuid)>,
) -> impl IntoResponse {
    let pool = match state.pg_pool.as_ref() {
        Some(p) => p.clone(),
        None => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "数据库连接不可用"}))).into_response(),
    };
    let user_id = Uuid::new_v4(); // TODO: 从认证中间件获取
    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo.approve_artifact(id, &artifact_type, artifact_id, user_id).await {
        Ok(()) => (StatusCode::OK, Json(json!({
            "id": artifact_id,
            "status": "approved",
            "approved_by": user_id
        }))).into_response(),
        Err(novex_production_crew::error::ProductionError::ArtifactNotFound { .. }) => {
            (StatusCode::NOT_FOUND, Json(json!({"error": "产物不存在"}))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// GET /api/v1/production/productions/:id/artifacts/:artifact_type/all
pub async fn list_artifacts(
    State(state): State<AppState>,
    Path((id, artifact_type)): Path<(Uuid, String)>,
) -> impl IntoResponse {
    let pool = match state.pg_pool.as_ref() {
        Some(p) => p.clone(),
        None => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "数据库连接不可用"}))).into_response(),
    };
    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo.list_artifacts_by_type(id, &artifact_type).await {
        Ok(items) => (StatusCode::OK, Json(json!({ "items": items }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
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
        None => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "数据库连接不可用"}))).into_response(),
    };
    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo.create_collaboration_suggestion(id, req).await {
        Ok(suggestion) => (StatusCode::CREATED, Json(suggestion)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
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
        None => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "数据库连接不可用"}))).into_response(),
    };
    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo.list_collaboration_suggestions(id, query.to_role, query.status).await {
        Ok((items, total)) => (StatusCode::OK, Json(json!({
            "items": items,
            "total": total
        }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
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
        None => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "数据库连接不可用"}))).into_response(),
    };
    let user_id = Uuid::new_v4(); // TODO: 从认证中间件获取
    let status = req.get("status").and_then(|s| s.as_str()).unwrap_or("accepted").to_string();
    let note = req.get("response_note").and_then(|s| s.as_str()).map(String::from);

    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo.respond_to_suggestion(id, suggestion_id, user_id, status, note).await {
        Ok(suggestion) => (StatusCode::OK, Json(suggestion)).into_response(),
        Err(novex_production_crew::error::ProductionError::SuggestionNotFound { .. }) => {
            (StatusCode::NOT_FOUND, Json(json!({"error": "建议不存在"}))).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// POST /api/v1/production/productions/:id/fast-lane
pub async fn execute_fast_lane(
    Path(id): Path<Uuid>,
    Json(req): Json<FastLaneRequest>,
) -> impl IntoResponse {
    match crew_fast_lane(id, CrewFastLaneRequest {
        prompt: req.prompt,
        platform: req.platform,
        duration_seconds: req.duration_seconds,
    }).await {
        Ok(result) => (StatusCode::ACCEPTED, Json(json!({
            "job_id": result.job_id,
            "status": "queued",
            "estimated_time_seconds": result.estimated_time_seconds
        }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// GET /api/v1/production/productions/:id/fast-lane/:job_id
pub async fn get_fast_lane_status(
    Path((_id, job_id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
    // TODO: 从 Redis 查询 job 状态
    (StatusCode::NOT_IMPLEMENTED, Json(json!({
        "job_id": job_id,
        "status": "processing"
    }))).into_response()
}

/// GET /api/v1/production/productions/:id/audit-log
pub async fn get_audit_log(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let pool = match state.pg_pool.as_ref() {
        Some(p) => p.clone(),
        None => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "数据库连接不可用"}))).into_response(),
    };
    let repo = novex_production_crew::state::ProductionStateRepository::new(pool);
    match repo.get_audit_log(id).await {
        Ok(items) => (StatusCode::OK, Json(json!({ "items": items }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

/// 将 `ProductionError` 映射到标准化的 HTTP 错误响应。
///
/// HTTP 状态码和 error_code 对应关系见 design.md 错误处理映射章节。
pub fn production_error_response(
    error: novex_production_crew::error::ProductionError,
) -> (StatusCode, Json<serde_json::Value>) {
    use novex_production_crew::error::ProductionError;
    match error {
        ProductionError::MissingInputArtifact { artifact_type } => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "missing_input_artifact",
                "message": format!("角色需要 '{}'，但当前无已批准或草稿版本", artifact_type),
                "required_artifacts": [artifact_type]
            })),
        ),
        ProductionError::InvalidArtifactSchema { details } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "error": "invalid_artifact_schema",
                "message": "角色输出不符合产物 schema",
                "details": details
            })),
        ),
        ProductionError::RoleNotFound { role_key } => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "role_not_found",
                "message": format!("角色 '{}' 不存在", role_key)
            })),
        ),
        ProductionError::GateRejected { gate_name, reason } => (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "gate_rejected",
                "message": format!("{}: {}", gate_name, reason)
            })),
        ),
        ProductionError::GateWaitApproval { artifact_id } => (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "waiting_approval",
                "message": "需要人工审批才能继续",
                "artifact_id": artifact_id
            })),
        ),
        ProductionError::AgentExecution(msg) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "error": "agent_execution_failed",
                "message": msg
            })),
        ),
        ProductionError::ProjectNotFound { project_id } => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "project_not_found",
                "message": format!("制作项目 {} 不存在", project_id)
            })),
        ),
        other => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "internal_error",
                "message": other.to_string()
            })),
        ),
    }
}
