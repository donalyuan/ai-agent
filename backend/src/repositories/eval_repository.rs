use novex_eval::{CandidateRef, EvalDefinitionKind, EvalMode, EvalReport, EvalRunSpec};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use std::fmt;
use uuid::Uuid;

#[derive(Clone)]
pub struct PostgresEvalRepository {
    pool: PgPool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredEvalRun {
    pub id: Uuid,
    pub status: String,
    pub validation_mode: String,
    pub definition_kind: String,
    pub candidate_key: String,
    pub candidate_version: String,
    pub candidate_digest: String,
    pub approval_snapshot: Value,
    pub context_case_set: Option<Value>,
    pub context_policy: Option<Value>,
    pub tokenizer_profile: Option<Value>,
    pub actual_real_model_calls: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredEvalReport {
    pub id: Uuid,
    pub eval_run_id: Uuid,
    pub passed: bool,
    pub gate_results: Value,
    pub aggregate_metrics: Value,
    pub redacted_case_results: Value,
    pub context_node_results: Option<Value>,
    pub context_selection_diff: Option<Value>,
    pub context_budget_ledgers: Option<Value>,
    pub tokenizer_metrics: Option<Value>,
    pub source_deleted: bool,
}

impl PostgresEvalRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_run(
        &self,
        spec: &EvalRunSpec,
    ) -> Result<StoredEvalRun, EvalRepositoryError> {
        validate_spec(spec)?;
        let model_id = spec
            .model_binding
            .as_ref()
            .map(|binding| Uuid::parse_str(&binding.model_id))
            .transpose()
            .map_err(|_| EvalRepositoryError::InvalidInput("invalid model_id".into()))?;
        let fingerprint = spec
            .model_binding
            .as_ref()
            .map(|binding| binding.behavior_fingerprint.as_str());
        let approval_snapshot = json!({
            "schema_version": if spec.context.is_some() { "2" } else { "1" },
            "approved_real_calls": spec.budget.approved_real_calls,
            "model_binding": spec.model_binding,
            "budget": spec.budget,
            "definition_kind": definition_kind_name(spec.candidate.definition_kind),
            "context": spec.context,
        });
        let context_case_set = spec.context.as_ref().map(|context| {
            json!({
                "schema_version": context.schema_version,
                "version": spec.case_set_version,
            })
        });
        let context_policy = spec
            .context
            .as_ref()
            .map(|context| serde_json::to_value(&context.policy))
            .transpose()?;
        let tokenizer_profile = spec
            .context
            .as_ref()
            .map(|context| serde_json::to_value(&context.tokenizer_profile))
            .transpose()?;
        let row = sqlx::query(
            r#"
            INSERT INTO eval_runs (
                definition_kind, candidate_key, candidate_version, candidate_digest,
                baseline_key, baseline_version, case_set_version, evaluator_version,
                validation_mode, model_id, behavior_fingerprint, approved_real_calls,
                approval_snapshot, max_cases, max_input_tokens, max_output_tokens,
                max_retries, max_cost_micros, context_case_set, context_policy,
                tokenizer_profile
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16, $17, $18, $19, $20, $21
            )
            RETURNING id, status, validation_mode, definition_kind, candidate_key,
                      candidate_version, candidate_digest, approval_snapshot,
                      context_case_set, context_policy, tokenizer_profile,
                      actual_real_model_calls
            "#,
        )
        .bind(definition_kind_name(spec.candidate.definition_kind))
        .bind(&spec.candidate.key)
        .bind(&spec.candidate.version)
        .bind(&spec.candidate.digest)
        .bind(spec.baseline.as_ref().map(|baseline| baseline.key.as_str()))
        .bind(
            spec.baseline
                .as_ref()
                .map(|baseline| baseline.version.as_str()),
        )
        .bind(&spec.case_set_version)
        .bind(&spec.evaluator_version)
        .bind(mode_name(spec.mode))
        .bind(model_id)
        .bind(fingerprint)
        .bind(spec.budget.approved_real_calls)
        .bind(approval_snapshot)
        .bind(to_i32(spec.budget.max_cases, "max_cases")?)
        .bind(to_i64(spec.budget.max_input_tokens, "max_input_tokens")?)
        .bind(to_i64(spec.budget.max_output_tokens, "max_output_tokens")?)
        .bind(to_i32(spec.budget.max_retries, "max_retries")?)
        .bind(to_i64(spec.budget.max_cost_micros, "max_cost_micros")?)
        .bind(context_case_set)
        .bind(context_policy)
        .bind(tokenizer_profile)
        .fetch_one(&self.pool)
        .await?;
        stored_run(row)
    }

    pub async fn complete_run(
        &self,
        run_id: Uuid,
        report: &EvalReport,
    ) -> Result<StoredEvalReport, EvalRepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let run = sqlx::query(
            r#"
            SELECT definition_kind, candidate_key, candidate_version, candidate_digest,
                   validation_mode, status
            FROM eval_runs WHERE id = $1 FOR UPDATE
            "#,
        )
        .bind(run_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(EvalRepositoryError::NotFound(run_id))?;
        let status: String = run.try_get("status")?;
        if status != "pending" && status != "running" {
            return Err(EvalRepositoryError::Immutable);
        }
        if run.try_get::<String, _>("definition_kind")?
            != definition_kind_name(report.candidate.definition_kind)
            || run.try_get::<String, _>("candidate_key")? != report.candidate.key
            || run.try_get::<String, _>("candidate_version")? != report.candidate.version
            || run.try_get::<String, _>("candidate_digest")? != report.candidate.digest
            || run.try_get::<String, _>("validation_mode")? != mode_name(report.mode)
        {
            return Err(EvalRepositoryError::EvidenceMismatch);
        }
        let usage = &report.usage;
        let terminal = if report.passed { "passed" } else { "failed" };
        let context_node_results = report
            .context
            .as_ref()
            .map(|context| serde_json::to_value(&context.node_results))
            .transpose()?;
        let context_selection_diff = report
            .context
            .as_ref()
            .map(|context| serde_json::to_value(&context.selection_diff))
            .transpose()?;
        let context_budget_ledgers = report
            .context
            .as_ref()
            .map(|context| serde_json::to_value(&context.budget_ledgers))
            .transpose()?;
        let tokenizer_metrics = report
            .context
            .as_ref()
            .map(|context| serde_json::to_value(&context.tokenizer_metrics))
            .transpose()?;
        sqlx::query(
            r#"
            UPDATE eval_runs
            SET status = $2, actual_real_model_calls = $3, actual_cases = $4,
                actual_input_tokens = $5, actual_output_tokens = $6,
                actual_retries = $7, actual_cost_micros = $8,
                started_at = COALESCE(started_at, NOW()), completed_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(run_id)
        .bind(terminal)
        .bind(to_i32(
            report.actual_real_model_calls,
            "actual_real_model_calls",
        )?)
        .bind(to_i32(usage.cases, "actual_cases")?)
        .bind(to_i64(usage.input_tokens, "actual_input_tokens")?)
        .bind(to_i64(usage.output_tokens, "actual_output_tokens")?)
        .bind(to_i32(usage.retries, "actual_retries")?)
        .bind(to_i64(usage.cost_micros, "actual_cost_micros")?)
        .execute(&mut *transaction)
        .await?;
        let row = sqlx::query(
            r#"
            INSERT INTO eval_reports (
                eval_run_id, schema_version, passed, gate_results, aggregate_metrics,
                redacted_case_results, context_node_results, context_selection_diff,
                context_budget_ledgers, tokenizer_metrics
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id, eval_run_id, passed, gate_results, aggregate_metrics,
                      redacted_case_results, context_node_results,
                      context_selection_diff, context_budget_ledgers,
                      tokenizer_metrics, source_deleted
            "#,
        )
        .bind(run_id)
        .bind(&report.schema_version)
        .bind(report.passed)
        .bind(serde_json::to_value(&report.gates)?)
        .bind(serde_json::to_value(&report.usage)?)
        .bind(serde_json::to_value(&report.redacted_case_results)?)
        .bind(context_node_results)
        .bind(context_selection_diff)
        .bind(context_budget_ledgers)
        .bind(tokenizer_metrics)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        stored_report(row)
    }

    pub async fn activation_report_id(
        &self,
        candidate: &CandidateRef,
    ) -> Result<Uuid, EvalRepositoryError> {
        sqlx::query_scalar(
            r#"
            SELECT reports.id
            FROM eval_reports reports
            JOIN eval_runs runs ON runs.id = reports.eval_run_id
            WHERE runs.definition_kind = $1
              AND runs.candidate_key = $2
              AND runs.candidate_version = $3
              AND runs.candidate_digest = $4
              AND (
                    (runs.definition_kind IN ('context_policy', 'tokenizer_profile')
                        AND runs.validation_mode IN ('golden_baseline', 'zero_cost', 'real_model')
                        AND reports.context_node_results IS NOT NULL)
                    OR
                    (runs.definition_kind IN ('agent', 'prompt')
                        AND runs.validation_mode IN ('golden_baseline', 'real_model'))
              )
              AND runs.status = 'passed'
              AND reports.passed
            ORDER BY reports.completed_at DESC
            LIMIT 1
            "#,
        )
        .bind(definition_kind_name(candidate.definition_kind))
        .bind(&candidate.key)
        .bind(&candidate.version)
        .bind(&candidate.digest)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(EvalRepositoryError::ActivationEvidenceMissing)
    }
}

fn validate_spec(spec: &EvalRunSpec) -> Result<(), EvalRepositoryError> {
    let context_candidate = matches!(
        spec.candidate.definition_kind,
        EvalDefinitionKind::ContextPolicy | EvalDefinitionKind::TokenizerProfile
    );
    if spec.candidate.key.is_empty()
        || spec.candidate.version.is_empty()
        || spec.candidate.digest.len() != 64
        || spec.budget.max_cases == 0
        || context_candidate != spec.context.is_some()
    {
        return Err(EvalRepositoryError::InvalidInput(
            "candidate or budget is invalid".into(),
        ));
    }
    match spec.mode {
        EvalMode::RealModel if !spec.budget.approved_real_calls || spec.model_binding.is_none() => {
            Err(EvalRepositoryError::ApprovalRequired)
        }
        EvalMode::GoldenBaseline | EvalMode::ZeroCost
            if spec.budget.approved_real_calls
                || spec.model_binding.is_some()
                || spec.budget.max_cost_micros != 0 =>
        {
            Err(EvalRepositoryError::InvalidInput(
                "zero-cost run contains paid execution approval".into(),
            ))
        }
        _ => Ok(()),
    }
}

fn mode_name(mode: EvalMode) -> &'static str {
    match mode {
        EvalMode::GoldenBaseline => "golden_baseline",
        EvalMode::ZeroCost => "zero_cost",
        EvalMode::RealModel => "real_model",
    }
}

pub(crate) fn definition_kind_name(kind: EvalDefinitionKind) -> &'static str {
    match kind {
        EvalDefinitionKind::Agent => "agent",
        EvalDefinitionKind::Prompt => "prompt",
        EvalDefinitionKind::ContextPolicy => "context_policy",
        EvalDefinitionKind::TokenizerProfile => "tokenizer_profile",
    }
}

fn to_i64(value: u64, name: &str) -> Result<i64, EvalRepositoryError> {
    i64::try_from(value)
        .map_err(|_| EvalRepositoryError::InvalidInput(format!("{name} exceeds database range")))
}

fn to_i32(value: u64, name: &str) -> Result<i32, EvalRepositoryError> {
    i32::try_from(value)
        .map_err(|_| EvalRepositoryError::InvalidInput(format!("{name} exceeds database range")))
}

fn stored_run(row: sqlx::postgres::PgRow) -> Result<StoredEvalRun, EvalRepositoryError> {
    Ok(StoredEvalRun {
        id: row.try_get("id")?,
        status: row.try_get("status")?,
        validation_mode: row.try_get("validation_mode")?,
        definition_kind: row.try_get("definition_kind")?,
        candidate_key: row.try_get("candidate_key")?,
        candidate_version: row.try_get("candidate_version")?,
        candidate_digest: row.try_get("candidate_digest")?,
        approval_snapshot: row.try_get("approval_snapshot")?,
        context_case_set: row.try_get("context_case_set")?,
        context_policy: row.try_get("context_policy")?,
        tokenizer_profile: row.try_get("tokenizer_profile")?,
        actual_real_model_calls: row.try_get("actual_real_model_calls")?,
    })
}

fn stored_report(row: sqlx::postgres::PgRow) -> Result<StoredEvalReport, EvalRepositoryError> {
    Ok(StoredEvalReport {
        id: row.try_get("id")?,
        eval_run_id: row.try_get("eval_run_id")?,
        passed: row.try_get("passed")?,
        gate_results: row.try_get("gate_results")?,
        aggregate_metrics: row.try_get("aggregate_metrics")?,
        redacted_case_results: row.try_get("redacted_case_results")?,
        context_node_results: row.try_get("context_node_results")?,
        context_selection_diff: row.try_get("context_selection_diff")?,
        context_budget_ledgers: row.try_get("context_budget_ledgers")?,
        tokenizer_metrics: row.try_get("tokenizer_metrics")?,
        source_deleted: row.try_get("source_deleted")?,
    })
}

#[derive(Debug)]
pub enum EvalRepositoryError {
    Storage(sqlx::Error),
    Serialization(serde_json::Error),
    InvalidInput(String),
    ApprovalRequired,
    NotFound(Uuid),
    Immutable,
    EvidenceMismatch,
    ActivationEvidenceMissing,
}

impl fmt::Display for EvalRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(formatter, "eval repository error: {error}"),
            Self::Serialization(error) => write!(formatter, "eval serialization error: {error}"),
            Self::InvalidInput(message) => formatter.write_str(message),
            Self::ApprovalRequired => {
                formatter.write_str("real model evaluation requires approval")
            }
            Self::NotFound(id) => write!(formatter, "eval run not found: {id}"),
            Self::Immutable => formatter.write_str("completed eval run is immutable"),
            Self::EvidenceMismatch => {
                formatter.write_str("eval report does not match the fixed run")
            }
            Self::ActivationEvidenceMissing => {
                formatter.write_str("candidate has no immutable passing activation report")
            }
        }
    }
}

impl std::error::Error for EvalRepositoryError {}

impl From<sqlx::Error> for EvalRepositoryError {
    fn from(value: sqlx::Error) -> Self {
        Self::Storage(value)
    }
}

impl From<serde_json::Error> for EvalRepositoryError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serialization(value)
    }
}
