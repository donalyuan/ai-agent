//! ProductionOrchestrator：核心编排逻辑，路由决策 + 角色调度 + 检查点管理

pub mod fast_lane;
pub mod full_crew;
pub mod route;

pub use route::{ExecutionPlan, ProjectType};

use crate::error::{ProductionError, ProductionResult};
use crate::executor::role_executor::{RoleExecutionContext, RoleExecutionResult, RoleExecutor};
use crate::gates::GateRegistry;
use crate::roles::RoleRegistry;
use crate::state::ProductionStateRepository;
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
    /// # 模型 ID 解析顺序
    /// 1. `request_model_id`（请求体中的覆盖值）
    /// 2. `ProductionProject.metadata.preferred_model_id`
    /// 3. 若全部为空，返回 `AgentExecution` 错误（500）
    pub async fn execute_role(
        &self,
        project_id: Uuid,
        role_key: String,
        user_input: Option<String>,
        request_model_id: Option<Uuid>,
    ) -> ProductionResult<RoleExecutionResult> {
        // 1. 检查执行器和注册表是否已注入
        let audited_executor = self.audited_executor.clone().ok_or_else(|| {
            ProductionError::AgentExecution("executor not configured".into())
        })?;
        let _definition_registry = self.definition_registry.clone().ok_or_else(|| {
            ProductionError::AgentExecution("definition registry not configured".into())
        })?;

        // 2. 加载项目，解析优选模型 ID
        let project = self.state_repository.get_project(project_id).await?;
        let preferred_model_id = request_model_id
            .or_else(|| {
                project
                    .metadata
                    .get("preferred_model_id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<Uuid>().ok())
            })
            .ok_or_else(|| {
                ProductionError::AgentExecution(
                    "no model configured for production crew (set preferred_model_id in project metadata)".into()
                )
            })?;

        // 3. 在执行角色前运行 Gate 检查（提前拦截不满足条件的请求）
        self.run_pre_gate_checks(&role_key, project_id).await?;

        // 4. 构建执行上下文并委托给 RoleExecutor
        let ctx = RoleExecutionContext {
            pool: self.pool.clone(),
            definition_registry: _definition_registry,
            audited_executor,
            project_id,
            role_key,
            user_input,
            preferred_model_id,
        };

        RoleExecutor::execute(ctx, &self.role_registry).await
    }

    /// 执行前 Gate 检查（当前仅在 producer 执行前检查预算 Gate）。
    ///
    /// Gate 失败时透传对应的 `ProductionError`（GateRejected / GateWaitApproval）。
    async fn run_pre_gate_checks(
        &self,
        role_key: &str,
        project_id: Uuid,
    ) -> ProductionResult<()> {
        use crate::gates::GateContext;
        use crate::gates::GateDecision;

        // 仅在 producer（首个角色）执行前运行预算 Gate
        if role_key != "producer" {
            return Ok(());
        }
        let gate = match self.gate_registry.get("budget_gate") {
            Some(g) => g,
            None => return Ok(()),
        };

        let project = self.state_repository.get_project(project_id).await?;
        let ctx = GateContext {
            project_id,
            user_id: project.user_id,
            artifacts: std::collections::HashMap::new(),
            project_metadata: project.metadata,
        };

        match gate.check(&ctx).await? {
            GateDecision::Pass => Ok(()),
            GateDecision::Reject { reason } => Err(ProductionError::GateRejected {
                gate_name: "budget_gate".into(),
                reason,
            }),
            GateDecision::WaitApproval { artifact_id } => {
                Err(ProductionError::GateWaitApproval { artifact_id })
            }
        }
    }
}
