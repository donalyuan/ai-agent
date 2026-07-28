//! 剧本草稿（ScriptDraft）：编剧产出，按场景和节拍组织
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptDraft {
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

impl ScriptDraft {
    pub fn validate(content: &Value) -> Result<(), String> {
        let scenes = content.get("scenes").and_then(|v| v.as_array());
        if scenes.map(|s| s.is_empty()).unwrap_or(true) {
            return Err("script_draft 的 scenes 不能为空".into());
        }
        Ok(())
    }
    pub fn is_approved(&self) -> bool {
        self.status == "approved"
    }
}
