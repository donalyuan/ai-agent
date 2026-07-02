use super::{Scene, Script, ScriptStatus, ScriptSummary};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptStyle {
    Knowledge,
    Story,
    Tutorial,
}

impl Default for ScriptStyle {
    fn default() -> Self {
        Self::Knowledge
    }
}

impl ScriptStyle {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Knowledge => "knowledge",
            Self::Story => "story",
            Self::Tutorial => "tutorial",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Knowledge => "知识科普类",
            Self::Story => "故事叙述类",
            Self::Tutorial => "教程讲解类",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Validate)]
pub struct GenerateScriptRequest {
    pub project_id: Uuid,
    #[validate(length(min = 10, max = 200))]
    pub topic: String,
    #[serde(default)]
    pub style: Option<ScriptStyle>,
    #[serde(default)]
    #[validate(range(min = 5, max = 8))]
    pub scene_count: Option<u8>,
    #[serde(default)]
    pub parent_id: Option<Uuid>,
}

impl GenerateScriptRequest {
    pub fn style_or_default(&self) -> ScriptStyle {
        self.style.clone().unwrap_or_default()
    }

    pub fn scene_count_or_default(&self) -> u8 {
        self.scene_count.unwrap_or(6)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Validate)]
pub struct ScriptListFilter {
    #[serde(default)]
    pub status: Option<ScriptStatus>,
    #[serde(default)]
    #[validate(range(min = 1, max = 100))]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

impl ScriptListFilter {
    pub fn limit_or_default(&self) -> u32 {
        self.limit.unwrap_or(20)
    }

    pub fn offset_or_default(&self) -> u32 {
        self.offset.unwrap_or(0)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScriptResponse {
    pub script_id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub hook: String,
    pub scenes: Vec<SceneResponse>,
    pub status: ScriptStatus,
    pub parent_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Script> for ScriptResponse {
    fn from(script: Script) -> Self {
        Self {
            script_id: script.id,
            project_id: script.project_id,
            title: script.title,
            hook: script.hook,
            scenes: script.scenes.into_iter().map(SceneResponse::from).collect(),
            status: script.status,
            parent_id: script.parent_id,
            created_at: script.created_at,
            updated_at: script.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScriptSummaryResponse {
    pub script_id: Uuid,
    pub title: String,
    pub status: ScriptStatus,
    pub scene_count: usize,
    pub parent_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl From<Script> for ScriptSummaryResponse {
    fn from(script: Script) -> Self {
        Self {
            script_id: script.id,
            title: script.title,
            status: script.status,
            scene_count: script.scenes.len(),
            parent_id: script.parent_id,
            created_at: script.created_at,
        }
    }
}

impl From<ScriptSummary> for ScriptSummaryResponse {
    fn from(summary: ScriptSummary) -> Self {
        Self {
            script_id: summary.script_id,
            title: summary.title,
            status: summary.status,
            scene_count: usize::try_from(summary.scene_count).unwrap_or(usize::MAX),
            parent_id: summary.parent_id,
            created_at: summary.created_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScriptListResponse {
    pub scripts: Vec<ScriptSummaryResponse>,
    pub total: i64,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UpdateScriptStatusRequest {
    pub status: ScriptStatus,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UpdateScriptStatusResponse {
    pub script_id: Uuid,
    pub status: ScriptStatus,
    pub updated_at: DateTime<Utc>,
}

impl From<Script> for UpdateScriptStatusResponse {
    fn from(script: Script) -> Self {
        Self {
            script_id: script.id,
            status: script.status,
            updated_at: script.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SceneResponse {
    pub scene_id: Uuid,
    pub sequence: i32,
    pub narration: String,
    pub visual_description: String,
    pub emotion: String,
    pub duration_sec: i32,
}

impl From<Scene> for SceneResponse {
    fn from(scene: Scene) -> Self {
        Self {
            scene_id: scene.id,
            sequence: scene.sequence,
            narration: scene.narration,
            visual_description: scene.visual_description,
            emotion: scene.emotion,
            duration_sec: scene.duration_sec,
        }
    }
}
