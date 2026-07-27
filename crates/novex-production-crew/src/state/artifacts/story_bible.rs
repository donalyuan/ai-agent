//! 故事圣经（StoryBible）：编剧产出，世界观、主题和叙事弧线
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryBible {
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

impl StoryBible {
    pub fn validate(content: &Value) -> Result<(), String> {
        let premise = content.get("premise").and_then(|v| v.as_str()).unwrap_or("");
        if premise.is_empty() { return Err("story_bible 必须包含非空 premise".into()); }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn test_validate_ok() {
        assert!(StoryBible::validate(&json!({"premise":"一个关于科技的故事","themes":[],"world_rules":[],"narrative_arc":{}})).is_ok());
    }
}
