//! 表演简报（PerformanceBrief）：表演指导产出，定义角色情绪弧线和肢体语言
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBrief {
    pub id: Uuid,
    pub production_project_id: Uuid,
    pub character_id: String,
    pub version: i32,
    pub status: String,
    pub content: Value,
    pub created_by: String,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PerformanceBrief {
    pub fn validate(content: &Value) -> Result<(), String> {
        let arc = content.get("emotional_arc").and_then(|v| v.as_array());
        if arc.map(|a| a.is_empty()).unwrap_or(true) {
            return Err("performance_brief 的 emotional_arc 不能为空".into());
        }
        Ok(())
    }
}
