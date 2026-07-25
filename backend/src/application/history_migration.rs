use crate::application::agents::kernel::active_rust_definition_binding;
use novex_ai_core::{behavior_fingerprint, DefinitionRegistry, ModelBehavior, ModelCapabilities};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use std::{fmt, sync::Arc};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationDisposition {
    AutoBindWithModel,
    AwaitFirstModelBinding,
    LegacyPartialAudit,
    UnmappedReadOnly,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HistoryMigrationItem {
    pub entity_type: String,
    pub entity_id: Uuid,
    pub source_type: String,
    pub definition_key: Option<String>,
    pub disposition: MigrationDisposition,
    pub evidence: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HistoryMigrationPlan {
    pub schema_version: String,
    pub dry_run: bool,
    pub items: Vec<HistoryMigrationItem>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HistoryMigrationBackupEvidence {
    pub reference: String,
    pub sha256: String,
}

#[derive(Clone)]
pub struct PostgresHistoryMigrator {
    pool: PgPool,
    registry: Arc<DefinitionRegistry>,
}

impl PostgresHistoryMigrator {
    pub fn new(pool: PgPool, registry: Arc<DefinitionRegistry>) -> Self {
        Self { pool, registry }
    }

    /// Produces a deterministic plan without writing bindings, events, calls, or domain data.
    pub async fn plan(&self) -> Result<HistoryMigrationPlan, HistoryMigrationError> {
        let mut items = Vec::new();
        let conversations = sqlx::query(
            r#"
            SELECT conversation.id, conversation.agent_type
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
            let Some(definition_key) = conversation_definition_key(&agent_type) else {
                items.push(HistoryMigrationItem {
                    entity_type: "conversation".into(),
                    entity_id: id,
                    source_type: agent_type,
                    definition_key: None,
                    disposition: MigrationDisposition::UnmappedReadOnly,
                    evidence: json!({"reason":"unknown_agent_type"}),
                });
                continue;
            };
            let model = trusted_conversation_model(&self.pool, id).await?;
            items.push(HistoryMigrationItem {
                entity_type: "conversation".into(),
                entity_id: id,
                source_type: agent_type,
                definition_key: Some(definition_key.into()),
                disposition: if model.is_some() {
                    MigrationDisposition::AutoBindWithModel
                } else {
                    MigrationDisposition::AwaitFirstModelBinding
                },
                evidence: model
                    .as_ref()
                    .map(TrustedModelEvidence::redacted_json)
                    .unwrap_or_else(|| json!({"model_evidence":"insufficient"})),
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
            entity_type: "agent_run".into(),
            entity_id: row.get("id"),
            source_type: row.get("agent_type"),
            definition_key: None,
            disposition: MigrationDisposition::LegacyPartialAudit,
            evidence: json!({
                "prompt_snapshot":"missing",
                "context_snapshot":"missing",
                "model_call_created":false
            }),
        }));
        Ok(HistoryMigrationPlan {
            schema_version: "1".into(),
            dry_run: true,
            items,
        })
    }

    pub async fn apply(
        &self,
        backup: &HistoryMigrationBackupEvidence,
    ) -> Result<HistoryMigrationPlan, HistoryMigrationError> {
        if backup.reference.trim().is_empty() || !valid_sha256(&backup.sha256) {
            return Err(HistoryMigrationError::InvalidPlan(
                "valid backup reference and sha256 are required before migration".into(),
            ));
        }
        let plan = self.plan().await?;
        let mut transaction = self.pool.begin().await?;
        let before = source_inventory(&mut transaction).await?;
        for item in &plan.items {
            match item.entity_type.as_str() {
                "conversation" => {
                    let Some(definition_key) = item.definition_key.as_deref() else {
                        record_event(&mut transaction, item, backup).await?;
                        continue;
                    };
                    let mut definition =
                        active_rust_definition_binding(&self.registry, definition_key)
                            .map_err(HistoryMigrationError::Definition)?;
                    definition.migration_source = Some("history_v1".into());
                    let model = if item.disposition == MigrationDisposition::AutoBindWithModel {
                        trusted_conversation_model_in(&mut transaction, item.entity_id).await?
                    } else {
                        None
                    };
                    sqlx::query(
                        r#"
                        INSERT INTO agent_conversation_bindings (
                            conversation_id, agent_key, agent_version, agent_digest,
                            prompt_bindings, registry_digest, model_id, behavior_fingerprint,
                            model_capabilities, binding_status, migration_source
                        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
                        ON CONFLICT (conversation_id) DO NOTHING
                        "#,
                    )
                    .bind(item.entity_id)
                    .bind(&definition.agent_key)
                    .bind(&definition.agent_version)
                    .bind(&definition.agent_digest)
                    .bind(&definition.prompt_bindings)
                    .bind(&definition.registry_digest)
                    .bind(model.as_ref().map(|evidence| evidence.model_id))
                    .bind(
                        model
                            .as_ref()
                            .map(|evidence| evidence.behavior_fingerprint.as_str()),
                    )
                    .bind(model.as_ref().map(|evidence| &evidence.capabilities))
                    .bind(if model.is_some() {
                        "executable"
                    } else {
                        "definition_bound"
                    })
                    .bind("history_v1")
                    .execute(&mut *transaction)
                    .await?;
                    record_event(&mut transaction, item, backup).await?;
                }
                "agent_run" => {
                    sqlx::query(
                        "UPDATE agent_runs SET legacy_partial_audit=TRUE WHERE id=$1 AND NOT legacy_partial_audit",
                    )
                    .bind(item.entity_id)
                    .execute(&mut *transaction)
                    .await?;
                    record_event(&mut transaction, item, backup).await?;
                }
                _ => {
                    return Err(HistoryMigrationError::InvalidPlan(
                        "unknown migration entity".into(),
                    ))
                }
            }
        }
        if source_inventory(&mut transaction).await? != before {
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

fn conversation_definition_key(agent_type: &str) -> Option<&'static str> {
    match agent_type {
        "script" => Some("video.script"),
        "topic" => Some("video.topic"),
        "sound" => Some("video.sound"),
        "work" => Some("video.work"),
        _ => None,
    }
}

struct TrustedModelEvidence {
    model_id: Uuid,
    behavior_fingerprint: String,
    capabilities: Value,
}

impl TrustedModelEvidence {
    fn redacted_json(&self) -> Value {
        json!({
            "model_id":self.model_id,
            "behavior_fingerprint":self.behavior_fingerprint,
            "capabilities":self.capabilities
        })
    }
}

fn parse_model_evidence(
    model_id: Uuid,
    snapshot: Value,
) -> Result<Option<TrustedModelEvidence>, HistoryMigrationError> {
    let Some(snapshot_model_id) = snapshot
        .get("model_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
    else {
        return Ok(None);
    };
    let Some(protocol) = snapshot.get("api_protocol").and_then(Value::as_str) else {
        return Ok(None);
    };
    let Some(request_base_url) = snapshot.get("request_base_url").and_then(Value::as_str) else {
        return Ok(None);
    };
    let Some(upstream_model) = snapshot.get("upstream_model").and_then(Value::as_str) else {
        return Ok(None);
    };
    let Some(max_output_tokens) = snapshot
        .get("max_output_tokens")
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
    else {
        return Ok(None);
    };
    let Some(settings) = snapshot.get("settings").filter(|value| value.is_object()) else {
        return Ok(None);
    };
    let Some(context_window) = settings
        .get("context_window")
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
    else {
        return Ok(None);
    };
    if snapshot_model_id != model_id
        || snapshot.get("model_type").and_then(Value::as_str) != Some("text")
        || max_output_tokens > u32::MAX as u64
    {
        return Ok(None);
    }
    let reasoning_effort = match snapshot.get("reasoning_effort") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) if !value.is_empty() => Some(value.clone()),
        _ => return Ok(None),
    };
    let behavior = ModelBehavior {
        protocol: protocol.into(),
        request_base_url: request_base_url.into(),
        upstream_model: upstream_model.into(),
        reasoning_effort: reasoning_effort.clone(),
        max_output_tokens: max_output_tokens as u32,
        context_window,
        settings: settings.clone(),
    };
    let Ok((behavior_fingerprint, _)) = behavior_fingerprint(&behavior) else {
        return Ok(None);
    };
    let capabilities = ModelCapabilities {
        text: true,
        tool_calling: false,
        structured_output: matches!(protocol, "openai_responses" | "openai_chat_completions"),
        vision: false,
        reasoning: reasoning_effort.is_some(),
        context_window,
    };
    Ok(Some(TrustedModelEvidence {
        model_id,
        behavior_fingerprint,
        capabilities: serde_json::to_value(capabilities)?,
    }))
}

async fn trusted_conversation_model(
    pool: &PgPool,
    conversation_id: Uuid,
) -> Result<Option<TrustedModelEvidence>, HistoryMigrationError> {
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
    row.map(|row| parse_model_evidence(row.get("model_id"), row.get("model_snapshot")))
        .transpose()
        .map(Option::flatten)
}

async fn trusted_conversation_model_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    conversation_id: Uuid,
) -> Result<Option<TrustedModelEvidence>, HistoryMigrationError> {
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
    row.map(|row| parse_model_evidence(row.get("model_id"), row.get("model_snapshot")))
        .transpose()
        .map(Option::flatten)
}

async fn record_event(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    item: &HistoryMigrationItem,
    backup: &HistoryMigrationBackupEvidence,
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
        "backup": backup,
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
        MigrationDisposition::AutoBindWithModel => "auto_bind_with_model",
        MigrationDisposition::AwaitFirstModelBinding => "await_first_model_binding",
        MigrationDisposition::LegacyPartialAudit => "legacy_partial_audit",
        MigrationDisposition::UnmappedReadOnly => "unmapped_read_only",
    }
}

#[derive(Debug)]
pub enum HistoryMigrationError {
    Storage(sqlx::Error),
    Serialization(serde_json::Error),
    Definition(String),
    ModelEvidence(String),
    InvalidPlan(String),
}

impl fmt::Display for HistoryMigrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(formatter, "history migration storage error: {error}"),
            Self::Serialization(error) => {
                write!(formatter, "history migration serialization error: {error}")
            }
            Self::Definition(message)
            | Self::ModelEvidence(message)
            | Self::InvalidPlan(message) => formatter.write_str(message),
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
