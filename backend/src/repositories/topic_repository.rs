use crate::agents::models::{
    ContentTopic, ContentTopicFilter, ContentTopicSource, ContentTopicStatus, TopicGenerationBatch,
    TopicGenerationBatchStatus, TopicGenerationBatchSummary, TopicGroupReviewFreshness,
    TopicGroupScriptPriority, TopicGroupScriptPriorityMetrics, TopicGroupScriptPriorityStatus,
    TopicGroupSort, TopicGroupSummary, TopicReviewPriority, TopicReviewResult,
    TopicReviewRiskFlag, TopicReviewSnapshot, TopicReviewSnapshotStatus,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{postgres::PgRow, PgPool, Row};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
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
    pub supplement_of_batch_id: Option<Uuid>,
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

#[derive(Clone, Debug, PartialEq)]
pub struct CreateTopicReviewSnapshotInput {
    pub project_id: Uuid,
    pub root_batch_id: Uuid,
    pub source_run_id: Option<Uuid>,
    pub status: TopicReviewSnapshotStatus,
    pub review_summary: String,
    pub result: TopicReviewResult,
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

    async fn list_topics_for_batch_group(
        &self,
        project_id: Uuid,
        root_batch_id: Uuid,
    ) -> Result<Vec<ContentTopic>, TopicRepositoryError>;

    async fn count_topics_by_status(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<(ContentTopicStatus, i64)>, TopicRepositoryError>;

    async fn soft_delete_topic(&self, topic_id: Uuid)
        -> Result<ContentTopic, TopicRepositoryError>;

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

    async fn resolve_supplement_root_batch(
        &self,
        project_id: Uuid,
        target_batch_id: Uuid,
    ) -> Result<TopicGenerationBatch, TopicRepositoryError>;

    async fn list_generation_batches(
        &self,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<TopicGenerationBatchSummary>, TopicRepositoryError>;

    async fn list_topic_group_summaries(
        &self,
        project_id: Uuid,
        sort: TopicGroupSort,
        limit: i64,
    ) -> Result<Vec<TopicGroupSummary>, TopicRepositoryError>;

    async fn create_topic_review_snapshot(
        &self,
        input: CreateTopicReviewSnapshotInput,
    ) -> Result<TopicReviewSnapshot, TopicRepositoryError>;

    async fn get_latest_topic_review_snapshot(
        &self,
        project_id: Uuid,
        root_batch_id: Uuid,
    ) -> Result<Option<TopicReviewSnapshot>, TopicRepositoryError>;
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
                      deleted_at,
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
                      deleted_at,
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
                      deleted_at,
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
                   deleted_at,
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
                   deleted_at,
                   created_at, updated_at
            FROM content_topics
            WHERE project_id = $1
              AND deleted_at IS NULL
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

    async fn list_topics_for_batch_group(
        &self,
        project_id: Uuid,
        root_batch_id: Uuid,
    ) -> Result<Vec<ContentTopic>, TopicRepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT t.id, t.project_id, t.batch_id, t.title, t.angle, t.target_audience,
                   t.hook_points, t.content_type, t.score, t.score_reason, t.tags,
                   t.source, t.status, t.metadata, t.deleted_at, t.created_at, t.updated_at
            FROM content_topics t
            INNER JOIN topic_generation_batches b ON b.id = t.batch_id
            WHERE t.project_id = $1
              AND b.project_id = $1
              AND t.deleted_at IS NULL
              AND (b.id = $2 OR b.supplement_of_batch_id = $2)
            ORDER BY t.created_at ASC, t.id ASC
            "#,
        )
        .bind(project_id)
        .bind(root_batch_id)
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
              AND deleted_at IS NULL
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

    async fn soft_delete_topic(
        &self,
        topic_id: Uuid,
    ) -> Result<ContentTopic, TopicRepositoryError> {
        let current = self.get_topic(topic_id).await?;
        if current.deleted_at.is_some() || current.status == ContentTopicStatus::Scripted {
            return Err(TopicRepositoryError::TopicCannotBeDeleted(topic_id));
        }

        let referenced_by_script = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM scripts
                WHERE topic_id = $1
            )
            "#,
        )
        .bind(topic_id)
        .fetch_one(&self.pool)
        .await
        .map_err(TopicRepositoryError::from)?;
        if referenced_by_script {
            return Err(TopicRepositoryError::TopicCannotBeDeleted(topic_id));
        }

        let row = sqlx::query(
            r#"
            UPDATE content_topics
            SET deleted_at = NOW(),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, project_id, batch_id, title, angle, target_audience, hook_points,
                      content_type, score, score_reason, tags, source, status, metadata,
                      deleted_at, created_at, updated_at
            "#,
        )
        .bind(topic_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(TopicRepositoryError::from)?
        .ok_or(TopicRepositoryError::TopicNotFound(topic_id))?;

        topic_from_row(row)
    }

    async fn create_generation_batch(
        &self,
        input: CreateTopicGenerationBatchInput,
    ) -> Result<TopicGenerationBatch, TopicRepositoryError> {
        let row = sqlx::query(
            r#"
            INSERT INTO topic_generation_batches (
                project_id, source_run_id, supplement_of_batch_id, prompt, requested_count,
                status, error_message, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, project_id, source_run_id, supplement_of_batch_id,
                      prompt, requested_count, status,
                      error_message, metadata, created_at, updated_at
            "#,
        )
        .bind(input.project_id)
        .bind(input.source_run_id)
        .bind(input.supplement_of_batch_id)
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
            RETURNING id, project_id, source_run_id, supplement_of_batch_id,
                      prompt, requested_count, status,
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
            SELECT id, project_id, source_run_id, supplement_of_batch_id,
                   prompt, requested_count, status,
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

    async fn resolve_supplement_root_batch(
        &self,
        project_id: Uuid,
        target_batch_id: Uuid,
    ) -> Result<TopicGenerationBatch, TopicRepositoryError> {
        let target = self
            .get_generation_batch_for_project(project_id, target_batch_id)
            .await?;
        if target.status != TopicGenerationBatchStatus::Succeeded {
            return Err(TopicRepositoryError::BatchCannotBeSupplemented(
                target_batch_id,
            ));
        }

        let visible_topic_count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM content_topics
            WHERE project_id = $1
              AND batch_id = $2
              AND deleted_at IS NULL
            "#,
        )
        .bind(project_id)
        .bind(target_batch_id)
        .fetch_one(&self.pool)
        .await
        .map_err(TopicRepositoryError::from)?;
        if visible_topic_count == 0 {
            return Err(TopicRepositoryError::BatchCannotBeSupplemented(
                target_batch_id,
            ));
        }

        let root_batch_id = target.supplement_of_batch_id.unwrap_or(target.id);
        if root_batch_id == target.id {
            return Ok(target);
        }
        self.get_generation_batch_for_project(project_id, root_batch_id)
            .await
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
                b.supplement_of_batch_id,
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
                AND t.deleted_at IS NULL
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

    async fn list_topic_group_summaries(
        &self,
        project_id: Uuid,
        sort: TopicGroupSort,
        limit: i64,
    ) -> Result<Vec<TopicGroupSummary>, TopicRepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT
                root.id AS root_batch_id,
                root.project_id,
                root.prompt,
                root.created_at,
                COUNT(DISTINCT topic.id) AS topic_count,
                COUNT(DISTINCT supplement.id) AS supplement_batch_count
            FROM topic_generation_batches root
            INNER JOIN topic_generation_batches batch
                ON batch.project_id = root.project_id
               AND batch.status = 'succeeded'
               AND (batch.id = root.id OR batch.supplement_of_batch_id = root.id)
            INNER JOIN content_topics topic
                ON topic.project_id = root.project_id
               AND topic.batch_id = batch.id
               AND topic.deleted_at IS NULL
            LEFT JOIN topic_generation_batches supplement
                ON supplement.project_id = root.project_id
               AND supplement.status = 'succeeded'
               AND supplement.supplement_of_batch_id = root.id
            WHERE root.project_id = $1
              AND root.status = 'succeeded'
              AND root.supplement_of_batch_id IS NULL
            GROUP BY root.id, root.project_id, root.prompt, root.created_at
            ORDER BY root.created_at DESC, root.id DESC
            "#,
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(TopicRepositoryError::from)?;

        let mut summaries = Vec::with_capacity(rows.len());
        for row in rows {
            let group = root_topic_group_from_row(row);
            let topics = self
                .list_topics_for_batch_group(project_id, group.root_batch_id)
                .await?;
            let latest_snapshot = self
                .get_latest_topic_review_snapshot(project_id, group.root_batch_id)
                .await?;
            let latest_review_snapshot_id = latest_snapshot.as_ref().map(|snapshot| snapshot.id);
            let review_freshness = topic_group_review_freshness(&topics, latest_snapshot.as_ref());
            let script_priority =
                topic_group_script_priority(&topics, latest_snapshot.as_ref(), &review_freshness);

            summaries.push(TopicGroupSummary {
                root_batch_id: group.root_batch_id,
                project_id: group.project_id,
                prompt: group.prompt,
                created_at: group.created_at,
                topic_count: group.topic_count,
                supplement_batch_count: group.supplement_batch_count,
                latest_review_snapshot_id,
                review_freshness,
                script_priority,
            });
        }

        sort_topic_group_summaries(&mut summaries, sort);
        summaries.truncate(limit.clamp(1, 100) as usize);
        Ok(summaries)
    }

    async fn create_topic_review_snapshot(
        &self,
        input: CreateTopicReviewSnapshotInput,
    ) -> Result<TopicReviewSnapshot, TopicRepositoryError> {
        let result = serde_json::to_value(input.result)
            .map_err(|error| TopicRepositoryError::Storage(error.to_string()))?;
        let row = sqlx::query(
            r#"
            INSERT INTO topic_review_snapshots (
                project_id, root_batch_id, source_run_id, status, review_summary,
                result, error_message, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, project_id, root_batch_id, source_run_id, status,
                      review_summary, result, error_message, metadata,
                      created_at, updated_at
            "#,
        )
        .bind(input.project_id)
        .bind(input.root_batch_id)
        .bind(input.source_run_id)
        .bind(input.status.as_str())
        .bind(input.review_summary)
        .bind(result)
        .bind(input.error_message)
        .bind(input.metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(TopicRepositoryError::from)?;

        topic_review_snapshot_from_row(row)
    }

    async fn get_latest_topic_review_snapshot(
        &self,
        project_id: Uuid,
        root_batch_id: Uuid,
    ) -> Result<Option<TopicReviewSnapshot>, TopicRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT id, project_id, root_batch_id, source_run_id, status,
                   review_summary, result, error_message, metadata,
                   created_at, updated_at
            FROM topic_review_snapshots
            WHERE project_id = $1
              AND root_batch_id = $2
              AND status = 'succeeded'
            ORDER BY created_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .bind(project_id)
        .bind(root_batch_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(TopicRepositoryError::from)?;

        row.map(topic_review_snapshot_from_row).transpose()
    }
}

impl PostgresTopicRepository {
    async fn get_generation_batch_for_project(
        &self,
        project_id: Uuid,
        batch_id: Uuid,
    ) -> Result<TopicGenerationBatch, TopicRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT id, project_id, source_run_id, supplement_of_batch_id,
                   prompt, requested_count, status,
                   error_message, metadata, created_at, updated_at
            FROM topic_generation_batches
            WHERE id = $1
              AND project_id = $2
            "#,
        )
        .bind(batch_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(TopicRepositoryError::from)?
        .ok_or(TopicRepositoryError::BatchNotFound(batch_id))?;

        batch_from_row(row)
    }
}

struct RootTopicGroupRow {
    root_batch_id: Uuid,
    project_id: Uuid,
    prompt: String,
    created_at: DateTime<Utc>,
    topic_count: i64,
    supplement_batch_count: i64,
}

fn root_topic_group_from_row(row: PgRow) -> RootTopicGroupRow {
    RootTopicGroupRow {
        root_batch_id: row.get("root_batch_id"),
        project_id: row.get("project_id"),
        prompt: row.get("prompt"),
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
        topic_count: row.get("topic_count"),
        supplement_batch_count: row.get("supplement_batch_count"),
    }
}

fn topic_group_review_freshness(
    topics: &[ContentTopic],
    snapshot: Option<&TopicReviewSnapshot>,
) -> TopicGroupReviewFreshness {
    let Some(snapshot) = snapshot else {
        return TopicGroupReviewFreshness::Missing;
    };

    let current_topic_ids: HashSet<Uuid> = topics.iter().map(|topic| topic.id).collect();
    let reviewed_topic_ids: HashSet<Uuid> = snapshot
        .result
        .topic_reviews
        .iter()
        .map(|review| review.topic_id)
        .collect();

    if current_topic_ids == reviewed_topic_ids {
        TopicGroupReviewFreshness::Fresh
    } else {
        TopicGroupReviewFreshness::Stale
    }
}

fn topic_group_script_priority(
    topics: &[ContentTopic],
    snapshot: Option<&TopicReviewSnapshot>,
    freshness: &TopicGroupReviewFreshness,
) -> TopicGroupScriptPriority {
    match freshness {
        TopicGroupReviewFreshness::Missing => {
            return TopicGroupScriptPriority {
                status: TopicGroupScriptPriorityStatus::NeedsReview,
                score: None,
                reason: "缺少成功主题组评审快照，请先评审当前主题组。".to_string(),
                metrics: TopicGroupScriptPriorityMetrics::default(),
                recommended_topic_ids: Vec::new(),
            };
        }
        TopicGroupReviewFreshness::Stale => {
            return TopicGroupScriptPriority {
                status: TopicGroupScriptPriorityStatus::NeedsReview,
                score: None,
                reason: "评审已过期，请重新评审当前主题组。".to_string(),
                metrics: TopicGroupScriptPriorityMetrics::default(),
                recommended_topic_ids: Vec::new(),
            };
        }
        TopicGroupReviewFreshness::Fresh => {}
    }

    let topic_by_id: HashMap<Uuid, &ContentTopic> =
        topics.iter().map(|topic| (topic.id, topic)).collect();
    let mut metrics = TopicGroupScriptPriorityMetrics {
        high_score_topic_count: topics
            .iter()
            .filter(|topic| topic.score.is_some_and(|score| score >= 80.0))
            .count() as i64,
        ..TopicGroupScriptPriorityMetrics::default()
    };
    let review_items = snapshot
        .map(|snapshot| snapshot.result.topic_reviews.as_slice())
        .unwrap_or_default();

    for review in review_items {
        if !topic_by_id.contains_key(&review.topic_id) {
            continue;
        }

        match review.priority {
            TopicReviewPriority::Priority => metrics.priority_count += 1,
            TopicReviewPriority::Backup => metrics.backup_count += 1,
            TopicReviewPriority::Reject => metrics.reject_count += 1,
        }
        if review.risk_flags.contains(&TopicReviewRiskFlag::Duplicate) {
            metrics.duplicate_count += 1;
        }
        if review
            .risk_flags
            .contains(&TopicReviewRiskFlag::HardToScript)
        {
            metrics.hard_to_script_count += 1;
        }
        if review
            .risk_flags
            .contains(&TopicReviewRiskFlag::OffPositioning)
        {
            metrics.off_positioning_count += 1;
        }
        if review
            .risk_flags
            .contains(&TopicReviewRiskFlag::ComplianceRisk)
        {
            metrics.compliance_risk_count += 1;
        }
    }

    let mut ready_candidates = review_items
        .iter()
        .filter(|review| {
            topic_by_id.contains_key(&review.topic_id)
                && review.priority == TopicReviewPriority::Priority
                && !has_ready_candidate_blocking_risk(&review.risk_flags)
        })
        .collect::<Vec<_>>();
    ready_candidates.sort_by(|left, right| {
        let left_topic = topic_by_id
            .get(&left.topic_id)
            .expect("ready candidate topic should exist");
        let right_topic = topic_by_id
            .get(&right.topic_id)
            .expect("ready candidate topic should exist");
        let score_order = right_topic
            .score
            .unwrap_or(-1.0)
            .partial_cmp(&left_topic.score.unwrap_or(-1.0))
            .unwrap_or(Ordering::Equal);
        score_order
            .then_with(|| right_topic.created_at.cmp(&left_topic.created_at))
            .then_with(|| right_topic.id.to_string().cmp(&left_topic.id.to_string()))
    });
    metrics.ready_candidate_count = ready_candidates.len() as i64;

    let raw_score = metrics.ready_candidate_count * 22
        + metrics.priority_count * 8
        + metrics.high_score_topic_count * 5
        + metrics.backup_count * 2
        - metrics.reject_count * 6
        - metrics.duplicate_count * 5
        - metrics.hard_to_script_count * 10
        - metrics.off_positioning_count * 10
        - metrics.compliance_risk_count * 15;
    let score = raw_score.clamp(0, 100) as i32;
    let status = if metrics.ready_candidate_count > 0 {
        TopicGroupScriptPriorityStatus::ReadyForScript
    } else if metrics.priority_count > 0 || metrics.backup_count > metrics.reject_count {
        TopicGroupScriptPriorityStatus::NeedsSupplement
    } else {
        TopicGroupScriptPriorityStatus::Defer
    };
    let reason = match status {
        TopicGroupScriptPriorityStatus::ReadyForScript => format!(
            "存在 {} 个无明显风险的优先推荐选题，脚本化路径清晰。",
            metrics.ready_candidate_count
        ),
        TopicGroupScriptPriorityStatus::NeedsSupplement => {
            "主题方向有潜力，但当前缺少无明显风险的脚本候选。".to_string()
        }
        TopicGroupScriptPriorityStatus::Defer => {
            "淘汰、重复、偏离定位或脚本化困难信号占主导，暂不建议推进。".to_string()
        }
        TopicGroupScriptPriorityStatus::NeedsReview => {
            "请先完成当前主题组评审。".to_string()
        }
    };
    let recommended_topic_ids = ready_candidates
        .into_iter()
        .take(3)
        .map(|review| review.topic_id)
        .collect();

    TopicGroupScriptPriority {
        status,
        score: Some(score),
        reason,
        metrics,
        recommended_topic_ids,
    }
}

fn has_ready_candidate_blocking_risk(risk_flags: &[TopicReviewRiskFlag]) -> bool {
    risk_flags.iter().any(|risk_flag| {
        matches!(
            risk_flag,
            TopicReviewRiskFlag::Duplicate
                | TopicReviewRiskFlag::HardToScript
                | TopicReviewRiskFlag::OffPositioning
                | TopicReviewRiskFlag::TooGeneric
                | TopicReviewRiskFlag::ComplianceRisk
        )
    })
}

fn sort_topic_group_summaries(summaries: &mut [TopicGroupSummary], sort: TopicGroupSort) {
    match sort {
        TopicGroupSort::CreatedAt => summaries.sort_by(compare_topic_group_created_at),
        TopicGroupSort::ScriptPriority => summaries.sort_by(|left, right| {
            topic_group_status_rank(&left.script_priority.status)
                .cmp(&topic_group_status_rank(&right.script_priority.status))
                .then_with(|| {
                    right
                        .script_priority
                        .score
                        .unwrap_or(-1)
                        .cmp(&left.script_priority.score.unwrap_or(-1))
                })
                .then_with(|| compare_topic_group_created_at(left, right))
        }),
    }
}

fn topic_group_status_rank(status: &TopicGroupScriptPriorityStatus) -> i32 {
    match status {
        TopicGroupScriptPriorityStatus::ReadyForScript => 0,
        TopicGroupScriptPriorityStatus::NeedsSupplement => 1,
        TopicGroupScriptPriorityStatus::Defer => 2,
        TopicGroupScriptPriorityStatus::NeedsReview => 3,
    }
}

fn compare_topic_group_created_at(left: &TopicGroupSummary, right: &TopicGroupSummary) -> Ordering {
    right
        .created_at
        .cmp(&left.created_at)
        .then_with(|| right.root_batch_id.to_string().cmp(&left.root_batch_id.to_string()))
}

#[derive(Debug)]
pub enum TopicRepositoryError {
    TopicNotFound(Uuid),
    BatchNotFound(Uuid),
    BatchCannotBeSupplemented(Uuid),
    TopicCannotBeDeleted(Uuid),
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
            Self::BatchCannotBeSupplemented(batch_id) => {
                write!(
                    formatter,
                    "topic generation batch cannot be supplemented: {batch_id}"
                )
            }
            Self::TopicCannotBeDeleted(topic_id) => {
                write!(formatter, "content topic cannot be deleted: {topic_id}")
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
        deleted_at: row.get("deleted_at"),
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
        supplement_of_batch_id: row.get("supplement_of_batch_id"),
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

fn topic_review_snapshot_from_row(row: PgRow) -> Result<TopicReviewSnapshot, TopicRepositoryError> {
    let status_value: String = row.get("status");
    let status = TopicReviewSnapshotStatus::try_from(status_value.as_str())
        .map_err(|error| TopicRepositoryError::Storage(error.to_string()))?;
    let result_value: Value = row.get("result");
    let result = serde_json::from_value(result_value)
        .map_err(|error| TopicRepositoryError::Storage(error.to_string()))?;

    Ok(TopicReviewSnapshot {
        id: row.get("id"),
        project_id: row.get("project_id"),
        root_batch_id: row.get("root_batch_id"),
        source_run_id: row.get("source_run_id"),
        status,
        review_summary: row.get("review_summary"),
        result,
        error_message: row.get("error_message"),
        metadata: row.get("metadata"),
        created_at: row.get::<DateTime<Utc>, _>("created_at"),
        updated_at: row.get::<DateTime<Utc>, _>("updated_at"),
    })
}
