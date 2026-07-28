//! Full Crew 命令式 Application Service；HTTP 不直接拼接流程 SQL 或执行角色。

use novex_agent::AuditedModelExecutor;
use novex_ai_core::DefinitionRegistry;
use novex_production_crew::{
    durable::{
        package::GateDecision,
        plan::{FullCrewPlanRegistry, ResourceLimits},
        repository::{
            AcceptedProductionCommand, CreateIntentCommand, DurableProductionRepository,
            PackageDecisionCommand, PersistedGateDecision, ProductionActor, ProductionIntentRecord,
            ProductionRunRecord, ProductionRunView, ResumeRunCommand, RetryStepCommand,
            StartRunCommand,
        },
    },
    executor::RoleExecutor,
    state::artifacts::output_contract::validate_role_output_schema_compatibility,
    ProductionError, ProductionResult,
};
use serde_json::{Map, Value};
use sqlx::PgPool;
use std::{collections::BTreeSet, sync::Arc};
use uuid::Uuid;

const FULL_CREW_ROLE_KEYS: [&str; 9] = [
    "producer",
    "screenwriter",
    "character_critic",
    "director",
    "cinematographer",
    "performance_director",
    "sound_director",
    "editor",
    "qc",
];

pub struct ProductionWorkflowService {
    pool: PgPool,
    repository: DurableProductionRepository,
    definitions: Arc<DefinitionRegistry>,
    audited_executor: Arc<AuditedModelExecutor>,
}

impl ProductionWorkflowService {
    pub fn new(
        pool: PgPool,
        definitions: Arc<DefinitionRegistry>,
        audited_executor: Arc<AuditedModelExecutor>,
    ) -> Self {
        Self {
            repository: DurableProductionRepository::new(pool.clone()),
            pool,
            definitions,
            audited_executor,
        }
    }

    pub async fn create_intent(
        &self,
        project_id: Uuid,
        topic_id: Uuid,
        title: String,
        description: Option<String>,
        initial_input: Value,
        idempotency_key: String,
    ) -> ProductionResult<ProductionIntentRecord> {
        self.repository
            .create_intent(CreateIntentCommand {
                project_id,
                topic_id,
                title,
                description,
                initial_input,
                actor: ProductionActor::local_operator(),
                idempotency_key,
            })
            .await
    }

    pub async fn start_run(
        &self,
        intent_id: Uuid,
        idempotency_key: String,
    ) -> ProductionResult<ProductionRunRecord> {
        let actor = ProductionActor::local_operator();
        if let Some(run) = self
            .repository
            .replay_start_run(intent_id, actor.clone(), &idempotency_key)
            .await?
        {
            self.enqueue_current_steps(run.id).await?;
            return Ok(run);
        }
        self.validate_active_output_contracts()?;
        let model_id = self.default_enabled_text_model().await?;
        let role_bindings = self.freeze_role_bindings(model_id).await?;
        let plan = FullCrewPlanRegistry::snapshot_v1(
            true,
            Value::Object(role_bindings),
            ResourceLimits::strict_default(),
        )?;
        let run = self
            .repository
            .start_run(StartRunCommand {
                intent_id,
                plan,
                actor,
                idempotency_key,
            })
            .await?;
        self.enqueue_current_steps(run.id).await?;
        Ok(run)
    }

    pub async fn get_intent(&self, intent_id: Uuid) -> ProductionResult<ProductionIntentRecord> {
        self.repository.get_intent(intent_id).await
    }

    pub async fn delete_intent(
        &self,
        intent_id: Uuid,
        idempotency_key: String,
    ) -> ProductionResult<()> {
        self.repository
            .delete_intent(
                intent_id,
                ProductionActor::local_operator(),
                &idempotency_key,
            )
            .await
    }

    pub async fn archive_intent(
        &self,
        intent_id: Uuid,
        idempotency_key: String,
    ) -> ProductionResult<ProductionIntentRecord> {
        self.repository
            .archive_intent(
                intent_id,
                ProductionActor::local_operator(),
                &idempotency_key,
            )
            .await
    }

    pub async fn get_run(&self, run_id: Uuid) -> ProductionResult<ProductionRunView> {
        self.repository.get_run(run_id).await
    }

