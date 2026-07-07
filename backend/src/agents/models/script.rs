use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptStatus {
    Draft,
    Approved,
    Archived,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptStatusParseError {
    value: String,
}

impl ScriptStatusParseError {
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for ScriptStatusParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown script status: {}", self.value)
    }
}

impl std::error::Error for ScriptStatusParseError {}

impl ScriptStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Approved => "approved",
            Self::Archived => "archived",
        }
    }
}

impl TryFrom<&str> for ScriptStatus {
    type Error = ScriptStatusParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "draft" => Ok(Self::Draft),
            "approved" => Ok(Self::Approved),
            "archived" => Ok(Self::Archived),
            _ => Err(ScriptStatusParseError {
                value: value.to_string(),
            }),
        }
    }
}

impl TryFrom<String> for ScriptStatus {
    type Error = ScriptStatusParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

impl From<ScriptStatus> for String {
    fn from(status: ScriptStatus) -> Self {
        status.as_str().to_string()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Script {
    pub id: Uuid,
    pub project_id: Uuid,
    pub topic_id: Option<Uuid>,
    pub title: String,
    pub hook: String,
    pub content: Value,
    pub status: ScriptStatus,
    pub parent_id: Option<Uuid>,
    pub scenes: Vec<Scene>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptSummary {
    pub script_id: Uuid,
    pub topic_id: Option<Uuid>,
    pub source_topic_title: Option<String>,
    pub title: String,
    pub status: ScriptStatus,
    pub scene_count: i64,
    pub parent_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl Script {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Uuid,
        project_id: Uuid,
        topic_id: Option<Uuid>,
        title: String,
        hook: String,
        content: Value,
        status: ScriptStatus,
        parent_id: Option<Uuid>,
        mut scenes: Vec<Scene>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        // Script owns scene ordering so downstream generation receives a stable storyboard.
        scenes.sort_by_key(|scene| scene.sequence);

        Self {
            id,
            project_id,
            topic_id,
            title,
            hook,
            content,
            status,
            parent_id,
            scenes,
            created_at,
            updated_at,
        }
    }

    pub fn total_duration_sec(&self) -> i32 {
        self.scenes.iter().map(|scene| scene.duration_sec).sum()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scene {
    pub id: Uuid,
    pub sequence: i32,
    pub narration: String,
    pub visual_description: String,
    pub emotion: String,
    pub duration_sec: i32,
}
