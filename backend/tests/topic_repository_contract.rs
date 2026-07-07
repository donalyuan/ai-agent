use async_trait::async_trait;
use chrono::Utc;
use novex_api::agents::models::{
    ContentTopic, ContentTopicFilter, ContentTopicSource, ContentTopicStatus, TopicGenerationBatch,
    TopicGenerationBatchStatus, TopicGenerationBatchSummary,
};
use novex_api::repositories::{
    CreateContentTopicInput, CreateTopicGenerationBatchInput, TopicRepository,
    TopicRepositoryError, UpdateContentTopicInput, UpdateTopicGenerationBatchInput,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Default)]
struct MemoryTopicRepository {
    topics: Mutex<HashMap<Uuid, ContentTopic>>,
    batches: Mutex<HashMap<Uuid, TopicGenerationBatch>>,
}

#[async_trait]
impl TopicRepository for MemoryTopicRepository {
    async fn create_topic(
        &self,
        input: CreateContentTopicInput,
    ) -> Result<ContentTopic, TopicRepositoryError> {
        let now = Utc::now();
        let topic = ContentTopic {
            id: Uuid::new_v4(),
            project_id: input.project_id,
            batch_id: input.batch_id,
            title: input.title,
            angle: input.angle,
            target_audience: input.target_audience,
            hook_points: input.hook_points,
            content_type: input.content_type,
            score: input.score,
            score_reason: input.score_reason,
            tags: input.tags,
            source: input.source,
            status: ContentTopicStatus::Idea,
            metadata: input.metadata,
            deleted_at: None,
            created_at: now,
            updated_at: now,
        };
        self.topics.lock().unwrap().insert(topic.id, topic.clone());
        Ok(topic)
    }

    async fn update_topic(
        &self,
        topic_id: Uuid,
        input: UpdateContentTopicInput,
    ) -> Result<ContentTopic, TopicRepositoryError> {
        let mut topics = self.topics.lock().unwrap();
        let topic = topics
            .get_mut(&topic_id)
            .ok_or(TopicRepositoryError::TopicNotFound(topic_id))?;
        topic.title = input.title;
        topic.angle = input.angle;
        topic.target_audience = input.target_audience;
        topic.hook_points = input.hook_points;
        topic.content_type = input.content_type;
        topic.score = input.score;
        topic.score_reason = input.score_reason;
        topic.tags = input.tags;
        topic.updated_at = Utc::now();
        Ok(topic.clone())
    }

    async fn update_topic_status(
        &self,
        topic_id: Uuid,
        status: ContentTopicStatus,
    ) -> Result<ContentTopic, TopicRepositoryError> {
        let mut topics = self.topics.lock().unwrap();
        let topic = topics
            .get_mut(&topic_id)
            .ok_or(TopicRepositoryError::TopicNotFound(topic_id))?;
        topic.status = status;
        topic.updated_at = Utc::now();
        Ok(topic.clone())
    }

    async fn get_topic(&self, topic_id: Uuid) -> Result<ContentTopic, TopicRepositoryError> {
        self.topics
            .lock()
            .unwrap()
            .get(&topic_id)
            .cloned()
            .ok_or(TopicRepositoryError::TopicNotFound(topic_id))
    }

    async fn list_topics(
        &self,
        project_id: Uuid,
        filter: ContentTopicFilter,
    ) -> Result<Vec<ContentTopic>, TopicRepositoryError> {
        let mut topics = self
            .topics
            .lock()
            .unwrap()
            .values()
            .filter(|topic| topic.project_id == project_id)
            .filter(|topic| topic.deleted_at.is_none())
            .filter(|topic| {
                filter
                    .status
                    .as_ref()
                    .is_none_or(|status| &topic.status == status)
            })
            .filter(|topic| {
                filter
                    .source
                    .as_ref()
                    .is_none_or(|source| &topic.source == source)
            })
            .filter(|topic| {
                filter
                    .batch_id
                    .is_none_or(|batch_id| topic.batch_id == Some(batch_id))
            })
            .cloned()
            .collect::<Vec<_>>();
        topics.sort_by(compare_topic_pool_order);
        Ok(topics)
    }

