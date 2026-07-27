//! Full Crew 执行计划：定义角色依赖关系和 Gate 插入位置

use crate::error::ProductionResult;
use crate::orchestrator::route::ExecutionPlan;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 流程执行记录：追踪当前流程的进度和状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowExecution {
    /// 流程唯一 ID
    pub flow_id: Uuid,
    /// 所属项目 ID
    pub project_id: Uuid,
    /// 当前流程状态
    pub status: FlowStatus,
    /// 已完成的角色列表
    pub completed_roles: Vec<String>,
    /// 当前正在执行的角色（若有）
    pub current_role: Option<String>,
    /// 待执行的角色列表
    pub pending_roles: Vec<String>,
    /// 等待中的 Gate 信息（若处于 waiting_approval 状态）
    pub waiting_for: Option<GateWaiting>,
}

/// 流程状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowStatus {
    Running,
    WaitingApproval,
    Completed,
    Failed,
}

/// Gate 等待信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateWaiting {
    /// 触发等待的 Gate 名称
    pub gate_name: String,
    /// 需要审批的产物 ID
    pub artifact_id: Option<Uuid>,
}

/// 根据执行计划生成 Full Crew 流程初始状态
pub fn plan_full_crew(project_id: Uuid, plan: &ExecutionPlan) -> ProductionResult<FlowExecution> {
    Ok(FlowExecution {
        flow_id: Uuid::new_v4(),
        project_id,
        status: FlowStatus::Running,
        completed_roles: vec![],
        current_role: plan.role_sequence.first().cloned(),
        pending_roles: plan.role_sequence[1..].to_vec(),
        waiting_for: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::route::ProjectType;

    #[test]
    fn test_plan_full_crew_initializes_correctly() {
        let project_id = Uuid::new_v4();
        let plan = ExecutionPlan {
            project_type: ProjectType::FullCrew,
            role_sequence: vec!["producer".to_string(), "screenwriter".to_string()],
            gate_checkpoints: vec![],
        };
        let flow = plan_full_crew(project_id, &plan).unwrap();
        assert_eq!(flow.project_id, project_id);
        assert_eq!(flow.current_role, Some("producer".to_string()));
        assert_eq!(flow.pending_roles, vec!["screenwriter".to_string()]);
        assert_eq!(flow.status, FlowStatus::Running);
    }
}
