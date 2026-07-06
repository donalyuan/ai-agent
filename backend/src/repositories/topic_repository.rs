use crate::agents::models::{
    ContentTopic, ContentTopicFilter, ContentTopicSource, ContentTopicStatus, TopicGenerationBatch,
    TopicGenerationBatchStatus, TopicGenerationBatchSummary,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{postgres::PgRow, PgPool, Row};
use std::fmt;
use uuid::Uuid;

#[derive(Clone)]
pub struct PostgresTopicRepository {
    pool: PgPool,
}

impl PostgresTopicRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateContentTopicInput {
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
    pub metadata: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateContentTopicInput {
    pub title: String,
    pub angle: String,
    pub target_audience: String,
    pub hook_points: Vec<String>,
    pub content_type: String,
    pub score: Option<f64>,
    pub score_reason: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateTopicGenerationBatchInput {
    pub project_id: Uuid,
    pub source_run_id: Option<Uuid>,
    pub prompt: String,
    pub requested_count: i32,
    pub status: TopicGenerationBatchStatus,
    pub error_message: Option<String>,
    pub metadata: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateTopicGenerationBatchInput {
    pub status: TopicGenerationBatchStatus,
    pub error_message: Option<String>,
    pub metadata: Value,
}

#[async_trait]
pub trait TopicRepository: Send + Sync {
    async fn create_topic(
        &self,
        input: CreateContentTopicInput,
    ) -> Result<ContentTopic, TopicRepositoryError>;

    async fn update_topic(
        &self,
        topic_id: Uuid,
        input: UpdateContentTopicInput,
    ) -> Result<ContentTopic, TopicRepositoryError>;

    async fn update_topic_status(
        &self,
        topic_id: Uuid,
        status: ContentTopicStatus,
    ) -> Result<ContentTopic, TopicRepositoryError>;

    async fn get_topic(&self, topic_id: Uuid) -> Result<ContentTopic, TopicRepositoryError>;

    async fn list_topics(
        &self,
        project_id: Uuid,
        filter: ContentTopicFilter,
    ) -> Result<Vec<ContentTopic>, TopicRepositoryError>;

    async fn count_topics_by_status(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<(ContentTopicStatus, i64)>, TopicRepositoryError>;

    async fn create_generation_batch(
        &self,
        input: CreateTopicGenerationBatchInput,
    ) -> Result<TopicGenerationBatch, TopicRepositoryError>;

    async fn update_generation_batch(
        &self,
        batch_id: Uuid,
        input: UpdateTopicGenerationBatchInput,
    ) -> Result<TopicGenerationBatch, TopicRepositoryError>;

    async fn get_generation_batch(
        &self,
        batch_id: Uuid,
    ) -> Result<TopicGenerationBatch, TopicRepositoryError>;

    async fn list_generation_batches(
        &self,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<TopicGenerationBatchSummary>, TopicRepositoryError>;
}

#[async_trait]
impl TopicRepository for PostgresTopicRepository {
    async fn create_topic(
        &self,
        input: CreateContentTopicInput,
    ) -> Result<ContentTopic, TopicRepositoryError> {
        let row = sqlx::query(
            r#"
            INSERT INTO content_topics (
                project_id, batch_id, title, angle, target_audience, hook_points,
                content_type, score, score_reason, tags, source, status, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 'idea', $12)
            RETURNING id, project_id, batch_id, title, angle, target_audience, hook_points,
                      content_type, score, score_reason, tags, source, status, metadata,
                      created_at, updated_at
            "#,
        )
        .bind(input.project_id)
        .bind(input.batch_id)
        .bind(input.title)
        .bind(input.angle)
        .bind(input.target_audience)
        .bind(input.hook_points)
        .bind(input.content_type)
        .bind(input.score)
        .bind(input.score_reason)
        .bind(input.tags)
        .bind(input.source.as_str())
        .bind(input.metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(TopicRepositoryError::from)?;

        topic_from_row(row)
    }

    async fn update_topic(
        &self,
        topic_id: Uuid,
        input: UpdateContentTopicInput,
    ) -> Result<ContentTopic, TopicRepositoryError> {
        let row = sqlx::query(
            r#"
            UPDATE content_topics
            SET title = $2,
                angle = $3,
                target_audience = $4,
                hook_points = $5,
                content_type = $6,
                score = $7,
                score_reason = $8,
                tags = $9,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, project_id, batch_id, title, angle, target_audience, hook_points,
                      content_type, score, score_reason, tags, source, status, metadata,
                      created_at, updated_at
            "#,
        )
        .bind(topic_id)
        .bind(input.title)
        .bind(input.angle)
        .bind(input.target_audience)
        .bind(input.hook_points)
        .bind(input.content_type)
        .bind(input.score)
        .bind(input.score_reason)
        .bind(input.tags)
        .fetch_optional(&self.pool)
        .await
        .map_err(TopicRepositoryError::from)?
        .ok_or(TopicRepositoryError::TopicNotFound(topic_id))?;

        topic_from_row(row)
    }

    async fn update_topic_status(
        &self,
        topic_id: Uuid,
        status: ContentTopicStatus,
    ) -> Result<ContentTopic, TopicRepositoryError> {
        let current = self.get_topic(topic_id).await?;
        if !current.status.can_transition_to(&status) {
            return Err(TopicRepositoryError::InvalidStatusTransition {
                topic_id,
                from: current.status,
                to: status,
            });
        }

        let row = sqlx::query(
            r#"
            UPDATE content_topics
            SET status = $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, project_id, batch_id, title, angle, target_audience, hook_points,
                      content_type, score, score_reason, tags, source, status, metadata,
                      created_at, updated_at
            "#,
        )
        .bind(topic_id)
        .bind(status.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(TopicRepositoryError::from)?
        .ok_or(TopicRepositoryError::TopicNotFound(topic_id))?;

        topic_from_row(row)
    }

    async fn get_topic(&self, topic_id: Uuid) -> Result<ContentTopic, TopicRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT id, project_id, batch_id, title, angle, target_audience, hook_points,
                   content_type, score, score_reason, tags, source, status, metadata,
                   created_at, updated_at
            FROM content_topics
            WHERE id = $1
            "#,
        )
        .bind(topic_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(TopicRepositoryError::from)?
        .ok_or(TopicRepositoryError::TopicNotFound(topic_id))?;

        topic_from_row(row)
    }

    async fn list_topics(
        &self,
        project_id: Uuid,
        filter: ContentTopicFilter,
    ) -> Result<Vec<ContentTopic>, TopicRepositoryError> {
        let status = filter.status.as_ref().map(ContentTopicStatus::as_str);
        let source = filter.source.as_ref().map(ContentTopicSource::as_str);
        let rows = sqlx::query(
            r#"
            SELECT id, project_id, batch_id, title, angle, target_audience, hook_points,
                   content_type, score, score_reason, tags, source, status, metadata,
                   created_at, updated_at
            FROM content_topics
            WHERE project_id = $1
              AND ($2::text IS NULL OR status = $2)
              AND ($3::text IS NULL OR source = $3)
              AND ($4::uuid IS NULL OR batch_id = $4)
            ORDER BY score DESC NULLS LAST, created_at DESC, id DESC
            "#,
        )
        .bind(project_id)
        .bind(status)
        .bind(source)
        .bind(filter.batch_id)
        .fetch_all(&self.pool)
        .await
        .map_err(TopicRepositoryError::from)?;

        rows.into_iter().map(topic_from_row).collect()
    }

    async fn count_topics_by_status(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<(ContentTopicStatus, i64)>, TopicRepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT status, COUNT(*) AS topic_count
            FROM content_topics
            WHERE project_id = $1
            GROUP BY status
            "#,
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(TopicRepositoryError::from)?;

        rows.into_iter()
            .map(|row| {
                let status_value: String = row.get("status");
                let status = ContentTopicStatus::try_from(status_value.as_str())
                    .map_err(|error| TopicRepositoryError::Storage(error.to_string()))?;
                let count: i64 = row.get("topic_count");
                Ok((status, count))
            })
            .collect()
    }

    async fn create_generation_batch(
        &self,
        input: CreateTopicGenerationBatchInput,
    ) -> Result<TopicGenerationBatch, TopicRepositoryError> {
        let row = sqlx::query(
            r#"
            INSERT INTO topic_generation_batches (
                project_id, source_run_id, prompt, requested_count, status, error_message, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, project_id, source_run_id, prompt, requested_count, status,
                      error_message, metadata, created_at, updated_at
            "#,
        )
        .bind(input.project_id)
        .bind(input.source_run_id)
        .bind(input.prompt)
        .bind(input.requested_count)
        .bind(input.status.as_str())
        .bind(input.error_message)
        .bind(input.metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(TopicRepositoryError::from)?;

        batch_from_row(row)
    }

    async fn update_generation_batch(
        &self,
        batch_id: Uuid,
        input: UpdateTopicGenerationBatchInput,
    ) -> Result<TopicGenerationBatch, TopicRepositoryError> {
        let row = sqlx::query(
            r#"
            UPDATE topic_generation_batches
            SET status = $2,
                error_message = $3,
                metadata = $4,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, project_id, source_run_id, prompt, requested_count, status,
                      error_message, metadata, created_at, updated_at
            "#,
        )
        .bind(batch_id)
        .bind(input.status.as_str())
        .bind(input.error_message)
        .bind(input.metadata)
        .fetch_optional(&self.pool)
        .await
        .map_err(TopicRepositoryError::from)?
        .ok_or(TopicRepositoryError::BatchNotFound(batch_id))?;

        batch_from_row(row)
    }

    async fn get_generation_batch(
        &self,
        batch_id: Uuid,
    ) -> Result<TopicGenerationBatch, TopicRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT id, project_id, source_run_id, prompt, requested_count, status,
                   error_message, metadata, created_at, updated_at
            FROM topic_generation_batches
            WHERE id = $1
            "#,
        )
        .bind(batch_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(TopicRepositoryError::from)?
        .ok_or(TopicRepositoryError::BatchNotFound(batch_id))?;

        batch_from_row(row)
    }

    async fn list_generation_batches(
        &self,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<TopicGenerationBatchSummary>, TopicRepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT
                b.id,
                b.project_id,
                b.source_run_id,
                b.prompt,
                b.requested_count,
                b.status,
                b.error_message,
                b.metadata,
                b.created_at,
                b.updated_at,
                COUNT(t.id) AS topic_count
            FROM topic_generation_batches b
            LEFT JOIN content_topics t ON t.batch_id = b.id
            WHERE b.project_id = $1
              AND b.status = 'succeeded'
            GROUP BY b.id
            HAVING COUNT(t.id) > 0
            ORDER BY b.created_at DESC, b.id DESC
            LIMIT $2
            "#,
        )
        .bind(project_id)
        .bind(limit.clamp(1, 100))
        .fetch_all(&self.pool)
        .await
        .map_err(TopicRepositoryError::from)?;

        rows.into_iter().map(batch_summary_from_row).collect()
    }
}

#[derive(Debug)]
pub enum TopicRepositoryError {
    TopicNotFound(Uuid),
    BatchNotFound(Uuid),
    InvalidStatusTransition {
        topic_id: Uuid,
        from: ContentTopicStatus,
        to: ContentTopicStatus,
    },
    Storage(String),
}

impl From<sqlx::Error> for TopicRepositoryError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

impl fmt::Display for TopicRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TopicNotFound(topic_id) => {
                write!(formatter, "content topic not found: {topic_id}")
            }
            Self::BatchNotFound(batch_id) => {
                write!(formatter, "topic generation batch not found: {batch_id}")
            }
            Self::InvalidStatusTransition { topic_id, from, to } => write!(
                formatter,
                "invalid content topic status transition: topic_id={topic_id}, from={}, to={}",
                from.as_str(),
                to.as_str()
            ),
            Self::Storage(message) => write!(formatter, "topic storage error: {message}"),
        }
    }
}

impl std::error::Error for TopicRepositoryError {}

fn topic_from_row(row: PgRow) -> Result<ContentTopic, TopicRepositoryError> {
    let source_value: String = row.get("source");
    let source = ContentTopicSource::try_from(source_value.as_str())
        .map_err(|error| TopicRepositoryError::Storage(error.to_string()))?;
    let status_value: String = row.get("status");
    let status = ContentTopicStatus::try_from(status_value.as_str())
        .map_err(|error| TopicRepositoryError::Storage(error.to_string()))?;

    Ok(ContentTopic {
        id: row.get("id"),
        project_id: row.get("project_id"),
        batch_id: row.get("batch_id"),
        title: row.get("title"),
        angle: row.get("angle"),
        target_audience: row.get("target_audience"),
        hook_points: row.get("hook_points"),
        content_type: row.get("content_type"),
        score: row.get("score"),
        score_reason: row.get("score_reason"),
        tags: row.get("tags"),
        source,
        status,
        metadata: row.get("metadata"),
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
        updated_at: row.get::<DateTime<Utc>, _>("updated_at"),
    })
}

fn batch_from_row(row: PgRow) -> Result<TopicGenerationBatch, TopicRepositoryError> {
    let status_value: String = row.get("status");
    let status = TopicGenerationBatchStatus::try_from(status_value.as_str())
        .map_err(|error| TopicRepositoryError::Storage(error.to_string()))?;

    Ok(TopicGenerationBatch {
        id: row.get("id"),
        project_id: row.get("project_id"),
        source_run_id: row.get("source_run_id"),
        prompt: row.get("prompt"),
        requested_count: row.get("requested_count"),
        status,
        error_message: row.get("error_message"),
        metadata: row.get("metadata"),
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
        updated_at: row.get::<DateTime<Utc>, _>("updated_at"),
    })
}

fn batch_summary_from_row(row: PgRow) -> Result<TopicGenerationBatchSummary, TopicRepositoryError> {
    let topic_count: i64 = row.get("topic_count");
    Ok(TopicGenerationBatchSummary {
        batch: batch_from_row(row)?,
        topic_count,
    })
}
