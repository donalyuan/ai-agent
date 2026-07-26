use async_trait::async_trait;
use chrono::{DateTime, SecondsFormat, Utc};
use novex_agent::{
    AuditedCallOwner, ContextAuditStore, PersistContextCompileAttempt, PersistContextSnapshot,
};
use novex_ai_core::{
    redact_audit_value, validate_asset_references, validate_audit_payload, CompileFailureStage,
    ContextCompileAttempt, ContextDecisionCode, ContextSnapshot, ExecutorOwner,
    CONTEXT_SCHEMA_VERSION,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use sqlx::{postgres::PgRow, PgPool, Postgres, Row, Transaction};
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub struct ContextSnapshotRecord {
    pub id: Uuid,
    pub owner: AuditedCallOwner,
    pub snapshot: ContextSnapshot,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContextCompileAttemptRecord {
    pub id: Uuid,
    pub owner: AuditedCallOwner,
    pub attempt: ContextCompileAttempt,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ContextAuditRecord {
    Snapshot(Box<ContextSnapshotRecord>),
    CompileAttempt(Box<ContextCompileAttemptRecord>),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContextAuditListFilter {
    pub owner: Option<AuditedCallOwner>,
    pub record_type: Option<String>,
    pub node_key: Option<String>,
}

#[derive(Clone)]
pub struct PostgresContextAuditRepository {
    pool: PgPool,
}

impl PostgresContextAuditRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn persist_snapshot_record(
        &self,
        input: PersistContextSnapshot,
    ) -> Result<ContextSnapshotRecord, ContextAuditRepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let record =
            Self::persist_snapshot_in_transaction(&mut transaction, Uuid::new_v4(), input).await?;
        transaction.commit().await?;
        Ok(record)
    }

    pub(crate) async fn persist_snapshot_in_transaction(
        transaction: &mut Transaction<'_, Postgres>,
        id: Uuid,
        input: PersistContextSnapshot,
    ) -> Result<ContextSnapshotRecord, ContextAuditRepositoryError> {
        let snapshot = sanitize_snapshot(input.owner, input.snapshot, &input.known_secrets)?;
        let compiled_at = parse_compiled_at(&snapshot.compiled_at)?;
        let (conversation_id, agent_run_id, eval_run_id) = owner_columns(input.owner);
        let row = sqlx::query(
            r#"
            INSERT INTO context_snapshots (
                id, schema_version, conversation_id, agent_run_id, eval_run_id, node_key,
                compiled_at, policy_key, policy_version, tokenizer_profile_key,
                tokenizer_profile_version, tokenizer_mode, model_context_window,
                budget_ledger, decisions, selected_order, logical_input, context_digest
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18
            ) RETURNING *
            "#,
        )
        .bind(id)
        .bind(&snapshot.schema_version)
        .bind(conversation_id)
        .bind(agent_run_id)
        .bind(eval_run_id)
        .bind(&snapshot.node_key)
        .bind(compiled_at)
        .bind(&snapshot.policy_key)
        .bind(&snapshot.policy_version)
        .bind(&snapshot.tokenizer_profile_key)
        .bind(&snapshot.tokenizer_profile_version)
        .bind(&snapshot.tokenizer_mode)
        .bind(snapshot.budget.model_context_window as i64)
        .bind(serde_json::to_value(&snapshot.budget)?)
        .bind(serde_json::to_value(&snapshot.decisions)?)
        .bind(serde_json::to_value(&snapshot.selected_order)?)
        .bind(serde_json::to_value(&snapshot.logical_input)?)
        .bind(&snapshot.digest)
        .fetch_one(&mut **transaction)
        .await?;
        context_snapshot_from_row(row)
    }

    pub async fn persist_attempt_record(
        &self,
        input: PersistContextCompileAttempt,
    ) -> Result<ContextCompileAttemptRecord, ContextAuditRepositoryError> {
        let attempt = sanitize_attempt(input.owner, input.attempt, &input.known_secrets)?;
        let compiled_at = parse_compiled_at(&attempt.compiled_at)?;
        let (conversation_id, agent_run_id, eval_run_id) = owner_columns(input.owner);
        let row = sqlx::query(
            r#"
            INSERT INTO context_compile_attempts (
                schema_version, conversation_id, agent_run_id, eval_run_id, node_key,
                compiled_at, stage, code, budget_ledger, decisions, attempt_digest
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING *
            "#,
        )
        .bind(&attempt.schema_version)
        .bind(conversation_id)
        .bind(agent_run_id)
        .bind(eval_run_id)
        .bind(&attempt.node_key)
        .bind(compiled_at)
        .bind(stage_name(attempt.stage))
        .bind(&attempt.code)
        .bind(
            attempt
                .budget
                .as_ref()
                .map(serde_json::to_value)
                .transpose()?,
        )
        .bind(serde_json::to_value(&attempt.decisions)?)
        .bind(&attempt.digest)
        .fetch_one(&self.pool)
        .await?;
        context_attempt_from_row(row)
    }

    pub async fn get_snapshot(
        &self,
        id: Uuid,
    ) -> Result<ContextSnapshotRecord, ContextAuditRepositoryError> {
        let row = sqlx::query("SELECT * FROM context_snapshots WHERE id=$1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(ContextAuditRepositoryError::SnapshotNotFound(id))?;
        context_snapshot_from_row(row)
    }

    pub async fn get_attempt(
        &self,
        id: Uuid,
    ) -> Result<ContextCompileAttemptRecord, ContextAuditRepositoryError> {
        let row = sqlx::query("SELECT * FROM context_compile_attempts WHERE id=$1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(ContextAuditRepositoryError::AttemptNotFound(id))?;
        context_attempt_from_row(row)
    }

    pub async fn list(
        &self,
        filter: &ContextAuditListFilter,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<ContextAuditRecord>, i64), ContextAuditRepositoryError> {
        let (conversation_id, agent_run_id, eval_run_id) = filter
            .owner
            .map(owner_columns)
            .unwrap_or((None, None, None));
        let record_type = filter.record_type.clone();
        let node_key = filter.node_key.clone();
        let records = r#"
            WITH records AS (
                SELECT id, 'snapshot'::text AS record_type, conversation_id, agent_run_id,
                       eval_run_id, node_key, compiled_at
                FROM context_snapshots
                UNION ALL
                SELECT id, 'compile_attempt'::text AS record_type, conversation_id, agent_run_id,
                       eval_run_id, node_key, compiled_at
                FROM context_compile_attempts
            )
        "#;
        let count_sql = format!(
            "{records} SELECT COUNT(*) FROM records WHERE ($1::text IS NULL OR record_type=$1) \
             AND ($2::uuid IS NULL OR conversation_id=$2) AND ($3::uuid IS NULL OR agent_run_id=$3) \
             AND ($4::uuid IS NULL OR eval_run_id=$4) AND ($5::text IS NULL OR node_key=$5)"
        );
        let total = sqlx::query_scalar::<_, i64>(&count_sql)
            .bind(&record_type)
            .bind(conversation_id)
            .bind(agent_run_id)
            .bind(eval_run_id)
            .bind(&node_key)
            .fetch_one(&self.pool)
            .await?;
        let list_sql = format!(
            "{records} SELECT id, record_type FROM records WHERE ($1::text IS NULL OR record_type=$1) \
             AND ($2::uuid IS NULL OR conversation_id=$2) AND ($3::uuid IS NULL OR agent_run_id=$3) \
             AND ($4::uuid IS NULL OR eval_run_id=$4) AND ($5::text IS NULL OR node_key=$5) \
             ORDER BY compiled_at DESC, id DESC LIMIT $6 OFFSET $7"
        );
        let rows = sqlx::query(&list_sql)
            .bind(&record_type)
            .bind(conversation_id)
            .bind(agent_run_id)
            .bind(eval_run_id)
            .bind(&node_key)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let id: Uuid = row.try_get("id")?;
            let record_type: String = row.try_get("record_type")?;
            items.push(match record_type.as_str() {
                "snapshot" => ContextAuditRecord::Snapshot(Box::new(self.get_snapshot(id).await?)),
                "compile_attempt" => {
                    ContextAuditRecord::CompileAttempt(Box::new(self.get_attempt(id).await?))
                }
                _ => {
                    return Err(ContextAuditRepositoryError::InvalidRecord(
                        "未知 Context record_type".into(),
                    ))
                }
            });
        }
        Ok((items, total))
    }

    /// Adds immutable failure evidence references while leaving existing terminal state fields intact.
    pub async fn link_failure(
        &self,
        owner: AuditedCallOwner,
        attempt_id: Uuid,
        step_id: Option<Uuid>,
    ) -> Result<(), ContextAuditRepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let attempt = sqlx::query(
            "SELECT conversation_id, agent_run_id, eval_run_id FROM context_compile_attempts WHERE id=$1 FOR SHARE",
        )
        .bind(attempt_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(ContextAuditRepositoryError::AttemptNotFound(attempt_id))?;
        let stored_owner = owner_from_row(&attempt)?;
        if stored_owner != owner {
            return Err(ContextAuditRepositoryError::OwnerMismatch {
                expected: owner_uuid(owner).to_string(),
                actual: owner_uuid(stored_owner).to_string(),
            });
        }

        match owner {
            AuditedCallOwner::Conversation(conversation_id) => {
                if step_id.is_some() {
                    return Err(ContextAuditRepositoryError::InvalidRecord(
                        "Conversation Context failure 不得关联 Run Step".into(),
                    ));
                }
                let updated = sqlx::query(
                    "UPDATE agent_conversations SET last_context_compile_attempt_id=$2 WHERE id=$1",
                )
                .bind(conversation_id)
                .bind(attempt_id)
                .execute(&mut *transaction)
                .await?
                .rows_affected();
                if updated != 1 {
                    return Err(ContextAuditRepositoryError::OwnerNotFound(conversation_id));
                }
            }
            AuditedCallOwner::AgentRun(run_id) => {
                let updated =
                    sqlx::query("UPDATE agent_runs SET context_compile_attempt_id=$2 WHERE id=$1")
                        .bind(run_id)
                        .bind(attempt_id)
                        .execute(&mut *transaction)
                        .await?
                        .rows_affected();
                if updated != 1 {
                    return Err(ContextAuditRepositoryError::OwnerNotFound(run_id));
                }
                if let Some(step_id) = step_id {
                    let step_updated = sqlx::query(
                        "UPDATE agent_steps SET context_compile_attempt_id=$2 WHERE id=$1 AND agent_run_id=$3",
                    )
                    .bind(step_id)
                    .bind(attempt_id)
                    .bind(run_id)
                    .execute(&mut *transaction)
                    .await?
                    .rows_affected();
                    if step_updated != 1 {
                        return Err(ContextAuditRepositoryError::StepOwnerMismatch(step_id));
                    }
                }
                sqlx::query(
                    r#"
                    UPDATE agent_conversations
                    SET last_context_compile_attempt_id=$2
                    WHERE id::text = (
                        SELECT input->>'conversation_id' FROM agent_runs WHERE id=$1
                    )
                    "#,
                )
                .bind(run_id)
                .bind(attempt_id)
                .execute(&mut *transaction)
                .await?;
            }
            AuditedCallOwner::EvalRun(_) => {
                return Err(ContextAuditRepositoryError::InvalidRecord(
                    "EvalRun Context failure 由 Eval 状态机关联".into(),
                ));
            }
        }
        transaction.commit().await?;
        Ok(())
    }
}

fn sanitize_snapshot(
    owner: AuditedCallOwner,
    snapshot: ContextSnapshot,
    known_secrets: &[String],
) -> Result<ContextSnapshot, ContextAuditRepositoryError> {
    validate_common_record(
        owner,
        snapshot.owner,
        &snapshot.owner_id,
        &snapshot.schema_version,
        &snapshot.node_key,
        &snapshot.digest,
    )?;
    if snapshot.tokenizer_mode != "exact" && snapshot.tokenizer_mode != "conservative" {
        return Err(ContextAuditRepositoryError::InvalidRecord(
            "tokenizer_mode 不受支持".into(),
        ));
    }
    for decision in &snapshot.decisions {
        match decision.decision {
            ContextDecisionCode::Selected if decision.selected_payload.is_none() => {
                return Err(ContextAuditRepositoryError::InvalidRecord(
                    "selected decision 缺少 payload".into(),
                ));
            }
            ContextDecisionCode::Selected => {}
            _ if decision.selected_payload.is_some() => {
                return Err(ContextAuditRepositoryError::InvalidRecord(
                    "排除 decision 禁止保存 payload".into(),
                ));
            }
            _ => {}
        }
    }
    sanitize_record(snapshot, known_secrets)
}

fn sanitize_attempt(
    owner: AuditedCallOwner,
    attempt: ContextCompileAttempt,
    known_secrets: &[String],
) -> Result<ContextCompileAttempt, ContextAuditRepositoryError> {
    validate_common_record(
        owner,
        attempt.owner,
        &attempt.owner_id,
        &attempt.schema_version,
        &attempt.node_key,
        &attempt.digest,
    )?;
    if attempt
        .decisions
        .iter()
        .any(|decision| decision.selected_payload.is_some())
    {
        return Err(ContextAuditRepositoryError::InvalidRecord(
            "CompileAttempt 禁止保存候选 payload".into(),
        ));
    }
    sanitize_record(attempt, known_secrets)
}

fn validate_common_record(
    owner: AuditedCallOwner,
    executor_owner: ExecutorOwner,
    owner_id: &str,
    schema_version: &str,
    node_key: &str,
    digest: &str,
) -> Result<(), ContextAuditRepositoryError> {
    if executor_owner != ExecutorOwner::Rust {
        return Err(ContextAuditRepositoryError::InvalidRecord(
            "PostgreSQL Context 记录必须属于 Rust Runtime".into(),
        ));
    }
    let expected_owner_id = owner_uuid(owner).to_string();
    if owner_id != expected_owner_id {
        return Err(ContextAuditRepositoryError::OwnerMismatch {
            expected: expected_owner_id,
            actual: owner_id.into(),
        });
    }
    if schema_version != CONTEXT_SCHEMA_VERSION {
        return Err(ContextAuditRepositoryError::InvalidRecord(format!(
            "不支持的 Context schema_version: {schema_version}"
        )));
    }
    if node_key.trim().is_empty() || !valid_digest(digest) {
        return Err(ContextAuditRepositoryError::InvalidRecord(
            "Context node 或 digest 非法".into(),
        ));
    }
    Ok(())
}

fn sanitize_record<T>(record: T, known_secrets: &[String]) -> Result<T, ContextAuditRepositoryError>
where
    T: Serialize + DeserializeOwned,
{
    let value = redact_audit_value(&serde_json::to_value(record)?, known_secrets);
    validate_audit_payload(&value)
        .map_err(|error| ContextAuditRepositoryError::UnsafeAudit(error.to_string()))?;
    validate_context_assets(&value)?;
    Ok(serde_json::from_value(value)?)
}

fn validate_context_assets(value: &Value) -> Result<(), ContextAuditRepositoryError> {
    let assets = value
        .get("decisions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|decision| decision.get("selected_payload"))
        .filter(|payload| payload.get("type") == Some(&Value::String("asset".into())))
        .filter_map(|payload| payload.get("asset"))
        .cloned()
        .collect::<Vec<_>>();
    validate_asset_references(&Value::Array(assets))
        .map_err(|error| ContextAuditRepositoryError::UnsafeAudit(error.to_string()))
}

fn context_snapshot_from_row(
    row: PgRow,
) -> Result<ContextSnapshotRecord, ContextAuditRepositoryError> {
    let owner = owner_from_row(&row)?;
    let compiled_at: DateTime<Utc> = row.try_get("compiled_at")?;
    let snapshot = ContextSnapshot {
        schema_version: row.try_get("schema_version")?,
        owner: ExecutorOwner::Rust,
        owner_id: owner_uuid(owner).to_string(),
        node_key: row.try_get("node_key")?,
        compiled_at: compiled_at.to_rfc3339_opts(SecondsFormat::AutoSi, true),
        policy_key: row.try_get("policy_key")?,
        policy_version: row.try_get("policy_version")?,
        tokenizer_profile_key: row.try_get("tokenizer_profile_key")?,
        tokenizer_profile_version: row.try_get("tokenizer_profile_version")?,
        tokenizer_mode: row.try_get("tokenizer_mode")?,
        budget: serde_json::from_value(row.try_get("budget_ledger")?)?,
        decisions: serde_json::from_value(row.try_get("decisions")?)?,
        selected_order: serde_json::from_value(row.try_get("selected_order")?)?,
        logical_input: serde_json::from_value(row.try_get("logical_input")?)?,
        digest: row.try_get("context_digest")?,
    };
    Ok(ContextSnapshotRecord {
        id: row.try_get("id")?,
        owner,
        snapshot,
        created_at: row.try_get("created_at")?,
    })
}

fn context_attempt_from_row(
    row: PgRow,
) -> Result<ContextCompileAttemptRecord, ContextAuditRepositoryError> {
    let owner = owner_from_row(&row)?;
    let compiled_at: DateTime<Utc> = row.try_get("compiled_at")?;
    let budget: Option<Value> = row.try_get("budget_ledger")?;
    let attempt = ContextCompileAttempt {
        schema_version: row.try_get("schema_version")?,
        owner: ExecutorOwner::Rust,
        owner_id: owner_uuid(owner).to_string(),
        node_key: row.try_get("node_key")?,
        compiled_at: compiled_at.to_rfc3339_opts(SecondsFormat::AutoSi, true),
        stage: parse_stage(row.try_get("stage")?)?,
        code: row.try_get("code")?,
        budget: budget.map(serde_json::from_value).transpose()?,
        decisions: serde_json::from_value(row.try_get("decisions")?)?,
        digest: row.try_get("attempt_digest")?,
    };
    Ok(ContextCompileAttemptRecord {
        id: row.try_get("id")?,
        owner,
        attempt,
        created_at: row.try_get("created_at")?,
    })
}

fn parse_compiled_at(value: &str) -> Result<DateTime<Utc>, ContextAuditRepositoryError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| ContextAuditRepositoryError::InvalidRecord("compiled_at 非法".into()))
}

fn owner_columns(owner: AuditedCallOwner) -> (Option<Uuid>, Option<Uuid>, Option<Uuid>) {
    match owner {
        AuditedCallOwner::Conversation(id) => (Some(id), None, None),
        AuditedCallOwner::AgentRun(id) => (None, Some(id), None),
        AuditedCallOwner::EvalRun(id) => (None, None, Some(id)),
    }
}

fn owner_uuid(owner: AuditedCallOwner) -> Uuid {
    match owner {
        AuditedCallOwner::Conversation(id)
        | AuditedCallOwner::AgentRun(id)
        | AuditedCallOwner::EvalRun(id) => id,
    }
}

fn owner_from_row(row: &PgRow) -> Result<AuditedCallOwner, ContextAuditRepositoryError> {
    let conversation_id: Option<Uuid> = row.try_get("conversation_id")?;
    let agent_run_id: Option<Uuid> = row.try_get("agent_run_id")?;
    let eval_run_id: Option<Uuid> = row.try_get("eval_run_id")?;
    match (conversation_id, agent_run_id, eval_run_id) {
        (Some(id), None, None) => Ok(AuditedCallOwner::Conversation(id)),
        (None, Some(id), None) => Ok(AuditedCallOwner::AgentRun(id)),
        (None, None, Some(id)) => Ok(AuditedCallOwner::EvalRun(id)),
        _ => Err(ContextAuditRepositoryError::InvalidRecord(
            "Context owner 列非法".into(),
        )),
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn stage_name(stage: CompileFailureStage) -> &'static str {
    match stage {
        CompileFailureStage::Schema => "schema",
        CompileFailureStage::Eligibility => "eligibility",
        CompileFailureStage::Conflict => "conflict",
        CompileFailureStage::Tokenizer => "tokenizer",
        CompileFailureStage::Budget => "budget",
        CompileFailureStage::Finalize => "finalize",
    }
}

fn parse_stage(value: String) -> Result<CompileFailureStage, ContextAuditRepositoryError> {
    match value.as_str() {
        "schema" => Ok(CompileFailureStage::Schema),
        "eligibility" => Ok(CompileFailureStage::Eligibility),
        "conflict" => Ok(CompileFailureStage::Conflict),
        "tokenizer" => Ok(CompileFailureStage::Tokenizer),
        "budget" => Ok(CompileFailureStage::Budget),
        "finalize" => Ok(CompileFailureStage::Finalize),
        _ => Err(ContextAuditRepositoryError::InvalidRecord(format!(
            "未知 CompileAttempt stage: {value}"
        ))),
    }
}

#[derive(Debug)]
pub enum ContextAuditRepositoryError {
    Storage(sqlx::Error),
    Serialization(serde_json::Error),
    SnapshotNotFound(Uuid),
    AttemptNotFound(Uuid),
    OwnerNotFound(Uuid),
    StepOwnerMismatch(Uuid),
    OwnerMismatch { expected: String, actual: String },
    InvalidRecord(String),
    UnsafeAudit(String),
}

impl fmt::Display for ContextAuditRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(formatter, "Context audit storage failed: {error}"),
            Self::Serialization(error) => {
                write!(formatter, "Context audit serialization failed: {error}")
            }
            Self::SnapshotNotFound(id) => write!(formatter, "ContextSnapshot not found: {id}"),
            Self::AttemptNotFound(id) => write!(formatter, "ContextCompileAttempt not found: {id}"),
            Self::OwnerNotFound(id) => write!(formatter, "Context owner not found: {id}"),
            Self::StepOwnerMismatch(id) => {
                write!(formatter, "Context failure Step owner mismatch: {id}")
            }
            Self::OwnerMismatch { expected, actual } => write!(
                formatter,
                "Context owner mismatch: expected {expected}, actual {actual}"
            ),
            Self::InvalidRecord(message) => write!(formatter, "invalid Context audit: {message}"),
            Self::UnsafeAudit(message) => write!(formatter, "unsafe Context audit: {message}"),
        }
    }
}

impl std::error::Error for ContextAuditRepositoryError {}

impl From<sqlx::Error> for ContextAuditRepositoryError {
    fn from(value: sqlx::Error) -> Self {
        Self::Storage(value)
    }
}

impl From<serde_json::Error> for ContextAuditRepositoryError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serialization(value)
    }
}

