//! FlowExecutor：编排完整流程，顺序执行各角色并在 Gate 检查点暂停或推进

use crate::error::ProductionResult;
use crate::gates::gate_trait::GateDecision;
use crate::orchestrator::full_crew::{FlowExecution, FlowStatus, GateWaiting};
use crate::orchestrator::route::ExecutionPlan;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 流程执行器：管理一个 Full Crew 流程的推进
pub struct FlowExecutor;

impl FlowExecutor {
    /// 推进流程到下一个角色或 Gate 检查点
    ///
    /// # 行为逻辑
    /// 1. 找到当前待执行角色
    /// 2. 执行所有在该位置的 Gate 检查点
    /// 3. Gate 通过 → 标记角色完成，移到下一个
    /// 4. Gate WaitApproval → 暂停流程，返回等待状态
    /// 5. Gate Reject → 标记流程失败
    ///
    /// 实际的 Gate.check 调用和 RoleExecutor.execute 调用需注入依赖，
    /// 当前为结构骨架，实际业务逻辑由 ProductionOrchestrator 协调完成。
    pub fn advance_flow(
        mut flow: FlowExecution,
        _plan: &ExecutionPlan,
        gate_decisions: Vec<(String, GateDecision)>,
    ) -> ProductionResult<FlowExecution> {
        // 处理 Gate 决策
        for (gate_name, decision) in gate_decisions {
            match decision {
                GateDecision::Pass => {
                    tracing::debug!(gate = %gate_name, "Gate 通过");
                }
                GateDecision::Reject { reason } => {
                    tracing::warn!(gate = %gate_name, reason = %reason, "Gate 拒绝，流程中止");
                    flow.status = FlowStatus::Failed;
                    return Ok(flow);
                }
                GateDecision::WaitApproval { artifact_id } => {
                    tracing::info!(gate = %gate_name, artifact_id = %artifact_id, "Gate 等待审批");
                    flow.status = FlowStatus::WaitingApproval;
                    flow.waiting_for = Some(GateWaiting {
                        gate_name,
                        artifact_id: Some(artifact_id),
                    });
                    return Ok(flow);
                }
            }
        }

        // 移动当前角色到已完成列表
        if let Some(current) = flow.current_role.take() {
            flow.completed_roles.push(current);
        }

        // 取下一个角色
        if flow.pending_roles.is_empty() {
            flow.status = FlowStatus::Completed;
            flow.current_role = None;
        } else {
            flow.current_role = Some(flow.pending_roles.remove(0));
        }

        Ok(flow)
    }
}

/// 流程查询响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowStatusResponse {
    pub flow_id: Uuid,
    pub status: FlowStatus,
    pub completed_roles: Vec<String>,
    pub current_role: Option<String>,
    pub pending_roles: Vec<String>,
    pub waiting_for: Option<GateWaiting>,
}

impl From<FlowExecution> for FlowStatusResponse {
    fn from(flow: FlowExecution) -> Self {
        Self {
            flow_id: flow.flow_id,
            status: flow.status,
            completed_roles: flow.completed_roles,
            current_role: flow.current_role,
            pending_roles: flow.pending_roles,
            waiting_for: flow.waiting_for,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::full_crew::{FlowExecution, FlowStatus};
    use crate::orchestrator::route::{ExecutionPlan, ProjectType};

    fn make_flow(roles: Vec<&str>) -> FlowExecution {
        FlowExecution {
            flow_id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            status: FlowStatus::Running,
            completed_roles: vec![],
            current_role: roles.first().map(|r| r.to_string()),
            pending_roles: roles[1..].iter().map(|r| r.to_string()).collect(),
            waiting_for: None,
        }
    }

    fn make_plan(roles: Vec<&str>) -> ExecutionPlan {
        ExecutionPlan {
            project_type: ProjectType::FullCrew,
            role_sequence: roles.iter().map(|r| r.to_string()).collect(),
            gate_checkpoints: vec![],
        }
    }

    #[test]
    fn test_advance_with_pass_gates() {
        let flow = make_flow(vec!["producer", "screenwriter"]);
        let plan = make_plan(vec!["producer", "screenwriter"]);
        let decisions = vec![("budget_gate".to_string(), GateDecision::Pass)];
        let result = FlowExecutor::advance_flow(flow, &plan, decisions).unwrap();
        assert!(result.completed_roles.contains(&"producer".to_string()));
        assert_eq!(result.current_role, Some("screenwriter".to_string()));
        assert_eq!(result.status, FlowStatus::Running);
    }

    #[test]
    fn test_flow_completes_when_all_done() {
        let flow = make_flow(vec!["producer"]);
        let plan = make_plan(vec!["producer"]);
        let result = FlowExecutor::advance_flow(flow, &plan, vec![]).unwrap();
        assert_eq!(result.status, FlowStatus::Completed);
        assert_eq!(result.current_role, None);
    }

    #[test]
    fn test_gate_reject_stops_flow() {
        let flow = make_flow(vec!["screenwriter"]);
        let plan = make_plan(vec!["screenwriter"]);
        let decisions = vec![(
            "script_approval_gate".to_string(),
            GateDecision::Reject { reason: "未批准剧本".to_string() },
        )];
        let result = FlowExecutor::advance_flow(flow, &plan, decisions).unwrap();
        assert_eq!(result.status, FlowStatus::Failed);
    }

    #[test]
    fn test_gate_wait_approval_pauses_flow() {
        let artifact_id = Uuid::new_v4();
        let flow = make_flow(vec!["screenwriter"]);
        let plan = make_plan(vec!["screenwriter"]);
        let decisions = vec![(
            "script_approval_gate".to_string(),
            GateDecision::WaitApproval { artifact_id },
        )];
        let result = FlowExecutor::advance_flow(flow, &plan, decisions).unwrap();
        assert_eq!(result.status, FlowStatus::WaitingApproval);
        assert!(result.waiting_for.is_some());
    }
}
