use crate::bootstrap::{AppState, AppStateError};
use crate::repositories::{
    ModelCallListFilter, ModelCallOwner, ModelCallRecord, ModelCallRepositoryError, ModelCallStatus,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use novex_ai_core::{canonical_json, sha256_hex, PromptCompileInput, PromptCompiler};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use uuid::Uuid;

const DEFAULT_LIMIT: i64 = 100;
const MAX_LIMIT: i64 = 500;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/model-calls", get(list_model_calls))
        .route("/model-calls/:model_call_id", get(get_model_call))
        .route("/model-calls/:model_call_id/export", get(export_model_call))
        .route(
            "/model-calls/:model_call_id/replay",
            post(replay_model_call),
        )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelCallListQuery {
    owner_type: Option<String>,
    owner_id: Option<Uuid>,
    node_key: Option<String>,
    agent_key: Option<String>,
    agent_version: Option<String>,
    prompt_key: Option<String>,
    prompt_version: Option<String>,
    model_id: Option<Uuid>,
    status: Option<String>,
    prepared_from: Option<DateTime<Utc>>,
    prepared_to: Option<DateTime<Utc>>,
    limit: Option<i64>,
    offset: Option<i64>,
}

impl ModelCallListQuery {
    fn parse(self) -> Result<(ModelCallListFilter, i64, i64), ModelCallApiError> {
        let owner = match (self.owner_type.as_deref(), self.owner_id) {
            (None, None) => None,
            (Some("conversation"), Some(id)) => Some(ModelCallOwner::Conversation(id)),
            (Some("agent_run"), Some(id)) => Some(ModelCallOwner::AgentRun(id)),
            (Some("eval_run"), Some(id)) => Some(ModelCallOwner::EvalRun(id)),
            _ => {
                return Err(ModelCallApiError::bad_request(
                    "owner_type 与 owner_id 必须成对且类型有效",
                ))
            }
        };
        let status = match self.status.as_deref() {
            None => None,
            Some("prepared") => Some(ModelCallStatus::Prepared),
            Some("succeeded") => Some(ModelCallStatus::Succeeded),
            Some("failed") => Some(ModelCallStatus::Failed),
            Some("aborted") => Some(ModelCallStatus::Aborted),
            Some(_) => return Err(ModelCallApiError::bad_request("status 无效")),
        };
        let limit = self.limit.unwrap_or(DEFAULT_LIMIT);
        let offset = self.offset.unwrap_or(0);
        if !(1..=MAX_LIMIT).contains(&limit) || offset < 0 {
            return Err(ModelCallApiError::bad_request("ModelCall 分页参数无效"));
        }
        if self
            .prepared_from
            .zip(self.prepared_to)
            .is_some_and(|(from, to)| from > to)
        {
            return Err(ModelCallApiError::bad_request(
                "prepared_from 不得晚于 prepared_to",
            ));
        }
        Ok((
            ModelCallListFilter {
                owner,
                node_key: normalized(self.node_key),
                agent_key: normalized(self.agent_key),
                agent_version: normalized(self.agent_version),
                prompt_key: normalized(self.prompt_key),
                prompt_version: normalized(self.prompt_version),
                model_id: self.model_id,
                status,
                prepared_from: self.prepared_from,
                prepared_to: self.prepared_to,
            },
            limit,
            offset,
        ))
    }
}

fn normalized(value: Option<String>) -> Option<String> {
    value.and_then(|value| (!value.trim().is_empty()).then(|| value.trim().to_string()))
}

#[derive(Clone, Debug, Serialize)]
struct OwnerDto {
    r#type: &'static str,
    id: Uuid,
}

#[derive(Clone, Debug, Serialize)]
struct ExecutionDto {
    phase: Option<String>,
    entry_id: Option<Uuid>,
    step_id: Option<Uuid>,
}

#[derive(Clone, Debug, Serialize)]
struct DefinitionDto {
    agent_key: String,
    agent_version: String,
    prompt_key: String,
    prompt_version: String,
    registry_digest: String,
}

#[derive(Clone, Debug, Serialize)]
struct SummaryModelDto {
    id: Uuid,
    behavior_fingerprint: String,
}

#[derive(Clone, Debug, Serialize)]
struct DetailModelDto {
    id: Uuid,
    behavior_fingerprint: String,
    snapshot: Value,
}

#[derive(Clone, Debug, Serialize)]
struct UsageSummaryDto {
    input_tokens: Option<Value>,
    output_tokens: Option<Value>,
    total_tokens: Option<Value>,
    cost_usd: Option<Value>,
}

#[derive(Clone, Debug, Serialize)]
struct ModelCallSummaryDto {
    id: Uuid,
    owner: OwnerDto,
    execution: ExecutionDto,
    node_key: String,
    attempt: i32,
    status: &'static str,
    definition: DefinitionDto,
    model: SummaryModelDto,
    usage: UsageSummaryDto,
    prepared_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize)]
