//! 路由决策：根据项目类型返回执行计划

use crate::error::{ProductionError, ProductionResult};
use crate::state::repository::ProductionProject;
use serde::{Deserialize, Serialize};

/// 项目类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectType {
    /// 快速通道：简化流程，直接生成
    FastLane,
    /// 完整团队：全角色协作流程
    FullCrew,
}

/// 执行计划：描述当前项目应该走哪条执行路径
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub project_type: ProjectType,
    /// 按顺序执行的角色列表（FastLane 为空）
    pub role_sequence: Vec<String>,
    /// 每个 Gate 检查点：(插入位置 index, gate_name)
    pub gate_checkpoints: Vec<(usize, String)>,
}

/// 根据项目确定执行路由
pub fn route_execution(project: &ProductionProject) -> ProductionResult<ExecutionPlan> {
    match project.project_type.as_str() {
        "fast_lane" => Ok(ExecutionPlan {
            project_type: ProjectType::FastLane,
            role_sequence: vec![],
            gate_checkpoints: vec![
                (0, "budget_gate".to_string()),
            ],
        }),
        "full_crew" => Ok(ExecutionPlan {
            project_type: ProjectType::FullCrew,
            role_sequence: vec![
                "producer".to_string(),
                "screenwriter".to_string(),
                "director".to_string(),
                "cinematographer".to_string(),
                "performance_director".to_string(),
                "sound_director".to_string(),
                "editor".to_string(),
                "qc".to_string(),
            ],
            // Gate 检查点：在第 N 个角色执行前插入
            gate_checkpoints: vec![
                (0, "budget_gate".to_string()),         // Producer 执行前
                (1, "producer_gate".to_string()),       // Screenwriter 执行前
                (2, "script_approval_gate".to_string()), // Director 执行前
                (3, "technical_feasibility_gate".to_string()), // Cinematographer 后
                (7, "quality_gate".to_string()),        // QC 完成后
            ],
        }),
        other => Err(ProductionError::InvalidRoleSequence {
            message: format!("未知的项目类型: {}", other),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    fn make_project(project_type: &str) -> ProductionProject {
        ProductionProject {
            id: Uuid::new_v4(),
            title: "测试项目".to_string(),
            description: None,
            project_type: project_type.to_string(),
            status: "created".to_string(),
            user_id: Uuid::new_v4(),
            metadata: json!({}),
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            deleted_at: None,
        }
    }

    #[test]
    fn test_fast_lane_route() {
        let project = make_project("fast_lane");
        let plan = route_execution(&project).unwrap();
        assert_eq!(plan.project_type, ProjectType::FastLane);
        assert!(plan.role_sequence.is_empty());
    }

    #[test]
    fn test_full_crew_route() {
        let project = make_project("full_crew");
        let plan = route_execution(&project).unwrap();
        assert_eq!(plan.project_type, ProjectType::FullCrew);
        assert!(!plan.role_sequence.is_empty());
        assert_eq!(plan.role_sequence[0], "producer");
    }

    #[test]
    fn test_unknown_type_error() {
        let project = make_project("unknown");
        assert!(route_execution(&project).is_err());
    }
}
