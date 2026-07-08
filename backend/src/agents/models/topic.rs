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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TopicReviewSnapshotStatus {
    Succeeded,
    Failed,
}

impl TopicReviewSnapshotStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopicReviewSnapshotStatusParseError {
    value: String,
}

impl fmt::Display for TopicReviewSnapshotStatusParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unknown topic review snapshot status: {}",
            self.value
        )
    }
}

impl std::error::Error for TopicReviewSnapshotStatusParseError {}

impl TryFrom<&str> for TopicReviewSnapshotStatus {
    type Error = TopicReviewSnapshotStatusParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            _ => Err(TopicReviewSnapshotStatusParseError {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TopicReviewPriority {
    Priority,
    Backup,
    Reject,
}

impl TopicReviewPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Priority => "priority",
            Self::Backup => "backup",
            Self::Reject => "reject",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopicReviewPriorityParseError {
    value: String,
}

impl fmt::Display for TopicReviewPriorityParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown topic review priority: {}", self.value)
    }
}

impl std::error::Error for TopicReviewPriorityParseError {}

impl TryFrom<&str> for TopicReviewPriority {
    type Error = TopicReviewPriorityParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "priority" => Ok(Self::Priority),
            "backup" => Ok(Self::Backup),
            "reject" => Ok(Self::Reject),
            _ => Err(TopicReviewPriorityParseError {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TopicReviewRiskFlag {
    TooGeneric,
    Duplicate,
    HardToScript,
    OffPositioning,
    ComplianceRisk,
}

impl TopicReviewRiskFlag {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TooGeneric => "too_generic",
            Self::Duplicate => "duplicate",
            Self::HardToScript => "hard_to_script",
            Self::OffPositioning => "off_positioning",
            Self::ComplianceRisk => "compliance_risk",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopicReviewRiskFlagParseError {
    value: String,
}

impl fmt::Display for TopicReviewRiskFlagParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown topic review risk flag: {}", self.value)
    }
}

impl std::error::Error for TopicReviewRiskFlagParseError {}

impl TryFrom<&str> for TopicReviewRiskFlag {
    type Error = TopicReviewRiskFlagParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "too_generic" => Ok(Self::TooGeneric),
            "duplicate" => Ok(Self::Duplicate),
            "hard_to_script" => Ok(Self::HardToScript),
            "off_positioning" => Ok(Self::OffPositioning),
            "compliance_risk" => Ok(Self::ComplianceRisk),
            _ => Err(TopicReviewRiskFlagParseError {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct TopicReviewResult {
    #[serde(default)]
    pub topic_reviews: Vec<TopicReviewItem>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TopicReviewItem {
    pub topic_id: Uuid,
    pub priority: TopicReviewPriority,
    pub reason: String,
    #[serde(default)]
    pub risk_flags: Vec<TopicReviewRiskFlag>,
    #[serde(default)]
    pub similar_topic_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TopicReviewSnapshot {
    pub id: Uuid,
    pub project_id: Uuid,
    pub root_batch_id: Uuid,
    pub source_run_id: Option<Uuid>,
    pub status: TopicReviewSnapshotStatus,
    pub review_summary: String,
    pub result: TopicReviewResult,
    pub error_message: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
