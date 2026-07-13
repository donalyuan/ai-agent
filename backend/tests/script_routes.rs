use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::{routing::post, Json, Router};
use novex_api::bootstrap::{AppConfig, AppState};
use novex_api::build_app_with_state;
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tower::ServiceExt;
use uuid::Uuid;

mod support;

use support::test_database::{insert_enabled_text_model_with_base_url, TestDatabase};

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
        .expect("temporary route database should be created");
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
    let database_name = format!("video_agent_route_test_{}", suffix);
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
        .expect("temporary route database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .expect("migrations should run for route test database");

    (admin_pool, test_pool, database_name, test_url)
}

async fn insert_project(pool: &PgPool) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO projects (name, positioning, description)
        VALUES ('科技博主', '科技知识账号', '脚本路由测试项目')
        RETURNING id
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("project fixture should be inserted")
}

async fn insert_script_with_scene(pool: &PgPool, project_id: Uuid) -> Uuid {
    let script_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO scripts (project_id, title, hook, content, status)
        VALUES ($1, '程序员必看：ChatGPT工作流', '还在手写重复代码？', $2, 'draft')
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(json!({"topic": "ChatGPT如何改变程序员工作流"}))
    .fetch_one(pool)
    .await
    .expect("script fixture should be inserted");

    sqlx::query(
        r#"
        INSERT INTO scenes (script_id, sequence, narration, visual_description, emotion, duration_sec)
        VALUES ($1, 1, '传统程序员每天要写大量重复代码。', '程序员盯着屏幕，快速切换多个代码文件。', '焦虑', 8)
        "#,
    )
    .bind(script_id)
    .execute(pool)
    .await
    .expect("scene fixture should be inserted");

    script_id
}

async fn insert_content_topic(pool: &PgPool, project_id: Uuid, status: &str) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO content_topics (
            project_id, title, angle, target_audience, hook_points, content_type,
            score, score_reason, tags, source, status
        )
        VALUES (
            $1,
            'AI 工具如何重塑内容团队',
            '强调协作流程',
            '内容负责人',
            ARRAY['流程重构']::TEXT[],
            'knowledge',
            91,
            '标题更具体',
            ARRAY['AI工具']::TEXT[],
            'manual',
            $2
        )
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(status)
    .fetch_one(pool)
    .await
    .expect("content topic fixture should be inserted")
}

async fn openai_handler(Json(_payload): Json<Value>) -> Json<Value> {
    Json(json!({
        "choices": [
            {
                "message": {
                    "content": json!({
                        "title": "程序员必看：ChatGPT工作流",
                        "hook": "还在手写重复代码？",
                        "scenes": [
                            {
                                "sequence": 1,
                                "narration": "传统程序员每天要写大量重复代码，复制粘贴改参数，枯燥又容易出错，团队还要花很多时间检查这些重复劳动带来的隐藏问题。",
                                "visual_description": "程序员盯着屏幕，快速切换多个代码文件。",
                                "emotion": "焦虑",
                                "duration_sec": 8
                            },
                            {
                                "sequence": 2,
                                "narration": "现在只要描述需求，AI 就能快速生成初稿，让开发者把时间放回设计和验证，从重复劳动转向架构判断、边界测试和真实业务理解。",
                                "visual_description": "屏幕上弹出代码建议，程序员露出惊喜表情。",
                                "emotion": "惊喜",
                                "duration_sec": 9
                            },
                            {
                                "sequence": 3,
                                "narration": "更重要的是，AI 可以帮你解释陌生代码，让新人快速理解项目结构、关键流程和历史取舍，减少只靠猜测修改代码的风险。",
                                "visual_description": "代码结构图展开，重点模块被高亮标注。",
                                "emotion": "好奇",
                                "duration_sec": 9
                            },
                            {
                                "sequence": 4,
                                "narration": "遇到报错时，把日志和上下文交给 AI，它能给出排查方向，但最终仍要由程序员验证证据、复现实验并确认根因。",
                                "visual_description": "终端错误日志旁边出现排查清单。",
                                "emotion": "紧张",
                                "duration_sec": 10
                            },
                            {
                                "sequence": 5,
                                "narration": "未来的竞争不是谁会复制答案，而是谁能把 AI 产出的初稿打磨成可靠系统，并用工程纪律保证结果长期可维护。",
                                "visual_description": "程序员提交通过测试的代码，仪表盘显示绿色通过。",
                                "emotion": "平静",
                                "duration_sec": 10
                            }
                        ]
                    }).to_string()
                }
            }
        ]
    }))
}

