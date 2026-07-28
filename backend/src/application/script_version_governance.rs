//! 正式 Script/Scene 引用与脚本修订的版本治理边界。

use novex_production_crew::{
    durable::{
        canonical_digest,
        command_store::{
            ProductionAggregateType, ProductionCommandScope, ProductionCommandStore,
            ProductionCommandType,
        },
        plan::PlanSnapshot,
        repository::ProductionActor,
    },
    ProductionError, ProductionResult,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionArtifactOwner {
    Director,
    PerformanceDirector,
    SoundDirector,
}

impl ProductionArtifactOwner {
    fn as_str(self) -> &'static str {
        match self {
            Self::Director => "director",
            Self::PerformanceDirector => "performance_director",
            Self::SoundDirector => "sound_director",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptRevisionScope {
    ScriptSemantic { director_suggestion_id: Uuid },
    ProductionExpression { owner: ProductionArtifactOwner },
}

#[derive(Clone, Debug)]
pub struct ScriptRevisionCommand {
    pub run_id: Uuid,
    pub current_script_id: Uuid,
    pub scope: ScriptRevisionScope,
    pub reason: String,
    pub instruction: String,
    pub actor: ProductionActor,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScriptRevisionResult {
    pub revision_epoch: i32,
    pub owner_role: String,
    pub requires_new_script_package: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DirectorSceneReference {
    pub scene_id: Uuid,
}

#[derive(Clone)]
pub struct ScriptVersionGovernanceService {
    pool: PgPool,
}

impl ScriptVersionGovernanceService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn validate_director_scene_references(
        &self,
        run_id: Uuid,
        script_id: Uuid,
        references: &[DirectorSceneReference],
    ) -> ProductionResult<()> {
        if references.is_empty() {
            return Err(ProductionError::InvalidArtifactSchema {
                details: "Director must reference at least one formal Scene UUID".into(),
            });
        }
        let scene_ids = references
            .iter()
            .map(|reference| reference.scene_id)
            .collect::<BTreeSet<_>>();
        if scene_ids.len() != references.len() {
            return Err(ProductionError::InvalidArtifactSchema {
                details: "Director Scene references must be unique".into(),
            });
        }
        let current_script = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM scripts script
                JOIN production_runs run ON run.id = script.production_run_id
                JOIN production_domain_links link
                  ON link.run_id = run.id AND link.link_type = 'script'
                 AND link.script_id = script.id
                WHERE run.id = $1 AND script.id = $2 AND script.status = 'approved'
                  AND script.source_revision_epoch <= run.current_revision_epoch
                  AND NOT EXISTS (
                      SELECT 1 FROM scripts newer
                      WHERE newer.production_run_id = run.id
                        AND newer.source_revision_epoch <= run.current_revision_epoch
                        AND newer.source_revision_epoch > script.source_revision_epoch
                  )
            )
            "#,
        )
        .bind(run_id)
        .bind(script_id)
        .fetch_one(&self.pool)
        .await?;
        if !current_script {
            return Err(ProductionError::InvalidArtifactSchema {
                details: "Director must use the current formal Script for this Run".into(),
            });
        }
        let matched = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM scenes WHERE script_id = $1 AND id = ANY($2)",
        )
        .bind(script_id)
        .bind(scene_ids.iter().copied().collect::<Vec<_>>())
        .fetch_one(&self.pool)
        .await?;
        if matched != scene_ids.len() as i64 {
            return Err(ProductionError::InvalidArtifactSchema {
                details: "Director contains an unknown or cross-Script Scene UUID".into(),
            });
        }
        Ok(())
    }

    pub async fn request_revision(
        &self,
        command: ScriptRevisionCommand,
    ) -> ProductionResult<ScriptRevisionResult> {
        validate_revision_command(&command)?;
        let request_digest = ProductionCommandStore::canonical_request_digest(&json!({
            "run_id": command.run_id,
            "current_script_id": command.current_script_id,
            "scope": command.scope,
            "reason": command.reason,
            "instruction": command.instruction,
        }))?;
        let command_scope = ProductionCommandScope::new(
            command.actor.clone(),
            ProductionCommandType::ScriptRevision,
            ProductionAggregateType::ProductionRun,
            command.run_id,
            &command.idempotency_key,
        );
        let instruction = command.instruction.trim();
        let instruction_digest = canonical_digest(&instruction)?;
        let mut tx = self.pool.begin().await?;
        let context =
            lock_revision_context(&mut tx, command.run_id, command.current_script_id).await?;
        if let Some(result) =
            ProductionCommandStore::replay(&mut tx, &command_scope, &request_digest).await?
        {
            tx.commit().await?;
            return serde_json::from_value(result).map_err(Into::into);
        }
        if !matches!(
            context.run_status.as_str(),
            "queued" | "running" | "waiting_approval" | "blocked"
        ) {
            return Err(ProductionError::TransitionConflict {
                reason: "Run is not in a revisable state".into(),
            });
        }

        let (owner_role, reason_type, requires_new_script_package, source_package_id) =
            match command.scope {
                ScriptRevisionScope::ScriptSemantic {
                    director_suggestion_id,
                } => {
                    validate_director_suggestion(&mut tx, &context, director_suggestion_id).await?;
                    enforce_script_revision_limit(&mut tx, &context).await?;
                    (
                        "screenwriter",
                        "script_semantic_revision",
                        true,
                        Some(context.script_package_id),
                    )
                }
                ScriptRevisionScope::ProductionExpression { owner } => {
                    let owner_role = owner.as_str();
                    require_completed_owner_step(&mut tx, &context, owner_role).await?;
                    let source_package_id = sqlx::query_scalar::<_, Uuid>(
                        r#"
                        SELECT id FROM artifact_package_snapshots
                        WHERE run_id = $1 AND revision_epoch = $2
                          AND package_type = 'production'
                        ORDER BY package_version DESC LIMIT 1
                        "#,
                    )
                    .bind(context.run_id)
                    .bind(context.current_revision_epoch)
                    .fetch_optional(&mut *tx)
                    .await?;
                    (
                        owner_role,
                        "production_expression_revision",
                        false,
                        source_package_id,
                    )
                }
            };
        let next_epoch = context.current_revision_epoch + 1;
        sqlx::query(
            r#"
            INSERT INTO production_revision_epochs (
                run_id, epoch, reason_type, reason, affected_owners, source_package_id,
                actor_type, actor_id, instruction_digest
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(context.run_id)
        .bind(next_epoch)
        .bind(reason_type)
        .bind(command.reason.trim())
        .bind(json!([owner_role]))
        .bind(source_package_id)
        .bind(&command.actor.actor_type)
        .bind(&command.actor.actor_id)
        .bind(&instruction_digest)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO production_revision_instructions (
                run_id, revision_epoch, owner_role, actor_type, actor_id,
                source, trust, instruction, instruction_digest
            ) VALUES ($1, $2, $3, $4, $5, 'script_revision_command',
                      'user_instruction', $6, $7)
            "#,
        )
        .bind(context.run_id)
        .bind(next_epoch)
        .bind(owner_role)
        .bind(&command.actor.actor_type)
        .bind(&command.actor.actor_id)
        .bind(instruction)
        .bind(&instruction_digest)
        .execute(&mut *tx)
        .await?;
        create_revision_steps(
            &mut tx,
            &context.plan,
            context.run_id,
            next_epoch,
            owner_role,
        )
        .await?;
        sqlx::query(
            r#"
            UPDATE production_steps
            SET status = 'superseded', lease_owner = NULL, lease_expires_at = NULL,
                completed_at = COALESCE(completed_at, NOW()), updated_at = NOW()
            WHERE run_id = $1 AND revision_epoch = $2
              AND status IN (
                  'blocked', 'queued', 'running', 'waiting_approval',
                  'external_wait', 'failed', 'attention_required'
              )
            "#,
        )
        .bind(context.run_id)
        .bind(context.current_revision_epoch)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE production_runs
            SET current_revision_epoch = $2, status = 'queued',
                error_code = NULL, error_details = NULL, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(context.run_id)
        .bind(next_epoch)
        .execute(&mut *tx)
        .await?;
        let production_status = if requires_new_script_package {
            "scripting"
        } else {
            "directing"
        };
        sqlx::query("UPDATE production_projects SET status = $2, updated_at = NOW() WHERE id = $1")
            .bind(context.production_project_id)
            .bind(production_status)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            r#"
            INSERT INTO production_wakeups (run_id, step_id)
            SELECT run_id, id FROM production_steps
            WHERE run_id = $1 AND revision_epoch = $2
              AND role_key = $3 AND status = 'queued'
            ON CONFLICT (step_id, status) DO NOTHING
            "#,
        )
        .bind(context.run_id)
        .bind(next_epoch)
        .bind(owner_role)
        .execute(&mut *tx)
        .await?;
        let result = ScriptRevisionResult {
            revision_epoch: next_epoch,
            owner_role: owner_role.into(),
            requires_new_script_package,
        };
        let result_json = serde_json::to_value(&result)?;
        ProductionCommandStore::record(&mut tx, &command_scope, &request_digest, result_json)
            .await?;
        tx.commit().await?;
        Ok(result)
    }
}

#[derive(FromRow)]
struct RevisionContext {
    run_id: Uuid,
    production_project_id: Uuid,
    current_revision_epoch: i32,
    run_status: String,
    plan: Value,
    script_id: Uuid,
    script_package_id: Uuid,
}

fn validate_revision_command(command: &ScriptRevisionCommand) -> ProductionResult<()> {
    if command.actor.actor_type != "local_operator" || command.actor.actor_id.trim().is_empty() {
        return Err(ProductionError::Unauthorized {
            message: "script revision requires the stable local_operator actor".into(),
        });
    }
    if command.idempotency_key.trim().is_empty()
        || command.idempotency_key.len() > 200
        || command.reason.trim().is_empty()
        || command.instruction.trim().is_empty()
    {
        return Err(ProductionError::TransitionConflict {
            reason: "revision key, reason, and instruction must be non-empty".into(),
        });
    }
    Ok(())
}

async fn lock_revision_context(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    script_id: Uuid,
) -> ProductionResult<RevisionContext> {
    sqlx::query_as::<_, RevisionContext>(
        r#"
        SELECT run.id AS run_id, production.id AS production_project_id,
               run.current_revision_epoch, run.status AS run_status,
               snapshot.plan, script.id AS script_id, script.script_package_id
        FROM production_runs run
        JOIN production_projects production ON production.id = run.production_project_id
        JOIN production_plan_snapshots snapshot ON snapshot.id = run.plan_snapshot_id
        JOIN scripts script ON script.production_run_id = run.id AND script.id = $2
        JOIN production_domain_links link
          ON link.run_id = run.id AND link.link_type = 'script' AND link.script_id = script.id
        WHERE run.id = $1 AND script.status = 'approved'
          AND script.source_revision_epoch <= run.current_revision_epoch
          AND NOT EXISTS (
              SELECT 1 FROM scripts newer
              WHERE newer.production_run_id = run.id
                AND newer.source_revision_epoch <= run.current_revision_epoch
                AND newer.source_revision_epoch > script.source_revision_epoch
          )
        FOR UPDATE OF run, production, script
        "#,
    )
    .bind(run_id)
    .bind(script_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| ProductionError::TransitionConflict {
        reason: "revision must target the current formal Script of this Run".into(),
    })
}

async fn validate_director_suggestion(
    tx: &mut Transaction<'_, Postgres>,
    context: &RevisionContext,
    suggestion_id: Uuid,
) -> ProductionResult<()> {
    let accepted = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM collaboration_suggestions suggestion
            JOIN collaboration_suggestion_responses response
              ON response.suggestion_id = suggestion.id
            WHERE suggestion.id = $1 AND suggestion.run_id = $2
              AND suggestion.revision_epoch = $3
              AND suggestion.from_role = 'director'
              AND suggestion.to_role = 'screenwriter'
              AND suggestion.artifact_type = 'script'
              AND suggestion.artifact_id = $4
              AND suggestion.suggestion_type = 'revision'
              AND response.decision = 'accepted'
        )
        "#,
    )
    .bind(suggestion_id)
    .bind(context.run_id)
    .bind(context.current_revision_epoch)
    .bind(context.script_id)
    .fetch_one(&mut **tx)
    .await?;
    if !accepted {
        return Err(ProductionError::TransitionConflict {
            reason:
                "semantic revision requires an accepted Director suggestion for the current Script"
                    .into(),
        });
    }
    Ok(())
}