    async fn count_topics_by_status(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<(ContentTopicStatus, i64)>, TopicRepositoryError> {
        let mut counts: HashMap<ContentTopicStatus, i64> = HashMap::new();
        for topic in self
            .topics
            .lock()
            .unwrap()
            .values()
            .filter(|topic| topic.project_id == project_id)
            .filter(|topic| topic.deleted_at.is_none())
        {
            *counts.entry(topic.status.clone()).or_insert(0) += 1;
        }
        Ok(counts.into_iter().collect())
    }

    async fn soft_delete_topic(
        &self,
        topic_id: Uuid,
    ) -> Result<ContentTopic, TopicRepositoryError> {
        let mut topics = self.topics.lock().unwrap();
        let topic = topics
            .get_mut(&topic_id)
            .ok_or(TopicRepositoryError::TopicNotFound(topic_id))?;
        if topic.status == ContentTopicStatus::Scripted {
            return Err(TopicRepositoryError::TopicCannotBeDeleted(topic_id));
        }
        topic.deleted_at = Some(Utc::now());
        topic.updated_at = Utc::now();
        Ok(topic.clone())
    }

    async fn create_generation_batch(
        &self,
        input: CreateTopicGenerationBatchInput,
    ) -> Result<TopicGenerationBatch, TopicRepositoryError> {
        let now = Utc::now();
        let batch = TopicGenerationBatch {
            id: Uuid::new_v4(),
            project_id: input.project_id,
            source_run_id: input.source_run_id,
            prompt: input.prompt,
            requested_count: input.requested_count,
            status: input.status,
            error_message: input.error_message,
            metadata: input.metadata,
            created_at: now,
            updated_at: now,
        };
        self.batches.lock().unwrap().insert(batch.id, batch.clone());
        Ok(batch)
    }

    async fn update_generation_batch(
        &self,
        batch_id: Uuid,
        input: UpdateTopicGenerationBatchInput,
    ) -> Result<TopicGenerationBatch, TopicRepositoryError> {
        let mut batches = self.batches.lock().unwrap();
        let batch = batches
            .get_mut(&batch_id)
            .ok_or(TopicRepositoryError::BatchNotFound(batch_id))?;
        batch.status = input.status;
        batch.error_message = input.error_message;
        batch.metadata = input.metadata;
        batch.updated_at = Utc::now();
        Ok(batch.clone())
    }

    async fn get_generation_batch(
        &self,
        batch_id: Uuid,
    ) -> Result<TopicGenerationBatch, TopicRepositoryError> {
        self.batches
            .lock()
            .unwrap()
            .get(&batch_id)
            .cloned()
            .ok_or(TopicRepositoryError::BatchNotFound(batch_id))
    }

    async fn list_generation_batches(
        &self,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<TopicGenerationBatchSummary>, TopicRepositoryError> {
        let topics = self.topics.lock().unwrap();
        let mut batches = self
            .batches
            .lock()
            .unwrap()
            .values()
            .filter(|batch| {
                batch.project_id == project_id
                    && batch.status == TopicGenerationBatchStatus::Succeeded
            })
            .map(|batch| TopicGenerationBatchSummary {
                batch: batch.clone(),
                topic_count: topics
                    .values()
                    .filter(|topic| topic.batch_id == Some(batch.id))
                    .filter(|topic| topic.deleted_at.is_none())
                    .count() as i64,
            })
            .filter(|summary| summary.topic_count > 0)
            .collect::<Vec<_>>();
        batches.sort_by(|left, right| {
            right
                .batch
                .created_at
                .cmp(&left.batch.created_at)
                .then_with(|| right.batch.id.cmp(&left.batch.id))
        });
        batches.truncate(limit.clamp(1, 100) as usize);
        Ok(batches)
    }
}

fn compare_topic_pool_order(left: &ContentTopic, right: &ContentTopic) -> std::cmp::Ordering {
    match (left.score, right.score) {
        (Some(left_score), Some(right_score)) => right_score
            .total_cmp(&left_score)
            .then_with(|| right.created_at.cmp(&left.created_at))
            .then_with(|| right.id.cmp(&left.id)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| right.id.cmp(&left.id)),
    }
}

fn manual_topic(project_id: Uuid) -> CreateContentTopicInput {
    CreateContentTopicInput {
        project_id,
        batch_id: None,
        title: "AI 工具正在重塑内容团队".to_string(),
        angle: "从内容生产流程角度解释 AI 工具落地".to_string(),
        target_audience: "中小团队内容运营负责人".to_string(),
        hook_points: vec!["低成本提效".to_string(), "减少重复劳动".to_string()],
        content_type: "knowledge".to_string(),
        score: Some(86.0),
        score_reason: "选题贴近当前项目定位，且适合脚本化表达。".to_string(),
        tags: vec!["AI工具".to_string(), "内容运营".to_string()],
        source: ContentTopicSource::Manual,
        metadata: json!({}),
    }
}

#[tokio::test]
async fn topic_repository_trait_supports_topic_lifecycle_and_filters() {
    let repository = MemoryTopicRepository::default();
    let project_id = Uuid::new_v4();
    let other_project_id = Uuid::new_v4();

    let topic = repository
        .create_topic(manual_topic(project_id))
        .await
        .unwrap();
    let _other = repository
        .create_topic(manual_topic(other_project_id))
        .await
        .unwrap();

    assert_eq!(topic.status, ContentTopicStatus::Idea);
    assert_eq!(topic.source, ContentTopicSource::Manual);

    let approved = repository
        .update_topic_status(topic.id, ContentTopicStatus::Approved)
        .await
        .unwrap();
    assert_eq!(approved.status, ContentTopicStatus::Approved);

    let updated = repository
        .update_topic(
            topic.id,
            UpdateContentTopicInput {
                title: "AI 工具如何重塑内容团队".to_string(),
                angle: "强调团队协作和流程改造".to_string(),
                target_audience: "内容负责人".to_string(),
                hook_points: vec!["流程重构".to_string()],
                content_type: "knowledge".to_string(),
                score: Some(91.0),
                score_reason: "标题更聚焦，受众更明确。".to_string(),
                tags: vec!["AI工具".to_string()],
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.title, "AI 工具如何重塑内容团队");

    let listed = repository
        .list_topics(
            project_id,
            ContentTopicFilter {
                status: Some(ContentTopicStatus::Approved),
                source: Some(ContentTopicSource::Manual),
                batch_id: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, topic.id);

    let counts = repository.count_topics_by_status(project_id).await.unwrap();
    assert!(counts.contains(&(ContentTopicStatus::Approved, 1)));
}

#[tokio::test]
async fn topic_repository_trait_supports_generation_batches() {
    let repository = MemoryTopicRepository::default();
    let project_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();

    let batch = repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: Some(run_id),
            prompt: "本周 AI 工具方向，生成 3 个选题".to_string(),
            requested_count: 3,
            status: TopicGenerationBatchStatus::Running,
            error_message: None,
            metadata: json!({ "requested_topic_count": 3 }),
        })
        .await
        .unwrap();

    let topic = repository
        .create_topic(CreateContentTopicInput {
            batch_id: Some(batch.id),
            source: ContentTopicSource::Agent,
            ..manual_topic(project_id)
        })
        .await
        .unwrap();
    assert_eq!(topic.batch_id, Some(batch.id));
    assert_eq!(topic.source, ContentTopicSource::Agent);

    let finished = repository
        .update_generation_batch(
            batch.id,
            UpdateTopicGenerationBatchInput {
                status: TopicGenerationBatchStatus::Succeeded,
                error_message: None,
                metadata: json!({
                    "created_topic_ids": [topic.id],
                    "topic_count": 1
                }),
            },
        )
        .await
        .unwrap();
    assert_eq!(finished.status, TopicGenerationBatchStatus::Succeeded);
    assert_eq!(finished.metadata["topic_count"], 1);

    let by_batch = repository
        .list_topics(
            project_id,
            ContentTopicFilter {
                status: None,
                source: Some(ContentTopicSource::Agent),
                batch_id: Some(batch.id),
            },
        )
        .await
        .unwrap();
    assert_eq!(by_batch.len(), 1);
    assert_eq!(by_batch[0].id, topic.id);

    let batches = repository
        .list_generation_batches(project_id, 20)
        .await
        .unwrap();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].batch.id, batch.id);
    assert_eq!(batches[0].topic_count, 1);
}

