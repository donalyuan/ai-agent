use novex_api::domain::topic::{
    ContentTopicSource, TopicGenerationBatchStatus, TopicReviewItem, TopicReviewPriority,
    TopicReviewResult, TopicReviewRiskFlag, TopicReviewSnapshotStatus,
};
use novex_api::repositories::{
    CreateContentTopicInput, CreateTopicGenerationBatchInput, CreateTopicReviewSnapshotInput,
    PostgresProjectRepository, PostgresTopicRepository, ProjectRepository, TopicRepository,
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
        .expect("temporary topic review database should be created");
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
    let database_name = format!("video_agent_topic_review_repo_test_{}", suffix);
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
        .expect("temporary topic review database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .expect("migrations should run for topic review repository test database");

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

async fn insert_topic_group(
    repository: &PostgresTopicRepository,
    project_id: Uuid,
) -> (Uuid, Uuid) {
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
    let topic = repository
        .create_topic(CreateContentTopicInput {
            project_id,
            batch_id: Some(batch.id),
            title: "AI 工具如何改变选题会".to_string(),
            angle: "对比传统选题会和 AI 辅助选题".to_string(),
            target_audience: "内容运营负责人".to_string(),
            hook_points: vec!["三分钟生成候选".to_string()],
            content_type: "knowledge".to_string(),
            score: Some(92.0),
            score_reason: "贴合账号定位且容易脚本化".to_string(),
            tags: vec!["AI工具".to_string()],
            source: ContentTopicSource::Agent,
            metadata: json!({}),
        })
        .await
        .unwrap();

    (batch.id, topic.id)
}

fn review_result(topic_id: Uuid, priority: TopicReviewPriority) -> TopicReviewResult {
    TopicReviewResult {
        topic_reviews: vec![TopicReviewItem {
            topic_id,
            priority,
            reason: "账号匹配度高，脚本化路径清晰。".to_string(),
            risk_flags: vec![TopicReviewRiskFlag::Duplicate],
            similar_topic_ids: Vec::new(),
        }],
    }
}

#[tokio::test]
async fn postgres_topic_repository_creates_and_reads_latest_topic_review_snapshot() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let other_project_id = insert_project(&test_pool).await;
    let repository = PostgresTopicRepository::new(test_pool.clone());
    let (root_batch_id, topic_id) = insert_topic_group(&repository, project_id).await;

    let first = repository
        .create_topic_review_snapshot(CreateTopicReviewSnapshotInput {
            project_id,
            root_batch_id,
            source_run_id: None,
            status: TopicReviewSnapshotStatus::Succeeded,
            review_summary: "第一版评审摘要".to_string(),
            result: review_result(topic_id, TopicReviewPriority::Backup),
            error_message: None,
            metadata: json!({ "round": 1 }),
        })
        .await
        .unwrap();
    assert_eq!(first.project_id, project_id);
    assert_eq!(first.root_batch_id, root_batch_id);
    assert_eq!(
        first.result.topic_reviews[0].priority,
        TopicReviewPriority::Backup
    );

    let failed = repository
        .create_topic_review_snapshot(CreateTopicReviewSnapshotInput {
            project_id,
            root_batch_id,
            source_run_id: None,
            status: TopicReviewSnapshotStatus::Failed,
            review_summary: String::new(),
            result: TopicReviewResult::default(),
            error_message: Some("invalid priority".to_string()),
            metadata: json!({ "round": "failed" }),
        })
        .await
        .unwrap();
    assert_eq!(failed.status, TopicReviewSnapshotStatus::Failed);

    let latest_after_failed = repository
        .get_latest_topic_review_snapshot(project_id, root_batch_id)
        .await
        .unwrap()
        .expect("latest succeeded snapshot should exist");
    assert_eq!(latest_after_failed.id, first.id);
    assert_eq!(latest_after_failed.review_summary, "第一版评审摘要");

    let second = repository
        .create_topic_review_snapshot(CreateTopicReviewSnapshotInput {
            project_id,
            root_batch_id,
            source_run_id: None,
            status: TopicReviewSnapshotStatus::Succeeded,
            review_summary: "第二版评审摘要".to_string(),
            result: review_result(topic_id, TopicReviewPriority::Priority),
            error_message: None,
            metadata: json!({ "round": 2 }),
        })
        .await
        .unwrap();

    let latest = repository
        .get_latest_topic_review_snapshot(project_id, root_batch_id)
        .await
        .unwrap()
        .expect("latest succeeded snapshot should exist");
    assert_eq!(latest.id, second.id);
    assert_eq!(latest.source_run_id, None);
    assert_eq!(
        latest.result.topic_reviews[0].priority,
        TopicReviewPriority::Priority
    );

    let cross_project = repository
        .get_latest_topic_review_snapshot(other_project_id, root_batch_id)
        .await
        .unwrap();
    assert!(cross_project.is_none());

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
