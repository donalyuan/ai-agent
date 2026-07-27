//! ProductionOrchestrator：核心编排逻辑，路由决策 + 角色调度 + 检查点管理

pub mod fast_lane;
pub mod full_crew;
pub mod route;

pub use route::{ExecutionPlan, ProjectType};

use crate::error::ProductionResult;
use crate::gates::GateRegistry;
use crate::roles::RoleRegistry;
use crate::state::ProductionStateRepository;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

/// 核心编排器：持有所有依赖，无请求级状态
pub struct ProductionOrchestrator {
    pub role_registry: Arc<RoleRegistry>,
    pub state_repository: Arc<ProductionStateRepository>,
    pub gate_registry: Arc<GateRegistry>,
    pub pool: PgPool,
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
        }
    }

    /// 路由决策：根据 project_type 返回执行计划
    pub async fn route_execution(&self, project_id: Uuid) -> ProductionResult<ExecutionPlan> {
        let project = self.state_repository.get_project(project_id).await?;
        route::route_execution(&project)
    }
}
