use novex_ai_core::{
    definition_digest, ActivationEvidence, DefinitionKind, DefinitionRegistry,
    DefinitionReleaseEvidence, DefinitionStatus, ExecutorOwner,
};
use sqlx::{PgPool, Row};
use std::fmt;
use uuid::Uuid;

#[derive(Clone)]
pub struct PostgresDefinitionReleaseRepository {
    pool: PgPool,
}

impl PostgresDefinitionReleaseRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Records only immutable key/version/digest evidence; template bodies remain code-owned.
    pub async fn publish_registry(
        &self,
        registry: &DefinitionRegistry,
    ) -> Result<(), DefinitionReleaseError> {
        let mut transaction = self.pool.begin().await?;
        let manifest_id = publish_manifest(&mut transaction, registry.digest()).await?;
        for agent in registry.agents() {
            let digest = definition_digest(agent)
                .map_err(|error| DefinitionReleaseError::Serialization(error.to_string()))?;
            validate_activation_evidence(
                &mut transaction,
                registry,
                DefinitionKind::Agent,
                &agent.agent_key,
                &agent.version,
                &digest,
                agent.status,
            )
            .await?;
            publish_one(
                &mut transaction,
                "agent",
                &agent.agent_key,
                &agent.version,
                digest,
                registry.digest(),
                agent.status,
                agent.executor_owner,
            )
            .await?;
            publish_manifest_entry(
                &mut transaction,
                manifest_id,
                "agent",
                &agent.agent_key,
                &agent.version,
                &definition_digest(agent)
                    .map_err(|error| DefinitionReleaseError::Serialization(error.to_string()))?,
                agent.status,
                agent.executor_owner,
            )
            .await?;
        }
        for prompt in registry.prompts() {
            let digest = definition_digest(prompt)
                .map_err(|error| DefinitionReleaseError::Serialization(error.to_string()))?;
            validate_activation_evidence(
                &mut transaction,
                registry,
                DefinitionKind::Prompt,
                &prompt.prompt_key,
                &prompt.version,
                &digest,
                prompt.status,
            )
            .await?;
            publish_one(
                &mut transaction,
                "prompt",
                &prompt.prompt_key,
                &prompt.version,
                digest,
                registry.digest(),
                prompt.status,
                prompt.executor_owner,
            )
            .await?;
            publish_manifest_entry(
                &mut transaction,
                manifest_id,
                "prompt",
                &prompt.prompt_key,
                &prompt.version,
                &definition_digest(prompt)
                    .map_err(|error| DefinitionReleaseError::Serialization(error.to_string()))?,
                prompt.status,
                prompt.executor_owner,
            )
            .await?;
        }
        let entry_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM definition_release_manifest_entries WHERE manifest_id = $1",
        )
        .bind(manifest_id)
        .fetch_one(&mut *transaction)
        .await?;
        let expected_count = (registry.agents().len() + registry.prompts().len()) as i64;
        if entry_count != expected_count {
            return Err(DefinitionReleaseError::Conflict(format!(
                "registry manifest {} has {entry_count} entries, expected {expected_count}",
                registry.digest()
            )));
        }
        transaction.commit().await?;
        Ok(())
    }
}

async fn publish_manifest(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    registry_digest: &str,
) -> Result<Uuid, DefinitionReleaseError> {
    sqlx::query(
        r#"
        INSERT INTO definition_release_manifests (registry_digest)
        VALUES ($1)
        ON CONFLICT (registry_digest) DO NOTHING
        "#,
    )
    .bind(registry_digest)
    .execute(&mut **transaction)
    .await?;
    Ok(
        sqlx::query_scalar(
            "SELECT id FROM definition_release_manifests WHERE registry_digest = $1",
        )
        .bind(registry_digest)
        .fetch_one(&mut **transaction)
        .await?,
    )
}

