//! BudgetGate：在视频生成前检查用户预算是否充足

use crate::error::ProductionResult;
use crate::gates::gate_trait::{Gate, GateContext, GateDecision};
use async_trait::async_trait;

pub struct BudgetGate;

#[async_trait]
impl Gate for BudgetGate {
    fn name(&self) -> &str {
        "budget_gate"
    }

    async fn check(&self, context: &GateContext) -> ProductionResult<GateDecision> {
        // 从项目元数据获取预算约束
        let budget_limit = context
            .project_metadata
            .get("budget")
            .and_then(|v| v.as_f64());

        // 若无预算限制，直接通过
        let budget_limit = match budget_limit {
            Some(b) if b > 0.0 => b,
            _ => return Ok(GateDecision::Pass),
        };

        // 估算生成成本：每个 shot_contract 对应一次生成，固定单价
        // 实际成本计算应对接计费系统，此处用简化估算
        let shot_count = context
            .artifacts
            .get("shot_contract")
            .map(|s| s.len())
            .unwrap_or(0);

        // 保守估算：每个镜头生成成本 0.5 元
        let estimated_cost = shot_count as f64 * 0.5;

        if estimated_cost > budget_limit {
            return Ok(GateDecision::Reject {
                reason: format!(
                    "预估生成成本 {:.2} 元超出预算限制 {:.2} 元（共 {} 个镜头）",
                    estimated_cost, budget_limit, shot_count
                ),
            });
        }

        Ok(GateDecision::Pass)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_pass_without_budget_limit() {
        let gate = BudgetGate;
        let ctx = GateContext {
            project_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            artifacts: HashMap::new(),
            project_metadata: json!({}),
        };
        assert!(matches!(
            gate.check(&ctx).await.unwrap(),
            GateDecision::Pass
        ));
    }

    #[tokio::test]
    async fn test_reject_when_over_budget() {
        let gate = BudgetGate;
        let mut artifacts = HashMap::new();
        // 3个镜头 = 1.5元，但预算只有 1.0 元
        artifacts.insert(
            "shot_contract".to_string(),
            vec![json!({}), json!({}), json!({})],
        );
        let ctx = GateContext {
            project_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            artifacts,
            project_metadata: json!({ "budget": 1.0 }),
        };
        assert!(matches!(
            gate.check(&ctx).await.unwrap(),
            GateDecision::Reject { .. }
        ));
    }
}
