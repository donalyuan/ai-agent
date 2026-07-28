//! Production 命令的统一 actor、幂等作用域与事务内存储。

use super::canonical_digest;
use crate::{ProductionError, ProductionResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, Postgres, Transaction};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductionActor {
    pub actor_type: String,
    pub actor_id: String,
}

impl ProductionActor {
    pub fn local_operator() -> Self {
        Self {
            actor_type: "local_operator".into(),
            actor_id: "local_operator".into(),
        }
    }

    pub fn validate(&self) -> ProductionResult<()> {
        if self.actor_type != "local_operator" || self.actor_id.trim().is_empty() {
            return Err(ProductionError::Unauthorized {
                message: "production commands require the stable local_operator actor".into(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionCommandType {
    CreateIntent,
    StartRun,
    ApprovePackage,
    RejectPackage,
    ResumeRun,
    RetryStep,
    CancelRun,
    PromoteScript,
    QualityRework,
    ScriptRevision,
    FailRun,
    DeleteIntent,
    ArchiveIntent,
}

impl ProductionCommandType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CreateIntent => "create_intent",
            Self::StartRun => "start_run",
            Self::ApprovePackage => "approve_package",
            Self::RejectPackage => "reject_package",
            Self::ResumeRun => "resume_run",
            Self::RetryStep => "retry_step",
            Self::CancelRun => "cancel_run",
            Self::PromoteScript => "promote_script",
            Self::QualityRework => "quality_rework",
            Self::ScriptRevision => "script_revision",
            Self::FailRun => "fail_run",
            Self::DeleteIntent => "delete_intent",
            Self::ArchiveIntent => "archive_intent",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionAggregateType {
    Topic,
    ProductionIntent,
    ProductionRun,
    ProductionStep,
}

impl ProductionAggregateType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Topic => "topic",
            Self::ProductionIntent => "production_intent",
            Self::ProductionRun => "production_run",
            Self::ProductionStep => "production_step",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProductionCommandScope {
    actor: ProductionActor,
    command_type: ProductionCommandType,
    aggregate_type: ProductionAggregateType,
    aggregate_id: Uuid,
    idempotency_key: String,
}

impl ProductionCommandScope {
    pub fn new(
        actor: ProductionActor,
        command_type: ProductionCommandType,
        aggregate_type: ProductionAggregateType,
        aggregate_id: Uuid,
        idempotency_key: impl Into<String>,
    ) -> Self {
        Self {
            actor,
            command_type,
            aggregate_type,
            aggregate_id,
            idempotency_key: idempotency_key.into(),
        }
    }

    fn validate(&self) -> ProductionResult<()> {
        self.actor.validate()?;
        if self.idempotency_key.trim().is_empty() || self.idempotency_key.len() > 200 {
            return Err(ProductionError::IdempotencyConflict);
        }
        Ok(())
    }
}

#[derive(FromRow)]
struct StoredCommand {
    id: Uuid,
    request_digest: String,
    result: Value,
}

pub struct ProductionCommandStore;

impl ProductionCommandStore {
    /// 先递归排序对象键，再计算 SHA-256，避免 JSON 构造顺序影响请求身份。
    pub fn canonical_request_digest<T: Serialize>(request: &T) -> ProductionResult<String> {
        let value = serde_json::to_value(request)?;
        canonical_digest(&canonicalize_json(value)).map_err(Into::into)
    }

    pub async fn replay(
        tx: &mut Transaction<'_, Postgres>,
        scope: &ProductionCommandScope,
        request_digest: &str,
    ) -> ProductionResult<Option<Value>> {
        scope.validate()?;
        validate_digest(request_digest)?;
        let existing = load(tx, scope).await?;
        match existing {
            Some(existing) if existing.request_digest.trim() == request_digest => {
                Ok(Some(existing.result))
            }
            Some(_) => Err(ProductionError::IdempotencyConflict),
            None => Ok(None),
        }
    }

    pub async fn record(
        tx: &mut Transaction<'_, Postgres>,
        scope: &ProductionCommandScope,
        request_digest: &str,
        result: Value,
    ) -> ProductionResult<Uuid> {
        scope.validate()?;
        validate_digest(request_digest)?;
        if !result.is_object() {
            return Err(ProductionError::TransitionConflict {
                reason: "production command result must be an object".into(),
            });
        }
        let id = Uuid::new_v4();
        let inserted = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO production_commands (
                id,actor_type,actor_id,command_type,aggregate_type,aggregate_id,
                idempotency_key,request_digest,result
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
            ON CONFLICT ON CONSTRAINT production_commands_idempotency_unique DO NOTHING
            RETURNING id
            "#,
        )
        .bind(id)
        .bind(&scope.actor.actor_type)
        .bind(&scope.actor.actor_id)
        .bind(scope.command_type.as_str())
        .bind(scope.aggregate_type.as_str())
        .bind(scope.aggregate_id)
        .bind(&scope.idempotency_key)
        .bind(request_digest)
        .bind(&result)
        .fetch_optional(&mut **tx)
        .await?;
        if let Some(id) = inserted {
            return Ok(id);
        }

        let existing = load(tx, scope)
            .await?
            .ok_or(ProductionError::IdempotencyConflict)?;
        if existing.request_digest.trim() != request_digest {
            return Err(ProductionError::IdempotencyConflict);
        }
        if existing.result != result {
            return Err(ProductionError::TransitionConflict {
                reason: "replayed production command has a different persisted result".into(),
            });
        }
        Ok(existing.id)
    }
}

async fn load(
    tx: &mut Transaction<'_, Postgres>,
    scope: &ProductionCommandScope,
) -> ProductionResult<Option<StoredCommand>> {
    sqlx::query_as::<_, StoredCommand>(
        r#"
        SELECT id,request_digest,result FROM production_commands
        WHERE actor_type=$1 AND actor_id=$2 AND command_type=$3
          AND aggregate_type=$4 AND aggregate_id=$5 AND idempotency_key=$6
        FOR UPDATE
        "#,
    )
    .bind(&scope.actor.actor_type)
    .bind(&scope.actor.actor_id)
    .bind(scope.command_type.as_str())
    .bind(scope.aggregate_type.as_str())
    .bind(scope.aggregate_id)
    .bind(&scope.idempotency_key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(Into::into)
}

fn validate_digest(value: &str) -> ProductionResult<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(ProductionError::IdempotencyConflict);
    }
    Ok(())
}

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let sorted = object
                .into_iter()
                .map(|(key, value)| (key, canonicalize_json(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_json).collect()),
        value => value,
    }
}
