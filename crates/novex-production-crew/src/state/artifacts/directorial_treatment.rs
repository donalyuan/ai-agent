//! 导演阐述（DirectorialTreatment）：导演产出，定义视觉风格和整体基调
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectorialTreatment {
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

impl DirectorialTreatment {
    pub fn validate(content: &Value) -> Result<(), String> {
        let vs = content.get("visual_style").and_then(|v| v.as_str()).unwrap_or("");
        if vs.is_empty() { return Err("directorial_treatment 必须包含 visual_style".into()); }
        Ok(())
    }
}
