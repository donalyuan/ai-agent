//! 镜头评审（TakeReview）：QC 产出，评审每个生成镜头的质量和合规性
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// TakeReview 无版本/审批字段（approved/rejected/needs_revision 由 status 直接表达）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeReview {
    pub id: Uuid,
    pub production_project_id: Uuid,
    pub shot_id: String,
    pub take_number: i32,
    pub status: String,
    pub content: Value,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TakeReview {
    pub fn is_approved(&self) -> bool { self.status == "approved" }
    pub fn is_rejected(&self) -> bool { self.status == "rejected" }

    pub fn validate(content: &Value) -> Result<(), String> {
        if content.get("quality_assessment").is_none() {
            return Err("take_review 必须包含 quality_assessment".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn test_validate_ok() {
        let c = json!({"quality_assessment":{"visual":8,"narrative":7,"technical":8,"notes":""},"contract_compliance":{"met":true,"issues":[]},"continuity_compliance":{"met":true,"violations":[]},"revision_notes":[]});
        assert!(TakeReview::validate(&c).is_ok());
    }
}