#[tokio::test]
async fn topic_repository_trait_soft_deletes_visible_topics_only() {
    let repository = MemoryTopicRepository::default();
    let project_id = Uuid::new_v4();
    let batch = repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: None,
            prompt: "本周 AI 工具方向，生成 2 个选题".to_string(),
            requested_count: 2,
            status: TopicGenerationBatchStatus::Succeeded,
            error_message: None,
            metadata: json!({}),
        })
        .await
        .unwrap();
    let topic = repository
        .create_topic(CreateContentTopicInput {
            batch_id: Some(batch.id),
            source: ContentTopicSource::Agent,
            ..manual_topic(project_id)
        })
        .await
        .unwrap();

    let deleted = repository.soft_delete_topic(topic.id).await.unwrap();
    assert!(deleted.deleted_at.is_some());
    assert_eq!(deleted.status, ContentTopicStatus::Idea);

    let listed = repository
        .list_topics(project_id, ContentTopicFilter::default())
        .await
        .unwrap();
    assert!(listed.is_empty());

    let counts = repository.count_topics_by_status(project_id).await.unwrap();
    assert!(counts.is_empty());

    let batches = repository
        .list_generation_batches(project_id, 20)
        .await
        .unwrap();
    assert!(batches.is_empty());
}

#[tokio::test]
async fn topic_repository_trait_rejects_soft_deleting_scripted_topic() {
    let repository = MemoryTopicRepository::default();
    let project_id = Uuid::new_v4();
    let topic = repository
        .create_topic(manual_topic(project_id))
        .await
        .unwrap();
    repository
        .update_topic_status(topic.id, ContentTopicStatus::Approved)
        .await
        .unwrap();
    repository
        .update_topic_status(topic.id, ContentTopicStatus::Scripted)
        .await
        .unwrap();

    let error = repository.soft_delete_topic(topic.id).await.unwrap_err();
    assert!(matches!(
        error,
        TopicRepositoryError::TopicCannotBeDeleted(topic_id) if topic_id == topic.id
    ));
}
