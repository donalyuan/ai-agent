use crate::application::agents::kernel::active_rust_definition_binding;
use crate::domain::conversation::ModelBindingEvidence;
use crate::model_routing::model_binding_evidence;
use novex_ai_core::{sha256_hex, DefinitionRegistry};
use novex_model::ModelExecutionSnapshot;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use std::{collections::BTreeMap, fmt, fs, sync::Arc};
use uuid::Uuid;

const CONTEXT_EVAL_CONTRACT: &str =
    include_str!("../../../agent-definitions/fixtures/context-eval-contract.json");
const CONTEXT_EVAL_CONTRACT_REFERENCE: &str =
    "agent-definitions/fixtures/context-eval-contract.json";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationDisposition {
    Equivalent,
    ContextMigrationRequired,
    ModelConfigurationMissing,
    Unmappable,
    LegacyPartialAudit,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HistoryMigrationItem {
    pub runtime: String,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub source_type: String,
    pub agent_key: Option<String>,
    pub node_keys: Vec<String>,
    pub parent_entity_id: Option<Uuid>,
    pub disposition: MigrationDisposition,
    pub reason_code: String,
    pub evidence: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HistoryMigrationPlan {
    pub schema_version: String,
    pub dry_run: bool,
    pub summary: BTreeMap<String, u64>,
    pub items: Vec<HistoryMigrationItem>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HistoryMigrationBackupEvidence {
    pub reference: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContextBaselineEvidence {
    pub report_id: String,
    pub reference: String,
    pub sha256: String,
    pub equivalent_nodes: Vec<String>,
}

#[derive(Deserialize)]
struct ContextEvalContract {
    production_nodes: Vec<String>,
    baseline_report: ContextBaselineReport,
}

#[derive(Deserialize)]
struct ContextBaselineReport {
    report_id: String,
    mode: String,
    passed: bool,
    actual_real_model_calls: u64,
    node_results: Vec<ContextBaselineNodeResult>,
}

#[derive(Deserialize)]
struct ContextBaselineNodeResult {
    node_key: String,
    equivalent: bool,
}

impl ContextBaselineEvidence {
    pub fn from_contract_json(value: &str, reference: &str) -> Result<Self, HistoryMigrationError> {
        let contract: ContextEvalContract = serde_json::from_str(value)?;
        let mut production_nodes = contract.production_nodes;
        production_nodes.sort();
        production_nodes.dedup();
        let mut equivalent_nodes = contract
            .baseline_report
            .node_results
            .into_iter()
            .filter_map(|result| result.equivalent.then_some(result.node_key))
            .collect::<Vec<_>>();
        equivalent_nodes.sort();
        equivalent_nodes.dedup();
        if contract.baseline_report.report_id.trim().is_empty()
            || contract.baseline_report.mode != "golden_baseline"
            || !contract.baseline_report.passed
            || contract.baseline_report.actual_real_model_calls != 0
            || production_nodes != equivalent_nodes
        {
            return Err(HistoryMigrationError::InvalidPlan(
                "Context baseline evidence is incomplete or non-equivalent".into(),
            ));
        }
        Ok(Self {
            report_id: contract.baseline_report.report_id,
            reference: reference.into(),
            sha256: sha256_hex(value.as_bytes()),
            equivalent_nodes,
        })
    }

    fn covers(&self, node_keys: &[String]) -> bool {
        node_keys
            .iter()
            .all(|node_key| self.equivalent_nodes.binary_search(node_key).is_ok())
    }

    fn redacted_json(&self) -> Value {
        json!({
            "report_id": self.report_id,
            "reference": self.reference,
            "sha256": self.sha256,
            "real_model_calls": 0,
        })
    }
}

#[derive(Clone)]
pub struct PostgresHistoryMigrator {
    pool: PgPool,
    registry: Arc<DefinitionRegistry>,
    baseline: Option<ContextBaselineEvidence>,
}

impl PostgresHistoryMigrator {
    pub fn new(pool: PgPool, registry: Arc<DefinitionRegistry>) -> Self {
        let baseline = ContextBaselineEvidence::from_contract_json(
            CONTEXT_EVAL_CONTRACT,
            CONTEXT_EVAL_CONTRACT_REFERENCE,
        )
        .ok();
        Self {
            pool,
            registry,
            baseline,
        }
    }

    pub fn with_baseline_evidence(
        pool: PgPool,
        registry: Arc<DefinitionRegistry>,
        baseline: Option<ContextBaselineEvidence>,
    ) -> Self {
        Self {
            pool,
            registry,
            baseline,
        }
    }

    /// Produces a deterministic report without writing bindings, events, calls, or domain data.
    pub async fn plan(&self) -> Result<HistoryMigrationPlan, HistoryMigrationError> {
        let mut items = Vec::new();
        let conversations = sqlx::query(
            r#"
            SELECT conversation.id, conversation.agent_type, conversation.metadata
            FROM agent_conversations conversation
            LEFT JOIN agent_conversation_bindings binding
              ON binding.conversation_id = conversation.id
            LEFT JOIN agent_history_migration_events event
              ON event.entity_type = 'conversation' AND event.entity_id = conversation.id
            WHERE binding.conversation_id IS NULL
              AND event.entity_id IS NULL
            ORDER BY conversation.id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        for row in conversations {
            let id: Uuid = row.try_get("id")?;
            let agent_type: String = row.try_get("agent_type")?;
            let metadata: Value = row.try_get("metadata")?;
            let parent_entity_id = parent_conversation_id(&metadata);
            let Some(agent_key) = conversation_definition_key(&agent_type) else {
                items.push(HistoryMigrationItem {
                    runtime: "rust".into(),
                    entity_type: "conversation".into(),
                    entity_id: id,
                    source_type: agent_type,
                    agent_key: None,
                    node_keys: Vec::new(),
                    parent_entity_id,
                    disposition: MigrationDisposition::Unmappable,
                    reason_code: "unknown_agent_type".into(),
                    evidence: json!({"legacy_text_exposed":false}),
                });
                continue;
            };
            let definition = active_rust_definition_binding(&self.registry, agent_key)
                .map_err(HistoryMigrationError::Definition)?;
            let node_keys = sorted_object_keys(&definition.context_policy_bindings)?;
            let baseline = self
                .baseline
                .as_ref()
                .filter(|evidence| evidence.covers(&node_keys));
            let model = trusted_conversation_model(&self.pool, &self.registry, id).await?;
            let (disposition, reason_code) = if baseline.is_none() {
                (
                    MigrationDisposition::ContextMigrationRequired,
                    "baseline_equivalence_evidence_missing",
                )
            } else if model.is_none() {
                (
                    MigrationDisposition::ModelConfigurationMissing,
                    "model_configuration_missing",
                )
            } else {
                (MigrationDisposition::Equivalent, "baseline_equivalent")
            };
            items.push(HistoryMigrationItem {
                runtime: "rust".into(),
                entity_type: "conversation".into(),
                entity_id: id,
                source_type: agent_type,
                agent_key: Some(agent_key.into()),
                node_keys,
                parent_entity_id,
                disposition,
                reason_code: reason_code.into(),
                evidence: json!({
                    "baseline": baseline.map(ContextBaselineEvidence::redacted_json),
                    "model": model.as_ref().map(redacted_model_evidence),
                    "legacy_text_exposed": false,
                }),
            });
        }

        let runs = sqlx::query(
            r#"
            SELECT run.id, run.agent_type
            FROM agent_runs run
            LEFT JOIN agent_run_bindings binding ON binding.agent_run_id = run.id
            LEFT JOIN agent_history_migration_events event
              ON event.entity_type = 'agent_run' AND event.entity_id = run.id
            WHERE binding.agent_run_id IS NULL
              AND event.entity_id IS NULL
              AND NOT run.legacy_partial_audit
            ORDER BY run.id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        items.extend(runs.into_iter().map(|row| HistoryMigrationItem {
            runtime: "rust".into(),
            entity_type: "agent_run".into(),
            entity_id: row.get("id"),
            source_type: row.get("agent_type"),
            agent_key: None,
            node_keys: Vec::new(),
            parent_entity_id: None,
            disposition: MigrationDisposition::LegacyPartialAudit,
            reason_code: "historical_context_evidence_incomplete".into(),
            evidence: json!({
                "prompt_snapshot":"missing",
                "context_snapshot":"missing",
                "model_call_created":false
            }),
        }));
        Ok(plan_with_items(true, items))
    }

    pub async fn apply(
        &self,
        backup: &HistoryMigrationBackupEvidence,
    ) -> Result<HistoryMigrationPlan, HistoryMigrationError> {
        verify_backup(backup)?;
        let plan = self.plan().await?;
        let mut transaction = self.pool.begin().await?;
        let before = source_inventory(&mut transaction).await?;
        for item in &plan.items {
            match item.entity_type.as_str() {
                "conversation" => {
                    let Some(agent_key) = item.agent_key.as_deref() else {
                        record_event(&mut transaction, item, backup, &before).await?;
                        continue;
                    };
                    let mut definition = active_rust_definition_binding(&self.registry, agent_key)
                        .map_err(HistoryMigrationError::Definition)?;
                    definition.migration_source = Some(
                        match item.disposition {
                            MigrationDisposition::Equivalent => "context_history_v2_equivalent",
                            MigrationDisposition::ModelConfigurationMissing => {
                                "context_history_v2_model_missing"
                            }
                            MigrationDisposition::ContextMigrationRequired => {
                                "context_history_v2_read_only"
                            }
                            _ => "context_history_v2_unmapped",
                        }
                        .into(),
                    );
                    definition.parent_conversation_id = item.parent_entity_id;
                    let model = trusted_conversation_model_in(
                        &mut transaction,
                        &self.registry,
                        item.entity_id,
                    )
                    .await?;
                    if item.disposition == MigrationDisposition::Equivalent && model.is_none() {
                        return Err(HistoryMigrationError::InvalidPlan(
                            "model evidence changed after migration planning".into(),
                        ));
                    }
                    let has_equivalent_context = matches!(
                        item.disposition,
                        MigrationDisposition::Equivalent
                            | MigrationDisposition::ModelConfigurationMissing
                    );
                    let binding_status = match item.disposition {
                        MigrationDisposition::Equivalent => "executable",
                        MigrationDisposition::ModelConfigurationMissing => "definition_bound",
                        _ => "read_only",
                    };
                    sqlx::query(
                        r#"
                        INSERT INTO agent_conversation_bindings (
                            conversation_id, agent_key, agent_version, agent_digest,
                            prompt_bindings, context_policy_bindings, registry_digest,
                            model_id, behavior_fingerprint, model_capabilities,
                            tokenizer_profile_key, tokenizer_profile_version,
                            tokenizer_profile_digest, binding_status, migration_source,
                            parent_conversation_id
                        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)
                        ON CONFLICT (conversation_id) DO NOTHING
                        "#,
                    )
                    .bind(item.entity_id)
                    .bind(&definition.agent_key)
                    .bind(&definition.agent_version)
                    .bind(&definition.agent_digest)
                    .bind(&definition.prompt_bindings)
                    .bind(has_equivalent_context.then_some(&definition.context_policy_bindings))
                    .bind(&definition.registry_digest)
                    .bind(model.as_ref().map(|evidence| evidence.model_id))
                    .bind(
                        model
                            .as_ref()
                            .map(|evidence| evidence.behavior_fingerprint.as_str()),
                    )
                    .bind(model.as_ref().map(|evidence| &evidence.model_capabilities))
                    .bind(
                        model
                            .as_ref()
                            .map(|evidence| evidence.tokenizer_profile_key.as_str()),
                    )
                    .bind(
                        model
                            .as_ref()
                            .map(|evidence| evidence.tokenizer_profile_version.as_str()),
                    )
                    .bind(
                        model
                            .as_ref()
                            .map(|evidence| evidence.tokenizer_profile_digest.as_str()),
                    )
                    .bind(binding_status)
                    .bind(&definition.migration_source)
                    .bind(definition.parent_conversation_id)
                    .execute(&mut *transaction)
                    .await?;
                    record_event(&mut transaction, item, backup, &before).await?;
                }
                "agent_run" => {
                    sqlx::query(
                        "UPDATE agent_runs SET legacy_partial_audit=TRUE WHERE id=$1 AND NOT legacy_partial_audit",
                    )
                    .bind(item.entity_id)
                    .execute(&mut *transaction)
                    .await?;
                    record_event(&mut transaction, item, backup, &before).await?;
                }
                _ => {
                    return Err(HistoryMigrationError::InvalidPlan(
                        "unknown migration entity".into(),
                    ))
                }
            }
        }
        let after = source_inventory(&mut transaction).await?;
        if after != before {
            return Err(HistoryMigrationError::InvalidPlan(
                "source Conversation/Message/Run identity changed during migration".into(),
            ));
        }
        transaction.commit().await?;
        Ok(HistoryMigrationPlan {
            dry_run: false,
            ..plan
        })
    }
}

fn plan_with_items(dry_run: bool, items: Vec<HistoryMigrationItem>) -> HistoryMigrationPlan {
    let mut summary = BTreeMap::new();
    for item in &items {
        *summary
            .entry(disposition_name(&item.disposition).to_string())
            .or_insert(0) += 1;
    }
    HistoryMigrationPlan {
        schema_version: "2".into(),
        dry_run,
        summary,
        items,
    }
}

fn conversation_definition_key(agent_type: &str) -> Option<&'static str> {
    match agent_type {
        "script" => Some("video.script"),
        "topic" => Some("video.topic"),
        "sound" => Some("video.sound"),
        "work" => Some("video.work"),
        _ => None,
    }
}

fn parent_conversation_id(metadata: &Value) -> Option<Uuid> {
    metadata
        .get("parent_conversation_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
}

fn sorted_object_keys(value: &Value) -> Result<Vec<String>, HistoryMigrationError> {
    let mut keys = value
        .as_object()
        .ok_or_else(|| {
            HistoryMigrationError::Definition("Context Policy binding is invalid".into())
        })?
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    Ok(keys)
}

fn redacted_model_evidence(evidence: &ModelBindingEvidence) -> Value {
    json!({
        "model_id": evidence.model_id,
        "behavior_fingerprint": evidence.behavior_fingerprint,
        "capabilities": evidence.model_capabilities,
        "tokenizer_profile": {
            "key": evidence.tokenizer_profile_key,
            "version": evidence.tokenizer_profile_version,
            "digest": evidence.tokenizer_profile_digest,
        }
    })
}

fn parse_model_evidence(
    registry: &DefinitionRegistry,
    model_id: Uuid,
    snapshot: Value,
) -> Result<Option<ModelBindingEvidence>, HistoryMigrationError> {
    let Ok(snapshot) = serde_json::from_value::<ModelExecutionSnapshot>(snapshot) else {
        return Ok(None);
    };
    if snapshot.model_id != model_id {
        return Ok(None);
    }
    Ok(model_binding_evidence(registry, &snapshot).ok())
}

async fn trusted_conversation_model(
    pool: &PgPool,
    registry: &DefinitionRegistry,
    conversation_id: Uuid,
) -> Result<Option<ModelBindingEvidence>, HistoryMigrationError> {
    let row = sqlx::query(
        r#"
        SELECT model_id, model_snapshot
        FROM agent_runs
        WHERE input->>'conversation_id' = $1
          AND model_id IS NOT NULL
          AND model_snapshot IS NOT NULL
        ORDER BY started_at ASC, id ASC
        LIMIT 1
        "#,
    )
    .bind(conversation_id.to_string())
    .fetch_optional(pool)
    .await?;
    row.map(|row| parse_model_evidence(registry, row.get("model_id"), row.get("model_snapshot")))
        .transpose()
        .map(Option::flatten)
}

async fn trusted_conversation_model_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    registry: &DefinitionRegistry,
    conversation_id: Uuid,
) -> Result<Option<ModelBindingEvidence>, HistoryMigrationError> {
    let row = sqlx::query(
        r#"
        SELECT model_id, model_snapshot
        FROM agent_runs
        WHERE input->>'conversation_id' = $1
          AND model_id IS NOT NULL
          AND model_snapshot IS NOT NULL
        ORDER BY started_at ASC, id ASC
        LIMIT 1
        "#,
    )
    .bind(conversation_id.to_string())
    .fetch_optional(&mut **transaction)
    .await?;
    row.map(|row| parse_model_evidence(registry, row.get("model_id"), row.get("model_snapshot")))
        .transpose()
        .map(Option::flatten)
}

fn verify_backup(backup: &HistoryMigrationBackupEvidence) -> Result<(), HistoryMigrationError> {
    if backup.reference.trim().is_empty() || !valid_sha256(&backup.sha256) {
        return Err(HistoryMigrationError::InvalidPlan(
            "valid backup reference and sha256 are required before migration".into(),
        ));
    }
    let bytes = fs::read(&backup.reference).map_err(|error| {
        HistoryMigrationError::InvalidPlan(format!("migration backup is not readable: {error}"))
    })?;
    if sha256_hex(&bytes) != backup.sha256 {
        return Err(HistoryMigrationError::InvalidPlan(
            "migration backup sha256 does not match the referenced file".into(),
        ));
    }
    Ok(())
}

async fn record_event(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    item: &HistoryMigrationItem,
    backup: &HistoryMigrationBackupEvidence,
    before: &SourceInventory,
) -> Result<(), HistoryMigrationError> {
    sqlx::query(
        r#"
        INSERT INTO agent_history_migration_events (
            entity_type, entity_id, disposition, evidence
        ) VALUES ($1,$2,$3,$4)
        ON CONFLICT (entity_type, entity_id) DO NOTHING
        "#,
    )
    .bind(&item.entity_type)
    .bind(item.entity_id)
    .bind(disposition_name(&item.disposition))
    .bind(json!({
        "source_evidence": item.evidence,
        "reason_code": item.reason_code,
        "node_keys": item.node_keys,
        "parent_entity_id": item.parent_entity_id,
        "backup": backup,
        "source_counts_before": before.counts(),
    }))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

#[derive(Eq, PartialEq)]
struct SourceInventory {
    conversations: Vec<Uuid>,
    messages: Vec<Uuid>,
    runs: Vec<Uuid>,
}

impl SourceInventory {
    fn counts(&self) -> Value {
        json!({
            "conversations": self.conversations.len(),
            "messages": self.messages.len(),
            "runs": self.runs.len(),
        })
    }
}

async fn source_inventory(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<SourceInventory, HistoryMigrationError> {
    Ok(SourceInventory {
        conversations: sqlx::query_scalar("SELECT id FROM agent_conversations ORDER BY id")
            .fetch_all(&mut **transaction)
            .await?,
        messages: sqlx::query_scalar("SELECT id FROM agent_messages ORDER BY id")
            .fetch_all(&mut **transaction)
            .await?,
        runs: sqlx::query_scalar("SELECT id FROM agent_runs ORDER BY id")
            .fetch_all(&mut **transaction)
            .await?,
    })
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn disposition_name(value: &MigrationDisposition) -> &'static str {
    match value {
        MigrationDisposition::Equivalent => "equivalent",
        MigrationDisposition::ContextMigrationRequired => "context_migration_required",
        MigrationDisposition::ModelConfigurationMissing => "model_configuration_missing",
        MigrationDisposition::Unmappable => "unmappable",
        MigrationDisposition::LegacyPartialAudit => "legacy_partial_audit",
    }
}

#[derive(Debug)]
pub enum HistoryMigrationError {
    Storage(sqlx::Error),
    Serialization(serde_json::Error),
    Definition(String),
    InvalidPlan(String),
}

impl fmt::Display for HistoryMigrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(formatter, "history migration storage error: {error}"),
            Self::Serialization(error) => {
                write!(formatter, "history migration serialization error: {error}")
            }
            Self::Definition(message) | Self::InvalidPlan(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for HistoryMigrationError {}

impl From<sqlx::Error> for HistoryMigrationError {
    fn from(value: sqlx::Error) -> Self {
        Self::Storage(value)
    }
}

impl From<serde_json::Error> for HistoryMigrationError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serialization(value)
    }
}
