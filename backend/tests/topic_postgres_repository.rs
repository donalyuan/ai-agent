use novex_api::domain::topic::{
    ContentTopicFilter, ContentTopicSource, ContentTopicStatus, TopicGenerationBatchStatus,
    TopicQualityDecision, TopicQualityEvaluationStatus, TopicQualityFlag, TopicQualityGateItem,
    TopicQualityGateResult,
};
use novex_api::repositories::{
    CreateContentTopicInput, CreateTopicGenerationBatchInput, CreateTopicQualityEvaluationInput,
    PostgresProjectRepository, PostgresTopicRepository, ProjectRepository, TopicRepository,
    UpdateContentTopicInput, UpdateTopicGenerationBatchInput,
};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

mod support;

use support::test_database::TestDatabase;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@biga-postgres:5432/video_agent".to_string()
    })
}

fn with_database_name(database_url: &str, database_name: &str) -> String {
    let query_start = database_url.find('?');
    let (base, query) = match query_start {
        Some(index) => (&database_url[..index], &database_url[index..]),
        None => (database_url, ""),
    };

    let slash_index = base
        .rfind('/')
        .expect("DATABASE_URL must include database name");
    format!("{}{}{}", &base[..=slash_index], database_name, query)
}

async fn create_database(
    admin_pool: &PgPool,
    admin_url: &str,
    database_name: &str,
) -> TestDatabase {
    let query = format!(r#"CREATE DATABASE "{}""#, database_name);
    sqlx::query(&query)
        .execute(admin_pool)
        .await
        .expect("temporary topic database should be created");
    TestDatabase::new(admin_url, database_name)
}

async fn drop_database(admin_pool: &PgPool, database_name: &str) {
    let disconnect = format!(
        r#"
        SELECT pg_terminate_backend(pid)
        FROM pg_stat_activity
        WHERE datname = '{}'
        "#,
        database_name
    );
    let drop = format!(r#"DROP DATABASE IF EXISTS "{}""#, database_name);

    let _ = sqlx::query(&disconnect).execute(admin_pool).await;
    let _ = sqlx::query(&drop).execute(admin_pool).await;
}

async fn migrated_pool() -> (PgPool, PgPool, TestDatabase) {
    let base_url = database_url();
    let suffix = Uuid::new_v4().simple().to_string();
    let database_name = format!("video_agent_topic_repo_test_{}", suffix);
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .expect("admin database should be reachable");
    let database_name = create_database(&admin_pool, &admin_url, &database_name).await;

    let test_pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(&test_url)
        .await
        .expect("temporary topic database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .expect("migrations should run for topic repository test database");

    (admin_pool, test_pool, database_name)
}

async fn insert_project(pool: &PgPool) -> Uuid {
    let repository = PostgresProjectRepository::new(pool.clone());
    repository
        .create_project(novex_api::repositories::CreateProjectInput {
            name: "科技博主".to_string(),
            positioning: "AI 工具和内容生产效率".to_string(),
            description: "面向内容运营负责人的科技知识账号".to_string(),
            strategy_profile: novex_api::repositories::AccountStrategyProfile::default(),
        })
        .await
        .unwrap()
        .id
}

async fn insert_agent_run(pool: &PgPool, project_id: Uuid) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO agent_runs (project_id, agent_type, status, input)
        VALUES ($1, 'topic', 'running', $2)
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(json!({"intent": "generate_topics"}))
    .fetch_one(pool)
    .await
    .expect("agent run fixture should be inserted")
}

async fn insert_script_for_topic(pool: &PgPool, project_id: Uuid, topic_id: Uuid) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO scripts (project_id, topic_id, title, hook, content, status)
        VALUES ($1, $2, '选题生成脚本', '选题脚本 hook', $3, 'draft')
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(topic_id)
    .bind(json!({"topic_snapshot": {"topic_id": topic_id}}))
    .fetch_one(pool)
    .await
    .expect("script fixture should be inserted for topic")
}

#[tokio::test]
async fn postgres_topic_repository_persists_topic_quality_evaluations() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let other_project_id = insert_project(&test_pool).await;
    let run_id = insert_agent_run(&test_pool, project_id).await;
    let repository = PostgresTopicRepository::new(test_pool.clone());
    let batch = repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: Some(run_id),
            supplement_of_batch_id: None,
            prompt: "生成 AI 工具方向选题".to_string(),
            requested_count: 3,
            status: TopicGenerationBatchStatus::Running,
            error_message: None,
            metadata: json!({}),
        })
        .await
        .unwrap();

    let first = repository
        .create_topic_quality_evaluation(CreateTopicQualityEvaluationInput {
            project_id,
            batch_id: batch.id,
            source_run_id: Some(run_id),
            status: TopicQualityEvaluationStatus::Succeeded,
            pass_count: 1,
            reject_count: 2,
            rewrite_triggered: false,
            result: TopicQualityGateResult {
                summary: "首轮 3 条中 1 条通过。".to_string(),
                items: vec![TopicQualityGateItem {
                    candidate_key: "candidate-1".to_string(),
                    title: "AI 工具如何改变选题会".to_string(),
                    decision: TopicQualityDecision::Pass,
                    quality_score: 84,
                    flags: vec![],
                    reason: "贴合账号定位。".to_string(),
                }],
            },
            error_message: None,
        })
        .await
        .unwrap();
    let latest = repository
        .create_topic_quality_evaluation(CreateTopicQualityEvaluationInput {
            project_id,
            batch_id: batch.id,
            source_run_id: Some(run_id),
            status: TopicQualityEvaluationStatus::Succeeded,
            pass_count: 2,
            reject_count: 1,
            rewrite_triggered: true,
            result: TopicQualityGateResult {
                summary: "重写后 3 条中 2 条通过，1 条重复淘汰。".to_string(),
                items: vec![TopicQualityGateItem {
                    candidate_key: "candidate-2".to_string(),
                    title: "AI 工作流复盘选题".to_string(),
                    decision: TopicQualityDecision::Reject,
                    quality_score: 58,
                    flags: vec![TopicQualityFlag::Duplicate],
                    reason: "与同主题组已有选题重复。".to_string(),
                }],
            },
            error_message: None,
        })
        .await
        .unwrap();

    let loaded = repository
        .get_latest_topic_quality_evaluation(project_id, batch.id)
        .await
        .unwrap()
        .expect("latest topic quality evaluation should exist");
    assert_eq!(loaded.id, latest.id);
    assert_ne!(loaded.id, first.id);
    assert_eq!(loaded.project_id, project_id);
    assert_eq!(loaded.batch_id, batch.id);
    assert_eq!(loaded.source_run_id, Some(run_id));
    assert_eq!(loaded.pass_count, 2);
    assert_eq!(loaded.reject_count, 1);
    assert!(loaded.rewrite_triggered);
    assert_eq!(
        loaded.result.summary,
        "重写后 3 条中 2 条通过，1 条重复淘汰。"
    );
    assert_eq!(
        loaded.result.items[0].flags,
        vec![TopicQualityFlag::Duplicate]
    );

    let isolated = repository
        .get_latest_topic_quality_evaluation(other_project_id, batch.id)
        .await
        .unwrap();
    assert!(isolated.is_none());

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

