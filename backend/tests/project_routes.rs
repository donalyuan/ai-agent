use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use novex_api::agents::{LLMClient, LLMError};
use novex_api::bootstrap::{AppConfig, AppState};
use novex_api::build_app_with_state;
use novex_model::LLMPrompt;
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
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
            asset_storage_root: "/app/storage/assets".to_string(),
            asset_generation_providers: vec!["gpt-image-2".to_string(), "jimeng".to_string()],
        },
        pool,
        None,
    )
    .unwrap()
}

struct RecordingLLMClient {
    responses: Mutex<VecDeque<Result<String, LLMError>>>,
    prompts: Mutex<Vec<LLMPrompt>>,
}

impl RecordingLLMClient {
    fn returning(response: Value) -> Self {
        Self {
            responses: Mutex::new(VecDeque::from([Ok(response.to_string())])),
            prompts: Mutex::new(Vec::new()),
        }
    }

    fn returning_raw(response: &str) -> Self {
        Self {
            responses: Mutex::new(VecDeque::from([Ok(response.to_string())])),
            prompts: Mutex::new(Vec::new()),
        }
    }

    fn returning_sequence(responses: Vec<Result<String, LLMError>>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            prompts: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl LLMClient for RecordingLLMClient {
    async fn generate_script(&self, prompt: LLMPrompt) -> Result<String, LLMError> {
        self.prompts.lock().unwrap().push(prompt);
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("fixture must provide one response per model call")
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
    let model_id = insert_enabled_text_model(&test_pool).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/strategy-profile/draft"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "direction_notes": "面向内容运营负责人，不要夸大收益。",
                        "model_id": model_id
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
    let run = sqlx::query_as::<_, (Uuid, String, Option<Uuid>, Value)>(
        "SELECT id, status, model_id, model_snapshot FROM agent_runs WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(&test_pool)
    .await
    .unwrap();
    assert_eq!(run.1, "succeeded");
    assert_eq!(run.2, Some(model_id));
    assert_eq!(run.3["model_id"], model_id.to_string());
    assert!(run.3.get("api_key").is_none());
    assert!(run.3.get("api_secret").is_none());
    let call: (String, i32, String, String, Value, Value, Option<Uuid>) = sqlx::query_as(
        r#"
        SELECT node_key, attempt, status, agent_key, prompt_snapshot, context_sources,
               context_snapshot_id
        FROM model_calls WHERE agent_run_id = $1
        "#,
    )
    .bind(run.0)
    .fetch_one(&test_pool)
    .await
    .unwrap();
    assert_eq!(call.0, "project.strategy_draft");
    assert_eq!(call.1, 1);
    assert_eq!(call.2, "succeeded");
    assert_eq!(call.3, "video.project-strategy");
    assert_eq!(call.4["fragments"], json!([]));
    assert_eq!(call.5[0]["source"], "project");
    assert_eq!(call.5[0]["trust"], "confirmed_fact");
    assert_eq!(call.5[1]["source"], "user_instruction");
    assert_eq!(call.5[1]["trust"], "user_instruction");
    let context_snapshot_id = call.6.unwrap();
    let context: (Value, Value, Value) = sqlx::query_as(
        "SELECT decisions, selected_order, logical_input FROM context_snapshots WHERE id=$1",
    )
    .bind(context_snapshot_id)
    .fetch_one(&test_pool)
    .await
    .unwrap();
    let decisions = context.0.as_array().unwrap();
    assert_eq!(decisions.len(), 2);
    let project_decision = decisions
        .iter()
        .find(|decision| decision["source_kind"] == "project")
        .unwrap();
    assert_eq!(project_decision["trust"], "confirmed_fact");
    assert_eq!(project_decision["priority"], "p1");
    assert_eq!(project_decision["required"], true);
    assert_eq!(project_decision["render_order"], 0);
    assert_eq!(project_decision["decision"], "selected");
    let instruction_decision = decisions
        .iter()
        .find(|decision| decision["source_kind"] == "user_instruction")
        .unwrap();
    assert_eq!(instruction_decision["trust"], "user_instruction");
    assert_eq!(instruction_decision["priority"], "p0");
    assert_eq!(instruction_decision["required"], true);
    assert_eq!(instruction_decision["render_order"], 1);
    assert_eq!(instruction_decision["decision"], "selected");
    assert_eq!(
        context.1,
        json!([
            format!("project:{project_id}:strategy-profile"),
            format!("project:{project_id}:strategy-direction")
        ])
    );
    assert_eq!(context.2, call.4["logical_input"]);
    let run_binding: (String, String, Uuid, String) = sqlx::query_as(
        r#"
        SELECT agent_key, agent_version, model_id, behavior_fingerprint
        FROM agent_run_bindings WHERE agent_run_id = $1
        "#,
    )
    .bind(run.0)
    .fetch_one(&test_pool)
    .await
    .unwrap();
    assert_eq!(run_binding.0, "video.project-strategy");
    assert_eq!(run_binding.1, "2.0.0");
    assert_eq!(run_binding.2, model_id);
    assert_eq!(run_binding.3.len(), 64);
    {
        let prompts = llm_client.prompts.lock().unwrap();
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].max_output_tokens, Some(1_200));
        assert_eq!(prompts[0].system, call.4["system"].as_str().unwrap());
        assert_eq!(prompts[0].user, call.4["user"].as_str().unwrap());
        assert_eq!(
            prompts[0].user,
            r#"请基于当前内容账号资料和补充方向，生成结构化账号策略草稿。

当前账号名称：AI 工具账号
定位摘要：AI 工具教程账号
账号描述：面向内容运营负责人的短视频
当前目标受众：
当前内容支柱：无
当前表达风格：
当前禁区方向：无
当前参考账号：无
当前选题偏好：

补充方向：面向内容运营负责人，不要夸大收益。

输出要求：
1. 只生成草稿，不要表达已保存或已生效。
2. draft 必须包含 target_audience、content_pillars、tone_style、forbidden_topics、reference_accounts、topic_preferences。
3. content_pillars、forbidden_topics、reference_accounts 每组最多 20 项。
4. 不得生成夸大收益、灰产引流或虚假承诺方向。
5. draft_summary 用一句中文总结草稿策略取向。"#
        );
    }

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn project_strategy_retry_creates_distinct_model_call_attempts_with_one_binding() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO projects (name, positioning) VALUES ('重试账号', '测试审计') RETURNING id",
    )
    .fetch_one(&test_pool)
    .await
    .unwrap();
    let success = json!({
        "draft": {
            "target_audience": "开发者",
            "content_pillars": ["工程实践"],
            "tone_style": "直接",
            "forbidden_topics": [],
            "reference_accounts": [],
            "topic_preferences": "可复现案例"
        },
        "draft_summary": "面向开发者的工程实践内容。"
    })
    .to_string();
    let llm_client = Arc::new(RecordingLLMClient::returning_sequence(vec![
        Err(LLMError::Provider("503 temporarily unavailable".into())),
        Ok(success),
    ]));
    let app = build_app_with_state(
        app_state(test_url, test_pool.clone()).with_llm_client(llm_client.clone()),
    );
    let model_id = insert_enabled_text_model(&test_pool).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/strategy-profile/draft"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "direction_notes": "保留一次重试证据", "model_id": model_id })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(llm_client.prompts.lock().unwrap().len(), 2);

    let rows = sqlx::query_as::<
        _,
        (
            Uuid,
            Option<Uuid>,
            i32,
            String,
            String,
            String,
            Option<Uuid>,
        ),
    >(
        r#"
        SELECT id, root_call_id, attempt, status, agent_key, behavior_fingerprint,
               context_snapshot_id
        FROM model_calls ORDER BY attempt
        "#,
    )
    .fetch_all(&test_pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        (rows[0].1, rows[0].2, rows[0].3.as_str()),
        (None, 1, "failed")
    );
    assert_eq!(
        (rows[1].1, rows[1].2, rows[1].3.as_str()),
        (Some(rows[0].0), 2, "succeeded")
    );
    assert_eq!(rows[0].4, "video.project-strategy");
    assert_eq!(rows[1].4, rows[0].4);
    assert_eq!(rows[1].5, rows[0].5);
    assert_ne!(rows[0].6, rows[1].6);
    assert!(rows[0].6.is_some() && rows[1].6.is_some());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM agent_run_bindings")
            .fetch_one(&test_pool)
            .await
            .unwrap(),
        1
    );

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn project_routes_generate_strategy_profile_draft_missing_project_returns_not_found() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));
    let missing_project_id = Uuid::new_v4();
    let model_id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/projects/{missing_project_id}/strategy-profile/draft"
                ))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "direction_notes": "补充方向", "model_id": model_id }).to_string(),
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
    let model_id = insert_enabled_text_model(&test_pool).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/strategy-profile/draft"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "direction_notes": "补充方向", "model_id": model_id }).to_string(),
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
    let run_status =
        sqlx::query_scalar::<_, String>("SELECT status FROM agent_runs WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&test_pool)
            .await
            .unwrap();
    assert_eq!(run_status, "failed");

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn strategy_draft_rejects_disabled_and_image_models_before_llm_execution() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO projects (name, positioning) VALUES ('模型路由账号', '测试') RETURNING id",
    )
    .fetch_one(&test_pool)
    .await
    .unwrap();
    let disabled_text_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO ai_models (
            display_name, model_type, provider_name, api_protocol, auth_scheme,
            request_base_url, upstream_model, api_key, status
        ) VALUES (
            '停用文本模型', 'text', 'test', 'openai_chat_completions', 'bearer',
            'https://example.invalid/v1', 'test-model', 'test-key', 'disabled'
        ) RETURNING id
        "#,
    )
    .fetch_one(&test_pool)
    .await
    .unwrap();
    let image_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO ai_models (
            display_name, model_type, provider_name, api_protocol, auth_scheme,
            request_base_url, upstream_model, api_key, status,
            settings
        ) VALUES (
            '图片模型', 'image', 'test', 'openai_images', 'bearer',
            'https://example.invalid/v1', 'test-image', 'test-key', 'enabled',
            '{"supported_sizes":["1024x1024"],"default_size":"1024x1024"}'::jsonb
        ) RETURNING id
        "#,
    )
    .fetch_one(&test_pool)
    .await
    .unwrap();
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));

    for (model_id, expected_status, expected_code) in [
        (disabled_text_id, StatusCode::CONFLICT, "model_disabled"),
        (
            image_id,
            StatusCode::UNPROCESSABLE_ENTITY,
            "model_type_mismatch",
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/projects/{project_id}/strategy-profile/draft"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({ "direction_notes": "测试", "model_id": model_id }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected_status);
        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], expected_code);
    }
    let run_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM agent_runs")
        .fetch_one(&test_pool)
        .await
        .unwrap();
    assert_eq!(run_count, 0);

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
