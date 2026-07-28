//! Full Crew 非金额资源闸门。

use crate::durable::plan::ResourceLimits;
use crate::durable::resource::{
    ResourceRequest, ResourceSafetyGate as DomainResourceSafetyGate, ResourceUsageLedger,
};
use crate::gates::gate_trait::{Gate, GateContext, GateDecision};
use crate::{ProductionError, ProductionResult};
use async_trait::async_trait;

pub struct ResourceSafetyGate;

#[async_trait]
impl Gate for ResourceSafetyGate {
    fn name(&self) -> &str {
        "resource_safety_gate"
    }

    async fn check(&self, context: &GateContext) -> ProductionResult<GateDecision> {
        let limits: ResourceLimits = serde_json::from_value(
            context
                .project_metadata
                .get("resource_limits")
                .cloned()
                .ok_or_else(|| ProductionError::InvalidArtifactSchema {
                    details: "resource_limits snapshot is required".into(),
                })?,
        )?;
        let request: ResourceRequest = serde_json::from_value(
            context
                .project_metadata
                .get("resource_request")
                .cloned()
                .ok_or_else(|| ProductionError::InvalidArtifactSchema {
                    details: "resource_request is required".into(),
                })?,
        )?;
        let mut ledger = ResourceUsageLedger::new(limits);
        match DomainResourceSafetyGate::reserve(&mut ledger, request) {
            Ok(_) => Ok(GateDecision::Pass),
            Err(error) => Ok(GateDecision::Reject {
                reason: error.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gates::gate_trait::GateContext;
    use serde_json::{json, to_value};
    use std::collections::HashMap;
    use uuid::Uuid;

    #[tokio::test]
    async fn rejects_before_side_effect_when_token_limit_would_be_exceeded() {
        let mut limits = ResourceLimits::strict_default();
        limits.max_input_tokens = 10;
        let context = GateContext {
            project_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            artifacts: HashMap::new(),
            project_metadata: json!({
                "resource_limits": to_value(limits).unwrap(),
                "resource_request": to_value(ResourceRequest::role_call(11, 1)).unwrap()
            }),
        };
        assert!(matches!(
            ResourceSafetyGate.check(&context).await.unwrap(),
            GateDecision::Reject { .. }
        ));
    }
}
