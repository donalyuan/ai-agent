//! Production API 请求 DTO；所有 Full Crew 命令拒绝未声明字段。

use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

/// 旧 ProductionProject 入口仅保留 Fast Lane，Full Crew 必须使用 intents API。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateProductionRequest {
    pub title: String,
    pub description: Option<String>,
    pub project_type: String,
    pub initial_input: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateProductionIntentRequest {
    pub project_id: Uuid,
    pub topic_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub initial_input: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartProductionRunRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovePackageRequest {
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RejectPackageRequest {
    pub reason: String,
    pub affected_owners: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyProductionCommandRequest {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelProductionRunRequest {
    pub reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FastLaneRequest {
    pub prompt: String,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub duration_seconds: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::{EmptyProductionCommandRequest, StartProductionRunRequest};
    use serde_json::json;

    #[test]
    fn production_commands_reject_plan_role_model_actor_and_context_overrides() {
        assert!(serde_json::from_value::<StartProductionRunRequest>(json!({})).is_ok());
        assert!(serde_json::from_value::<EmptyProductionCommandRequest>(json!({})).is_ok());
        for payload in [
            json!({"roles": ["producer"]}),
            json!({"auto_approve": true}),
            json!({"skip_gates": ["brief_approval"]}),
            json!({"plan_version": "client-version"}),
            json!({"preferred_model_id": uuid::Uuid::new_v4()}),
            json!({"context": {"source": "client"}}),
            json!({"user_id": uuid::Uuid::new_v4()}),
            json!({"user_input": "绕过计划输入"}),
        ] {
            assert!(
                serde_json::from_value::<StartProductionRunRequest>(payload.clone()).is_err(),
                "dynamic production payload must be rejected: {payload}"
            );
        }
    }
}