fn manual_topic(project_id: Uuid) -> CreateContentTopicInput {
    CreateContentTopicInput {
        project_id,
        batch_id: None,
        title: "AI 工具正在重塑内容团队".to_string(),
        angle: "从流程协同角度切入".to_string(),
        target_audience: "内容运营负责人".to_string(),
        hook_points: vec!["提效".to_string(), "降本".to_string()],
        content_type: "knowledge".to_string(),
        score: Some(88.0),
        score_reason: "贴合账号定位".to_string(),
        tags: vec!["AI工具".to_string(), "内容运营".to_string()],
        source: ContentTopicSource::Manual,
        metadata: json!({}),
    }
}

#[tokio::test]
async fn postgres_topic_repository_persists_topics_batches_and_filters() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let other_project_id = insert_project(&test_pool).await;
    let repository = PostgresTopicRepository::new(test_pool.clone());

    let batch = repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: None,
            supplement_of_batch_id: None,
            prompt: "生成 AI 工具方向选题".to_string(),
            requested_count: 2,
            status: TopicGenerationBatchStatus::Running,
            error_message: None,
            metadata: json!({ "requested_topic_count": 2 }),
        })
        .await
        .unwrap();

    let manual = repository
        .create_topic(manual_topic(project_id))
        .await
        .unwrap();
    let agent = repository
        .create_topic(CreateContentTopicInput {
            batch_id: Some(batch.id),
            source: ContentTopicSource::Agent,
            title: "AI 工具如何改变选题会".to_string(),
            ..manual_topic(project_id)
        })
        .await
        .unwrap();
    let _other_project = repository
        .create_topic(manual_topic(other_project_id))
        .await
        .unwrap();

    assert_eq!(manual.status, ContentTopicStatus::Idea);
    assert_eq!(agent.batch_id, Some(batch.id));

    let approved = repository
        .update_topic_status(manual.id, ContentTopicStatus::Approved)
        .await
        .unwrap();
    assert_eq!(approved.status, ContentTopicStatus::Approved);

    let updated = repository
        .update_topic(
            manual.id,
            UpdateContentTopicInput {
                title: "AI 工具如何重塑内容团队".to_string(),
                angle: "强调协作流程".to_string(),
                target_audience: "内容负责人".to_string(),
                hook_points: vec!["流程重构".to_string()],
                content_type: "knowledge".to_string(),
                score: Some(91.0),
                score_reason: "标题更具体".to_string(),
                tags: vec!["AI工具".to_string()],
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.score, Some(91.0));

    let approved_topics = repository
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
    assert_eq!(approved_topics.len(), 1);
    assert_eq!(approved_topics[0].id, manual.id);

    let batch_topics = repository
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
    assert_eq!(batch_topics.len(), 1);
    assert_eq!(batch_topics[0].id, agent.id);

    let counts = repository.count_topics_by_status(project_id).await.unwrap();
    assert!(counts.contains(&(ContentTopicStatus::Approved, 1)));
    assert!(counts.contains(&(ContentTopicStatus::Idea, 1)));

    let finished_batch = repository
        .update_generation_batch(
            batch.id,
            UpdateTopicGenerationBatchInput {
                status: TopicGenerationBatchStatus::Succeeded,
                error_message: None,
                metadata: json!({ "created_topic_ids": [agent.id], "topic_count": 1 }),
            },
        )
        .await
        .unwrap();
    assert_eq!(finished_batch.status, TopicGenerationBatchStatus::Succeeded);
    assert_eq!(finished_batch.metadata["topic_count"], 1);

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn postgres_topic_repository_soft_deletes_only_unreferenced_visible_topics() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let repository = PostgresTopicRepository::new(test_pool.clone());

    let batch = repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: None,
            supplement_of_batch_id: None,
            prompt: "生成 AI 工具方向选题".to_string(),
            requested_count: 2,
            status: TopicGenerationBatchStatus::Succeeded,
            error_message: None,
            metadata: json!({}),
        })
        .await
        .unwrap();
    let removable = repository
        .create_topic(CreateContentTopicInput {
            batch_id: Some(batch.id),
            source: ContentTopicSource::Agent,
            title: "未生成脚本的选题".to_string(),
            ..manual_topic(project_id)
        })
        .await
        .unwrap();
    let referenced = repository
        .create_topic(CreateContentTopicInput {
            batch_id: Some(batch.id),
            source: ContentTopicSource::Agent,
            title: "已被脚本引用的选题".to_string(),
            ..manual_topic(project_id)
        })
        .await
        .unwrap();
    insert_script_for_topic(&test_pool, project_id, referenced.id).await;

    let deleted = repository.soft_delete_topic(removable.id).await.unwrap();
    assert!(deleted.deleted_at.is_some());
    assert_eq!(deleted.status, ContentTopicStatus::Idea);

    let topics = repository
        .list_topics(project_id, ContentTopicFilter::default())
        .await
        .unwrap();
    assert_eq!(topics.len(), 1);
    assert_eq!(topics[0].id, referenced.id);

    let counts = repository.count_topics_by_status(project_id).await.unwrap();
    assert_eq!(counts, vec![(ContentTopicStatus::Idea, 1)]);

    let batches = repository
        .list_generation_batches(project_id, 20)
        .await
        .unwrap();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].topic_count, 1);

    let error = repository
        .soft_delete_topic(referenced.id)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        novex_api::repositories::TopicRepositoryError::TopicCannotBeDeleted(topic_id)
            if topic_id == referenced.id
    ));

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn postgres_topic_repository_tracks_supplement_batches_independently() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let repository = PostgresTopicRepository::new(test_pool.clone());

    let original_batch = repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: None,
            prompt: "原始生成 AI 工具方向选题".to_string(),
            requested_count: 2,
            status: TopicGenerationBatchStatus::Succeeded,
            error_message: None,
            metadata: json!({}),
            supplement_of_batch_id: None,
        })
        .await
        .unwrap();
    let original_topic = repository
        .create_topic(CreateContentTopicInput {
            batch_id: Some(original_batch.id),
            source: ContentTopicSource::Agent,
            title: "原始批次选题 1".to_string(),
            ..manual_topic(project_id)
        })
        .await
        .unwrap();
    repository
        .create_topic(CreateContentTopicInput {
            batch_id: Some(original_batch.id),
            source: ContentTopicSource::Agent,
            title: "原始批次选题 2".to_string(),
            ..manual_topic(project_id)
        })
        .await
        .unwrap();

    let supplement_batch = repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: None,
            prompt: "补充生成 AI 工具方向选题".to_string(),
            requested_count: 1,
            status: TopicGenerationBatchStatus::Succeeded,
            error_message: None,
            metadata: json!({}),
            supplement_of_batch_id: Some(original_batch.id),
        })
        .await
        .unwrap();
    assert_eq!(
        supplement_batch.supplement_of_batch_id,
        Some(original_batch.id)
    );
    let supplement_topic = repository
        .create_topic(CreateContentTopicInput {
            batch_id: Some(supplement_batch.id),
            source: ContentTopicSource::Agent,
            title: "补充批次选题".to_string(),
            ..manual_topic(project_id)
        })
        .await
        .unwrap();
    assert_eq!(supplement_topic.batch_id, Some(supplement_batch.id));

    repository
        .soft_delete_topic(original_topic.id)
        .await
        .unwrap();

    let batches = repository
        .list_generation_batches(project_id, 20)
        .await
        .unwrap();
    let original_summary = batches
        .iter()
        .find(|summary| summary.batch.id == original_batch.id)
        .expect("original batch should remain visible with one topic");
    let supplement_summary = batches
        .iter()
        .find(|summary| summary.batch.id == supplement_batch.id)
        .expect("supplement batch should be listed separately");

    assert_eq!(original_summary.topic_count, 1);
    assert_eq!(original_summary.batch.supplement_of_batch_id, None);
    assert_eq!(supplement_summary.topic_count, 1);
    assert_eq!(
        supplement_summary.batch.supplement_of_batch_id,
        Some(original_batch.id)
    );

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn postgres_topic_repository_resolves_supplement_root_and_rejects_unusable_targets() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let other_project_id = insert_project(&test_pool).await;
    let repository = PostgresTopicRepository::new(test_pool.clone());

    let original_batch = repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: None,
            prompt: "原始生成 AI 工具方向选题".to_string(),
            requested_count: 1,
            status: TopicGenerationBatchStatus::Succeeded,
            error_message: None,
            metadata: json!({}),
            supplement_of_batch_id: None,
        })
        .await
        .unwrap();
    repository
        .create_topic(CreateContentTopicInput {
            batch_id: Some(original_batch.id),
            source: ContentTopicSource::Agent,
            ..manual_topic(project_id)
        })
        .await
        .unwrap();

    let supplement_batch = repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: None,
            prompt: "补充生成 AI 工具方向选题".to_string(),
            requested_count: 1,
            status: TopicGenerationBatchStatus::Succeeded,
            error_message: None,
            metadata: json!({}),
            supplement_of_batch_id: Some(original_batch.id),
        })
        .await
        .unwrap();
    repository
        .create_topic(CreateContentTopicInput {
            batch_id: Some(supplement_batch.id),
            source: ContentTopicSource::Agent,
            ..manual_topic(project_id)
        })
        .await
        .unwrap();

    let resolved = repository
        .resolve_supplement_root_batch(project_id, supplement_batch.id)
        .await
        .unwrap();
    assert_eq!(resolved.id, original_batch.id);

    let cross_project = repository
        .resolve_supplement_root_batch(other_project_id, original_batch.id)
        .await
        .unwrap_err();
    assert!(matches!(
        cross_project,
        novex_api::repositories::TopicRepositoryError::BatchNotFound(batch_id)
            if batch_id == original_batch.id
    ));

    let failed_batch = repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: None,
            prompt: "失败批次".to_string(),
            requested_count: 1,
            status: TopicGenerationBatchStatus::Failed,
            error_message: Some("LLM 输出非法".to_string()),
            metadata: json!({}),
            supplement_of_batch_id: None,
        })
        .await
        .unwrap();
    let failed_error = repository
        .resolve_supplement_root_batch(project_id, failed_batch.id)
        .await
        .unwrap_err();
    assert!(matches!(
        failed_error,
        novex_api::repositories::TopicRepositoryError::BatchCannotBeSupplemented(batch_id)
            if batch_id == failed_batch.id
    ));

    let empty_batch = repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: None,
            prompt: "空批次".to_string(),
            requested_count: 1,
            status: TopicGenerationBatchStatus::Succeeded,
            error_message: None,
            metadata: json!({}),
            supplement_of_batch_id: None,
        })
        .await
        .unwrap();
    let empty_error = repository
        .resolve_supplement_root_batch(project_id, empty_batch.id)
        .await
        .unwrap_err();
    assert!(matches!(
        empty_error,
        novex_api::repositories::TopicRepositoryError::BatchCannotBeSupplemented(batch_id)
            if batch_id == empty_batch.id
    ));

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
