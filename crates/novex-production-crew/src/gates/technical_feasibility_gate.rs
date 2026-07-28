//! TechnicalFeasibilityGate：检查摄影指导提出的高优先级建议是否已全部响应

use crate::error::ProductionResult;
use crate::gates::gate_trait::{Gate, GateContext, GateDecision};
use async_trait::async_trait;

pub struct TechnicalFeasibilityGate;

#[async_trait]
impl Gate for TechnicalFeasibilityGate {
    fn name(&self) -> &str {
        "technical_feasibility_gate"
    }

    /// 如果有未响应的 high 优先级摄影指导建议，返回 WaitApproval；否则通过
    async fn check(&self, context: &GateContext) -> ProductionResult<GateDecision> {
        let suggestions = context
            .artifacts
            .get("collaboration_suggestions")
            .cloned()
            .unwrap_or_default();

        // 找出来自 cinematographer 且 priority=high 且 status=pending 的建议
        let blocking = suggestions.iter().find(|s| {
            s.get("from_role").and_then(|v| v.as_str()) == Some("cinematographer")
                && s.get("status").and_then(|v| v.as_str()) == Some("pending")
                && s.get("content")
                    .and_then(|c| c.get("priority"))
                    .and_then(|v| v.as_str())
                    == Some("high")
        });

        if let Some(suggestion) = blocking {
            let suggestion_id = suggestion
                .get("id")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<uuid::Uuid>().ok())
                .unwrap_or_else(uuid::Uuid::new_v4);
            return Ok(GateDecision::WaitApproval {
                artifact_id: suggestion_id,
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
    async fn test_pass_with_no_high_priority_suggestions() {
        let gate = TechnicalFeasibilityGate;
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
    async fn test_wait_with_pending_high_priority() {
        let gate = TechnicalFeasibilityGate;
        let mut artifacts = HashMap::new();
        artifacts.insert(
            "collaboration_suggestions".to_string(),
            vec![json!({
                "id": Uuid::new_v4().to_string(),
                "from_role": "cinematographer",
                "status": "pending",
                "content": { "priority": "high", "reason": "技术问题", "specific_change": "调整" }
            })],
        );
        let ctx = GateContext {
            project_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            artifacts,
            project_metadata: json!({}),
        };
        assert!(matches!(
            gate.check(&ctx).await.unwrap(),
            GateDecision::WaitApproval { .. }
        ));
    }
}
