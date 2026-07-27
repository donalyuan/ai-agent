//! QualityGate：检查所有 TakeReview，若有 rejected 则阻断流程

use crate::error::ProductionResult;
use crate::gates::gate_trait::{Gate, GateContext, GateDecision};
use async_trait::async_trait;

pub struct QualityGate;

#[async_trait]
impl Gate for QualityGate {
    fn name(&self) -> &str {
        "quality_gate"
    }

    async fn check(&self, context: &GateContext) -> ProductionResult<GateDecision> {
        let reviews = context.artifacts.get("take_review").cloned().unwrap_or_default();

        // 找出所有 rejected 的评审
        let rejected: Vec<&serde_json::Value> = reviews
            .iter()
            .filter(|r| r.get("status").and_then(|s| s.as_str()) == Some("rejected"))
            .collect();

        if !rejected.is_empty() {
            let shot_ids: Vec<&str> = rejected
                .iter()
                .filter_map(|r| r.get("shot_id").and_then(|s| s.as_str()))
                .collect();
            return Ok(GateDecision::Reject {
                reason: format!(
                    "以下镜头评审不通过，需要重新生成：{}",
                    shot_ids.join(", ")
                ),
            });
        }

        // 若有 needs_revision，也返回等待
        let needs_revision = reviews
            .iter()
            .find(|r| r.get("status").and_then(|s| s.as_str()) == Some("needs_revision"));

        if let Some(review) = needs_revision {
            let artifact_id = review
                .get("id")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<uuid::Uuid>().ok())
                .unwrap_or_else(uuid::Uuid::new_v4);
            return Ok(GateDecision::WaitApproval { artifact_id });
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
    async fn test_pass_with_all_approved() {
        let gate = QualityGate;
        let mut artifacts = HashMap::new();
        artifacts.insert("take_review".to_string(), vec![
            json!({ "id": Uuid::new_v4().to_string(), "shot_id": "shot_001", "status": "approved" }),
        ]);
        let ctx = GateContext {
            project_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            artifacts,
            project_metadata: json!({}),
        };
        assert!(matches!(gate.check(&ctx).await.unwrap(), GateDecision::Pass));
    }

    #[tokio::test]
    async fn test_reject_with_rejected_take() {
        let gate = QualityGate;
        let mut artifacts = HashMap::new();
        artifacts.insert("take_review".to_string(), vec![
            json!({ "id": Uuid::new_v4().to_string(), "shot_id": "shot_001", "status": "rejected" }),
        ]);
        let ctx = GateContext {
            project_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            artifacts,
            project_metadata: json!({}),
        };
        assert!(matches!(gate.check(&ctx).await.unwrap(), GateDecision::Reject { .. }));
    }
}
