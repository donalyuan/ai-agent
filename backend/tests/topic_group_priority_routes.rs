use axum::body::Body;
use axum::http::{Request, StatusCode};
use novex_api::{build_app_with_state, AppConfig, AppState};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use tower::ServiceExt;
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
        .expect("temporary topic group priority route database should be created");
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

async fn migrated_pool() -> (PgPool, PgPool, TestDatabase, String) {
    let base_url = database_url();
    let suffix = Uuid::new_v4().simple().to_string();
    let database_name = format!("video_agent_topic_group_priority_route_test_{}", suffix);
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
        .expect("temporary topic group priority route database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .expect("migrations should run for topic group priority route test database");

    (admin_pool, test_pool, database_name, test_url)
}

async fn insert_project(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO projects (name, positioning, description)
        VALUES ($1, 'AI 工具和内容生产效率', '面向内容运营负责人的科技知识账号')
        RETURNING id
        "#,
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .expect("project fixture should be inserted")
}

async fn insert_topic_batch(
    pool: &PgPool,
    project_id: Uuid,
    prompt: &str,
    offset_seconds: i32,
) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO topic_generation_batches (
            project_id, prompt, requested_count, status, created_at, updated_at
        )
        VALUES ($1, $2, 3, 'succeeded', NOW() + ($3 * INTERVAL '1 second'), NOW() + ($3 * INTERVAL '1 second'))
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(prompt)
    .bind(offset_seconds)
    .fetch_one(pool)
    .await
    .expect("topic generation batch fixture should be inserted")
}

async fn insert_agent_topic(
    pool: &PgPool,
    project_id: Uuid,
    batch_id: Uuid,
    title: &str,
    score: f64,
) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO content_topics (
            project_id, batch_id, title, angle, target_audience, hook_points,
            content_type, score, score_reason, tags, source, status
        )
        VALUES (
            $1, $2, $3, '批次选题角度', '程序员', ARRAY['看点']::TEXT[],
            'knowledge', $4, '批次评分理由', ARRAY['AI']::TEXT[], 'agent', 'idea'
        )
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(batch_id)
    .bind(title)
    .bind(score)
    .fetch_one(pool)
    .await
    .expect("agent topic fixture should be inserted")
}

async fn insert_review_snapshot(
    pool: &PgPool,
    project_id: Uuid,
    root_batch_id: Uuid,
    topic_reviews: Value,
) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO topic_review_snapshots (
            project_id, root_batch_id, status, review_summary, result
        )
        VALUES (
            $1, $2, 'succeeded', '优先推进能直接脚本化的工具落地选题。', $3
        )
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(root_batch_id)
    .bind(json!({ "topic_reviews": topic_reviews }))
    .fetch_one(pool)
    .await
    .expect("topic review snapshot fixture should be inserted")
}

fn review_item(topic_id: Uuid, priority: &str, risk_flags: Vec<&str>) -> Value {
    json!({
        "topic_id": topic_id,
        "priority": priority,
        "reason": "账号匹配度高，脚本化路径清晰。",
        "risk_flags": risk_flags,
        "similar_topic_ids": []
    })
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

fn app_state(test_url: String, pool: PgPool) -> AppState {
    AppState::new(
        AppConfig {
            environment: "test".to_string(),
            database_url: test_url,
            redis_url: "redis://127.0.0.1:6379/15".to_string(),
            openai_api_key: "test-key".to_string(),
            openai_base_url: "http://127.0.0.1:1/v1".to_string(),
            openai_model: "test-model".to_string(),
            openai_timeout_seconds: 5,
            openai_reasoning_effort: Some("low".to_string()),
            openai_max_output_tokens: 3000,
        },
        pool,
        None,
    )
    .unwrap()
}

#[tokio::test]
async fn topic_group_priority_route_returns_ranked_groups_for_project() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool, "科技博主").await;
    let ready_root = insert_topic_batch(&test_pool, project_id, "AI 工具实战", 0).await;
    let ready_topic =
        insert_agent_topic(&test_pool, project_id, ready_root, "低成本 AI 工具栈", 92.0).await;
    let risky_topic =
        insert_agent_topic(&test_pool, project_id, ready_root, "重复工具清单", 80.0).await;
    let ready_snapshot_id = insert_review_snapshot(
        &test_pool,
        project_id,
        ready_root,
        json!([
            review_item(ready_topic, "priority", vec![]),
            review_item(risky_topic, "reject", vec!["duplicate"])
        ]),
    )
    .await;

    let missing_root = insert_topic_batch(&test_pool, project_id, "未评审主题", 20).await;
    insert_agent_topic(&test_pool, project_id, missing_root, "未评审候选", 94.0).await;

    let app = build_app_with_state(app_state(test_url, test_pool.clone()));
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/projects/{project_id}/topic-groups"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    let groups = body["topic_groups"].as_array().unwrap();
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0]["root_batch_id"], ready_root.to_string());
    assert_eq!(
        groups[0]["latest_review_snapshot_id"],
        ready_snapshot_id.to_string()
    );
    assert_eq!(groups[0]["review_freshness"], "fresh");
    assert_eq!(groups[0]["script_priority"]["status"], "ready_for_script");
    assert_eq!(
        groups[0]["script_priority"]["recommended_topic_ids"][0],
        ready_topic.to_string()
    );
    assert_eq!(groups[1]["root_batch_id"], missing_root.to_string());
    assert_eq!(groups[1]["script_priority"]["status"], "needs_review");
    assert!(groups[1]["script_priority"]["score"].is_null());

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn topic_group_priority_route_supports_created_at_sort_and_project_isolation() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool, "科技博主").await;
    let other_project_id = insert_project(&test_pool, "生活博主").await;

    let older_root = insert_topic_batch(&test_pool, project_id, "较早主题", 0).await;
    let older_topic =
        insert_agent_topic(&test_pool, project_id, older_root, "较早候选", 85.0).await;
    insert_review_snapshot(
        &test_pool,
        project_id,
        older_root,
        json!([review_item(older_topic, "priority", vec![])]),
    )
    .await;

    let newer_root = insert_topic_batch(&test_pool, project_id, "较新主题", 100).await;
    let newer_topic =
        insert_agent_topic(&test_pool, project_id, newer_root, "较新候选", 60.0).await;
    insert_review_snapshot(
        &test_pool,
        project_id,
        newer_root,
        json!([review_item(newer_topic, "backup", vec![])]),
    )
    .await;

    let other_root = insert_topic_batch(&test_pool, other_project_id, "其他项目主题", 200).await;
    let other_topic = insert_agent_topic(
        &test_pool,
        other_project_id,
        other_root,
        "其他项目候选",
        99.0,
    )
    .await;
    insert_review_snapshot(
        &test_pool,
        other_project_id,
        other_root,
        json!([review_item(other_topic, "priority", vec![])]),
    )
    .await;

    let app = build_app_with_state(app_state(test_url, test_pool.clone()));
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/projects/{project_id}/topic-groups?sort=created_at"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    let groups = body["topic_groups"].as_array().unwrap();
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0]["root_batch_id"], newer_root.to_string());
    assert_eq!(groups[1]["root_batch_id"], older_root.to_string());
    assert!(!body.to_string().contains(&other_root.to_string()));

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
