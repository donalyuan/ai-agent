use crate::domain::script::ScriptStyle;
use crate::domain::topic::{
    ContentTopic, ContentTopicSource, ContentTopicStatus, TopicGenerationBatchStatus,
    TopicGenerationBatchSummary, TopicGroupReviewFreshness, TopicGroupScriptPriority,
    TopicGroupSort, TopicGroupSummary, TopicQualityEvaluation, TopicQualityEvaluationStatus,
    TopicQualityGateResult, TopicReviewResult, TopicReviewSnapshot, TopicReviewSnapshotStatus,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct TopicReviewRequest {
    pub model_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DeletedContentTopicResponse {
    pub topic_id: Uuid,
    pub deleted_at: DateTime<Utc>,
}

#[derive(Debug, Default, Deserialize)]
pub struct TopicGroupProjectQuery {
    #[serde(default)]
    pub project_id: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CreateContentTopicRequest {
    pub title: String,
    #[serde(default)]
    pub angle: String,
    #[serde(default)]
    pub target_audience: String,
    #[serde(default)]
    pub hook_points: Vec<String>,
    #[serde(default)]
    pub content_type: String,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub score_reason: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl CreateContentTopicRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        validate_topic_payload(TopicPayloadValidation {
            title: &self.title,
            angle: &self.angle,
            target_audience: &self.target_audience,
            hook_points: &self.hook_points,
            content_type: &self.content_type,
            score: self.score,
            score_reason: &self.score_reason,
            tags: &self.tags,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UpdateContentTopicRequest {
    pub title: String,
    #[serde(default)]
    pub angle: String,
    #[serde(default)]
    pub target_audience: String,
    #[serde(default)]
    pub hook_points: Vec<String>,
    #[serde(default)]
    pub content_type: String,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub score_reason: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl UpdateContentTopicRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        validate_topic_payload(TopicPayloadValidation {
            title: &self.title,
            angle: &self.angle,
            target_audience: &self.target_audience,
            hook_points: &self.hook_points,
            content_type: &self.content_type,
            score: self.score,
            score_reason: &self.score_reason,
            tags: &self.tags,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UpdateContentTopicStatusRequest {
    pub status: ContentTopicStatus,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Validate)]
pub struct PrepareScriptFromTopicRequest {
    #[serde(default)]
    pub style: Option<ScriptStyle>,
    #[serde(default)]
    #[validate(range(min = 3, max = 12))]
    pub scene_count: Option<u8>,
}

impl PrepareScriptFromTopicRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        self.validate()
            .map_err(|error| format!("脚本确认参数无效: {error}"))
    }

    pub fn style_or_default(&self) -> ScriptStyle {
        self.style.clone().unwrap_or_default()
    }

    pub fn scene_count_or_default(&self) -> u8 {
        self.scene_count.unwrap_or(6)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ContentTopicResponse {
    pub topic_id: Uuid,
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

impl From<ContentTopic> for ContentTopicResponse {
    fn from(topic: ContentTopic) -> Self {
        Self {
            topic_id: topic.id,
            project_id: topic.project_id,
            batch_id: topic.batch_id,
            title: topic.title,
            angle: topic.angle,
            target_audience: topic.target_audience,
            hook_points: topic.hook_points,
            content_type: topic.content_type,
            score: topic.score,
            score_reason: topic.score_reason,
            tags: topic.tags,
            source: topic.source,
            status: topic.status,
            metadata: topic.metadata,
            deleted_at: topic.deleted_at,
            created_at: topic.created_at,
            updated_at: topic.updated_at,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ContentTopicStatsResponse {
    pub total: i64,
    pub idea: i64,
    pub approved: i64,
    pub scripted: i64,
    pub archived: i64,
}

impl ContentTopicStatsResponse {
    pub fn from_counts(counts: Vec<(ContentTopicStatus, i64)>) -> Self {
        let mut stats = Self::default();
        for (status, count) in counts {
            stats.total += count;
            match status {
                ContentTopicStatus::Idea => stats.idea = count,
                ContentTopicStatus::Approved => stats.approved = count,
                ContentTopicStatus::Scripted => stats.scripted = count,
                ContentTopicStatus::Archived => stats.archived = count,
            }
        }
        stats
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ContentTopicListResponse {
    pub topics: Vec<ContentTopicResponse>,
    pub stats: ContentTopicStatsResponse,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopicGenerationBatchSummaryResponse {
    pub batch_id: Uuid,
    pub project_id: Uuid,
    pub supplement_of_batch_id: Option<Uuid>,
    pub prompt: String,
    pub requested_count: i32,
    pub topic_count: i64,
    pub status: TopicGenerationBatchStatus,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<TopicGenerationBatchSummary> for TopicGenerationBatchSummaryResponse {
    fn from(summary: TopicGenerationBatchSummary) -> Self {
        Self {
            batch_id: summary.batch.id,
            project_id: summary.batch.project_id,
            supplement_of_batch_id: summary.batch.supplement_of_batch_id,
            prompt: summary.batch.prompt,
            requested_count: summary.batch.requested_count,
            topic_count: summary.topic_count,
            status: summary.batch.status,
            error_message: summary.batch.error_message,
            created_at: summary.batch.created_at,
            updated_at: summary.batch.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopicGenerationBatchListResponse {
    pub batches: Vec<TopicGenerationBatchSummaryResponse>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct TopicGroupListQuery {
    #[serde(default)]
    pub sort: TopicGroupSort,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopicGroupSummaryResponse {
    pub root_batch_id: Uuid,
    pub project_id: Uuid,
    pub prompt: String,
    pub created_at: DateTime<Utc>,
    pub topic_count: i64,
    pub supplement_batch_count: i64,
    pub latest_review_snapshot_id: Option<Uuid>,
    pub review_freshness: TopicGroupReviewFreshness,
    pub script_priority: TopicGroupScriptPriority,
}

impl From<TopicGroupSummary> for TopicGroupSummaryResponse {
    fn from(summary: TopicGroupSummary) -> Self {
        Self {
            root_batch_id: summary.root_batch_id,
            project_id: summary.project_id,
            prompt: summary.prompt,
            created_at: summary.created_at,
            topic_count: summary.topic_count,
            supplement_batch_count: summary.supplement_batch_count,
            latest_review_snapshot_id: summary.latest_review_snapshot_id,
            review_freshness: summary.review_freshness,
            script_priority: summary.script_priority,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopicGroupListResponse {
    pub topic_groups: Vec<TopicGroupSummaryResponse>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopicReviewSnapshotResponse {
    pub snapshot_id: Uuid,
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

impl From<TopicReviewSnapshot> for TopicReviewSnapshotResponse {
    fn from(snapshot: TopicReviewSnapshot) -> Self {
        Self {
            snapshot_id: snapshot.id,
            project_id: snapshot.project_id,
            root_batch_id: snapshot.root_batch_id,
            source_run_id: snapshot.source_run_id,
            status: snapshot.status,
            review_summary: snapshot.review_summary,
            result: snapshot.result,
            error_message: snapshot.error_message,
            metadata: snapshot.metadata,
            created_at: snapshot.created_at,
            updated_at: snapshot.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopicQualityEvaluationResponse {
    pub evaluation_id: Uuid,
    pub project_id: Uuid,
    pub batch_id: Uuid,
    pub source_run_id: Option<Uuid>,
    pub status: TopicQualityEvaluationStatus,
    pub pass_count: i32,
    pub reject_count: i32,
    pub rewrite_triggered: bool,
    pub result: TopicQualityGateResult,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<TopicQualityEvaluation> for TopicQualityEvaluationResponse {
    fn from(evaluation: TopicQualityEvaluation) -> Self {
        Self {
            evaluation_id: evaluation.id,
            project_id: evaluation.project_id,
            batch_id: evaluation.batch_id,
            source_run_id: evaluation.source_run_id,
            status: evaluation.status,
            pass_count: evaluation.pass_count,
            reject_count: evaluation.reject_count,
            rewrite_triggered: evaluation.rewrite_triggered,
            result: evaluation.result,
            error_message: evaluation.error_message,
            created_at: evaluation.created_at,
            updated_at: evaluation.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopicScriptRequestPreview {
    pub project_id: Uuid,
    pub topic_id: Uuid,
    pub topic: String,
    pub style: ScriptStyle,
    pub scene_count: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PrepareScriptFromTopicResponse {
    pub topic: ContentTopicResponse,
    pub topic_snapshot: Value,
    pub script_request: TopicScriptRequestPreview,
}

struct TopicPayloadValidation<'a> {
    title: &'a str,
    angle: &'a str,
    target_audience: &'a str,
    hook_points: &'a [String],
    content_type: &'a str,
    score: Option<f64>,
    score_reason: &'a str,
    tags: &'a [String],
}

fn validate_topic_payload(payload: TopicPayloadValidation<'_>) -> Result<(), String> {
    if payload.title.trim().is_empty() {
        return Err("选题标题不能为空".to_string());
    }
    if payload.title.chars().count() > 160 {
        return Err("选题标题不能超过160个字符".to_string());
    }
    if payload.angle.chars().count() > 1000 {
        return Err("选题角度不能超过1000个字符".to_string());
    }
    if payload.target_audience.chars().count() > 500 {
        return Err("目标受众不能超过500个字符".to_string());
    }
    if payload.content_type.chars().count() > 80 {
        return Err("内容类型不能超过80个字符".to_string());
    }
    if let Some(score) = payload.score {
        if !(0.0..=100.0).contains(&score) {
            return Err("选题评分必须在0到100之间".to_string());
        }
    }
    if payload.score_reason.chars().count() > 1000 {
        return Err("评分理由不能超过1000个字符".to_string());
    }
    if payload.hook_points.len() > 10 {
        return Err("选题看点不能超过10个".to_string());
    }
    if payload.tags.len() > 20 {
        return Err("选题标签不能超过20个".to_string());
    }
    if payload
        .hook_points
        .iter()
        .any(|value| value.trim().is_empty())
    {
        return Err("选题看点不能为空".to_string());
    }
    if payload.tags.iter().any(|value| value.trim().is_empty()) {
        return Err("选题标签不能为空".to_string());
    }
    Ok(())
}