async fn local_openai_base_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().route("/v1/chat/completions", post(openai_handler));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{address}/v1")
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

async fn local_scripted_openai_base_url(
    responses: Vec<Value>,
    requests: Arc<Mutex<Vec<Value>>>,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let responses = Arc::new(Mutex::new(VecDeque::from(responses)));
    let app = Router::new().route(
        "/v1/chat/completions",
        post({
            let responses = responses.clone();
            let requests = requests.clone();
            move |Json(payload): Json<Value>| {
                let responses = responses.clone();
                let requests = requests.clone();
                async move {
                    requests.lock().unwrap().push(payload);
                    let content = responses
                        .lock()
                        .unwrap()
                        .pop_front()
                        .expect("scripted OpenAI response should exist");
                    chat_response(content)
                }
            }
        }),
    );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{address}/v1")
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn script_routes_generate_read_list_and_update_status() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let openai_base_url = local_openai_base_url().await;
    let model_id = insert_enabled_text_model_with_base_url(&test_pool, &openai_base_url).await;
    let app = build_app_with_state(
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
            test_pool.clone(),
            None,
        )
        .unwrap(),
    );

    let generate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/scripts/generate")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": project_id,
                        "model_id": model_id,
                        "topic": "ChatGPT如何改变程序员工作流",
                        "style": "knowledge",
                        "scene_count": 5
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(generate_response.status(), StatusCode::OK);
    let generated = response_json(generate_response).await;
    let script_id = generated["script_id"].as_str().unwrap();
    assert_eq!(generated["project_id"], project_id.to_string());
    assert_eq!(generated["scenes"].as_array().unwrap().len(), 5);
    let run = sqlx::query_as::<_, (String, Option<Uuid>, Value)>(
        "SELECT status, model_id, model_snapshot FROM agent_runs WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(&test_pool)
    .await
    .unwrap();
    assert_eq!(run.0, "succeeded");
    assert_eq!(run.1, Some(model_id));
    assert_eq!(run.2["model_id"], model_id.to_string());
    assert!(run.2.get("api_key").is_none());
    assert!(run.2.get("api_secret").is_none());

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/scripts/{script_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/projects/{project_id}/scripts?status=draft"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let listed = response_json(list_response).await;
    assert_eq!(listed["total"], 1);
    assert_eq!(listed["scripts"][0]["script_id"], script_id);

    let update_response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/scripts/{script_id}/status"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"status": "approved"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);
    let updated = response_json(update_response).await;
    assert_eq!(updated["status"], "approved");

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn generate_route_uses_stepwise_single_scene_mode_for_xhigh() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let requests = Arc::new(Mutex::new(Vec::new()));
    let openai_base_url = local_scripted_openai_base_url(
        vec![
            json!({
                "title": "AI时代人类新能力",
                "hook": "未来淘汰你的不是AI，而是不会用AI的人。"
            }),
            json!({
                "scene": {
                    "sequence": 1,
                    "narration": "AI 正在把重复劳动交给机器，把判断、创意和同理心留给人类。接受 AI 的关键，是学会提问、验证结果，并把它当成放大能力的工具。",
                    "visual_description": "人类和 AI 在同一张工作台前协作，屏幕显示分析结果和人工确认标记。",
                    "emotion": "理性",
                    "duration_sec": 9
                }
            }),
            json!({
                "scene": {
                    "sequence": 2,
                    "narration": "真正接受 AI 不是盲目依赖，而是把它放进清晰流程里。人负责目标、边界和责任，AI 负责加速搜索、整理和生成初稿。",
                    "visual_description": "画面展示目标清单、AI 输出草稿和人工检查标记依次出现。",
                    "emotion": "平静",
                    "duration_sec": 9
                }
            }),
            json!({
                "scene": {
                    "sequence": 3,
                    "narration": "当每个人都能调用 AI，稀缺的不再是答案，而是提出好问题、判断真假、整合资源，并持续做出负责决策的能力。",
                    "visual_description": "多人面对同一份 AI 答案，主角标出风险点并给出最终方案。",
                    "emotion": "鼓舞",
                    "duration_sec": 9
                }
            }),
        ],
        requests.clone(),
    )
    .await;
    let model_id = insert_enabled_text_model_with_base_url(&test_pool, &openai_base_url).await;
    sqlx::query("UPDATE ai_models SET reasoning_effort = 'xhigh' WHERE id = $1")
        .bind(model_id)
        .execute(&test_pool)
        .await
        .unwrap();
    let app = build_app_with_state(
        AppState::new(
            AppConfig {
                environment: "test".to_string(),
                database_url: test_url,
                redis_url: "redis://127.0.0.1:6379/15".to_string(),
                openai_api_key: "test-key".to_string(),
                openai_base_url,
                openai_model: "test-model".to_string(),
                openai_timeout_seconds: 5,
                openai_reasoning_effort: Some("xhigh".to_string()),
                openai_max_output_tokens: 3000,
                asset_storage_root: "/app/storage/assets".to_string(),
                asset_generation_providers: vec!["gpt-image-2".to_string(), "jimeng".to_string()],
            },
            test_pool.clone(),
            None,
        )
        .unwrap(),
    );

    let generate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/scripts/generate")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": project_id,
                        "model_id": model_id,
                        "topic": "AI 如何改变人类，人类该如何接受 AI",
                        "style": "knowledge",
                        "scene_count": 3
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(generate_response.status(), StatusCode::OK);
    let generated = response_json(generate_response).await;
    assert_eq!(generated["scenes"].as_array().unwrap().len(), 3);

    {
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        assert!(requests[0]["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("只输出 title 和 hook"));
        assert!(requests[1]["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("当前分镜序号：1"));
        assert!(requests[3]["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("当前分镜序号：3"));
    }

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn generate_route_persists_topic_link_snapshot_and_marks_topic_scripted() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let topic_id = insert_content_topic(&test_pool, project_id, "approved").await;
    let openai_base_url = local_openai_base_url().await;
    let model_id = insert_enabled_text_model_with_base_url(&test_pool, &openai_base_url).await;
    let app = build_app_with_state(
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
            test_pool.clone(),
            None,
        )
        .unwrap(),
    );

    let generate_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/scripts/generate")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": project_id,
                        "model_id": model_id,
                        "topic_id": topic_id,
                        "style": "knowledge",
                        "scene_count": 5
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(generate_response.status(), StatusCode::OK);
    let generated = response_json(generate_response).await;
    let script_id = Uuid::parse_str(generated["script_id"].as_str().unwrap()).unwrap();
    assert_eq!(generated["topic_id"], topic_id.to_string());

    let saved = sqlx::query_as::<_, (Option<Uuid>, Value, String)>(
        r#"
        SELECT s.topic_id, s.content, t.status
        FROM scripts s
        JOIN content_topics t ON t.id = $2
        WHERE s.id = $1
        "#,
    )
    .bind(script_id)
    .bind(topic_id)
    .fetch_one(&test_pool)
    .await
    .unwrap();
    assert_eq!(saved.0, Some(topic_id));
    assert_eq!(saved.1["topic_snapshot"]["topic_id"], topic_id.to_string());
    assert_eq!(
        saved.1["topic_snapshot"]["title"],
        "AI 工具如何重塑内容团队"
    );
    assert_eq!(saved.2, "scripted");

    let list_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/projects/{project_id}/scripts"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let listed = response_json(list_response).await;
    assert_eq!(listed["scripts"][0]["topic_id"], topic_id.to_string());
    assert_eq!(
        listed["scripts"][0]["source_topic_title"],
        "AI 工具如何重塑内容团队"
    );

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn generate_route_rejects_non_approved_or_cross_project_topics_without_creating_script() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let other_project_id = insert_project(&test_pool).await;
    let idea_topic_id = insert_content_topic(&test_pool, project_id, "idea").await;
    let other_project_topic_id =
        insert_content_topic(&test_pool, other_project_id, "approved").await;
    let openai_base_url = local_openai_base_url().await;
    let model_id = insert_enabled_text_model_with_base_url(&test_pool, &openai_base_url).await;
    let app = build_app_with_state(
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
            test_pool.clone(),
            None,
        )
        .unwrap(),
    );

    for topic_id in [idea_topic_id, other_project_topic_id] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/scripts/generate")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "project_id": project_id,
                            "model_id": model_id,
                            "topic_id": topic_id,
                            "style": "knowledge",
                            "scene_count": 5
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    let script_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM scripts")
        .fetch_one(&test_pool)
        .await
        .unwrap();
    assert_eq!(script_count, 0);
    let idea_status =
        sqlx::query_scalar::<_, String>("SELECT status FROM content_topics WHERE id = $1")
            .bind(idea_topic_id)
            .fetch_one(&test_pool)
            .await
            .unwrap();
    assert_eq!(idea_status, "idea");

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn generate_route_returns_script_generation_error_when_llm_output_is_invalid() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let requests = Arc::new(Mutex::new(Vec::new()));
    let openai_base_url = local_scripted_openai_base_url(
        vec![
            json!("不是 JSON"),
            json!("仍然不是 JSON"),
            json!("还是不是 JSON"),
        ],
        requests,
    )
    .await;
    let model_id = insert_enabled_text_model_with_base_url(&test_pool, &openai_base_url).await;
    let app = build_app_with_state(
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
            test_pool.clone(),
            None,
        )
        .unwrap(),
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/scripts/generate")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": project_id,
                        "model_id": model_id,
                        "topic": "ChatGPT如何改变程序员工作流",
                        "style": "knowledge",
                        "scene_count": 5
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = response_json(response).await;
    assert_eq!(body["error"], "脚本生成失败");
    assert!(body["details"]
        .as_str()
        .unwrap()
        .contains("script parse error"));
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
async fn script_read_routes_do_not_require_openai_config() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let script_id = insert_script_with_scene(&test_pool, project_id).await;
    let app = build_app_with_state(
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
            test_pool.clone(),
            None,
        )
        .unwrap(),
    );

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/scripts/{script_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/projects/{project_id}/scripts"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);

    let update_response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/scripts/{script_id}/status"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"status": "approved"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn generate_route_checks_project_before_openai_config() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let missing_project_id = Uuid::new_v4();
    let app = build_app_with_state(
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
            test_pool.clone(),
            None,
        )
        .unwrap(),
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/scripts/generate")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": missing_project_id,
                        "model_id": Uuid::new_v4(),
                        "topic": "ChatGPT如何改变程序员工作流",
                        "style": "knowledge",
                        "scene_count": 5
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn script_routes_return_structured_error_payloads() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let missing_project_id = Uuid::new_v4();
    let missing_script_id = Uuid::new_v4();
    let app = build_app_with_state(
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
            test_pool.clone(),
            None,
        )
        .unwrap(),
    );

    let missing_project_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/projects/{missing_project_id}/scripts"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_project_response.status(), StatusCode::NOT_FOUND);
    let missing_project_body = response_json(missing_project_response).await;
    assert_eq!(missing_project_body["error"], "项目不存在");
    assert_eq!(
        missing_project_body["project_id"],
        missing_project_id.to_string()
    );

    let missing_script_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/scripts/{missing_script_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_script_response.status(), StatusCode::NOT_FOUND);
    let missing_script_body = response_json(missing_script_response).await;
    assert_eq!(missing_script_body["error"], "脚本不存在");
    assert_eq!(
        missing_script_body["script_id"],
        missing_script_id.to_string()
    );

    let invalid_status_response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/scripts/{missing_script_id}/status"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"status": "deleted"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_status_response.status(), StatusCode::BAD_REQUEST);
    let invalid_status_body = response_json(invalid_status_response).await;
    assert_eq!(invalid_status_body["error"], "无效的状态值");
    assert_eq!(
        invalid_status_body["allowed"],
        json!(["draft", "approved", "archived"])
    );

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
