use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use novex_api::agents::{LLMClient, LLMError};
use novex_api::{build_app_with_state, AppConfig, AppState};
use novex_model::LLMPrompt;
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::sync::{Arc, Mutex};
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
        .expect("temporary project route database should be created");
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
    let database_name = format!("video_agent_project_route_test_{}", suffix);
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
        .expect("temporary project route database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .expect("migrations should run for project route test database");

    (admin_pool, test_pool, database_name, test_url)
}

fn app_state(test_url: String, pool: PgPool) -> AppState {
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
    .unwrap()
}

struct RecordingLLMClient {
    response: Mutex<Result<String, LLMError>>,
    prompts: Mutex<Vec<LLMPrompt>>,
}

impl RecordingLLMClient {
    fn returning(response: Value) -> Self {
        Self {
            response: Mutex::new(Ok(response.to_string())),
            prompts: Mutex::new(Vec::new()),
        }
    }

    fn returning_raw(response: &str) -> Self {
        Self {
            response: Mutex::new(Ok(response.to_string())),
            prompts: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl LLMClient for RecordingLLMClient {
    async fn generate_script(&self, prompt: LLMPrompt) -> Result<String, LLMError> {
        self.prompts.lock().unwrap().push(prompt);
        self.response.lock().unwrap().clone()
    }
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn project_routes_create_and_list_projects() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "科技博主",
                        "positioning": "科技知识账号",
                        "description": "面向程序员的知识短视频",
                        "strategy_profile": {
                            "target_audience": "内容运营负责人",
                            "content_pillars": ["AI 工具", "内容生产"],
                            "tone_style": "直接清晰",
                            "forbidden_topics": ["夸大收益"],
                            "reference_accounts": ["参考账号A"],
                            "topic_preferences": "优先教程和案例"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = response_json(create_response).await;
    assert_eq!(created["name"], "科技博主");
    assert_eq!(created["positioning"], "科技知识账号");
    assert_eq!(created["description"], "面向程序员的知识短视频");
    assert_eq!(
        created["strategy_profile"]["target_audience"],
        "内容运营负责人"
    );
    assert_eq!(
        created["strategy_profile"]["content_pillars"],
        json!(["AI 工具", "内容生产"])
    );
    assert_eq!(created["status"], "active");
    assert!(created["project_id"].as_str().unwrap().len() > 20);

    let list_response = app
        .oneshot(
            Request::builder()
                .uri("/api/projects")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let listed = response_json(list_response).await;
    assert_eq!(listed["projects"].as_array().unwrap().len(), 1);
    assert_eq!(listed["projects"][0]["project_id"], created["project_id"]);
    assert_eq!(
        listed["projects"][0]["strategy_profile"]["tone_style"],
        "直接清晰"
    );

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn project_routes_update_strategy_profile_for_one_project() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));

    let project_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO projects (name, positioning, description)
        VALUES ('旧账号', '旧定位', '旧描述')
        RETURNING id
        "#,
    )
    .fetch_one(&test_pool)
    .await
    .unwrap();
    let other_project_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO projects (name, positioning, description)
        VALUES ('其他账号', '其他定位', '其他描述')
        RETURNING id
        "#,
    )
    .fetch_one(&test_pool)
    .await
    .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/projects/{project_id}/strategy-profile"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "AI 工具账号",
                        "positioning": "AI 工具教程账号",
                        "description": "面向内容运营负责人的短视频",
                        "strategy_profile": {
                            "target_audience": "内容运营负责人",
                            "content_pillars": ["AI 工具", "内容生产", "AI 工具"],
                            "tone_style": "直接清晰",
                            "forbidden_topics": ["夸大收益"],
                            "reference_accounts": ["参考账号A"],
                            "topic_preferences": "优先教程和案例"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let updated = response_json(response).await;
    assert_eq!(updated["name"], "AI 工具账号");
    assert_eq!(
        updated["strategy_profile"]["content_pillars"],
        json!(["AI 工具", "内容生产"])
    );

    let other_name = sqlx::query_scalar::<_, String>("SELECT name FROM projects WHERE id = $1")
        .bind(other_project_id)
        .fetch_one(&test_pool)
        .await
        .unwrap();
    assert_eq!(other_name, "其他账号");

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn project_routes_reject_invalid_strategy_profile_without_partial_update() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));
    let project_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO projects (name, positioning, description, strategy_profile)
        VALUES ('旧账号', '旧定位', '旧描述', '{"target_audience":"旧受众"}'::jsonb)
        RETURNING id
        "#,
    )
    .fetch_one(&test_pool)
    .await
    .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/projects/{project_id}/strategy-profile"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "",
                        "positioning": "新定位",
                        "description": "新描述",
                        "strategy_profile": {
                            "content_pillars": (0..21).map(|index| format!("支柱{index}")).collect::<Vec<_>>()
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert_eq!(body["error"], "账号名称不能为空");
    let stored = sqlx::query_as::<_, (String, Value)>(
        "SELECT name, strategy_profile FROM projects WHERE id = $1",
    )
    .bind(project_id)
    .fetch_one(&test_pool)
    .await
    .unwrap();
    assert_eq!(stored.0, "旧账号");
    assert_eq!(stored.1["target_audience"], "旧受众");

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn project_routes_update_strategy_profile_missing_project_returns_not_found() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));
    let missing_project_id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/projects/{missing_project_id}/strategy-profile"
                ))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "AI 工具账号",
                        "positioning": "AI 工具教程账号",
                        "description": "面向内容运营负责人的短视频",
                        "strategy_profile": {
                            "target_audience": "内容运营负责人"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = response_json(response).await;
    assert_eq!(body["error"], "项目不存在");
    assert_eq!(body["project_id"], missing_project_id.to_string());

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn project_routes_update_strategy_profile_storage_failure_returns_error() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));
    test_pool.close().await;
    let project_id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/projects/{project_id}/strategy-profile"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "AI 工具账号",
                        "positioning": "AI 工具教程账号",
                        "description": "面向内容运营负责人的短视频",
                        "strategy_profile": {
                            "target_audience": "内容运营负责人"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = response_json(response).await;
    assert_eq!(body["error"], "项目存储失败");

    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn project_routes_generate_strategy_profile_draft_without_saving() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO projects (name, positioning, description)
        VALUES ('AI 工具账号', 'AI 工具教程账号', '面向内容运营负责人的短视频')
        RETURNING id
        "#,
    )
    .fetch_one(&test_pool)
    .await
    .unwrap();
    let llm_client = Arc::new(RecordingLLMClient::returning(json!({
        "draft": {
            "target_audience": "内容运营负责人",
            "content_pillars": ["AI 工具", "内容生产"],
            "tone_style": "直接清晰",
            "forbidden_topics": ["夸大收益"],
            "reference_accounts": ["参考账号A"],
            "topic_preferences": "优先教程和案例"
        },
        "draft_summary": "草稿聚焦 AI 工具教程和内容生产案例。"
    })));
    let app = build_app_with_state(
        app_state(test_url, test_pool.clone()).with_llm_client(llm_client.clone()),
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/strategy-profile/draft"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "direction_notes": "面向内容运营负责人，不要夸大收益。"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["draft"]["target_audience"], "内容运营负责人");
    assert_eq!(
        body["draft_summary"],
        "草稿聚焦 AI 工具教程和内容生产案例。"
    );

    let stored_profile =
        sqlx::query_scalar::<_, Value>("SELECT strategy_profile FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_one(&test_pool)
            .await
            .unwrap();
    assert_eq!(stored_profile, json!({}));
    {
        let prompts = llm_client.prompts.lock().unwrap();
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].max_output_tokens, Some(1_200));
        assert!(prompts[0].user.contains("AI 工具账号"));
        assert!(prompts[0]
            .user
            .contains("面向内容运营负责人，不要夸大收益。"));
    }

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn project_routes_generate_strategy_profile_draft_missing_project_returns_not_found() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));
    let missing_project_id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/projects/{missing_project_id}/strategy-profile/draft"
                ))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "direction_notes": "补充方向" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = response_json(response).await;
    assert_eq!(body["error"], "项目不存在");
    assert_eq!(body["project_id"], missing_project_id.to_string());

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn project_routes_reject_invalid_strategy_profile_draft_without_saving() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO projects (name, positioning, description, strategy_profile)
        VALUES ('AI 工具账号', 'AI 工具教程账号', '面向内容运营负责人的短视频', '{"target_audience":"旧受众"}'::jsonb)
        RETURNING id
        "#,
    )
    .fetch_one(&test_pool)
    .await
    .unwrap();
    let llm_client = Arc::new(RecordingLLMClient::returning_raw("{}"));
    let app =
        build_app_with_state(app_state(test_url, test_pool.clone()).with_llm_client(llm_client));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/strategy-profile/draft"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "direction_notes": "补充方向" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = response_json(response).await;
    assert_eq!(body["error"], "AI 策略草稿输出无效");
    let stored_profile =
        sqlx::query_scalar::<_, Value>("SELECT strategy_profile FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_one(&test_pool)
            .await
            .unwrap();
    assert_eq!(stored_profile["target_audience"], "旧受众");

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn project_routes_reject_invalid_project_name() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "",
                        "positioning": "科技知识账号",
                        "description": "面向程序员的知识短视频"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert_eq!(body["error"], "项目名称不能为空");

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
