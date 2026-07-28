//! Full Crew 进程级 Runner。
//!
//! Runner 不维护流程内存状态：Redis 消息只负责唤醒，所有步骤、依赖、租约和
//! 结果都通过 `DurableProductionRepository` 从 PostgreSQL 重建。外部领域动作
//! 统一复用 `ProductionOrchestrator`，避免 worker 自己拼接第二套流程。

use super::production_wakeups::{ProductionWakeupMessage, RedisProductionWakeupDispatcher};
use crate::application::script_package_promotion::{
    ScriptPackagePromotionCommand, ScriptPackagePromotionService,
};
use novex_production_crew::{
    durable::{
        canonical_digest,
        package::PackageType,
        repository::{DurableProductionRepository, ProductionActor, ProductionStepRecord},
    },
    executor::role_executor::{RoleExecutor, RoleFinalizeContext},
    orchestrator::{
        application_port::{ProductionWorkPlanOverrides, ProductionWorkPlanSettings},
        ProductionOrchestrator,
    },
    ProductionError, ProductionResult,
};
use serde_json::json;
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use tokio::{sync::watch, time::sleep};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RunnerReport {
    pub recovered: u64,
    pub delivered: u64,
    pub attempted: u64,
    pub completed: u64,
    pub skipped: u64,
    pub failed: u64,
}

#[derive(Clone)]
pub struct ProductionWorkflowRunner {
    pool: PgPool,
    repository: DurableProductionRepository,
    orchestrator: Arc<ProductionOrchestrator>,
    dispatcher: Option<RedisProductionWakeupDispatcher>,
    worker_id: Arc<str>,
    lease_ttl: Duration,
    batch_size: i64,
}

impl ProductionWorkflowRunner {
    pub fn new(
        pool: PgPool,
        orchestrator: Arc<ProductionOrchestrator>,
        redis_client: Option<redis::Client>,
        queue_key: impl Into<String>,
        worker_id: impl Into<String>,
        lease_ttl: Duration,
    ) -> ProductionResult<Self> {
        let worker_id: String = worker_id.into();
        if worker_id.trim().is_empty() || lease_ttl.is_zero() {
            return Err(ProductionError::TransitionConflict {
                reason: "production Runner requires a worker id and positive lease ttl".into(),
            });
        }
        Ok(Self {
            repository: DurableProductionRepository::new(pool.clone()),
            pool,
            orchestrator,
            dispatcher: redis_client
                .map(|client| RedisProductionWakeupDispatcher::new(client, queue_key)),
            worker_id: worker_id.into(),
            lease_ttl,
            batch_size: 32,
        })
    }

    pub fn with_batch_size(mut self, batch_size: i64) -> Self {
        self.batch_size = batch_size.clamp(1, 1000);
        self
    }

    /// 执行一轮恢复、Redis 消费和外部等待观察；可由合同测试或进程循环调用。
    pub async fn tick(&self) -> ProductionResult<RunnerReport> {
        let mut report = RunnerReport::default();
        if let Some(dispatcher) = &self.dispatcher {
            let wakeups = dispatcher
                .recover_and_dispatch(&self.repository, self.batch_size)
                .await?;
            report.recovered = wakeups.recovered;
            report.delivered = wakeups.delivered;
            for _ in 0..self.batch_size {
                // Redis 只承担唤醒职责：连接中断或单条坏消息不能阻断基于
                // PostgreSQL 的恢复扫描。坏消息被丢弃，权威状态仍由下方扫描重建。
                let message = match dispatcher.pop().await {
                    Ok(Some(message)) => message,
                    Ok(None) | Err(_) => break,
                };
                self.process_message(message, &mut report).await?;
            }
        }

        // Redis 不可用或消息丢失时，直接从 PostgreSQL 扫描可运行步骤。
        for step in self.repository.recoverable_steps(self.batch_size).await? {
            self.process_step(step, &mut report).await;
        }
        // external_wait 不自动 claim，只观察正式外部运行或保持人工等待。
        for step in self.repository.external_wait_steps(self.batch_size).await? {
            self.observe_external_step(step, &mut report).await;
        }
        Ok(report)
    }

