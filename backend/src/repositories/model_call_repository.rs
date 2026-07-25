use async_trait::async_trait;
use chrono::{DateTime, Utc};
use novex_agent::{
    AuditedCallOwner, AuditedTerminalStatus, FinishAuditedCall, ModelCallAuditStore,
    PrepareAuditedCall,
};
use novex_ai_core::{
    redact_audit_value, validate_asset_references, validate_audit_payload,
    MODEL_CALL_SCHEMA_VERSION,
};
use serde_json::Value;
use sqlx::{postgres::PgRow, PgPool, Postgres, QueryBuilder, Row};
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelCallOwner {
    Conversation(Uuid),
    AgentRun(Uuid),
    EvalRun(Uuid),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelCallStatus {
    Prepared,
    Succeeded,
    Failed,
    Aborted,
}

impl ModelCallStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Aborted => "aborted",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelCallTerminalStatus {
    Succeeded,
    Failed,
    Aborted,
}

impl ModelCallTerminalStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Aborted => "aborted",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrepareModelCall {
    pub owner: ModelCallOwner,
    pub root_call_id: Option<Uuid>,
    pub parent_call_id: Option<Uuid>,
    pub node_key: String,
    pub attempt: i32,
    pub agent_key: String,
    pub agent_version: String,
    pub prompt_key: String,
    pub prompt_version: String,
    pub registry_digest: String,
    pub prompt_snapshot: Value,
    pub context_sources: Value,
    pub memory_sources: Value,
    pub tool_schema: Option<Value>,
    pub model_id: Uuid,
    pub behavior_fingerprint: String,
    pub model_snapshot: Value,
    pub parameters: Value,
    pub asset_references: Value,
    pub known_secrets: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FinishModelCall {
    pub id: Uuid,
    pub status: ModelCallTerminalStatus,
    pub output_snapshot: Option<Value>,
    pub usage_snapshot: Option<Value>,
    pub error_snapshot: Option<Value>,
    pub structured_parse_status: Option<String>,
    pub known_secrets: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModelCallRecord {
    pub id: Uuid,
    pub schema_version: String,
    pub owner: ModelCallOwner,
    pub agent_step_id: Option<Uuid>,
    pub root_call_id: Option<Uuid>,
    pub parent_call_id: Option<Uuid>,
    pub node_key: String,
    pub attempt: i32,
    pub status: ModelCallStatus,
    pub agent_key: String,
    pub agent_version: String,
    pub prompt_key: String,
    pub prompt_version: String,
    pub registry_digest: String,
    pub prompt_snapshot: Value,
    pub context_sources: Value,
    pub memory_sources: Value,
    pub tool_schema: Option<Value>,
    pub model_id: Uuid,
    pub behavior_fingerprint: String,
    pub model_snapshot: Value,
    pub parameters: Value,
    pub asset_references: Value,
    pub output_snapshot: Option<Value>,
    pub usage_snapshot: Option<Value>,
    pub error_snapshot: Option<Value>,
    pub structured_parse_status: Option<String>,
    pub prepared_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelCallListFilter {
    pub owner: Option<ModelCallOwner>,
    pub node_key: Option<String>,
    pub agent_key: Option<String>,
    pub agent_version: Option<String>,
    pub prompt_key: Option<String>,
    pub prompt_version: Option<String>,
    pub model_id: Option<Uuid>,
    pub status: Option<ModelCallStatus>,
    pub prepared_from: Option<DateTime<Utc>>,
    pub prepared_to: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct PostgresModelCallRepository {
    pool: PgPool,
}

impl PostgresModelCallRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Persists the immutable logical request before any provider call is allowed.
    pub async fn prepare(
        &self,
        input: PrepareModelCall,
    ) -> Result<ModelCallRecord, ModelCallRepositoryError> {
        validate_attempt_shape(&input)?;
        let prompt_snapshot = redact_audit_value(&input.prompt_snapshot, &input.known_secrets);
        let context_sources = redact_audit_value(&input.context_sources, &input.known_secrets);
        let memory_sources = redact_audit_value(&input.memory_sources, &input.known_secrets);
        let tool_schema = input
            .tool_schema
            .as_ref()
            .map(|value| redact_audit_value(value, &input.known_secrets));
        let model_snapshot = redact_audit_value(&input.model_snapshot, &input.known_secrets);
        let parameters = redact_audit_value(&input.parameters, &input.known_secrets);
        let asset_references = redact_audit_value(&input.asset_references, &input.known_secrets);
        for value in [
            &prompt_snapshot,
            &context_sources,
            &memory_sources,
            &model_snapshot,
            &parameters,
        ] {
            validate_audit_payload(value)
                .map_err(|error| ModelCallRepositoryError::UnsafeAudit(error.to_string()))?;
        }
        if let Some(value) = tool_schema.as_ref() {
            validate_audit_payload(value)
                .map_err(|error| ModelCallRepositoryError::UnsafeAudit(error.to_string()))?;
        }
        validate_asset_references(&asset_references)
            .map_err(|error| ModelCallRepositoryError::UnsafeAudit(error.to_string()))?;
        let mut transaction = self.pool.begin().await?;
        if let Some(root_id) = input.root_call_id {
            let root = sqlx::query(
                r#"
                SELECT conversation_id, agent_run_id, eval_run_id, root_call_id, attempt, agent_key,
                       agent_version, model_id, behavior_fingerprint
                FROM model_calls WHERE id=$1 FOR SHARE
                "#,
            )
            .bind(root_id)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or(ModelCallRepositoryError::NotFound(root_id))?;
            let root_owner = owner_from_row(&root)?;
            let root_of_root: Option<Uuid> = root.try_get("root_call_id")?;
            let root_attempt: i32 = root.try_get("attempt")?;
            if root_owner != input.owner || root_of_root.is_some() || root_attempt != 1 {
                return Err(ModelCallRepositoryError::InvalidAttempt(
                    "root_call_id 必须指向相同 owner 的首个 attempt".into(),
                ));
            }
            let same_binding = root.try_get::<String, _>("agent_key")? == input.agent_key
                && root.try_get::<String, _>("agent_version")? == input.agent_version
                && root.try_get::<Uuid, _>("model_id")? == input.model_id
                && root.try_get::<String, _>("behavior_fingerprint")? == input.behavior_fingerprint;
            if !same_binding {
                return Err(ModelCallRepositoryError::InvalidAttempt(
                    "retry 不得改变 Definition 或模型 binding".into(),
                ));
            }
            let last_attempt: i32 = sqlx::query_scalar(
                "SELECT MAX(attempt) FROM model_calls WHERE id=$1 OR root_call_id=$1",
            )
            .bind(root_id)
            .fetch_one(&mut *transaction)
            .await?;
            if input.attempt != last_attempt + 1 {
                return Err(ModelCallRepositoryError::InvalidAttempt(format!(
                    "retry attempt 必须为 {}",
                    last_attempt + 1
                )));
            }
        }

        if let ModelCallOwner::EvalRun(eval_run_id) = input.owner {
            reserve_eval_budget(
                &mut transaction,
                eval_run_id,
                &input.behavior_fingerprint,
                &parameters,
            )
            .await?;
        }
        let (conversation_id, agent_run_id, eval_run_id) = owner_columns(input.owner);
        let row = sqlx::query(
            r#"
            INSERT INTO model_calls (
                conversation_id, agent_run_id, eval_run_id, root_call_id, parent_call_id, node_key,
                attempt, agent_key, agent_version, prompt_key, prompt_version,
                registry_digest, prompt_snapshot, context_sources, memory_sources,
                tool_schema, model_id, behavior_fingerprint, model_snapshot, parameters,
                asset_references
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21
            ) RETURNING *
            "#,
        )
        .bind(conversation_id)
        .bind(agent_run_id)
        .bind(eval_run_id)
        .bind(input.root_call_id)
        .bind(input.parent_call_id)
        .bind(input.node_key)
        .bind(input.attempt)
        .bind(input.agent_key)
        .bind(input.agent_version)
        .bind(input.prompt_key)
        .bind(input.prompt_version)
        .bind(input.registry_digest)
        .bind(prompt_snapshot)
        .bind(context_sources)
        .bind(memory_sources)
        .bind(tool_schema)
        .bind(input.model_id)
        .bind(input.behavior_fingerprint)
        .bind(model_snapshot)
        .bind(parameters)
        .bind(asset_references)
        .fetch_one(&mut *transaction)
        .await?;
        let record = model_call_from_row(row)?;
        transaction.commit().await?;
        Ok(record)
    }

    /// Links the audit record and business Step atomically and only within the owning Run.
    pub async fn associate_step(
        &self,
        model_call_id: Uuid,
        step_id: Uuid,
    ) -> Result<(), ModelCallRepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let call = sqlx::query(
            "SELECT agent_run_id, agent_step_id FROM model_calls WHERE id=$1 FOR UPDATE",
        )
        .bind(model_call_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(ModelCallRepositoryError::NotFound(model_call_id))?;
        let step = sqlx::query(
            "SELECT agent_run_id, model_call_id FROM agent_steps WHERE id=$1 FOR UPDATE",
        )
        .bind(step_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(ModelCallRepositoryError::StepNotFound(step_id))?;
        let call_run: Option<Uuid> = call.try_get("agent_run_id")?;
        let step_run: Uuid = step.try_get("agent_run_id")?;
        if call_run != Some(step_run) {
            return Err(ModelCallRepositoryError::OwnerMismatch);
        }
        let linked_step: Option<Uuid> = call.try_get("agent_step_id")?;
        let linked_call: Option<Uuid> = step.try_get("model_call_id")?;
        if linked_step == Some(step_id) && linked_call == Some(model_call_id) {
            transaction.commit().await?;
            return Ok(());
        }
        if linked_step.is_some() || linked_call.is_some() {
            return Err(ModelCallRepositoryError::AssociationConflict);
        }
        sqlx::query("UPDATE model_calls SET agent_step_id=$2 WHERE id=$1")
            .bind(model_call_id)
            .bind(step_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("UPDATE agent_steps SET model_call_id=$2 WHERE id=$1")
            .bind(step_id)
            .bind(model_call_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn finish(
        &self,
        input: FinishModelCall,
    ) -> Result<ModelCallRecord, ModelCallRepositoryError> {
        let output_snapshot = input
            .output_snapshot
            .as_ref()
            .map(|value| redact_audit_value(value, &input.known_secrets));
        let usage_snapshot = input
            .usage_snapshot
            .as_ref()
            .map(|value| redact_audit_value(value, &input.known_secrets));
        let error_snapshot = input
            .error_snapshot
            .as_ref()
            .map(|value| redact_audit_value(value, &input.known_secrets));
        for value in [&output_snapshot, &usage_snapshot, &error_snapshot]
            .into_iter()
            .flatten()
        {
            validate_audit_payload(value)
                .map_err(|error| ModelCallRepositoryError::UnsafeAudit(error.to_string()))?;
        }
        let row = sqlx::query(
            r#"
            UPDATE model_calls
            SET status=$2, output_snapshot=$3, usage_snapshot=$4, error_snapshot=$5,
                structured_parse_status=$6, completed_at=NOW()
            WHERE id=$1 AND status='prepared'
            RETURNING *
            "#,
        )
        .bind(input.id)
        .bind(input.status.as_str())
        .bind(output_snapshot)
        .bind(usage_snapshot)
        .bind(error_snapshot)
        .bind(input.structured_parse_status)
        .fetch_optional(&self.pool)
        .await?;
        if let Some(row) = row {
            return model_call_from_row(row);
        }
        let status: Option<String> =
            sqlx::query_scalar("SELECT status FROM model_calls WHERE id=$1")
                .bind(input.id)
                .fetch_optional(&self.pool)
                .await?;
        match status {
            Some(status) => Err(ModelCallRepositoryError::TerminalConflict(status)),
            None => Err(ModelCallRepositoryError::NotFound(input.id)),
        }
    }

    pub async fn get(&self, id: Uuid) -> Result<ModelCallRecord, ModelCallRepositoryError> {
        let row = sqlx::query("SELECT * FROM model_calls WHERE id=$1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(ModelCallRepositoryError::NotFound(id))?;
        model_call_from_row(row)
    }

    /// Returns a stable page ordered by time and ID without loading prompt/output bodies into list DTOs.
    pub async fn list(
        &self,
        filter: &ModelCallListFilter,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<ModelCallRecord>, i64), ModelCallRepositoryError> {
        let mut count =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM model_calls WHERE TRUE");
        push_list_filters(&mut count, filter);
        let total: i64 = count.build_query_scalar().fetch_one(&self.pool).await?;

        let mut query = QueryBuilder::<Postgres>::new("SELECT * FROM model_calls WHERE TRUE");
        push_list_filters(&mut query, filter);
        query
            .push(" ORDER BY prepared_at DESC, id DESC LIMIT ")
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind(offset);
        let rows = query.build().fetch_all(&self.pool).await?;
        let records = rows
            .into_iter()
            .map(model_call_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        Ok((records, total))
    }

    /// Links an immutable aggregate report to source evidence without copying source content.
    pub async fn attach_eval_report_source(
        &self,
        eval_report_id: Uuid,
        model_call_id: Uuid,
    ) -> Result<(), ModelCallRepositoryError> {
        sqlx::query(
            "INSERT INTO eval_report_sources (eval_report_id, model_call_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(eval_report_id)
        .bind(model_call_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Deletes the explicit owner in one transaction; FK cascades calls and marks linked reports.
    pub async fn delete_owner(
        &self,
        owner: ModelCallOwner,
    ) -> Result<(), ModelCallRepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let deleted = match owner {
            ModelCallOwner::Conversation(id) => {
                sqlx::query("DELETE FROM agent_conversations WHERE id=$1")
                    .bind(id)
                    .execute(&mut *transaction)
                    .await?
                    .rows_affected()
            }
            ModelCallOwner::AgentRun(id) => sqlx::query("DELETE FROM agent_runs WHERE id=$1")
                .bind(id)
                .execute(&mut *transaction)
                .await?
                .rows_affected(),
            ModelCallOwner::EvalRun(_) => {
                return Err(ModelCallRepositoryError::OwnerDeletionUnsupported)
            }
        };
        if deleted != 1 {
            return Err(ModelCallRepositoryError::OwnerNotFound(owner));
        }
        transaction.commit().await?;
        Ok(())
    }
}

fn push_list_filters<'args>(
    query: &mut QueryBuilder<'args, Postgres>,
    filter: &'args ModelCallListFilter,
) {
    if let Some(owner) = filter.owner {
        match owner {
            ModelCallOwner::Conversation(id) => {
                query.push(" AND conversation_id = ").push_bind(id);
            }
            ModelCallOwner::AgentRun(id) => {
                query.push(" AND agent_run_id = ").push_bind(id);
            }
            ModelCallOwner::EvalRun(id) => {
                query.push(" AND eval_run_id = ").push_bind(id);
            }
        }
    }
    for (column, value) in [
        ("node_key", filter.node_key.as_ref()),
        ("agent_key", filter.agent_key.as_ref()),
        ("agent_version", filter.agent_version.as_ref()),
        ("prompt_key", filter.prompt_key.as_ref()),
        ("prompt_version", filter.prompt_version.as_ref()),
    ] {
        if let Some(value) = value {
            query
                .push(" AND ")
                .push(column)
                .push(" = ")
                .push_bind(value);
        }
    }
    if let Some(model_id) = filter.model_id {
        query.push(" AND model_id = ").push_bind(model_id);
    }
    if let Some(status) = filter.status {
        query.push(" AND status = ").push_bind(status.as_str());
    }
    if let Some(prepared_from) = filter.prepared_from {
        query.push(" AND prepared_at >= ").push_bind(prepared_from);
    }
    if let Some(prepared_to) = filter.prepared_to {
        query.push(" AND prepared_at <= ").push_bind(prepared_to);
    }
}

async fn reserve_eval_budget(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    eval_run_id: Uuid,
    behavior_fingerprint: &str,
    parameters: &Value,
) -> Result<(), ModelCallRepositoryError> {
    let charge = parameters
        .get("eval_budget_charge")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            ModelCallRepositoryError::EvalBudget(
                "EvalRun ModelCall requires eval_budget_charge".into(),
            )
        })?;
    let input_tokens = charge_u64(charge, "input_tokens")?;
    let output_tokens = charge_u64(charge, "output_tokens")?;
    let cost_micros = charge_u64(charge, "cost_micros")?;
    let retry = charge
        .get("retry")
        .and_then(Value::as_bool)
        .ok_or_else(|| ModelCallRepositoryError::EvalBudget("retry must be boolean".into()))?;
    let row = sqlx::query(
        r#"
        SELECT validation_mode, approved_real_calls, behavior_fingerprint, status,
               max_cases, max_input_tokens, max_output_tokens, max_retries, max_cost_micros,
               actual_cases, actual_input_tokens, actual_output_tokens, actual_retries,
               actual_cost_micros, actual_real_model_calls
        FROM eval_runs WHERE id = $1 FOR UPDATE
        "#,
    )
    .bind(eval_run_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| ModelCallRepositoryError::EvalBudget("EvalRun not found".into()))?;
    let status: String = row.try_get("status")?;
    let configured_fingerprint: Option<String> = row.try_get("behavior_fingerprint")?;
    if row.try_get::<String, _>("validation_mode")? != "real_model"
        || !row.try_get::<bool, _>("approved_real_calls")?
        || !matches!(status.as_str(), "pending" | "running")
    {
        return Err(ModelCallRepositoryError::EvalBudget(
            "EvalRun is not an approved executable real-model run".into(),
        ));
    }
    if configured_fingerprint.as_deref() != Some(behavior_fingerprint) {
        return Err(ModelCallRepositoryError::EvalBudget(
            "behavior_fingerprint drift".into(),
        ));
    }

    let next_cases = row
        .try_get::<i32, _>("actual_cases")?
        .checked_add(1)
        .ok_or_else(|| ModelCallRepositoryError::EvalBudget("case counter overflow".into()))?;
    let next_input = checked_add_i64(&row, "actual_input_tokens", input_tokens)?;
    let next_output = checked_add_i64(&row, "actual_output_tokens", output_tokens)?;
    let next_cost = checked_add_i64(&row, "actual_cost_micros", cost_micros)?;
    let next_retries = row
        .try_get::<i32, _>("actual_retries")?
        .checked_add(i32::from(retry))
        .ok_or_else(|| ModelCallRepositoryError::EvalBudget("retry counter overflow".into()))?;
    if next_cases > row.try_get::<i32, _>("max_cases")?
        || next_input > row.try_get::<i64, _>("max_input_tokens")?
        || next_output > row.try_get::<i64, _>("max_output_tokens")?
        || next_retries > row.try_get::<i32, _>("max_retries")?
        || next_cost > row.try_get::<i64, _>("max_cost_micros")?
    {
        return Err(ModelCallRepositoryError::EvalBudget(
            "approved case/token/retry/cost limit reached".into(),
        ));
    }
    let next_real_calls = row
        .try_get::<i32, _>("actual_real_model_calls")?
        .checked_add(1)
        .ok_or_else(|| ModelCallRepositoryError::EvalBudget("call counter overflow".into()))?;
    sqlx::query(
        r#"
        UPDATE eval_runs
        SET status = 'running', started_at = COALESCE(started_at, NOW()),
            actual_cases = $2, actual_input_tokens = $3, actual_output_tokens = $4,
            actual_retries = $5, actual_cost_micros = $6, actual_real_model_calls = $7
        WHERE id = $1
        "#,
    )
    .bind(eval_run_id)
    .bind(next_cases)
    .bind(next_input)
    .bind(next_output)
    .bind(next_retries)
    .bind(next_cost)
    .bind(next_real_calls)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn charge_u64(
    charge: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<u64, ModelCallRepositoryError> {
    charge.get(field).and_then(Value::as_u64).ok_or_else(|| {
        ModelCallRepositoryError::EvalBudget(format!("{field} must be a non-negative integer"))
    })
}

fn checked_add_i64(row: &PgRow, field: &str, charge: u64) -> Result<i64, ModelCallRepositoryError> {
    let charge = i64::try_from(charge)
        .map_err(|_| ModelCallRepositoryError::EvalBudget(format!("{field} charge overflow")))?;
    row.try_get::<i64, _>(field)?
        .checked_add(charge)
        .ok_or_else(|| ModelCallRepositoryError::EvalBudget(format!("{field} counter overflow")))
}

fn validate_attempt_shape(input: &PrepareModelCall) -> Result<(), ModelCallRepositoryError> {
    if input.attempt == 1 && input.root_call_id.is_none() {
        return Ok(());
    }
    if input.attempt > 1 && input.root_call_id.is_some() {
        return Ok(());
    }
    Err(ModelCallRepositoryError::InvalidAttempt(
        "attempt 1 不得设置 root_call_id，retry 必须设置 root_call_id".into(),
    ))
}

fn owner_columns(owner: ModelCallOwner) -> (Option<Uuid>, Option<Uuid>, Option<Uuid>) {
    match owner {
        ModelCallOwner::Conversation(id) => (Some(id), None, None),
        ModelCallOwner::AgentRun(id) => (None, Some(id), None),
        ModelCallOwner::EvalRun(id) => (None, None, Some(id)),
    }
}

fn owner_from_row(row: &PgRow) -> Result<ModelCallOwner, ModelCallRepositoryError> {
    match (
        row.try_get::<Option<Uuid>, _>("conversation_id")?,
        row.try_get::<Option<Uuid>, _>("agent_run_id")?,
        row.try_get::<Option<Uuid>, _>("eval_run_id")?,
    ) {
        (Some(id), None, None) => Ok(ModelCallOwner::Conversation(id)),
        (None, Some(id), None) => Ok(ModelCallOwner::AgentRun(id)),
        (None, None, Some(id)) => Ok(ModelCallOwner::EvalRun(id)),
        _ => Err(ModelCallRepositoryError::InvalidOwner),
    }
}

fn model_call_from_row(row: PgRow) -> Result<ModelCallRecord, ModelCallRepositoryError> {
    let schema_version: String = row.try_get("schema_version")?;
    if schema_version != MODEL_CALL_SCHEMA_VERSION {
        return Err(ModelCallRepositoryError::InvalidSchemaVersion(
            schema_version,
        ));
    }
    let status = match row.try_get::<String, _>("status")?.as_str() {
        "prepared" => ModelCallStatus::Prepared,
        "succeeded" => ModelCallStatus::Succeeded,
        "failed" => ModelCallStatus::Failed,
        "aborted" => ModelCallStatus::Aborted,
        value => return Err(ModelCallRepositoryError::InvalidStatus(value.into())),
    };
    Ok(ModelCallRecord {
        id: row.try_get("id")?,
        schema_version,
        owner: owner_from_row(&row)?,
        agent_step_id: row.try_get("agent_step_id")?,
        root_call_id: row.try_get("root_call_id")?,
        parent_call_id: row.try_get("parent_call_id")?,
        node_key: row.try_get("node_key")?,
        attempt: row.try_get("attempt")?,
        status,
        agent_key: row.try_get("agent_key")?,
        agent_version: row.try_get("agent_version")?,
        prompt_key: row.try_get("prompt_key")?,
        prompt_version: row.try_get("prompt_version")?,
        registry_digest: row.try_get("registry_digest")?,
        prompt_snapshot: row.try_get("prompt_snapshot")?,
        context_sources: row.try_get("context_sources")?,
        memory_sources: row.try_get("memory_sources")?,
        tool_schema: row.try_get("tool_schema")?,
        model_id: row.try_get("model_id")?,
        behavior_fingerprint: row.try_get("behavior_fingerprint")?,
        model_snapshot: row.try_get("model_snapshot")?,
        parameters: row.try_get("parameters")?,
        asset_references: row.try_get("asset_references")?,
        output_snapshot: row.try_get("output_snapshot")?,
        usage_snapshot: row.try_get("usage_snapshot")?,
        error_snapshot: row.try_get("error_snapshot")?,
        structured_parse_status: row.try_get("structured_parse_status")?,
        prepared_at: row.try_get("prepared_at")?,
        completed_at: row.try_get("completed_at")?,
    })
}

#[derive(Debug)]
pub enum ModelCallRepositoryError {
    Storage(sqlx::Error),
    NotFound(Uuid),
    StepNotFound(Uuid),
    InvalidOwner,
    InvalidStatus(String),
    InvalidSchemaVersion(String),
    UnsafeAudit(String),
    InvalidAttempt(String),
    OwnerMismatch,
    AssociationConflict,
    TerminalConflict(String),
    OwnerNotFound(ModelCallOwner),
    OwnerDeletionUnsupported,
    EvalBudget(String),
}

impl fmt::Display for ModelCallRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(formatter, "model call storage error: {error}"),
            Self::NotFound(id) => write!(formatter, "model call not found: {id}"),
            Self::StepNotFound(id) => write!(formatter, "agent step not found: {id}"),
            Self::InvalidOwner => formatter.write_str("model call must have exactly one owner"),
            Self::InvalidStatus(status) => write!(formatter, "invalid model call status: {status}"),
            Self::InvalidSchemaVersion(version) => {
                write!(formatter, "invalid model call schema version: {version}")
            }
            Self::UnsafeAudit(message) => write!(formatter, "unsafe model call audit: {message}"),
            Self::InvalidAttempt(message) => formatter.write_str(message),
            Self::OwnerMismatch => {
                formatter.write_str("model call and step belong to different runs")
            }
            Self::AssociationConflict => {
                formatter.write_str("model call or step is already associated")
            }
            Self::TerminalConflict(status) => write!(
                formatter,
                "model call already has terminal status: {status}"
            ),
            Self::OwnerNotFound(owner) => {
                write!(formatter, "model call owner not found: {owner:?}")
            }
            Self::OwnerDeletionUnsupported => formatter
                .write_str("EvalRun audit evidence cannot be deleted through owner cleanup"),
            Self::EvalBudget(message) => write!(formatter, "eval budget rejected: {message}"),
        }
    }
}

impl std::error::Error for ModelCallRepositoryError {}

impl From<sqlx::Error> for ModelCallRepositoryError {
    fn from(value: sqlx::Error) -> Self {
        Self::Storage(value)
    }
}

#[async_trait]
impl ModelCallAuditStore for PostgresModelCallRepository {
    async fn prepare(&self, input: PrepareAuditedCall) -> Result<Uuid, novex_agent::BoxError> {
        let owner = match input.owner {
            AuditedCallOwner::Conversation(id) => ModelCallOwner::Conversation(id),
            AuditedCallOwner::AgentRun(id) => ModelCallOwner::AgentRun(id),
            AuditedCallOwner::EvalRun(id) => ModelCallOwner::EvalRun(id),
        };
        let record = PostgresModelCallRepository::prepare(
            self,
            PrepareModelCall {
                owner,
                root_call_id: input.root_call_id,
                parent_call_id: input.parent_call_id,
                node_key: input.snapshot.node_key.clone(),
                attempt: input.attempt,
                agent_key: input.snapshot.agent_key.clone(),
                agent_version: input.snapshot.agent_version.clone(),
                prompt_key: input.snapshot.prompt_key.clone(),
                prompt_version: input.snapshot.prompt_version.clone(),
                registry_digest: input.snapshot.registry_digest.clone(),
                prompt_snapshot: serde_json::to_value(&input.snapshot)?,
                context_sources: input.context_sources,
                memory_sources: input.memory_sources,
                tool_schema: input.snapshot.tool_schema.clone(),
                model_id: input.model_id,
                behavior_fingerprint: input.behavior_fingerprint,
                model_snapshot: input.model_snapshot,
                parameters: input.parameters,
                asset_references: input.asset_references,
                known_secrets: input.known_secrets,
            },
        )
        .await?;
        Ok(record.id)
    }

    async fn associate_step(
        &self,
        model_call_id: Uuid,
        step_id: Uuid,
    ) -> Result<(), novex_agent::BoxError> {
        PostgresModelCallRepository::associate_step(self, model_call_id, step_id)
            .await
            .map_err(Into::into)
    }

    async fn finish(&self, input: FinishAuditedCall) -> Result<(), novex_agent::BoxError> {
        let status = match input.status {
            AuditedTerminalStatus::Succeeded => ModelCallTerminalStatus::Succeeded,
            AuditedTerminalStatus::Failed => ModelCallTerminalStatus::Failed,
            AuditedTerminalStatus::Aborted => ModelCallTerminalStatus::Aborted,
        };
        PostgresModelCallRepository::finish(
            self,
            FinishModelCall {
                id: input.id,
                status,
                output_snapshot: input.output_snapshot,
                usage_snapshot: input.usage_snapshot,
                error_snapshot: input.error_snapshot,
                structured_parse_status: input.structured_parse_status,
                known_secrets: input.known_secrets,
            },
        )
        .await?;
        Ok(())
    }
}