async fn enforce_script_revision_limit(
    tx: &mut Transaction<'_, Postgres>,
    context: &RevisionContext,
) -> ProductionResult<()> {
    let plan: PlanSnapshot = serde_json::from_value(context.plan.clone())?;
    let limit = plan
        .max_package_revisions
        .get("script")
        .copied()
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "frozen plan has no Script revision limit".into(),
        })? as i64;
    let count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*) FROM production_revision_epochs
        WHERE run_id = $1
          AND reason_type IN ('script_reject', 'script_semantic_revision')
        "#,
    )
    .bind(context.run_id)
    .fetch_one(&mut **tx)
    .await?;
    if count >= limit {
        sqlx::query(
            "UPDATE production_runs SET status = 'attention_required', error_code = 'revision_limit_reached', updated_at = NOW() WHERE id = $1",
        )
        .bind(context.run_id)
        .execute(&mut **tx)
        .await?;
        return Err(ProductionError::AttentionRequired);
    }
    Ok(())
}

async fn require_completed_owner_step(
    tx: &mut Transaction<'_, Postgres>,
    context: &RevisionContext,
    owner_role: &str,
) -> ProductionResult<()> {
    let complete = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM production_steps
            WHERE run_id = $1 AND revision_epoch = $2
              AND role_key = $3 AND status = 'succeeded'
        )
        "#,
    )
    .bind(context.run_id)
    .bind(context.current_revision_epoch)
    .bind(owner_role)
    .fetch_one(&mut **tx)
    .await?;
    if !complete {
        return Err(ProductionError::TransitionConflict {
            reason: format!("{owner_role} has no completed artifact to revise"),
        });
    }
    Ok(())
}