    /// 启动长驻 worker；收到 shutdown=true 后等待当前 tick 返回再优雅退出。
    pub async fn run(&self, mut shutdown: watch::Receiver<bool>) -> ProductionResult<()> {
        loop {
            if *shutdown.borrow() {
                return Ok(());
            }
            self.tick().await?;
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() { return Ok(()); }
                }
                _ = sleep(Duration::from_millis(500)) => {}
            }
        }
    }

    async fn process_message(
        &self,
        message: ProductionWakeupMessage,
        report: &mut RunnerReport,
    ) -> ProductionResult<()> {
        let view = match self.repository.get_run(message.run_id).await {
            Ok(view) => view,
            Err(_) => {
                report.skipped += 1;
                return Ok(());
            }
        };
        let Some(step) = view
            .steps
            .into_iter()
            .find(|step| step.id == message.step_id)
        else {
            report.skipped += 1;
            return Ok(());
        };
        self.process_step(step, report).await;
        Ok(())
    }

    async fn process_step(&self, step: ProductionStepRecord, report: &mut RunnerReport) {
        report.attempted += 1;
        let result = match step.step_type.as_str() {
            "role" => self.execute_role_step(step).await,
            "gate" => self.execute_gate_step(step).await,
            "domain_command" => self.execute_domain_step(step).await,
            "external_wait" => self.execute_external_step(step).await,
            _ => Err(ProductionError::TransitionConflict {
                reason: "unknown production step type".into(),
            }),
        };
        match result {
            Ok(true) => report.completed += 1,
            Ok(false) => report.skipped += 1,
            Err(_) => report.failed += 1,
        }
    }

    async fn claim(&self, step: &ProductionStepRecord) -> ProductionResult<ProductionStepRecord> {
        let request_digest = canonical_digest(&json!({
            "kind": "production_runner_claim",
            "step_id": step.id,
            "worker_id": self.worker_id.as_ref(),
        }))?;
        let idempotency_key = format!("runner-claim-{}", step.id);
        self.repository
            .claim_step(
                step.id,
                self.worker_id.as_ref(),
                self.lease_ttl,
                &request_digest,
                &idempotency_key,
            )
            .await
    }

    async fn execute_role_step(&self, step: ProductionStepRecord) -> ProductionResult<bool> {
        let claimed = match self.claim(&step).await {
            Ok(claimed) => claimed,
            Err(ProductionError::TransitionConflict { .. }) => return Ok(false),
            Err(error) => return Err(error),
        };
        let prepared = self
            .orchestrator
            .prepare_role_step(claimed.id, self.worker_id.to_string(), claimed.attempt)
            .await?;
        let executed = RoleExecutor::execute_prepared(prepared).await;
        RoleExecutor::finalize(
            RoleFinalizeContext {
                pool: self.pool.clone(),
                lease_owner: self.worker_id.to_string(),
            },
            &executed,
        )
        .await?;
        Ok(true)
    }

    async fn execute_domain_step(&self, step: ProductionStepRecord) -> ProductionResult<bool> {
        // Script promotion 有独立事务边界，必须保持 queued 直到 promotion service 锁定它。
        if step.step_key == "promote_script" {
            let digest = self.step_input_digest(step.id).await?.ok_or_else(|| {
                ProductionError::TransitionConflict {
                    reason: "promote_script has no approved ScriptPackage digest".into(),
                }
            })?;
            ScriptPackagePromotionService::new(self.pool.clone())
                .promote(ScriptPackagePromotionCommand {
                    run_id: step.run_id,
                    package_digest: digest,
                    actor: ProductionActor::local_operator(),
                    idempotency_key: format!("runner-promote-{}", step.id),
                })
                .await?;
            return Ok(true);
        }

        if step.step_key == "create_work_plan" {
            let claimed = match self.claim(&step).await {
                Ok(claimed) => claimed,
                Err(ProductionError::TransitionConflict { .. }) => return Ok(false),
                Err(error) => return Err(error),
            };
            let package_digest = self.step_input_digest(step.id).await?.ok_or_else(|| {
                ProductionError::TransitionConflict {
                    reason: "create_work_plan has no ProductionPackage digest".into(),
                }
            })?;
            let input = self
                .repository
                .load_approved_production_input(claimed.run_id, &package_digest)
                .await?;
            let manifest = self
                .repository
                .load_scene_visual_manifest(claimed.run_id)
                .await?;
            let settings = self.default_work_plan_settings().await?;
            self.orchestrator
                .resume_create_work_plan(novex_production_crew::orchestrator::application_port::ProductionWorkPlanRequest {
                    production: input,
                    manifest,
                    operator_settings: settings,
                })
                .await?;
            return Ok(true);
        }

        let claimed = match self.claim(&step).await {
            Ok(claimed) => claimed,
            Err(ProductionError::TransitionConflict { .. }) => return Ok(false),
            Err(error) => return Err(error),
        };
        let output_digest = canonical_digest(&json!({
            "kind": "domain_command",
            "run_id": claimed.run_id,
            "revision_epoch": claimed.revision_epoch,
            "step_key": claimed.step_key,
        }))?;
        self.repository
            .complete_domain_step(
                claimed.id,
                self.worker_id.as_ref(),
                claimed.attempt,
                &output_digest,
            )
            .await?;
        Ok(true)
    }

    async fn execute_gate_step(&self, step: ProductionStepRecord) -> ProductionResult<bool> {
        let claimed = match self.claim(&step).await {
            Ok(claimed) => claimed,
            Err(ProductionError::TransitionConflict { .. }) => return Ok(false),
            Err(error) => return Err(error),
        };
        let package_type = match step.step_key.as_str() {
            "brief_approval" => PackageType::Brief,
            "script_package_approval" => PackageType::Script,
            "production_package_approval" => PackageType::Production,
            "quality_gate" => PackageType::Quality,
            _ => {
                return Err(ProductionError::TransitionConflict {
                    reason: "unknown package Gate step".into(),
                })
            }
        };
        let version = self
            .next_package_version(claimed.run_id, package_type)
            .await?;
        let package = match package_type {
            PackageType::Brief => {
                self.repository
                    .build_brief_package(claimed.run_id, version)
                    .await?
            }
            PackageType::Script => {
                self.repository
                    .build_script_package(claimed.run_id, version)
                    .await?
            }
            PackageType::Production => {
                self.repository
                    .build_production_package(claimed.run_id, version)
                    .await?
            }
            PackageType::Quality => {
                self.repository
                    .build_quality_package(claimed.run_id, version)
                    .await?
                    .package
            }
        };
        self.repository
            .save_claimed_package(
                &package,
                claimed.id,
                self.worker_id.as_ref(),
                claimed.attempt,
            )
            .await?;
        Ok(true)
    }

    async fn execute_external_step(&self, step: ProductionStepRecord) -> ProductionResult<bool> {
        if step.step_key == "work_plan_confirmation" {
            self.repository
                .mark_operator_wait(step.id, "work_generation_confirmation", "external_wait")
                .await?;
            return Ok(true);
        }
        if step.step_key == "wait_scene_visual_manifest" {
            let package_digest = self.step_input_digest(step.id).await?.ok_or_else(|| {
                ProductionError::TransitionConflict {
                    reason: "SceneVisualManifest wait has no package digest".into(),
                }
            })?;
            self.orchestrator
                .resume_scene_visual_manifest(step.run_id, &package_digest)
                .await?;
            return Ok(true);
        }
        if step.step_key == "wait_work_generation" {
            let external_run_id = sqlx::query_scalar::<_, Uuid>(
                r#"
                SELECT work_generation_run_id
                FROM production_domain_links
                WHERE run_id=$1 AND link_type='work_generation_run'
                ORDER BY created_at DESC, id DESC LIMIT 1
                "#,
            )
            .bind(step.run_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| ProductionError::TransitionConflict {
                reason: "work_generation external wait has no linked run".into(),
            })?;
            self.orchestrator
                .resume_work_generation(step.run_id, external_run_id)
                .await?;
            return Ok(true);
        }
        Ok(false)
    }

    async fn observe_external_step(&self, step: ProductionStepRecord, report: &mut RunnerReport) {
        if step.step_key == "work_plan_confirmation" {
            report.skipped += 1;
            return;
        }
        report.attempted += 1;
        match self.execute_external_step(step).await {
            Ok(true) => report.completed += 1,
            Ok(false) => report.skipped += 1,
            Err(_) => report.failed += 1,
        }
    }

    async fn default_work_plan_settings(&self) -> ProductionResult<ProductionWorkPlanSettings> {
        let llm_model_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM ai_models WHERE model_type='text' AND status='enabled' AND deleted_at IS NULL ORDER BY is_default DESC, sort_order, created_at, id LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| ProductionError::CapabilityMismatch {
            reason: "Full Crew Runner requires an enabled text model for WorkPlan defaults".into(),
        })?;
        let video_model_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM ai_models WHERE model_type='video' AND status='enabled' AND deleted_at IS NULL ORDER BY is_default DESC, sort_order, created_at, id LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| ProductionError::CapabilityMismatch {
            reason: "Full Crew Runner requires an enabled video model for WorkPlan defaults".into(),
        })?;
        Ok(ProductionWorkPlanSettings {
            llm_model_id,
            video_model_id,
            tts_model_id: None,
            tts_voice_type: None,
            duration_strategy: "script_total".into(),
            duration_seconds: None,
            aspect_ratio: "9:16".into(),
            resolution: "1080p".into(),
            audio_mode: "silent".into(),
            narration_override: None,
            audio_material_ids: Vec::new(),
            burn_subtitles: false,
            overrides: ProductionWorkPlanOverrides::default(),
        })
    }

    async fn step_input_digest(&self, step_id: Uuid) -> ProductionResult<Option<String>> {
        let digest = sqlx::query_scalar::<_, String>(
            "SELECT COALESCE(input_digest, '') FROM production_steps WHERE id=$1",
        )
        .bind(step_id)
        .fetch_one(&self.pool)
        .await
        .map_err(ProductionError::from)?;
        Ok((!digest.is_empty()).then_some(digest))
    }

    async fn next_package_version(
        &self,
        run_id: Uuid,
        package_type: PackageType,
    ) -> ProductionResult<u32> {
        let package_type = match package_type {
            PackageType::Brief => "brief",
            PackageType::Script => "script",
            PackageType::Production => "production",
            PackageType::Quality => "quality",
        };
        let next = sqlx::query_scalar::<_, i32>(
            r#"
            SELECT COALESCE(MAX(package_version),0) + 1
            FROM artifact_package_snapshots
            WHERE run_id=$1 AND package_type=$2
            "#,
        )
        .bind(run_id)
        .bind(package_type)
        .fetch_one(&self.pool)
        .await?;
        u32::try_from(next).map_err(|_| ProductionError::TransitionConflict {
            reason: "next package version is outside the supported range".into(),
        })
    }
}