struct ModelCallRecordDto {
    id: Uuid,
    owner: OwnerDto,
    execution: ExecutionDto,
    root_call_id: Option<Uuid>,
    parent_call_id: Option<Uuid>,
    node_key: String,
    attempt: i32,
    status: &'static str,
    definition: DefinitionDto,
    prompt_snapshot: Value,
    context_sources: Value,
    memory_sources: Value,
    tool_schema: Option<Value>,
    model: DetailModelDto,
    parameters: Value,
    asset_references: Value,
    output_snapshot: Option<Value>,
    usage_snapshot: Option<Value>,
    error_snapshot: Option<Value>,
    structured_parse_status: Option<String>,
    prepared_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
struct ListResponse {
    schema_version: &'static str,
    source_runtime: &'static str,
    items: Vec<ModelCallSummaryDto>,
    total: i64,
    limit: i64,
    offset: i64,
}

#[derive(Clone, Serialize)]
struct DetailResponse {
    schema_version: &'static str,
    source_runtime: &'static str,
    record_hash: String,
    record: ModelCallRecordDto,
}

async fn list_model_calls(
    State(state): State<AppState>,
    Query(query): Query<ModelCallListQuery>,
) -> Result<Json<ListResponse>, ModelCallApiError> {
    let (filter, limit, offset) = query.parse()?;
    let (records, total) = state
        .model_call_repository()?
        .list(&filter, limit, offset)
        .await?;
    Ok(Json(ListResponse {
        schema_version: "1",
        source_runtime: "rust",
        items: records.into_iter().map(summary_dto).collect(),
        total,
        limit,
        offset,
    }))
}

async fn get_model_call(
    State(state): State<AppState>,
    Path(model_call_id): Path<Uuid>,
) -> Result<Json<DetailResponse>, ModelCallApiError> {
    Ok(Json(detail_response(
        state.model_call_repository()?.get(model_call_id).await?,
    )?))
}

async fn export_model_call(
    State(state): State<AppState>,
    Path(model_call_id): Path<Uuid>,
) -> Result<Json<DetailResponse>, ModelCallApiError> {
    Ok(Json(detail_response(
        state.model_call_repository()?.get(model_call_id).await?,
    )?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplayRequest {
    mode: Option<String>,
}

async fn replay_model_call(
    State(state): State<AppState>,
    Path(model_call_id): Path<Uuid>,
    Json(request): Json<ReplayRequest>,
) -> Result<Json<Value>, ModelCallApiError> {
    if request
        .mode
        .as_deref()
        .is_some_and(|mode| mode != "dry_run")
    {
        return Err(ModelCallApiError::bad_request(
            "replay 仅支持 dry_run；真实对比必须创建 EvalRun",
        ));
    }
    let record = state.model_call_repository()?.get(model_call_id).await?;
    let detail = detail_response(record.clone())?;
    let registry = state.definition_registry()?;
    let historical = serde_json::from_value::<ReplayPromptSnapshot>(record.prompt_snapshot.clone());
    let (definition_resolved, compile_succeeded, diff) = match historical {
        Ok(historical) => {
            let definition_resolved = registry
                .agent(&record.agent_key, &record.agent_version)
                .is_ok()
                && registry.prompts().iter().any(|prompt| {
                    prompt.prompt_key == record.prompt_key
                        && prompt.version == record.prompt_version
                });
            if !definition_resolved {
                (
                    false,
                    false,
                    vec![json!({"path":"prompt_definition","kind":"missing"})],
                )
            } else {
                let input = PromptCompileInput {
                    schema_version: "1".into(),
                    variables: historical
                        .variables
                        .into_iter()
                        .filter(|(key, _)| key != "fragments")
                        .collect(),
                    fragments: historical.fragments,
                };
                match PromptCompiler::new(&registry).compile_for_replay(
                    &record.agent_key,
                    &record.agent_version,
                    &record.node_key,
                    input,
                    &historical.tool_profile,
                    historical.tool_schema,
                ) {
                    Ok(recompiled) => {
                        let current = serde_json::to_value(recompiled)
                            .map_err(|error| ModelCallApiError::internal(error.to_string()))?;
                        (
                            true,
                            true,
                            structured_diff(&record.prompt_snapshot, &current),
                        )
                    }
                    Err(error) => (
                        true,
                        false,
                        vec![json!({"path":"compile","kind":"error","message":error.to_string()})],
                    ),
                }
            }
        }
        Err(error) => (
            false,
            false,
            vec![json!({"path":"prompt_snapshot","kind":"invalid","message":error.to_string()})],
        ),
    };
    Ok(Json(json!({
        "schema_version":"1",
        "mode":"dry_run",
        "source_model_call_id":model_call_id,
        "source_record_hash":detail.record_hash,
        "definition_resolved":definition_resolved,
        "compile_succeeded":compile_succeeded,
        "side_effects":{"model_calls":0,"tools":0,"session_writes":0,"run_writes":0,"domain_writes":0},
        "diff":diff,
    })))
}

#[derive(Deserialize)]
struct ReplayPromptSnapshot {
    variables: Map<String, Value>,
    fragments: Vec<novex_ai_core::DynamicFragment>,
    tool_profile: String,
    tool_schema: Option<Value>,
}

fn detail_response(record: ModelCallRecord) -> Result<DetailResponse, ModelCallApiError> {
    let record = record_dto(record);
    let value = serde_json::to_value(&record)
        .map_err(|error| ModelCallApiError::internal(error.to_string()))?;
    Ok(DetailResponse {
        schema_version: "1",
        source_runtime: "rust",
        record_hash: sha256_hex(canonical_json(&value).as_bytes()),
        record,
    })
}

fn owner_dto(owner: ModelCallOwner) -> OwnerDto {
    match owner {
        ModelCallOwner::Conversation(id) => OwnerDto {
            r#type: "conversation",
            id,
        },
        ModelCallOwner::AgentRun(id) => OwnerDto {
            r#type: "agent_run",
            id,
        },
        ModelCallOwner::EvalRun(id) => OwnerDto {
            r#type: "eval_run",
            id,
        },
    }
}

fn definition_dto(record: &ModelCallRecord) -> DefinitionDto {
    DefinitionDto {
        agent_key: record.agent_key.clone(),
        agent_version: record.agent_version.clone(),
        prompt_key: record.prompt_key.clone(),
        prompt_version: record.prompt_version.clone(),
        registry_digest: record.registry_digest.clone(),
    }
}

fn execution_dto(record: &ModelCallRecord) -> ExecutionDto {
    ExecutionDto {
        phase: None,
        entry_id: None,
        step_id: record.agent_step_id,
    }
}

fn summary_dto(record: ModelCallRecord) -> ModelCallSummaryDto {
    ModelCallSummaryDto {
        id: record.id,
        owner: owner_dto(record.owner),
        execution: execution_dto(&record),
        node_key: record.node_key.clone(),
        attempt: record.attempt,
        status: record.status.as_str(),
        definition: definition_dto(&record),
        model: SummaryModelDto {
            id: record.model_id,
            behavior_fingerprint: record.behavior_fingerprint.clone(),
        },
        usage: usage_summary(record.usage_snapshot.as_ref()),
        prepared_at: record.prepared_at,
        completed_at: record.completed_at,
    }
}

fn record_dto(record: ModelCallRecord) -> ModelCallRecordDto {
    ModelCallRecordDto {
        id: record.id,
        owner: owner_dto(record.owner),
        execution: execution_dto(&record),
        root_call_id: record.root_call_id,
        parent_call_id: record.parent_call_id,
        node_key: record.node_key.clone(),
        attempt: record.attempt,
        status: record.status.as_str(),
        definition: definition_dto(&record),
        prompt_snapshot: record.prompt_snapshot,
        context_sources: record.context_sources,
        memory_sources: record.memory_sources,
        tool_schema: record.tool_schema,
        model: DetailModelDto {
            id: record.model_id,
            behavior_fingerprint: record.behavior_fingerprint,
            snapshot: record.model_snapshot,
        },
        parameters: record.parameters,
        asset_references: record.asset_references,
        output_snapshot: record.output_snapshot,
        usage_snapshot: record.usage_snapshot,
        error_snapshot: record.error_snapshot,
        structured_parse_status: record.structured_parse_status,
        prepared_at: record.prepared_at,
        completed_at: record.completed_at,
    }
}

fn usage_summary(usage: Option<&Value>) -> UsageSummaryDto {
    let field = |names: &[&str]| {
        names.iter().find_map(|name| {
            usage
                .and_then(|value| value.get(name))
                .filter(|value| value.is_number())
                .cloned()
        })
    };
    let cost = usage
        .and_then(|value| value.get("cost"))
        .and_then(|value| value.get("total"))
        .filter(|value| value.is_number())
        .cloned()
        .or_else(|| field(&["cost_usd", "cost"]));
    UsageSummaryDto {
        input_tokens: field(&["input_tokens", "input"]),
        output_tokens: field(&["output_tokens", "output"]),
        total_tokens: field(&["total_tokens", "totalTokens"]),
        cost_usd: cost,
    }
}

fn structured_diff(historical: &Value, current: &Value) -> Vec<Value> {
    let mut diff = Vec::new();
    collect_diff("", historical, current, &mut diff);
    diff
}

fn collect_diff(path: &str, historical: &Value, current: &Value, diff: &mut Vec<Value>) {
    match (historical, current) {
        (Value::Object(left), Value::Object(right)) => {
            let keys = left
                .keys()
                .chain(right.keys())
                .collect::<std::collections::BTreeSet<_>>();
            for key in keys {
                let next = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{path}.{key}")
                };
                match (left.get(key), right.get(key)) {
                    (Some(left), Some(right)) => collect_diff(&next, left, right, diff),
                    (Some(value), None) => {
                        diff.push(json!({"path":next,"kind":"removed","historical":value}))
                    }
                    (None, Some(value)) => {
                        diff.push(json!({"path":next,"kind":"added","current":value}))
                    }
                    (None, None) => {}
                }
            }
        }
        (Value::Array(left), Value::Array(right)) if left.len() == right.len() => {
            for (index, (left, right)) in left.iter().zip(right).enumerate() {
                collect_diff(&format!("{path}[{index}]"), left, right, diff);
            }
        }
        _ if historical != current => diff.push(json!({
            "path":path,
            "kind":"changed",
            "historical":historical,
            "current":current,
        })),
        _ => {}
    }
}

#[derive(Debug)]
struct ModelCallApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ModelCallApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            message: message.into(),
        }
    }
}

impl From<AppStateError> for ModelCallApiError {
    fn from(error: AppStateError) -> Self {
        Self::internal(error.to_string())
    }
}

impl From<ModelCallRepositoryError> for ModelCallApiError {
    fn from(error: ModelCallRepositoryError) -> Self {
        match error {
            ModelCallRepositoryError::NotFound(_) => Self {
                status: StatusCode::NOT_FOUND,
                code: "not_found",
                message: "ModelCall 不存在".into(),
            },
            other => Self::internal(other.to_string()),
        }
    }
}

impl IntoResponse for ModelCallApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({"error":{"code":self.code,"message":self.message}})),
        )
            .into_response()
    }
}
