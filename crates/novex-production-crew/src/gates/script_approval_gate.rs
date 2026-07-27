//! ScriptApprovalGate：检查 ScriptDraft 是否已获得人工批准

use crate::error::ProductionResult;
use crate::gates::gate_trait::{Gate, GateContext, GateDecision};
use async_trait::async_trait;
use uuid::Uuid;

pub struct ScriptApprovalGate;

#[async_trait]
impl Gate for ScriptApprovalGate {
    fn name(&self) -> &str {
        "script_approval_gate"
    }

    async fn check(&self, context: &GateContext) -> ProductionResult<GateDecision> {
        let drafts = context.artifacts.get("script_draft").cloned().unwrap_or_default();

        if drafts.is_empty() {
            return Ok(GateDecision::Reject {
                reason: "尚无 ScriptDraft，请先执行编剧角色".to_string(),
            });
        }

        // 查找最新版本的剧本（version 最大的那个）
        let latest = drafts.iter().max_by_key(|d| {
            d.get("version").and_then(|v| v.as_i64()).unwrap_or(0)
        });

        if let Some(draft) = latest {
            let status = draft.get("status").and_then(|s| s.as_str()).unwrap_or("");
            if status == "approved" {
                return Ok(GateDecision::Pass);
            }
            // 返回等待审批，带上剧本 ID
            let artifact_id = draft
                .get("id")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<Uuid>().ok())
                .unwrap_or_else(Uuid::new_v4);
            return Ok(GateDecision::WaitApproval { artifact_id });
        }

        Ok(GateDecision::Reject {
            reason: "ScriptDraft 数据异常".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_pass_with_approved_script() {
        let gate = ScriptApprovalGate;
        let mut artifacts = HashMap::new();
        let script_id = Uuid::new_v4();
        artifacts.insert("script_draft".to_string(), vec![json!({
            "id": script_id.to_string(),
            "status": "approved",
            "version": 1
        })]);
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
    async fn test_wait_approval_with_draft_script() {
        let gate = ScriptApprovalGate;
        let mut artifacts = HashMap::new();
        let script_id = Uuid::new_v4();
        artifacts.insert("script_draft".to_string(), vec![json!({
            "id": script_id.to_string(),
            "status": "draft",
            "version": 1
        })]);
        let ctx = GateContext {
            project_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            artifacts,
            project_metadata: json!({}),
        };
        let decision = gate.check(&ctx).await.unwrap();
        assert!(matches!(decision, GateDecision::WaitApproval { .. }));
    }
}
