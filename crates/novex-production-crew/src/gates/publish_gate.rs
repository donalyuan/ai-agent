//! PublishGate：验证所有必需产物已批准，用户具备发布权限

use crate::error::ProductionResult;
use crate::gates::gate_trait::{Gate, GateContext, GateDecision};
use async_trait::async_trait;

pub struct PublishGate;

/// Full Crew 模式发布前必须 approved 的产物类型
const REQUIRED_ARTIFACT_TYPES: &[&str] = &[
    "creative_brief",
    "story_bible",
    "script_draft",
    "directorial_treatment",
];

#[async_trait]
impl Gate for PublishGate {
    fn name(&self) -> &str {
        "publish_gate"
    }

    async fn check(&self, context: &GateContext) -> ProductionResult<GateDecision> {
        // 检查所有必需产物是否有 approved 版本
        for artifact_type in REQUIRED_ARTIFACT_TYPES {
            let artifacts = context.artifacts.get(*artifact_type).cloned().unwrap_or_default();
            let has_approved = artifacts
                .iter()
                .any(|a| a.get("status").and_then(|s| s.as_str()) == Some("approved"));
            if !has_approved {
                return Ok(GateDecision::Reject {
                    reason: format!("发布前必须批准 {}，当前尚无已批准版本", artifact_type),
                });
            }
        }

        // 检查 take_review：所有评审必须通过（无 rejected）
        let reviews = context.artifacts.get("take_review").cloned().unwrap_or_default();
        if !reviews.is_empty() {
            let has_rejected = reviews
                .iter()
                .any(|r| r.get("status").and_then(|s| s.as_str()) == Some("rejected"));
            if has_rejected {
                return Ok(GateDecision::Reject {
                    reason: "存在未通过 QC 的镜头，不可发布".to_string(),
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
    async fn test_reject_missing_required_artifact() {
        let gate = PublishGate;
        let ctx = GateContext {
            project_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            artifacts: HashMap::new(),
            project_metadata: json!({}),
        };
        assert!(matches!(gate.check(&ctx).await.unwrap(), GateDecision::Reject { .. }));
    }
}
