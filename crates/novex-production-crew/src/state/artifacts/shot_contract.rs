//! 镜头合约（ShotContract）：导演产出，定义每个镜头的构图、运动和时长
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShotContract {
    pub id: Uuid,
    pub production_project_id: Uuid,
    /// 镜头唯一标识，如 "shot_001"
    pub shot_id: String,
    /// 所属场景 ID
    pub scene_id: String,
    pub version: i32,
    pub status: String,
    pub content: Value,
    pub created_by: String,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ShotContract {
    pub fn validate(content: &Value) -> Result<(), String> {
        let st = content.get("shot_type").and_then(|v| v.as_str()).unwrap_or("");
        if st.is_empty() { return Err("shot_contract 必须包含 shot_type".into()); }
        Ok(())
    }
}
