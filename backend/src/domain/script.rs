//! 脚本聚合、分镜顺序、生成输入和状态解析等领域规则。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptStatus {
    Draft,
    Approved,
    Archived,
}

/// 脚本表达风格属于脚本领域语义，不由 HTTP DTO 或具体 Agent 实现持有。
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptStyle {
    #[default]
    Knowledge,
    Story,
    Tutorial,
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

/// 交给脚本 Agent 的领域输入；模型选择由 Application Service 在调用前完成。
#[derive(Clone, Debug, PartialEq, Validate)]
pub struct ScriptGenerationInput {
    pub project_id: Uuid,
    #[validate(length(max = 200))]
    pub topic: String,
    pub topic_id: Option<Uuid>,
    pub style: Option<ScriptStyle>,
    #[validate(range(min = 3, max = 12))]
    pub scene_count: Option<u8>,
    pub parent_id: Option<Uuid>,
}

impl ScriptGenerationInput {
    pub fn style_or_default(&self) -> ScriptStyle {
        self.style.clone().unwrap_or_default()
    }

    pub fn scene_count_or_default(&self) -> u8 {
        self.scene_count.unwrap_or(6)
    }
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
        // 由脚本聚合统一排序，确保下游素材与视频生成始终读取稳定的分镜顺序。
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

/// 脚本列表的领域查询条件，由应用层校验后交给 Repository 执行。
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
