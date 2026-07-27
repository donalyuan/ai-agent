//! 声音计划（SoundPlan）：声音指导产出，定义音乐风格、音效和对话录音方案
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundPlan {
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

impl SoundPlan {
    pub fn validate(content: &Value) -> Result<(), String> {
        let ms = content.get("music_style").and_then(|v| v.as_str()).unwrap_or("");
        if ms.is_empty() { return Err("sound_plan 必须包含 music_style".into()); }
        Ok(())
    }
}
