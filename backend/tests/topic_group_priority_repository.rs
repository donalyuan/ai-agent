use novex_api::agents::models::{
    ContentTopicSource, TopicGenerationBatchStatus, TopicGroupReviewFreshness,
    TopicGroupScriptPriorityStatus, TopicGroupSort, TopicReviewItem, TopicReviewPriority,
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
        .expect("temporary topic group priority database should be created");
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
    let database_name = format!("video_agent_topic_group_priority_repo_test_{}", suffix);
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
        .expect("temporary topic group priority database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .expect("migrations should run for topic group priority repository test database");

    (admin_pool, test_pool, database_name)
}

async fn insert_project(pool: &PgPool, name: &str) -> Uuid {
    let repository = PostgresProjectRepository::new(pool.clone());
    repository
        .create_project(novex_api::repositories::CreateProjectInput {
            name: name.to_string(),
            positioning: "AI 工具和内容生产效率".to_string(),
            description: "面向内容运营负责人的科技知识账号".to_string(),
        })
        .await
        .unwrap()
        .id
}

async fn insert_batch(
    repository: &PostgresTopicRepository,
    project_id: Uuid,
    prompt: &str,
    supplement_of_batch_id: Option<Uuid>,
) -> Uuid {
    repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: None,
            supplement_of_batch_id,
            prompt: prompt.to_string(),
            requested_count: 3,
            status: TopicGenerationBatchStatus::Succeeded,
            error_message: None,
            metadata: json!({}),
        })
        .await
        .unwrap()
        .id
}

async fn insert_topic(
    repository: &PostgresTopicRepository,
    project_id: Uuid,
    batch_id: Uuid,
    title: &str,
    score: f64,
) -> Uuid {
    repository
        .create_topic(CreateContentTopicInput {
            project_id,
            batch_id: Some(batch_id),
            title: title.to_string(),
            angle: "用真实运营场景解释工具价值".to_string(),
            target_audience: "内容运营负责人".to_string(),
            hook_points: vec!["开头用具体痛点".to_string()],
            content_type: "knowledge".to_string(),
            score: Some(score),
            score_reason: "贴合账号定位且容易脚本化".to_string(),
            tags: vec!["AI工具".to_string()],
            source: ContentTopicSource::Agent,
            metadata: json!({}),
        })
        .await
        .unwrap()
        .id
}

async fn create_review(
    repository: &PostgresTopicRepository,
    project_id: Uuid,
    root_batch_id: Uuid,
    topic_reviews: Vec<TopicReviewItem>,
) {
    repository
        .create_topic_review_snapshot(CreateTopicReviewSnapshotInput {
            project_id,
            root_batch_id,
            source_run_id: None,
            status: TopicReviewSnapshotStatus::Succeeded,
            review_summary: "优先推进可直接脚本化的选题".to_string(),
            result: TopicReviewResult { topic_reviews },
            error_message: None,
            metadata: json!({}),
        })
        .await
        .unwrap();
}

fn review_item(
    topic_id: Uuid,
    priority: TopicReviewPriority,
    risk_flags: Vec<TopicReviewRiskFlag>,
) -> TopicReviewItem {
    TopicReviewItem {
        topic_id,
        priority,
        reason: "账号匹配度高，脚本化路径清晰。".to_string(),
        risk_flags,
        similar_topic_ids: Vec::new(),
    }
}

