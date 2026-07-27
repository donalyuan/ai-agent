//! 角色圣经（CharacterBible）：编剧产出，可含多个角色，每个角色一条记录
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterBible {
    pub id: Uuid,
    pub production_project_id: Uuid,
    /// 角色唯一标识，如 "protagonist"
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

impl CharacterBible {
    pub fn validate(content: &Value) -> Result<(), String> {
        let name = content.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.is_empty() { return Err("character_bible 必须包含非空 name".into()); }
        Ok(())
    }
}
