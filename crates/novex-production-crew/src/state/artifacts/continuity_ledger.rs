//! 连续性台账（ContinuityLedger）：剪辑师维护，记录每个镜头的视觉事实
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// 连续性台账无版本/审批字段（每个 shot_id 只有一条记录）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuityLedger {
    pub id: Uuid,
    pub production_project_id: Uuid,
    pub shot_id: String,
    pub content: Value,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ContinuityLedger {
    pub fn validate(content: &Value) -> Result<(), String> {
        if content.get("visual_facts").is_none() {
            return Err("continuity_ledger 必须包含 visual_facts".into());
        }
        Ok(())
    }
}
