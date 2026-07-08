use async_trait::async_trait;
use chrono::Utc;
use novex_api::agents::models::{
    ContentTopic, ContentTopicFilter, ContentTopicSource, ContentTopicStatus, TopicGenerationBatch,
    TopicGenerationBatchStatus, TopicGenerationBatchSummary, TopicGroupReviewFreshness,
    TopicGroupScriptPriority, TopicGroupScriptPriorityMetrics, TopicGroupScriptPriorityStatus,
    TopicGroupSort, TopicGroupSummary, TopicReviewPriority, TopicReviewRiskFlag,
    TopicReviewSnapshot, TopicReviewSnapshotStatus,
};
use novex_api::repositories::{
    CreateContentTopicInput, CreateTopicGenerationBatchInput, CreateTopicReviewSnapshotInput,
    TopicRepository, TopicRepositoryError, UpdateContentTopicInput,
    UpdateTopicGenerationBatchInput,
};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Default)]
struct MemoryTopicRepository {
    topics: Mutex<HashMap<Uuid, ContentTopic>>,
    batches: Mutex<HashMap<Uuid, TopicGenerationBatch>>,
    review_snapshots: Mutex<HashMap<Uuid, TopicReviewSnapshot>>,
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

    async fn list_topics_for_batch_group(
        &self,
        project_id: Uuid,
        root_batch_id: Uuid,
    ) -> Result<Vec<ContentTopic>, TopicRepositoryError> {
        let batches = self.batches.lock().unwrap();
        let group_batch_ids = batches
            .values()
            .filter(|batch| batch.project_id == project_id)
            .filter(|batch| {
                batch.id == root_batch_id || batch.supplement_of_batch_id == Some(root_batch_id)
            })
            .map(|batch| batch.id)
            .collect::<HashSet<_>>();
        drop(batches);

        let mut topics = self
            .topics
            .lock()
            .unwrap()
            .values()
            .filter(|topic| topic.project_id == project_id)
            .filter(|topic| topic.deleted_at.is_none())
            .filter(|topic| {
                topic
                    .batch_id
                    .is_some_and(|batch_id| group_batch_ids.contains(&batch_id))
            })
            .cloned()
            .collect::<Vec<_>>();
        topics.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
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
            supplement_of_batch_id: input.supplement_of_batch_id,
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

    async fn resolve_supplement_root_batch(
        &self,
        project_id: Uuid,
        target_batch_id: Uuid,
    ) -> Result<TopicGenerationBatch, TopicRepositoryError> {
        let batches = self.batches.lock().unwrap();
        let target = batches
            .get(&target_batch_id)
            .filter(|batch| batch.project_id == project_id)
            .cloned()
            .ok_or(TopicRepositoryError::BatchNotFound(target_batch_id))?;
        if target.status != TopicGenerationBatchStatus::Succeeded {
            return Err(TopicRepositoryError::BatchCannotBeSupplemented(
                target_batch_id,
            ));
        }
        let visible_topic_count = self
            .topics
            .lock()
            .unwrap()
            .values()
            .filter(|topic| topic.project_id == project_id)
            .filter(|topic| topic.batch_id == Some(target_batch_id))
            .filter(|topic| topic.deleted_at.is_none())
            .count();
        if visible_topic_count == 0 {
            return Err(TopicRepositoryError::BatchCannotBeSupplemented(
                target_batch_id,
            ));
        }

        let root_batch_id = target.supplement_of_batch_id.unwrap_or(target.id);
        if root_batch_id == target.id {
            return Ok(target);
        }
        batches
            .get(&root_batch_id)
            .filter(|batch| batch.project_id == project_id)
            .cloned()
            .ok_or(TopicRepositoryError::BatchNotFound(root_batch_id))
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

    async fn list_topic_group_summaries(
        &self,
        project_id: Uuid,
        sort: TopicGroupSort,
        limit: i64,
    ) -> Result<Vec<TopicGroupSummary>, TopicRepositoryError> {
        let topics = self.topics.lock().unwrap().clone();
        let batches = self.batches.lock().unwrap().clone();
        let snapshots = self.review_snapshots.lock().unwrap().clone();
        let mut summaries = batches
            .values()
            .filter(|batch| {
                batch.project_id == project_id
                    && batch.status == TopicGenerationBatchStatus::Succeeded
                    && batch.supplement_of_batch_id.is_none()
            })
            .filter_map(|root_batch| {
                let group_batch_ids = batches
                    .values()
                    .filter(|batch| batch.project_id == project_id)
                    .filter(|batch| {
                        batch.status == TopicGenerationBatchStatus::Succeeded
                            && (batch.id == root_batch.id
                                || batch.supplement_of_batch_id == Some(root_batch.id))
                    })
                    .map(|batch| batch.id)
                    .collect::<HashSet<_>>();
                let group_topics = topics
                    .values()
                    .filter(|topic| topic.project_id == project_id)
                    .filter(|topic| topic.deleted_at.is_none())
                    .filter(|topic| {
                        topic
                            .batch_id
                            .is_some_and(|batch_id| group_batch_ids.contains(&batch_id))
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if group_topics.is_empty() {
                    return None;
                }

                let latest_snapshot = snapshots
                    .values()
                    .filter(|snapshot| snapshot.project_id == project_id)
                    .filter(|snapshot| snapshot.root_batch_id == root_batch.id)
                    .filter(|snapshot| snapshot.status == TopicReviewSnapshotStatus::Succeeded)
                    .max_by(|left, right| {
                        left.created_at
                            .cmp(&right.created_at)
                            .then_with(|| left.id.cmp(&right.id))
                    })
                    .cloned();
                let review_freshness =
                    memory_review_freshness(&group_topics, latest_snapshot.as_ref());
                let script_priority =
                    memory_script_priority(&group_topics, latest_snapshot.as_ref(), &review_freshness);
                let supplement_batch_count = batches
                    .values()
                    .filter(|batch| batch.project_id == project_id)
                    .filter(|batch| batch.status == TopicGenerationBatchStatus::Succeeded)
                    .filter(|batch| batch.supplement_of_batch_id == Some(root_batch.id))
                    .count() as i64;

                Some(TopicGroupSummary {
                    root_batch_id: root_batch.id,
                    project_id: root_batch.project_id,
                    prompt: root_batch.prompt.clone(),
                    created_at: root_batch.created_at,
                    topic_count: group_topics.len() as i64,
                    supplement_batch_count,
                    latest_review_snapshot_id: latest_snapshot.map(|snapshot| snapshot.id),
                    review_freshness,
                    script_priority,
                })
            })
            .collect::<Vec<_>>();
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
        summaries.truncate(limit.clamp(1, 100) as usize);
        Ok(summaries)
    }

    async fn create_topic_review_snapshot(
        &self,
        input: CreateTopicReviewSnapshotInput,
    ) -> Result<TopicReviewSnapshot, TopicRepositoryError> {
        let now = Utc::now();
        let snapshot = TopicReviewSnapshot {
            id: Uuid::new_v4(),
            project_id: input.project_id,
            root_batch_id: input.root_batch_id,
            source_run_id: input.source_run_id,
            status: input.status,
            review_summary: input.review_summary,
            result: input.result,
            error_message: input.error_message,
            metadata: input.metadata,
            created_at: now,
            updated_at: now,
        };
        self.review_snapshots
            .lock()
            .unwrap()
            .insert(snapshot.id, snapshot.clone());
        Ok(snapshot)
    }

    async fn get_latest_topic_review_snapshot(
        &self,
        project_id: Uuid,
        root_batch_id: Uuid,
    ) -> Result<Option<TopicReviewSnapshot>, TopicRepositoryError> {
        let latest = self
            .review_snapshots
            .lock()
            .unwrap()
            .values()
            .filter(|snapshot| snapshot.project_id == project_id)
            .filter(|snapshot| snapshot.root_batch_id == root_batch_id)
            .filter(|snapshot| snapshot.status == TopicReviewSnapshotStatus::Succeeded)
            .max_by(|left, right| {
                left.created_at
                    .cmp(&right.created_at)
                    .then_with(|| left.id.cmp(&right.id))
            })
            .cloned();
        Ok(latest)
    }
}

fn memory_review_freshness(
    topics: &[ContentTopic],
    snapshot: Option<&TopicReviewSnapshot>,
) -> TopicGroupReviewFreshness {
    let Some(snapshot) = snapshot else {
        return TopicGroupReviewFreshness::Missing;
    };
    let current_topic_ids = topics.iter().map(|topic| topic.id).collect::<HashSet<_>>();
    let reviewed_topic_ids = snapshot
        .result
        .topic_reviews
        .iter()
        .map(|review| review.topic_id)
        .collect::<HashSet<_>>();
    if current_topic_ids == reviewed_topic_ids {
        TopicGroupReviewFreshness::Fresh
    } else {
        TopicGroupReviewFreshness::Stale
    }
}

fn memory_script_priority(
    topics: &[ContentTopic],
    snapshot: Option<&TopicReviewSnapshot>,
    freshness: &TopicGroupReviewFreshness,
) -> TopicGroupScriptPriority {
    if freshness != &TopicGroupReviewFreshness::Fresh {
        return TopicGroupScriptPriority {
            status: TopicGroupScriptPriorityStatus::NeedsReview,
            score: None,
            reason: "请先完成当前主题组评审。".to_string(),
            metrics: TopicGroupScriptPriorityMetrics::default(),
            recommended_topic_ids: Vec::new(),
        };
    }

    let topic_by_id = topics
        .iter()
        .map(|topic| (topic.id, topic))
        .collect::<HashMap<_, _>>();
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

    let mut recommended_topic_ids = review_items
        .iter()
        .filter(|review| {
            topic_by_id.contains_key(&review.topic_id)
                && review.priority == TopicReviewPriority::Priority
                && !review.risk_flags.iter().any(|risk_flag| {
                    matches!(
                        risk_flag,
                        TopicReviewRiskFlag::Duplicate
                            | TopicReviewRiskFlag::HardToScript
                            | TopicReviewRiskFlag::OffPositioning
                            | TopicReviewRiskFlag::TooGeneric
                            | TopicReviewRiskFlag::ComplianceRisk
                    )
                })
        })
        .map(|review| review.topic_id)
        .collect::<Vec<_>>();
    recommended_topic_ids.sort_by(|left, right| {
        let left_topic = topic_by_id.get(left).expect("candidate topic should exist");
        let right_topic = topic_by_id.get(right).expect("candidate topic should exist");
        right_topic
            .score
            .unwrap_or(-1.0)
            .total_cmp(&left_topic.score.unwrap_or(-1.0))
            .then_with(|| right_topic.created_at.cmp(&left_topic.created_at))
            .then_with(|| right_topic.id.cmp(&left_topic.id))
    });
    recommended_topic_ids.truncate(3);
    metrics.ready_candidate_count = recommended_topic_ids.len() as i64;
    let score = (metrics.ready_candidate_count * 22
        + metrics.priority_count * 8
        + metrics.high_score_topic_count * 5
        + metrics.backup_count * 2
        - metrics.reject_count * 6
        - metrics.duplicate_count * 5
        - metrics.hard_to_script_count * 10
        - metrics.off_positioning_count * 10
        - metrics.compliance_risk_count * 15)
        .clamp(0, 100) as i32;
    let status = if metrics.ready_candidate_count > 0 {
        TopicGroupScriptPriorityStatus::ReadyForScript
    } else if metrics.priority_count > 0 || metrics.backup_count > metrics.reject_count {
        TopicGroupScriptPriorityStatus::NeedsSupplement
    } else {
        TopicGroupScriptPriorityStatus::Defer
    };

    TopicGroupScriptPriority {
        status,
        score: Some(score),
        reason: "内存仓储按主题组评审快照计算脚本优先级。".to_string(),
        metrics,
        recommended_topic_ids,
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

fn compare_topic_group_created_at(
    left: &TopicGroupSummary,
    right: &TopicGroupSummary,
) -> std::cmp::Ordering {
    right
        .created_at
        .cmp(&left.created_at)
        .then_with(|| right.root_batch_id.cmp(&left.root_batch_id))
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
            supplement_of_batch_id: None,
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
            supplement_of_batch_id: None,
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
