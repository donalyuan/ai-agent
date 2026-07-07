use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentTopicStatus {
    Idea,
    Approved,
    Scripted,
    Archived,
}

impl ContentTopicStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idea => "idea",
            Self::Approved => "approved",
            Self::Scripted => "scripted",
            Self::Archived => "archived",
        }
    }

    pub fn can_transition_to(&self, next: &Self) -> bool {
        matches!(
            (self, next),
            (Self::Idea, Self::Approved)
                | (Self::Approved, Self::Scripted)
                | (Self::Idea, Self::Archived)
                | (Self::Approved, Self::Archived)
                | (Self::Scripted, Self::Archived)
        ) || self == next
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentTopicStatusParseError {
    value: String,
}

impl fmt::Display for ContentTopicStatusParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown content topic status: {}", self.value)
    }
}

impl std::error::Error for ContentTopicStatusParseError {}

impl TryFrom<&str> for ContentTopicStatus {
    type Error = ContentTopicStatusParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "idea" => Ok(Self::Idea),
            "approved" => Ok(Self::Approved),
            "scripted" => Ok(Self::Scripted),
            "archived" => Ok(Self::Archived),
            _ => Err(ContentTopicStatusParseError {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentTopicSource {
    Manual,
    Agent,
}

impl ContentTopicSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Agent => "agent",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentTopicSourceParseError {
    value: String,
}

impl fmt::Display for ContentTopicSourceParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown content topic source: {}", self.value)
    }
}

impl std::error::Error for ContentTopicSourceParseError {}

impl TryFrom<&str> for ContentTopicSource {
    type Error = ContentTopicSourceParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "manual" => Ok(Self::Manual),
            "agent" => Ok(Self::Agent),
            _ => Err(ContentTopicSourceParseError {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TopicGenerationBatchStatus {
    Running,
    Succeeded,
    Failed,
}

impl TopicGenerationBatchStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopicGenerationBatchStatusParseError {
    value: String,
}

impl fmt::Display for TopicGenerationBatchStatusParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unknown topic generation batch status: {}",
            self.value
        )
    }
}

impl std::error::Error for TopicGenerationBatchStatusParseError {}

impl TryFrom<&str> for TopicGenerationBatchStatus {
    type Error = TopicGenerationBatchStatusParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            _ => Err(TopicGenerationBatchStatusParseError {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContentTopic {
    pub id: Uuid,
    pub project_id: Uuid,
    pub batch_id: Option<Uuid>,
    pub title: String,
    pub angle: String,
    pub target_audience: String,
    pub hook_points: Vec<String>,
    pub content_type: String,
    pub score: Option<f64>,
    pub score_reason: String,
    pub tags: Vec<String>,
    pub source: ContentTopicSource,
    pub status: ContentTopicStatus,
    pub metadata: Value,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ContentTopic {
    pub fn snapshot(&self) -> Value {
        serde_json::json!({
            "topic_id": self.id,
            "title": self.title,
            "angle": self.angle,
            "target_audience": self.target_audience,
            "hook_points": self.hook_points,
            "content_type": self.content_type,
            "score": self.score,
            "score_reason": self.score_reason,
            "tags": self.tags,
            "source": self.source,
            "status": self.status,
            "created_at": self.created_at,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TopicGenerationBatch {
    pub id: Uuid,
    pub project_id: Uuid,
    pub source_run_id: Option<Uuid>,
    pub supplement_of_batch_id: Option<Uuid>,
    pub prompt: String,
    pub requested_count: i32,
    pub status: TopicGenerationBatchStatus,
    pub error_message: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TopicGenerationBatchSummary {
    pub batch: TopicGenerationBatch,
    pub topic_count: i64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct ContentTopicFilter {
    #[serde(default)]
    pub status: Option<ContentTopicStatus>,
    #[serde(default)]
    pub source: Option<ContentTopicSource>,
    #[serde(default)]
    pub batch_id: Option<Uuid>,
}
