//! ProductionOrchestrator：核心编排逻辑，路由决策 + 角色调度 + 检查点管理

pub mod application_port;
pub mod fast_lane;
pub mod full_crew;
pub mod route;

pub use route::{ExecutionPlan, ProjectType};

use crate::durable::media::{MediaEvidenceSnapshot, RequiredTakeInventorySnapshot};
use crate::durable::production_input::ProductionPackageInput;
use crate::durable::repository::DurableProductionRepository;
use crate::error::{ProductionError, ProductionResult};
use crate::executor::role_executor::{
    PreparedRoleExecution, RoleExecutionResult, RoleExecutor, RolePrepareContext,
};
use crate::gates::GateRegistry;
use crate::roles::RoleRegistry;
use crate::state::ProductionStateRepository;
use application_port::{
    MediaEvidenceProvider, ProductionWorkPlanRequest, SceneVisualManifestPort,
    SceneVisualManifestReference, TemporaryMediaAccess, WorkGenerationPlanningPort,
    WorkGenerationRunDisposition, WorkGenerationRunPort, WorkGenerationRunReference,
    WorkPlanReference, WorkVersionReworkPort, WorkVersionReworkReference, WorkVersionReworkRequest,
};
use novex_agent::AuditedModelExecutor;
use novex_ai_core::DefinitionRegistry;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

/// 核心编排器：持有所有依赖，无请求级状态。
///
/// `audited_executor` 和 `definition_registry` 在 AppState 中注入；
/// 在测试环境（AppState::test()）中保持 None，调用 execute_role 时返回 AgentExecution 错误。
pub struct ProductionOrchestrator {
    pub role_registry: Arc<RoleRegistry>,
    pub state_repository: Arc<ProductionStateRepository>,
    pub gate_registry: Arc<GateRegistry>,
    pub pool: PgPool,
    /// 带审计的模型执行器（生产环境必须注入）
    pub audited_executor: Option<Arc<AuditedModelExecutor>>,
    /// Agent/Prompt 定义注册表（生产环境必须注入）
    pub definition_registry: Option<Arc<DefinitionRegistry>>,
    /// 既有 SceneVisualManifest Application Service 的类型化端口。
    pub scene_visual_manifest_port: Option<Arc<dyn SceneVisualManifestPort>>,
    /// 既有 WorkGenerationService 规划入口的类型化端口。
    pub work_generation_planning_port: Option<Arc<dyn WorkGenerationPlanningPort>>,
    /// 只读取既有人工确认结果和作品运行真实状态的类型化端口。
    pub work_generation_run_port: Option<Arc<dyn WorkGenerationRunPort>>,
    /// 读取真实媒体并只返回版本化脱敏分析的受控端口。
    pub media_evidence_provider: Option<Arc<dyn MediaEvidenceProvider>>,
    /// 通过既有 Work Library 派生返工草稿和差异计划的类型化端口。
    pub work_version_rework_port: Option<Arc<dyn WorkVersionReworkPort>>,
}

impl ProductionOrchestrator {
    pub fn new(
        pool: PgPool,
        role_registry: Arc<RoleRegistry>,
        gate_registry: Arc<GateRegistry>,
    ) -> Self {
        let state_repository = Arc::new(ProductionStateRepository::new(pool.clone()));
        Self {
            role_registry,
            state_repository,
            gate_registry,
            pool,
            audited_executor: None,
            definition_registry: None,
            scene_visual_manifest_port: None,
            work_generation_planning_port: None,
            work_generation_run_port: None,
            media_evidence_provider: None,
            work_version_rework_port: None,
        }
    }

    /// 路由决策：根据 project_type 返回执行计划
    pub async fn route_execution(&self, project_id: Uuid) -> ProductionResult<ExecutionPlan> {
        let project = self.state_repository.get_project(project_id).await?;
        route::route_execution(&project)
    }

    /// 执行单个角色，返回执行结果。
    ///
    /// # 依赖检查
    /// - `audited_executor` 或 `definition_registry` 未注入时，返回 `AgentExecution` 错误（500）。
    ///
    /// 旧单角色入口不具备 durable step/lease/package 身份，必须 fail-closed。
    pub async fn execute_role(
        &self,
        _project_id: Uuid,
        _role_key: String,
    ) -> ProductionResult<RoleExecutionResult> {
        Err(ProductionError::TransitionConflict {
            reason: "single-role production bypass is disabled; execute the current durable step"
                .into(),
        })
    }

