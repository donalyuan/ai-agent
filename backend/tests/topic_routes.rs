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
        .expect("temporary topic route database should be created");
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
    let database_name = format!("video_agent_topic_route_test_{}", suffix);
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
        .expect("temporary topic route database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .expect("migrations should run for topic route test database");

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
    requested_count: i32,
) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO topic_generation_batches (project_id, prompt, requested_count, status)
        VALUES ($1, $2, $3, 'succeeded')
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(prompt)
    .bind(requested_count)
    .fetch_one(pool)
    .await
    .expect("topic generation batch fixture should be inserted")
}

async fn insert_failed_topic_batch(
    pool: &PgPool,
    project_id: Uuid,
    prompt: &str,
    requested_count: i32,
) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO topic_generation_batches (project_id, prompt, requested_count, status, error_message)
        VALUES ($1, $2, $3, 'failed', 'invalid topic JSON')
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(prompt)
    .bind(requested_count)
    .fetch_one(pool)
    .await
    .expect("failed topic generation batch fixture should be inserted")
}

async fn insert_agent_topic(
    pool: &PgPool,
    project_id: Uuid,
    batch_id: Uuid,
    title: &str,
    score: i32,
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

async fn response_json(response: axum::response::Response) -> Value {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

fn test_app(test_url: String, pool: PgPool) -> axum::Router {
    build_app_with_state(
        AppState::new(
            AppConfig {
                environment: "test".to_string(),
                database_url: test_url,
                redis_url: "redis://127.0.0.1:6379/15".to_string(),
                openai_api_key: "".to_string(),
                openai_base_url: "https://example.invalid/v1".to_string(),
                openai_model: "test-model".to_string(),
                openai_timeout_seconds: 5,
                openai_reasoning_effort: Some("low".to_string()),
                openai_max_output_tokens: 3000,
            },
            pool,
            None,
        )
        .unwrap(),
    )
}

fn valid_topic_payload() -> Value {
    json!({
        "title": "AI 工具正在重塑内容团队",
        "angle": "从流程协同角度切入",
        "target_audience": "内容运营负责人",
        "hook_points": ["提效", "降本"],
        "content_type": "knowledge",
        "score": 88,
        "score_reason": "贴合账号定位",
        "tags": ["AI工具", "内容运营"]
    })
}

fn topic_payload_with_score(title: &str, score: i32) -> Value {
    let mut payload = valid_topic_payload();
    payload["title"] = json!(title);
    payload["score"] = json!(score);
    payload
}

#[tokio::test]
async fn topic_routes_list_generation_batches_with_topic_counts() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool, "科技博主").await;
    let other_project_id = insert_project(&test_pool, "生活博主").await;
    let previous_batch =
        insert_topic_batch(&test_pool, project_id, "上一批 AI 内容流水线选题", 5).await;
    let latest_batch = insert_topic_batch(&test_pool, project_id, "最新一批 AI 工具选题", 5).await;
    let failed_batch =
        insert_failed_topic_batch(&test_pool, project_id, "失败的 AI 选题生成", 5).await;
    let other_batch = insert_topic_batch(&test_pool, other_project_id, "其他项目选题", 5).await;
    insert_agent_topic(&test_pool, project_id, previous_batch, "历史批次选题", 82).await;
    insert_agent_topic(&test_pool, project_id, latest_batch, "最新批次选题 1", 94).await;
    insert_agent_topic(&test_pool, project_id, latest_batch, "最新批次选题 2", 91).await;
    insert_agent_topic(
        &test_pool,
        other_project_id,
        other_batch,
        "其他项目选题",
        80,
    )
    .await;
    let app = test_app(test_url, test_pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/projects/{project_id}/topic-generation-batches"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let listed = response_json(response).await;
    let batches = listed["batches"].as_array().unwrap();
    assert_eq!(batches.len(), 2);
    assert!(!batches
        .iter()
        .any(|batch| batch["batch_id"] == failed_batch.to_string()));
    assert_eq!(batches[0]["batch_id"], latest_batch.to_string());
    assert_eq!(batches[0]["prompt"], "最新一批 AI 工具选题");
    assert_eq!(batches[0]["topic_count"], 2);
    assert_eq!(batches[1]["batch_id"], previous_batch.to_string());
    assert_eq!(batches[1]["topic_count"], 1);

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn topic_routes_support_manual_create_query_update_status_and_prepare_script() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool, "科技博主").await;
    let other_project_id = insert_project(&test_pool, "生活博主").await;
    let app = test_app(test_url, test_pool.clone());

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/topics"))
                .header("content-type", "application/json")
                .body(Body::from(valid_topic_payload().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = response_json(create_response).await;
    let topic_id = created["topic_id"].as_str().unwrap().to_string();
    assert_eq!(created["status"], "idea");
    assert_eq!(created["source"], "manual");

    let _other_topic_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{other_project_id}/topics"))
                .header("content-type", "application/json")
                .body(Body::from(valid_topic_payload().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let approve_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/topics/{topic_id}/status"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"status": "approved"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(approve_response.status(), StatusCode::OK);
    let approved = response_json(approve_response).await;
    assert_eq!(approved["status"], "approved");

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/topics/{topic_id}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "AI 工具如何重塑内容团队",
                        "angle": "强调协作流程",
                        "target_audience": "内容负责人",
                        "hook_points": ["流程重构"],
                        "content_type": "knowledge",
                        "score": 91,
                        "score_reason": "标题更具体",
                        "tags": ["AI工具"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);
    let updated = response_json(update_response).await;
    assert_eq!(updated["title"], "AI 工具如何重塑内容团队");
    assert_eq!(updated["score"], 91.0);

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/projects/{project_id}/topics?status=approved&source=manual"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let listed = response_json(list_response).await;
    assert_eq!(listed["topics"].as_array().unwrap().len(), 1);
    assert_eq!(listed["topics"][0]["topic_id"], topic_id);
    assert_eq!(listed["stats"]["total"], 1);
    assert_eq!(listed["stats"]["approved"], 1);

    let prepare_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/topics/{topic_id}/prepare-script"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "style": "story",
                        "scene_count": 5
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(prepare_response.status(), StatusCode::OK);
    let prepared = response_json(prepare_response).await;
    assert_eq!(prepared["topic"]["topic_id"], topic_id);
    assert_eq!(prepared["script_request"]["style"], "story");
    assert_eq!(prepared["script_request"]["scene_count"], 5);

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn topic_routes_list_topics_by_score_descending() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool, "科技博主").await;
    let app = test_app(test_url, test_pool.clone());

    let high_score_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/topics"))
                .header("content-type", "application/json")
                .body(Body::from(
                    topic_payload_with_score("高分选题", 95).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(high_score_response.status(), StatusCode::CREATED);

    let low_score_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/topics"))
                .header("content-type", "application/json")
                .body(Body::from(
                    topic_payload_with_score("低分选题", 72).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(low_score_response.status(), StatusCode::CREATED);

    let list_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/projects/{project_id}/topics"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let listed = response_json(list_response).await;
    let topics = listed["topics"].as_array().unwrap();
    assert_eq!(topics[0]["title"], "高分选题");
    assert_eq!(topics[1]["title"], "低分选题");

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn topic_routes_soft_delete_hides_topic_and_rejects_referenced_topic() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool, "科技博主").await;
    let app = test_app(test_url, test_pool.clone());

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/topics"))
                .header("content-type", "application/json")
                .body(Body::from(valid_topic_payload().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = response_json(create_response).await;
    let removable_topic_id = created["topic_id"].as_str().unwrap().to_string();

    let approve_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/topics/{removable_topic_id}/status"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"status": "approved"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(approve_response.status(), StatusCode::OK);

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/topics/{removable_topic_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::OK);
    let deleted = response_json(delete_response).await;
    assert_eq!(deleted["topic_id"], removable_topic_id);
    assert!(deleted["deleted_at"].as_str().is_some());

    let duplicate_delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/topics/{removable_topic_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(duplicate_delete_response.status(), StatusCode::BAD_REQUEST);

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/projects/{project_id}/topics"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let listed = response_json(list_response).await;
    assert!(listed["topics"].as_array().unwrap().is_empty());
    assert_eq!(listed["stats"]["total"], 0);

    let prepare_deleted_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/topics/{removable_topic_id}/prepare-script"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"style": "knowledge", "scene_count": 5}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(prepare_deleted_response.status(), StatusCode::BAD_REQUEST);

    let referenced_topic_id = insert_agent_topic(
        &test_pool,
        project_id,
        insert_topic_batch(&test_pool, project_id, "引用选题批次", 1).await,
        "已生成脚本选题",
        93,
    )
    .await;
    insert_script_for_topic(&test_pool, project_id, referenced_topic_id).await;

    let delete_referenced_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/topics/{referenced_topic_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_referenced_response.status(), StatusCode::BAD_REQUEST);

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn topic_routes_return_errors_for_missing_project_invalid_status_and_archived_prepare() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool, "科技博主").await;
    let app = test_app(test_url, test_pool.clone());
    let missing_project_id = Uuid::new_v4();

    let missing_project_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{missing_project_id}/topics"))
                .header("content-type", "application/json")
                .body(Body::from(valid_topic_payload().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_project_response.status(), StatusCode::NOT_FOUND);

    let missing_topic_id = Uuid::new_v4();
    let missing_topic_status_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/topics/{missing_topic_id}/status"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"status": "approved"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        missing_topic_status_response.status(),
        StatusCode::NOT_FOUND
    );

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/topics"))
                .header("content-type", "application/json")
                .body(Body::from(valid_topic_payload().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let created = response_json(create_response).await;
    let topic_id = created["topic_id"].as_str().unwrap().to_string();

    let invalid_status_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/topics/{topic_id}/status"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"status": "scripted"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_status_response.status(), StatusCode::BAD_REQUEST);

    let archived_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/topics/{topic_id}/status"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"status": "archived"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(archived_response.status(), StatusCode::OK);

    let prepare_archived_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/topics/{topic_id}/prepare-script"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"style": "knowledge", "scene_count": 5}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(prepare_archived_response.status(), StatusCode::BAD_REQUEST);

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