async fn create_revision_steps(
    tx: &mut Transaction<'_, Postgres>,
    plan_value: &Value,
    run_id: Uuid,
    revision_epoch: i32,
    owner_role: &str,
) -> ProductionResult<()> {
    let plan: PlanSnapshot = serde_json::from_value(plan_value.clone())?;
    let owner_order = plan
        .steps
        .iter()
        .position(|step| step.role_key.as_deref() == Some(owner_role))
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "revision owner is outside the frozen plan".into(),
        })?;
    for (plan_order, step) in plan.steps.iter().enumerate() {
        let status = if step.role_key.as_deref() == Some(owner_role) {
            "queued"
        } else if plan_order < owner_order {
            "succeeded"
        } else {
            "blocked"
        };
        let step_type = serde_json::to_value(step.kind)?
            .as_str()
            .unwrap_or_default()
            .to_string();
        sqlx::query(
            r#"
            INSERT INTO production_steps (
                run_id, revision_epoch, plan_order, step_key, step_type,
                role_key, dependencies, status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(run_id)
        .bind(revision_epoch)
        .bind(plan_order as i32)
        .bind(&step.key)
        .bind(step_type)
        .bind(&step.role_key)
        .bind(serde_json::to_value(&step.dependencies)?)
        .bind(status)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}