#[tokio::test]
async fn topic_group_summaries_rank_fresh_reviewed_groups_for_script_production() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool, "科技博主").await;
    let repository = PostgresTopicRepository::new(test_pool.clone());

    let ready_root = insert_batch(&repository, project_id, "AI 工具实战", None).await;
    let ready_topic_1 = insert_topic(
        &repository,
        project_id,
        ready_root,
        "低成本 AI 工具栈",
        92.0,
    )
    .await;
    let ready_topic_2 =
        insert_topic(&repository, project_id, ready_root, "AI 复盘选题会", 88.0).await;
    let duplicate_topic =
        insert_topic(&repository, project_id, ready_root, "重复工具清单", 91.0).await;
    let backup_topic =
        insert_topic(&repository, project_id, ready_root, "自动化模板判断", 81.0).await;
    let reject_topic =
        insert_topic(&repository, project_id, ready_root, "宽泛效率故事", 70.0).await;
    create_review(
        &repository,
        project_id,
        ready_root,
        vec![
            review_item(ready_topic_1, TopicReviewPriority::Priority, vec![]),
            review_item(ready_topic_2, TopicReviewPriority::Priority, vec![]),
            review_item(
                duplicate_topic,
                TopicReviewPriority::Priority,
                vec![TopicReviewRiskFlag::Duplicate],
            ),
            review_item(backup_topic, TopicReviewPriority::Backup, vec![]),
            review_item(
                reject_topic,
                TopicReviewPriority::Reject,
                vec![TopicReviewRiskFlag::HardToScript],
            ),
        ],
    )
    .await;

    let low_root = insert_batch(&repository, project_id, "工具避坑清单", None).await;
    let low_topic = insert_topic(&repository, project_id, low_root, "工具试用避坑", 60.0).await;
    create_review(
        &repository,
        project_id,
        low_root,
        vec![review_item(low_topic, TopicReviewPriority::Backup, vec![])],
    )
    .await;

    let summaries = repository
        .list_topic_group_summaries(project_id, TopicGroupSort::ScriptPriority, 20)
        .await
        .unwrap();

    assert_eq!(summaries[0].root_batch_id, ready_root);
    assert_eq!(
        summaries[0].script_priority.status,
        TopicGroupScriptPriorityStatus::ReadyForScript
    );
    assert_eq!(summaries[0].script_priority.score, Some(69));
    assert_eq!(
        summaries[0].script_priority.metrics.ready_candidate_count,
        2
    );
    assert_eq!(summaries[0].script_priority.metrics.priority_count, 3);
    assert_eq!(
        summaries[0].script_priority.metrics.high_score_topic_count,
        4
    );
    assert_eq!(summaries[0].script_priority.metrics.backup_count, 1);
    assert_eq!(summaries[0].script_priority.metrics.reject_count, 1);
    assert_eq!(summaries[0].script_priority.metrics.duplicate_count, 1);
    assert_eq!(summaries[0].script_priority.metrics.hard_to_script_count, 1);
    assert_eq!(
        summaries[0].script_priority.recommended_topic_ids,
        vec![ready_topic_1, ready_topic_2]
    );
    assert_eq!(summaries[1].root_batch_id, low_root);
    assert_eq!(
        summaries[1].script_priority.status,
        TopicGroupScriptPriorityStatus::NeedsSupplement
    );

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn topic_group_summaries_mark_missing_and_stale_reviews_as_needs_review() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool, "科技博主").await;
    let repository = PostgresTopicRepository::new(test_pool.clone());

    let stale_root = insert_batch(&repository, project_id, "小团队落地案例", None).await;
    let reviewed_topic =
        insert_topic(&repository, project_id, stale_root, "已评审旧选题", 90.0).await;
    let _new_topic =
        insert_topic(&repository, project_id, stale_root, "评审后新增选题", 91.0).await;
    create_review(
        &repository,
        project_id,
        stale_root,
        vec![review_item(
            reviewed_topic,
            TopicReviewPriority::Priority,
            vec![],
        )],
    )
    .await;

    let missing_root = insert_batch(&repository, project_id, "未评审主题", None).await;
    insert_topic(
        &repository,
        project_id,
        missing_root,
        "还没有评审的选题",
        93.0,
    )
    .await;

    let summaries = repository
        .list_topic_group_summaries(project_id, TopicGroupSort::ScriptPriority, 20)
        .await
        .unwrap();
    let stale = summaries
        .iter()
        .find(|summary| summary.root_batch_id == stale_root)
        .unwrap();
    let missing = summaries
        .iter()
        .find(|summary| summary.root_batch_id == missing_root)
        .unwrap();

    assert_eq!(stale.review_freshness, TopicGroupReviewFreshness::Stale);
    assert_eq!(
        stale.script_priority.status,
        TopicGroupScriptPriorityStatus::NeedsReview
    );
    assert_eq!(stale.script_priority.score, None);
    assert_eq!(missing.review_freshness, TopicGroupReviewFreshness::Missing);
    assert_eq!(
        missing.script_priority.status,
        TopicGroupScriptPriorityStatus::NeedsReview
    );
    assert_eq!(missing.script_priority.score, None);

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn topic_group_summaries_fold_supplements_into_root_batch() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool, "科技博主").await;
    let repository = PostgresTopicRepository::new(test_pool.clone());

    let root_batch = insert_batch(&repository, project_id, "原始 AI 工具选题", None).await;
    let supplement_batch = insert_batch(
        &repository,
        project_id,
        "补充 AI 工具选题",
        Some(root_batch),
    )
    .await;
    let root_topic = insert_topic(&repository, project_id, root_batch, "原始批次候选", 88.0).await;
    let supplement_topic = insert_topic(
        &repository,
        project_id,
        supplement_batch,
        "补充批次候选",
        91.0,
    )
    .await;
    create_review(
        &repository,
        project_id,
        root_batch,
        vec![
            review_item(root_topic, TopicReviewPriority::Priority, vec![]),
            review_item(supplement_topic, TopicReviewPriority::Priority, vec![]),
        ],
    )
    .await;

    let summaries = repository
        .list_topic_group_summaries(project_id, TopicGroupSort::CreatedAt, 20)
        .await
        .unwrap();

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].root_batch_id, root_batch);
    assert_eq!(summaries[0].topic_count, 2);
    assert_eq!(summaries[0].supplement_batch_count, 1);
    assert_eq!(
        summaries[0].script_priority.recommended_topic_ids,
        vec![supplement_topic, root_topic]
    );

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn topic_group_summaries_count_only_supplements_with_visible_topics() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool, "科技博主").await;
    let repository = PostgresTopicRepository::new(test_pool.clone());

    let root_batch = insert_batch(&repository, project_id, "原始 AI 工具选题", None).await;
    let visible_supplement = insert_batch(
        &repository,
        project_id,
        "有可见选题的补充批次",
        Some(root_batch),
    )
    .await;
    let hidden_supplement = insert_batch(
        &repository,
        project_id,
        "选题已移除的补充批次",
        Some(root_batch),
    )
    .await;

    insert_topic(&repository, project_id, root_batch, "原始批次候选", 88.0).await;
    insert_topic(
        &repository,
        project_id,
        visible_supplement,
        "可见补充候选",
        91.0,
    )
    .await;
    let hidden_topic = insert_topic(
        &repository,
        project_id,
        hidden_supplement,
        "已移除补充候选",
        86.0,
    )
    .await;
    repository.soft_delete_topic(hidden_topic).await.unwrap();

    let summaries = repository
        .list_topic_group_summaries(project_id, TopicGroupSort::CreatedAt, 20)
        .await
        .unwrap();

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].topic_count, 2);
    assert_eq!(summaries[0].supplement_batch_count, 1);

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
