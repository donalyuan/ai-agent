use crate::bootstrap::{AppState, AppStateError};
use crate::repositories::{
    ContextAuditListFilter, ContextAuditRecord, ContextAuditRepositoryError,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use novex_agent::AuditedCallOwner;
use novex_ai_core::{canonical_json, sha256_hex, CompileFailureStage};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

const DEFAULT_LIMIT: i64 = 100;
const MAX_LIMIT: i64 = 500;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/contexts", get(list_contexts))
        .route("/contexts/:context_id", get(get_context))
        .route("/contexts/:context_id/export", get(export_context))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextListQuery {
    owner_type: Option<String>,
    owner_id: Option<Uuid>,
    record_type: Option<String>,
    node_key: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

impl ContextListQuery {
    fn parse(self) -> Result<(ContextAuditListFilter, i64, i64), ContextApiError> {
        let owner = match (self.owner_type.as_deref(), self.owner_id) {
            (None, None) => None,
            (Some("conversation"), Some(id)) => Some(AuditedCallOwner::Conversation(id)),
            (Some("agent_run"), Some(id)) => Some(AuditedCallOwner::AgentRun(id)),
            (Some("eval_run"), Some(id)) => Some(AuditedCallOwner::EvalRun(id)),
            _ => {
                return Err(ContextApiError::bad_request(
                    "owner_type 与 owner_id 必须成对且类型有效",
                ))
            }
        };
        if self
            .record_type
            .as_deref()
            .is_some_and(|value| value != "snapshot" && value != "compile_attempt")
        {
            return Err(ContextApiError::bad_request("record_type 无效"));
        }
        let limit = self.limit.unwrap_or(DEFAULT_LIMIT);
        let offset = self.offset.unwrap_or(0);
        if !(1..=MAX_LIMIT).contains(&limit) || offset < 0 {
            return Err(ContextApiError::bad_request("Context 分页参数无效"));
        }
        Ok((
            ContextAuditListFilter {
                owner,
                record_type: self.record_type,
                node_key: self
                    .node_key
                    .and_then(|value| (!value.trim().is_empty()).then(|| value.trim().into())),
            },
            limit,
            offset,
        ))
    }
}

async fn list_contexts(
    State(state): State<AppState>,
    Query(query): Query<ContextListQuery>,
) -> Result<Json<Value>, ContextApiError> {
    let (filter, limit, offset) = query.parse()?;
    let (records, total) = state
        .context_audit_repository()?
        .list(&filter, limit, offset)
        .await?;
    Ok(Json(json!({
        "schema_version":"2", "source_runtime":"rust",
        "items": records.iter().map(summary).collect::<Vec<_>>(),
        "total":total, "limit":limit, "offset":offset,
    })))
}

async fn get_context(
    State(state): State<AppState>,
    Path(context_id): Path<Uuid>,
) -> Result<Json<Value>, ContextApiError> {
    Ok(Json(detail(&state, context_id).await?))
}

async fn export_context(
    State(state): State<AppState>,
    Path(context_id): Path<Uuid>,
) -> Result<Json<Value>, ContextApiError> {
    Ok(Json(detail(&state, context_id).await?))
}

async fn detail(state: &AppState, id: Uuid) -> Result<Value, ContextApiError> {
    let repository = state.context_audit_repository()?;
    let record = match repository.get_snapshot(id).await {
        Ok(record) => ContextAuditRecord::Snapshot(Box::new(record)),
        Err(ContextAuditRepositoryError::SnapshotNotFound(_)) => {
            ContextAuditRecord::CompileAttempt(Box::new(repository.get_attempt(id).await?))
        }
        Err(error) => return Err(error.into()),
    };
    let record = record_value(&record);
    Ok(json!({
        "schema_version":"2",
        "source_runtime":"rust",
        "record_hash":sha256_hex(canonical_json(&record).as_bytes()),
        "record":record,
    }))
}

fn summary(record: &ContextAuditRecord) -> Value {
    let value = record_value(record);
    json!({
        "id":value["id"], "record_type":value["record_type"], "owner":value["owner"],
        "node_key":value["node_key"], "status":value["status"], "compiled_at":value["compiled_at"],
        "policy":value["policy"], "tokenizer_profile":value["tokenizer_profile"],
        "digest":value["digest"], "created_at":value["created_at"],
    })
}

fn record_value(record: &ContextAuditRecord) -> Value {
    match record {
        ContextAuditRecord::Snapshot(record) => json!({
            "id":record.id, "record_type":"snapshot", "owner":owner(record.owner),
            "node_key":record.snapshot.node_key, "status":"succeeded",
            "compiled_at":record.snapshot.compiled_at,
            "policy":{"key":record.snapshot.policy_key,"version":record.snapshot.policy_version},
            "tokenizer_profile":{"key":record.snapshot.tokenizer_profile_key,"version":record.snapshot.tokenizer_profile_version},
            "tokenizer_mode":record.snapshot.tokenizer_mode, "budget":record.snapshot.budget,
            "decisions":record.snapshot.decisions, "selected_order":record.snapshot.selected_order,
            "logical_input":record.snapshot.logical_input, "digest":record.snapshot.digest,
            "created_at":record.created_at,
        }),
        ContextAuditRecord::CompileAttempt(record) => json!({
            "id":record.id, "record_type":"compile_attempt", "owner":owner(record.owner),
            "node_key":record.attempt.node_key, "status":"failed", "compiled_at":record.attempt.compiled_at,
            "policy":Value::Null, "tokenizer_profile":Value::Null,
            "stage":stage(record.attempt.stage), "code":record.attempt.code,
            "budget":record.attempt.budget, "decisions":record.attempt.decisions,
            "digest":record.attempt.digest, "created_at":record.created_at,
        }),
    }
}

fn owner(owner: AuditedCallOwner) -> Value {
    match owner {
        AuditedCallOwner::Conversation(id) => json!({"type":"conversation","id":id}),
        AuditedCallOwner::AgentRun(id) => json!({"type":"agent_run","id":id}),
        AuditedCallOwner::EvalRun(id) => json!({"type":"eval_run","id":id}),
    }
}

fn stage(stage: CompileFailureStage) -> &'static str {
    match stage {
        CompileFailureStage::Schema => "schema",
        CompileFailureStage::Eligibility => "eligibility",
        CompileFailureStage::Conflict => "conflict",
        CompileFailureStage::Tokenizer => "tokenizer",
        CompileFailureStage::Budget => "budget",
        CompileFailureStage::Finalize => "finalize",
    }
}

struct ContextApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ContextApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: message.into(),
        }
    }
}

impl From<AppStateError> for ContextApiError {
    fn from(error: AppStateError) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "storage_unavailable",
            message: error.to_string(),
        }
    }
}

impl From<ContextAuditRepositoryError> for ContextApiError {
    fn from(error: ContextAuditRepositoryError) -> Self {
        let not_found = matches!(
            error,
            ContextAuditRepositoryError::SnapshotNotFound(_)
                | ContextAuditRepositoryError::AttemptNotFound(_)
        );
        Self {
            status: if not_found {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            },
            code: if not_found {
                "not_found"
            } else {
                "internal_error"
            },
            message: error.to_string(),
        }
    }
}

impl IntoResponse for ContextApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({"error":{"code":self.code,"message":self.message}})),
        )
            .into_response()
    }
}