#[allow(clippy::too_many_arguments)]
async fn publish_manifest_entry(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    manifest_id: Uuid,
    kind: &str,
    key: &str,
    version: &str,
    definition_digest: &str,
    status: DefinitionStatus,
    owner: ExecutorOwner,
) -> Result<(), DefinitionReleaseError> {
    sqlx::query(
        r#"
        INSERT INTO definition_release_manifest_entries (
            manifest_id, definition_kind, definition_key, definition_version,
            definition_digest, lifecycle_status, executor_owner
        ) VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (manifest_id, definition_kind, definition_key, definition_version) DO NOTHING
        "#,
    )
    .bind(manifest_id)
    .bind(kind)
    .bind(key)
    .bind(version)
    .bind(definition_digest)
    .bind(status_name(status))
    .bind(owner_name(owner))
    .execute(&mut **transaction)
    .await?;

    let row = sqlx::query(
        r#"
        SELECT definition_digest, lifecycle_status, executor_owner
        FROM definition_release_manifest_entries
        WHERE manifest_id = $1 AND definition_kind = $2
          AND definition_key = $3 AND definition_version = $4
        "#,
    )
    .bind(manifest_id)
    .bind(kind)
    .bind(key)
    .bind(version)
    .fetch_one(&mut **transaction)
    .await?;
    let stored_digest: String = row.try_get("definition_digest")?;
    let stored_status: String = row.try_get("lifecycle_status")?;
    let stored_owner: String = row.try_get("executor_owner")?;
    if stored_digest != definition_digest
        || stored_status != status_name(status)
        || stored_owner != owner_name(owner)
    {
        return Err(DefinitionReleaseError::Conflict(format!(
            "registry manifest entry {kind} {key}@{version} conflicts with immutable evidence"
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn validate_activation_evidence(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    registry: &DefinitionRegistry,
    kind: DefinitionKind,
    key: &str,
    version: &str,
    definition_digest: &str,
    status: DefinitionStatus,
) -> Result<(), DefinitionReleaseError> {
    if status != DefinitionStatus::Active {
        return Ok(());
    }
    let release = registry
        .release_evidence()
        .iter()
        .find(|release| {
            release.definition_kind == kind
                && release.definition_key == key
                && release.definition_version == version
                && release.definition_digest == definition_digest
        })
        .ok_or_else(|| {
            DefinitionReleaseError::ActivationEvidence(format!(
                "active {key}@{version} has no immutable activation evidence"
            ))
        })?;
    match &release.activation_evidence {
        ActivationEvidence::GoldenBaseline { .. } if version == "1.0.0" => Ok(()),
        ActivationEvidence::GoldenBaseline { .. } => {
            Err(DefinitionReleaseError::ActivationEvidence(format!(
                "golden baseline is only valid for initial v1 release: {key}@{version}"
            )))
        }
        ActivationEvidence::EvalReport { report_id } => {
            validate_eval_report(transaction, release, report_id).await
        }
    }
}

async fn validate_eval_report(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    release: &DefinitionReleaseEvidence,
    report_id: &str,
) -> Result<(), DefinitionReleaseError> {
    let report_id = Uuid::parse_str(report_id)
        .map_err(|_| DefinitionReleaseError::ActivationEvidence("invalid EvalReport id".into()))?;
    let matches = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM eval_reports reports
            JOIN eval_runs runs ON runs.id = reports.eval_run_id
            WHERE reports.id = $1
              AND reports.passed
              AND runs.status = 'passed'
              AND runs.candidate_key = $2
              AND runs.candidate_version = $3
              AND runs.candidate_digest = $4
              AND runs.validation_mode IN ('golden_baseline', 'real_model')
        )
        "#,
    )
    .bind(report_id)
    .bind(&release.definition_key)
    .bind(&release.definition_version)
    .bind(&release.definition_digest)
    .fetch_one(&mut **transaction)
    .await?;
    if matches {
        Ok(())
    } else {
        Err(DefinitionReleaseError::ActivationEvidence(format!(
            "EvalReport {report_id} does not authorize {}@{}",
            release.definition_key, release.definition_version
        )))
    }
}

#[allow(clippy::too_many_arguments)]
async fn publish_one(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    kind: &str,
    key: &str,
    version: &str,
    definition_digest: String,
    registry_digest: &str,
    status: DefinitionStatus,
    owner: ExecutorOwner,
) -> Result<(), DefinitionReleaseError> {
    sqlx::query(
        r#"
        INSERT INTO definition_releases (
            definition_kind, definition_key, definition_version, definition_digest,
            registry_digest, initial_status, executor_owner
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (definition_kind, definition_key, definition_version) DO NOTHING
        "#,
    )
    .bind(kind)
    .bind(key)
    .bind(version)
    .bind(&definition_digest)
    .bind(registry_digest)
    .bind(status_name(status))
    .bind(owner_name(owner))
    .execute(&mut **transaction)
    .await?;

    let row = sqlx::query(
        r#"
        SELECT definition_digest, executor_owner
        FROM definition_releases
        WHERE definition_kind = $1 AND definition_key = $2 AND definition_version = $3
        "#,
    )
    .bind(kind)
    .bind(key)
    .bind(version)
    .fetch_one(&mut **transaction)
    .await?;
    let stored_digest: String = row.try_get("definition_digest")?;
    let stored_owner: String = row.try_get("executor_owner")?;
    if stored_digest != definition_digest || stored_owner != owner_name(owner) {
        return Err(DefinitionReleaseError::Conflict(format!(
            "{kind} {key}@{version} already exists with different immutable evidence"
        )));
    }
    Ok(())
}

fn status_name(status: DefinitionStatus) -> &'static str {
    match status {
        DefinitionStatus::Candidate => "candidate",
        DefinitionStatus::Active => "active",
        DefinitionStatus::Supported => "supported",
        DefinitionStatus::Revoked => "revoked",
    }
}

fn owner_name(owner: ExecutorOwner) -> &'static str {
    match owner {
        ExecutorOwner::Rust => "rust",
        ExecutorOwner::Pi => "pi",
    }
}

#[derive(Debug)]
pub enum DefinitionReleaseError {
    Storage(sqlx::Error),
    Serialization(String),
    Conflict(String),
    ActivationEvidence(String),
}

impl fmt::Display for DefinitionReleaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(formatter, "definition release storage error: {error}"),
            Self::Serialization(message) => {
                write!(formatter, "definition serialization error: {message}")
            }
            Self::Conflict(message) => formatter.write_str(message),
            Self::ActivationEvidence(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for DefinitionReleaseError {}

impl From<sqlx::Error> for DefinitionReleaseError {
    fn from(value: sqlx::Error) -> Self {
        Self::Storage(value)
    }
}