    /// 为当前已认领的 durable role step 建立 provider 前审计和资源边界。
    pub async fn prepare_role_step(
        &self,
        step_id: Uuid,
        lease_owner: String,
        attempt: i32,
    ) -> ProductionResult<PreparedRoleExecution> {
        let audited_executor = self
            .audited_executor
            .clone()
            .ok_or_else(|| ProductionError::AgentExecution("executor not configured".into()))?;
        let definition_registry = self.definition_registry.clone().ok_or_else(|| {
            ProductionError::AgentExecution("definition registry not configured".into())
        })?;
        RoleExecutor::prepare(
            RolePrepareContext {
                pool: self.pool.clone(),
                definition_registry,
                audited_executor,
                step_id,
                lease_owner,
                attempt,
            },
            &self.role_registry,
        )
        .await
    }

    /// 将完整 ProductionPackage 输入提交给既有 SceneVisualManifest 边界。
    pub async fn prepare_scene_visual_manifest(
        &self,
        input: ProductionPackageInput,
    ) -> ProductionResult<SceneVisualManifestReference> {
        input.package_snapshot()?;
        let port = self.scene_visual_manifest_port.as_ref().ok_or_else(|| {
            ProductionError::CapabilityMismatch {
                reason: "SceneVisualManifest application port is not configured".into(),
            }
        })?;
        let result = port.prepare_scene_visual_manifest(input.clone()).await?;
        result.validate_for(&input)?;
        Ok(result)
    }

    /// 从 PostgreSQL 恢复当前已批准 package，并推进 manifest external wait。
    pub async fn resume_scene_visual_manifest(
        &self,
        run_id: Uuid,
        package_digest: &str,
    ) -> ProductionResult<SceneVisualManifestReference> {
        let repository = DurableProductionRepository::new(self.pool.clone());
        let input = repository
            .load_approved_production_input(run_id, package_digest)
            .await?;
        match self.prepare_scene_visual_manifest(input.clone()).await {
            Ok(manifest) => {
                repository
                    .complete_scene_visual_manifest_wait(&input, &manifest)
                    .await?;
                Ok(manifest)
            }
            Err(error @ ProductionError::ExternalWait { .. }) => {
                let details = error
                    .details()
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                repository
                    .mark_scene_visual_manifest_wait(&input, &details)
                    .await?;
                Err(error)
            }
            Err(error) => Err(error),
        }
    }

    /// 将当前 manifest 与人工参数提交给既有 WorkGenerationService 规划边界。
    pub async fn create_work_plan(
        &self,
        input: ProductionWorkPlanRequest,
    ) -> ProductionResult<WorkPlanReference> {
        input.validate()?;
        let port = self.work_generation_planning_port.as_ref().ok_or_else(|| {
            ProductionError::CapabilityMismatch {
                reason: "WorkGeneration planning application port is not configured".into(),
            }
        })?;
        let result = port.create_work_plan(input).await?;
        result.validate()?;
        Ok(result)
    }

    /// 调用既有规划入口后，只把返回的正式引用写入 ProductionRun。
    pub async fn resume_create_work_plan(
        &self,
        input: ProductionWorkPlanRequest,
    ) -> ProductionResult<WorkPlanReference> {
        let result = self.create_work_plan(input.clone()).await?;
        DurableProductionRepository::new(self.pool.clone())
            .complete_work_plan_creation(&input, &result)
            .await?;
        Ok(result)
    }

    /// 在既有人工确认完成后关联正式运行，并进入作品运行 external wait。
    pub async fn resume_work_plan_confirmation(
        &self,
        run_id: Uuid,
        plan: WorkPlanReference,
    ) -> ProductionResult<WorkGenerationRunReference> {
        plan.validate()?;
        let port = self.work_generation_run_port.as_ref().ok_or_else(|| {
            ProductionError::CapabilityMismatch {
                reason: "WorkGeneration run application port is not configured".into(),
            }
        })?;
        let external = port.confirmed_run_for_plan(plan.clone()).await?;
        external.validate_for(&plan)?;
        DurableProductionRepository::new(self.pool.clone())
            .record_work_generation_confirmation(run_id, &plan, &external)
            .await?;
        Ok(external)
    }