    pub async fn decide_package(
        &self,
        run_id: Uuid,
        package_digest: String,
        decision: GateDecision,
        reason: Option<String>,
        affected_owners: Vec<String>,
        idempotency_key: String,
    ) -> ProductionResult<PersistedGateDecision> {
        let record = self
            .repository
            .decide_package(PackageDecisionCommand {
                run_id,
                package_digest,
                decision,
                reason,
                affected_owners,
                actor: ProductionActor::local_operator(),
                idempotency_key,
            })
            .await?;
        self.enqueue_current_steps(run_id).await?;
        Ok(record)
    }

    pub async fn resume_run(
        &self,
        run_id: Uuid,
        idempotency_key: String,
    ) -> ProductionResult<AcceptedProductionCommand> {
        self.repository
            .resume_run(ResumeRunCommand {
                run_id,
                actor: ProductionActor::local_operator(),
                idempotency_key,
            })
            .await
    }

    pub async fn retry_step(
        &self,
        run_id: Uuid,
        step_id: Uuid,
        idempotency_key: String,
    ) -> ProductionResult<AcceptedProductionCommand> {
        self.repository
            .retry_step(RetryStepCommand {
                run_id,
                step_id,
                actor: ProductionActor::local_operator(),
                idempotency_key,
            })
            .await
    }

    async fn default_enabled_text_model(&self) -> ProductionResult<Uuid> {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id FROM ai_models
            WHERE model_type='text' AND status='enabled' AND deleted_at IS NULL
            ORDER BY is_default DESC,sort_order,created_at,id
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| ProductionError::CapabilityMismatch {
            reason: "Full Crew requires an enabled text model".into(),
        })
    }

    fn validate_active_output_contracts(&self) -> ProductionResult<()> {
        for role_key in FULL_CREW_ROLE_KEYS {
            let agent_key = format!("production.{role_key}");
            let node_key = format!("production.{role_key}.execute");
            let agent = self.definitions.active_agent(&agent_key).map_err(|error| {
                ProductionError::CapabilityMismatch {
                    reason: error.to_string(),
                }
            })?;
            let reference =
                agent
                    .nodes
                    .get(&node_key)
                    .ok_or_else(|| ProductionError::CapabilityMismatch {
                        reason: format!("active Definition {agent_key} has no node {node_key}"),
                    })?;
            let prompt = self
                .definitions
                .prompts()
                .iter()
                .find(|prompt| {
                    prompt.prompt_key == reference.key && prompt.version == reference.version
                })
                .ok_or_else(|| ProductionError::CapabilityMismatch {
                    reason: format!(
                        "active Definition {agent_key} references missing Prompt {}@{}",
                        reference.key, reference.version
                    ),
                })?;
            validate_role_output_schema_compatibility(role_key, prompt.output_schema.as_ref())?;
        }
        Ok(())
    }

    async fn freeze_role_bindings(&self, model_id: Uuid) -> ProductionResult<Map<String, Value>> {
        let mut bindings = Map::new();
        for role_key in FULL_CREW_ROLE_KEYS {
            let agent_key = format!("production.{role_key}");
            let version = self
                .definitions
                .active_agent(&agent_key)
                .map_err(|error| ProductionError::CapabilityMismatch {
                    reason: error.to_string(),
                })?
                .version
                .clone();
            let binding = RoleExecutor::freeze_active_binding(
                role_key,
                &version,
                &self.definitions,
                &self.audited_executor,
                model_id,
            )
            .await?;
            bindings.insert(role_key.into(), serde_json::to_value(binding)?);
        }
        Ok(bindings)
    }

    async fn enqueue_current_steps(&self, run_id: Uuid) -> ProductionResult<()> {
        let view = self.repository.get_run(run_id).await?;
        let epoch = view.run.current_revision_epoch;
        let succeeded = view
            .steps
            .iter()
            .filter(|step| step.revision_epoch == epoch && step.status == "succeeded")
            .map(|step| step.step_key.as_str())
            .collect::<BTreeSet<_>>();
        for step in view
            .steps
            .iter()
            .filter(|step| step.revision_epoch == epoch && step.status == "queued")
        {
            let dependencies_ready = step.dependencies.as_array().is_some_and(|dependencies| {
                dependencies.iter().all(|dependency| {
                    dependency
                        .as_str()
                        .is_some_and(|step_key| succeeded.contains(step_key))
                })
            });
            if dependencies_ready {
                self.repository.enqueue_wakeup(run_id, step.id).await?;
            }
        }
        Ok(())
    }
}
