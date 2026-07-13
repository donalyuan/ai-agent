use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::{routing::post, Json, Router};
use novex_api::bootstrap::{AppConfig, AppState};
use novex_api::build_app_with_state;
use novex_model::{ApiProtocol, OpenAIClient, OpenAIConfig};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tower::ServiceExt;
use uuid::Uuid;

mod support;

use support::test_database::{insert_enabled_text_model, TestDatabase};

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
        .expect("temporary topic review route database should be created");
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
    let database_name = format!("video_agent_topic_review_route_test_{}", suffix);
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
        .expect("temporary topic review route database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .expect("migrations should run for topic review route test database");

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

async fn insert_topic_batch(pool: &PgPool, project_id: Uuid, prompt: &str) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO topic_generation_batches (project_id, prompt, requested_count, status)
        VALUES ($1, $2, 2, 'succeeded')
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(prompt)
    .fetch_one(pool)
    .await
    .expect("topic generation batch fixture should be inserted")
}

async fn insert_agent_topic(pool: &PgPool, project_id: Uuid, batch_id: Uuid, title: &str) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO content_topics (
            project_id, batch_id, title, angle, target_audience, hook_points,
            content_type, score, score_reason, tags, source, status
        )
        VALUES (
            $1, $2, $3, '批次选题角度', '程序员', ARRAY['看点']::TEXT[],
            'knowledge', 90, '批次评分理由', ARRAY['AI']::TEXT[], 'agent', 'idea'
        )
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(batch_id)
    .bind(title)
    .fetch_one(pool)
    .await
    .expect("agent topic fixture should be inserted")
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

fn chat_response(content: Value) -> Json<Value> {
    Json(json!({
        "choices": [
            {
                "message": {
                    "content": content.to_string()
                }
            }
        ]
    }))
}

async fn local_topic_review_openai_base_url(
    requests: Arc<Mutex<Vec<Value>>>,
    topic_ids: Vec<Uuid>,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().route(
        "/v1/chat/completions",
        post(move |Json(payload): Json<Value>| {
            let requests = requests.clone();
            let topic_ids = topic_ids.clone();
            async move {
                requests.lock().unwrap().push(payload);
                chat_response(json!({
                    "review_summary": "优先推进能直接脚本化的工具落地选题。",
                    "topic_reviews": [
                        {
                            "topic_id": topic_ids[0],
                            "priority": "priority",
                            "reason": "账号匹配度高，脚本化路径清晰。",
                            "risk_flags": ["duplicate"],
                            "similar_topic_ids": [topic_ids[1]]
                        },
                        {
                            "topic_id": topic_ids[1],
                            "priority": "backup",
                            "reason": "可作为同主题后续补充。",
                            "risk_flags": [],
                            "similar_topic_ids": [topic_ids[0]]
                        }
                    ]
                }))
            }
        }),
    );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{address}/v1")
}

fn app_state(test_url: String, pool: PgPool, openai_base_url: String) -> AppState {
    let llm_client = OpenAIClient::new(OpenAIConfig {
        api_protocol: ApiProtocol::OpenAiChatCompletions,
        api_key: "test-key".to_string(),
        request_base_url: openai_base_url.clone(),
        upstream_model: "test-model".to_string(),
        timeout_seconds: 5,
        responses_reasoning_effort: Some("low".to_string()),
        responses_max_output_tokens: 3000,
    })
    .unwrap();
    AppState::new(
        AppConfig {
            environment: "test".to_string(),
            database_url: test_url,
            redis_url: "redis://127.0.0.1:6379/15".to_string(),
            openai_api_key: "test-key".to_string(),
            openai_base_url,
            openai_model: "test-model".to_string(),
            openai_timeout_seconds: 5,
            openai_reasoning_effort: Some("low".to_string()),
            openai_max_output_tokens: 3000,
            asset_storage_root: "/app/storage/assets".to_string(),
            asset_generation_providers: vec!["gpt-image-2".to_string(), "jimeng".to_string()],
        },
        pool,
        None,
    )
    .unwrap()
    .with_llm_client(Arc::new(llm_client))
}

#[tokio::test]
async fn topic_review_routes_create_and_read_latest_snapshot() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool, "科技博主").await;
    let root_batch_id =
        insert_topic_batch(&test_pool, project_id, "原始生成 AI 工具方向选题").await;
    let first_topic_id =
        insert_agent_topic(&test_pool, project_id, root_batch_id, "原始批次选题").await;
    let second_topic_id =
        insert_agent_topic(&test_pool, project_id, root_batch_id, "补充批次选题").await;
    let openai_requests = Arc::new(Mutex::new(Vec::new()));
    let openai_base_url = local_topic_review_openai_base_url(
        openai_requests.clone(),
        vec![first_topic_id, second_topic_id],
    )
    .await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone(), openai_base_url));
    let model_id = insert_enabled_text_model(&test_pool).await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/topic-groups/{root_batch_id}/reviews"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "model_id": model_id }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = response_json(create_response).await;
    assert_eq!(created["project_id"], project_id.to_string());
    assert_eq!(created["root_batch_id"], root_batch_id.to_string());
    assert_eq!(created["status"], "succeeded");
    assert_eq!(
        created["review_summary"],
        "优先推进能直接脚本化的工具落地选题。"
    );
    assert_eq!(
        created["result"]["topic_reviews"][0]["priority"],
        "priority"
    );
    assert!(created["source_run_id"].as_str().is_some());

    let latest_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/topic-groups/{root_batch_id}/reviews/latest"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(latest_response.status(), StatusCode::OK);
    let latest = response_json(latest_response).await;
    assert_eq!(latest["snapshot_id"], created["snapshot_id"]);
    assert_eq!(
        latest["result"]["topic_reviews"].as_array().unwrap().len(),
        2
    );

    {
        let requests = openai_requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].to_string().contains("原始生成 AI 工具方向选题"));
    }

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn topic_review_routes_return_stable_errors_for_missing_cross_project_and_empty_group() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool, "科技博主").await;
    let other_project_id = insert_project(&test_pool, "生活博主").await;
    let empty_batch_id = insert_topic_batch(&test_pool, project_id, "空主题组").await;
    let openai_requests = Arc::new(Mutex::new(Vec::new()));
    let openai_base_url = local_topic_review_openai_base_url(
        openai_requests.clone(),
        vec![Uuid::new_v4(), Uuid::new_v4()],
    )
    .await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone(), openai_base_url));
    let model_id = Uuid::new_v4();

    let missing_batch_id = Uuid::new_v4();
    let missing_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/topic-groups/{missing_batch_id}/reviews"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "model_id": model_id }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_response.status(), StatusCode::NOT_FOUND);

    let cross_project_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/topic-groups/{empty_batch_id}/reviews?project_id={other_project_id}"
                ))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "model_id": model_id }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cross_project_response.status(), StatusCode::NOT_FOUND);

    let empty_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/topic-groups/{empty_batch_id}/reviews"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "model_id": model_id }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(empty_response.status(), StatusCode::BAD_REQUEST);
    let empty_body = response_json(empty_response).await;
    assert_eq!(empty_body["error"], "主题组没有可评审选题");
    assert!(openai_requests.lock().unwrap().is_empty());

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
