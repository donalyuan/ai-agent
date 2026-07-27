//! 创意简报（CreativeBrief）：制片人产出，确定目标受众、调性和核心信息
use crate::state::artifacts::ArtifactStatus;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreativeBrief {
    pub id: Uuid,
    pub production_project_id: Uuid,
    pub version: i32,
    pub status: String,
    pub content: Value,
    pub created_by: String,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl CreativeBrief {
    /// 验证 content 包含必需字段
    pub fn validate(content: &Value) -> Result<(), String> {
        let ta = content.get("target_audience").and_then(|v| v.as_str()).unwrap_or("");
        if ta.is_empty() { return Err("creative_brief 必须包含 target_audience".into()); }
        let km = content.get("key_messages").and_then(|v| v.as_array());
        if km.map(|a| a.is_empty()).unwrap_or(true) {
            return Err("creative_brief 的 key_messages 不能为空".into());
        }
        Ok(())
    }
    pub fn is_approved(&self) -> bool { self.status == ArtifactStatus::Approved.as_str() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn test_validate_ok() {
        let c = json!({"target_audience":"科技爱好者","key_messages":["健康监测"],"tone":[],"constraints":{},"success_criteria":[]});
        assert!(CreativeBrief::validate(&c).is_ok());
    }
    #[test]
    fn test_validate_missing_ta() {
        let c = json!({"key_messages":["test"]});
        assert!(CreativeBrief::validate(&c).is_err());
    }
}