#[async_trait]
impl ContextAuditStore for PostgresContextAuditRepository {
    async fn binding_is_executable(
        &self,
        owner: AuditedCallOwner,
    ) -> Result<bool, novex_agent::BoxError> {
        let executable = match owner {
            AuditedCallOwner::Conversation(id) => {
                sqlx::query_scalar::<_, bool>(
                    "SELECT binding_status = 'executable' FROM agent_conversation_bindings WHERE conversation_id=$1",
                )
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
                .unwrap_or(true)
            }
            AuditedCallOwner::AgentRun(id) => {
                sqlx::query_scalar::<_, bool>(
                    r#"
                    SELECT COALESCE(
                        (
                            SELECT context_binding_status = 'executable'
                            FROM agent_run_bindings
                            WHERE agent_run_id=$1
                        ),
                        (
                            SELECT binding.binding_status = 'executable'
                            FROM agent_runs run
                            JOIN agent_conversation_bindings binding
                              ON binding.conversation_id = (run.input->>'conversation_id')::uuid
                            WHERE run.id=$1
                        ),
                        TRUE
                    )
                    "#,
                )
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
                .unwrap_or(false)
            }
            AuditedCallOwner::EvalRun(_) => true,
        };
        Ok(executable)
    }

    async fn block_tokenizer_profile_binding(
        &self,
        owner: AuditedCallOwner,
    ) -> Result<(), novex_agent::BoxError> {
        let affected = match owner {
            AuditedCallOwner::Conversation(id) => sqlx::query(
                "UPDATE agent_conversation_bindings SET binding_status='model_rebind_required' WHERE conversation_id=$1 AND binding_status='executable'",
            )
            .bind(id)
            .execute(&self.pool)
            .await?
            .rows_affected(),
            AuditedCallOwner::AgentRun(id) => {
                let run_binding = sqlx::query(
                    "UPDATE agent_run_bindings SET context_binding_status='tokenizer_profile_incompatible' WHERE agent_run_id=$1 AND context_binding_status='executable'",
                )
                .bind(id)
                .execute(&self.pool)
                .await?
                .rows_affected();
                if run_binding == 1 {
                    1
                } else {
                    sqlx::query(
                        r#"
                        UPDATE agent_conversation_bindings
                        SET binding_status='model_rebind_required'
                        WHERE conversation_id = (
                            SELECT (input->>'conversation_id')::uuid
                            FROM agent_runs
                            WHERE id=$1
                        )
                        AND binding_status='executable'
                        "#,
                    )
                    .bind(id)
                    .execute(&self.pool)
                    .await?
                    .rows_affected()
                }
            }
            AuditedCallOwner::EvalRun(_) => 1,
        };
        if affected != 1 {
            return Err(ContextAuditRepositoryError::InvalidRecord(
                "Context binding 已阻断或不存在".into(),
            )
            .into());
        }
        Ok(())
    }

    async fn persist_snapshot(
        &self,
        input: PersistContextSnapshot,
    ) -> Result<Uuid, novex_agent::BoxError> {
        Ok(self.persist_snapshot_record(input).await?.id)
    }

    async fn persist_attempt(
        &self,
        input: PersistContextCompileAttempt,
    ) -> Result<Uuid, novex_agent::BoxError> {
        Ok(self.persist_attempt_record(input).await?.id)
    }

    async fn link_failure(
        &self,
        owner: AuditedCallOwner,
        attempt_id: Uuid,
        step_id: Option<Uuid>,
    ) -> Result<(), novex_agent::BoxError> {
        PostgresContextAuditRepository::link_failure(self, owner, attempt_id, step_id).await?;
        Ok(())
    }
}
