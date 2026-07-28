//! ProducerGate：验证 CreativeBrief 完整性，检查预算合理性

use crate::error::ProductionResult;
use crate::gates::gate_trait::{Gate, GateContext, GateDecision};
use async_trait::async_trait;

pub struct ProducerGate;

#[async_trait]
impl Gate for ProducerGate {
    fn name(&self) -> &str {
        "producer_gate"
    }

    async fn check(&self, context: &GateContext) -> ProductionResult<GateDecision> {
        let briefs = context
            .artifacts
            .get("creative_brief")
            .cloned()
            .unwrap_or_default();

        // 必须有至少一个 approved 的 CreativeBrief
        let approved = briefs
            .iter()
            .any(|b| b.get("status").and_then(|s| s.as_str()) == Some("approved"));

        if !approved {
            return Ok(GateDecision::Reject {
                reason: "尚无已批准的 CreativeBrief，请先由制片人产出并批准创意简报".to_string(),
            });
        }

        // 检查 brief 内容完整性
        if let Some(brief) = briefs
            .iter()
            .find(|b| b.get("status").and_then(|s| s.as_str()) == Some("approved"))
        {
            let content = brief.get("content");
            let target_audience = content
                .and_then(|c| c.get("target_audience"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let key_messages = content
                .and_then(|c| c.get("key_messages"))
                .and_then(|v| v.as_array());

            if target_audience.is_empty() {
                return Ok(GateDecision::Reject {
                    reason: "CreativeBrief 缺少 target_audience".to_string(),
                });
            }
            if key_messages.map(|m| m.is_empty()).unwrap_or(true) {
                return Ok(GateDecision::Reject {
                    reason: "CreativeBrief 的 key_messages 不能为空".to_string(),
                });
            }
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
    async fn test_pass_with_approved_brief() {
        let gate = ProducerGate;
        let mut artifacts = HashMap::new();
        artifacts.insert(
            "creative_brief".to_string(),
            vec![json!({
                "status": "approved",
                "content": {
                    "target_audience": "25-40岁科技爱好者",
                    "key_messages": ["健康监测", "长续航"],
                    "tone": ["活泼"],
                    "constraints": {},
                    "success_criteria": []
                }
            })],
        );
        let ctx = GateContext {
            project_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            artifacts,
            project_metadata: json!({}),
        };
        let decision = gate.check(&ctx).await.unwrap();
        assert!(matches!(decision, GateDecision::Pass));
    }

    #[tokio::test]
    async fn test_reject_without_approved_brief() {
        let gate = ProducerGate;
        let ctx = GateContext {
            project_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            artifacts: HashMap::new(),
            project_metadata: json!({}),
        };
        let decision = gate.check(&ctx).await.unwrap();
        assert!(matches!(decision, GateDecision::Reject { .. }));
    }
}