    /// 观察既有作品运行并同步 Production wait；观察过程不创建 retry 或 provider 任务。
    pub async fn resume_work_generation(
        &self,
        run_id: Uuid,
        work_generation_run_id: Uuid,
    ) -> ProductionResult<WorkGenerationRunDisposition> {
        let port = self.work_generation_run_port.as_ref().ok_or_else(|| {
            ProductionError::CapabilityMismatch {
                reason: "WorkGeneration run application port is not configured".into(),
            }
        })?;
        let external = port.observe_run(work_generation_run_id).await?;
        if external.run_id != work_generation_run_id {
            return Err(ProductionError::TransitionConflict {
                reason: "WorkGeneration observer returned another run identity".into(),
            });
        }
        let repository = DurableProductionRepository::new(self.pool.clone());
        let disposition = repository
            .sync_work_generation_state(run_id, &external)
            .await?;
        if disposition == WorkGenerationRunDisposition::EvidenceBlocker
            && external.status == application_port::WorkGenerationRunStatus::Succeeded
            && external.final_media_ready
            && !external.take_inventory_ready
        {
            let inventory = repository.build_required_take_inventory(run_id).await?;
            repository.save_required_take_inventory(&inventory).await?;
            let refreshed = port.observe_run(work_generation_run_id).await?;
            return repository
                .sync_work_generation_state(run_id, &refreshed)
                .await;
        }
        Ok(disposition)
    }

    /// 从 PostgreSQL 正式事实构建并持久化当前 compose 的 required take inventory。
    pub async fn build_required_take_inventory(
        &self,
        run_id: Uuid,
    ) -> ProductionResult<RequiredTakeInventorySnapshot> {
        let repository = DurableProductionRepository::new(self.pool.clone());
        let inventory = repository.build_required_take_inventory(run_id).await?;
        repository.save_required_take_inventory(&inventory).await?;
        Ok(inventory)
    }

    /// 通过调用期临时访问读取真实媒体，并把不可变 evidence 保存到 PostgreSQL。
    pub async fn capture_media_evidence(
        &self,
        inventory: RequiredTakeInventorySnapshot,
        access: TemporaryMediaAccess,
    ) -> ProductionResult<MediaEvidenceSnapshot> {
        inventory.validate()?;
        if access.asset_id != inventory.final_asset.artifact_id {
            return Err(ProductionError::TransitionConflict {
                reason: "temporary media access does not target the inventory final asset".into(),
            });
        }
        let provider = self.media_evidence_provider.as_ref().ok_or_else(|| {
            ProductionError::CapabilityMismatch {
                reason: "MediaEvidence provider is not configured".into(),
            }
        })?;
        let analysis = provider.inspect_media(inventory.clone(), access).await?;
        let evidence = MediaEvidenceSnapshot::build(
            Uuid::new_v4(),
            inventory.run_id,
            inventory.source_step_id,
            inventory.source_attempt,
            inventory.revision_epoch,
            inventory.work_version_id,
            inventory.inventory_id,
            inventory.inventory_digest.clone(),
            inventory.final_asset.clone(),
            analysis.vision_capability_version,
            analysis.audio_capability_version,
            analysis.redacted_analysis,
        )?;
        let repository = DurableProductionRepository::new(self.pool.clone());
        repository.save_required_take_inventory(&inventory).await?;
        repository.save_media_evidence(&evidence).await?;
        Ok(evidence)
    }

    /// 创建有界质量返工草稿；只保存 Work Library 正式返回的版本和差异计划引用。
    pub async fn resume_quality_rework(
        &self,
        request: WorkVersionReworkRequest,
    ) -> ProductionResult<WorkVersionReworkReference> {
        request.validate()?;
        let repository = DurableProductionRepository::new(self.pool.clone());
        if let Some(reference) = repository.quality_rework_replay(&request).await? {
            return Ok(reference);
        }
        let port = self.work_version_rework_port.as_ref().ok_or_else(|| {
            ProductionError::CapabilityMismatch {
                reason: "WorkVersion rework application port is not configured".into(),
            }
        })?;
        repository.ensure_quality_rework_allowed(&request).await?;
        repository.reserve_quality_rework(&request).await?;
        let reference = match port.create_rework_draft(request.clone()).await {
            Ok(reference) => reference,
            Err(error) => {
                repository.release_quality_rework(&request).await?;
                return Err(error);
            }
        };
        reference.validate_for(&request)?;
        repository.record_quality_rework(&request, &reference).await
    }
}
