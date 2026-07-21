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
        .expect("temporary conversation route database should be created");
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
    let database_name = format!("video_agent_conversation_route_test_{}", suffix);
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
        .expect("temporary conversation route database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .expect("migrations should run for conversation route test database");

    (admin_pool, test_pool, database_name, test_url)
}

async fn insert_project(pool: &PgPool) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO projects (name, positioning, description)
        VALUES ('科技博主', '科技知识账号', '对话路由测试项目')
        RETURNING id
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("project fixture should be inserted")
}

async fn insert_script(pool: &PgPool, project_id: Uuid) -> Uuid {
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
        VALUES
          ($1, 1, '传统程序员每天要写大量重复代码。', '程序员盯着屏幕，快速切换多个代码文件。', '焦虑', 8),
          ($1, 2, '现在只要描述需求，AI 就能快速生成初稿。', '屏幕上弹出代码建议。', '惊喜', 9),
          ($1, 3, 'AI 可以帮助新人快速理解陌生项目。', '代码结构图展开，重点模块被高亮标注。', '好奇', 9)
        "#,
    )
    .bind(script_id)
    .execute(pool)
    .await
    .expect("scene fixture should be inserted");

    script_id
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

async fn local_scripted_openai_base_url(requests: Arc<Mutex<Vec<Value>>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().route(
        "/v1/chat/completions",
        post(move |Json(payload): Json<Value>| {
            let requests = requests.clone();
            async move {
                requests.lock().unwrap().push(payload);
                chat_response(json!({
                    "scene_sequence": 3,
                    "narration": "深夜办公室里，主角面对即将上线的故障代码，发现 AI 给出的解释和线上日志互相矛盾，只能立刻重新验证每一步判断。",
                    "visual_description": "凌晨两点的办公室只剩一盏灯，屏幕上同时显示 AI 建议、红色错误日志和倒计时发布窗口。",
                    "emotion": "紧张",
                    "duration_sec": 10,
                    "reply": "已把第 3 镜改成深夜上线前的冲突场景，强化紧迫感和人工验证。"
                }))
            }
        }),
    );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{address}/v1")
}

async fn local_sound_openai_base_url(requests: Arc<Mutex<Vec<Value>>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new().route(
        "/v1/chat/completions",
        post(move |Json(payload): Json<Value>| {
            let requests = requests.clone();
            async move {
                requests.lock().unwrap().push(payload);
                chat_response(json!({
                    "reply": "已按沉稳知识解说推荐测试音色；请试听并确认后再生成。",
                    "recommended_voice_type": "zh_female_fixture_mars_bigtts",
                    "language": "zh-cn",
                    "tts_text": "你好世界",
                    "subtitle_segments": ["你好", "世界"],
                    "parameters": {"speed_ratio": 1.0}
                }))
            }
        }),
    );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{address}/v1")
}

async fn insert_tts_model_with_voice(pool: &PgPool) -> Uuid {
    let model_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO ai_models (
            display_name, model_type, provider_name, api_protocol, protocol_version,
            auth_scheme, request_base_url, upstream_model, api_key,
            catalog_access_key, catalog_secret_key, timeout_seconds, settings, status
        )
        VALUES (
            'Doubao TTS', 'speech', '火山引擎', 'volcengine_tts_v3', 'v3',
            'api_key', 'https://openspeech.bytedance.com/api/v3',
            'doubao-seed-tts-2.0', 'runtime-secret', 'catalog-ak', 'catalog-sk', 120,
            $1, 'enabled'
        )
        RETURNING id
        "#,
    )
    .bind(json!({
        "resource_id": "seed-tts-2.0",
        "supported_audio_formats": ["mp3"],
        "default_audio_format": "mp3",
        "supported_sample_rates": [24000],
        "default_sample_rate": 24000,
        "max_input_characters": 3000,
        "max_audio_duration_seconds": null,
        "supports_word_timestamps": true,
        "word_timestamp_languages": ["zh-cn"],
        "catalog_sync_interval_minutes": 1440,
        "parameters": {"speed_ratio": {"type": "number", "minimum": 0.5, "maximum": 2.0}}
    }))
    .fetch_one(pool)
    .await
    .unwrap();
    let sync_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO voice_catalog_syncs (
            model_id, trigger_source, status, page_count, speaker_count, completed_at
        )
        VALUES ($1, 'admin', 'succeeded', 1, 1, NOW())
        RETURNING id
        "#,
    )
    .bind(model_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO voice_catalog_entries (
            model_id, voice_type, resource_id, name, languages, emotions,
            description, is_available, first_seen_sync_id, last_seen_sync_id
        )
        VALUES (
            $1, 'zh_female_fixture_mars_bigtts', 'seed-tts-2.0', '测试音色',
            '[{"Language":"zh-cn"}]', '[{"Value":"","Label":"","Icon":""}]',
            '沉稳知识解说', TRUE, $2, $2
        )
        "#,
    )
    .bind(model_id)
    .bind(sync_id)
    .execute(pool)
    .await
    .unwrap();
    model_id
}

async fn insert_large_voice_catalog(pool: &PgPool, model_id: Uuid) {
    let sync_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM voice_catalog_syncs WHERE model_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(model_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO voice_catalog_entries (
            model_id, voice_type, resource_id, name, languages, emotions,
            description, is_available, first_seen_sync_id, last_seen_sync_id
        )
        SELECT
            $1,
            'catalog_voice_' || LPAD(series::text, 3, '0'),
            'seed-tts-2.0',
            'voice-' || LPAD(series::text, 3, '0'),
            jsonb_build_array(jsonb_build_object(
                'Language', CASE WHEN series = 80 THEN 'en' ELSE 'zh-cn' END,
                'Text', CASE WHEN series = 80 THEN '目录试听文案不应进入Prompt' ELSE '试听文案' END
            )),
            '[]'::jsonb,
            CASE WHEN series = 80 THEN '完整目录末尾音色' ELSE '测试音色' END,
            TRUE,
            $2,
            $2
        FROM generate_series(0, 80) AS series
        "#,
    )
    .bind(model_id)
    .bind(sync_id)
    .execute(pool)
    .await
    .unwrap();
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
async fn conversation_routes_create_unbound_script_generation_conversation() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let openai_requests = Arc::new(Mutex::new(Vec::new()));
    let openai_base_url = local_scripted_openai_base_url(openai_requests.clone()).await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone(), openai_base_url));

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/conversations")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "agent_type": "script",
                        "project_id": project_id,
                        "title": "脚本生成"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let conversation = response_json(create_response).await;
    assert_eq!(conversation["agent_type"], "script");
    assert_eq!(conversation["project_id"], project_id.to_string());
    assert!(conversation["subject_type"].is_null());
    assert!(conversation["subject_id"].is_null());
    assert_eq!(conversation["status"], "active");

    let missing_project_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/conversations")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "agent_type": "script",
                        "title": "脚本生成"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_project_response.status(), StatusCode::BAD_REQUEST);
    let missing_project_body = response_json(missing_project_response).await;
    assert_eq!(missing_project_body["error"], "Agent 会话必须绑定项目");

    assert!(openai_requests.lock().unwrap().is_empty());

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn conversation_routes_create_topic_generation_conversation() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let openai_requests = Arc::new(Mutex::new(Vec::new()));
    let openai_base_url = local_scripted_openai_base_url(openai_requests.clone()).await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone(), openai_base_url));

    let create_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/conversations")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "agent_type": "topic",
                        "project_id": project_id,
                        "title": "选题生成"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let conversation = response_json(create_response).await;
    assert_eq!(conversation["agent_type"], "topic");
    assert_eq!(conversation["project_id"], project_id.to_string());
    assert!(conversation["subject_type"].is_null());
    assert!(conversation["subject_id"].is_null());
    assert_eq!(conversation["status"], "active");
    assert!(openai_requests.lock().unwrap().is_empty());

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn conversation_routes_create_send_and_list_messages() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let script_id = insert_script(&test_pool, project_id).await;
    let model_id = insert_enabled_text_model(&test_pool).await;
    let openai_requests = Arc::new(Mutex::new(Vec::new()));
    let openai_base_url = local_scripted_openai_base_url(openai_requests.clone()).await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone(), openai_base_url));

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/conversations")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "agent_type": "script",
                        "project_id": project_id,
                        "subject_type": "script",
                        "subject_id": script_id,
                        "title": "脚本改稿"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let conversation = response_json(create_response).await;
    let conversation_id = conversation["conversation_id"].as_str().unwrap();
    assert_eq!(conversation["agent_type"], "script");
    assert_eq!(conversation["subject_id"], script_id.to_string());
    assert_eq!(conversation["status"], "active");

    let send_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/agent/conversations/{conversation_id}/messages"
                ))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "content": "把第 3 镜改得更有冲突感，画面换成办公室深夜加班",
                        "model_id": model_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let send_status = send_response.status();
    let turn = response_json(send_response).await;
    assert_eq!(send_status, StatusCode::OK, "response body: {turn}");
    assert_eq!(turn["assistant_message"]["role"], "assistant");
    assert_eq!(turn["assistant_message"]["metadata"]["scene_sequence"], 3);
    assert_eq!(turn["run"]["status"], "succeeded");

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/agent/conversations/{conversation_id}/messages"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let messages = response_json(list_response).await;
    assert_eq!(messages["messages"].as_array().unwrap().len(), 2);
    assert_eq!(messages["messages"][0]["role"], "user");
    assert_eq!(messages["messages"][1]["role"], "assistant");

    let scene_narration = sqlx::query_scalar::<_, String>(
        "SELECT narration FROM scenes WHERE script_id = $1 AND sequence = 3",
    )
    .bind(script_id)
    .fetch_one(&test_pool)
    .await
    .unwrap();
    assert!(scene_narration.contains("线上日志互相矛盾"));
    assert_eq!(openai_requests.lock().unwrap().len(), 1);

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn conversation_routes_return_stable_errors() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let model_id = Uuid::new_v4();
    let missing_script_id = Uuid::new_v4();
    let openai_requests = Arc::new(Mutex::new(Vec::new()));
    let openai_base_url = local_scripted_openai_base_url(openai_requests.clone()).await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone(), openai_base_url));

    let missing_script_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/conversations")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "agent_type": "script",
                        "project_id": project_id,
                        "subject_type": "script",
                        "subject_id": missing_script_id,
                        "title": "脚本改稿"
                    })
                    .to_string(),
                ))
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

    let other_project_id = insert_project(&test_pool).await;
    let script_id = insert_script(&test_pool, project_id).await;
    let project_mismatch_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/conversations")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "agent_type": "script",
                        "project_id": other_project_id,
                        "subject_type": "script",
                        "subject_id": script_id,
                        "title": "脚本改稿"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(project_mismatch_response.status(), StatusCode::BAD_REQUEST);
    let project_mismatch_body = response_json(project_mismatch_response).await;
    assert_eq!(project_mismatch_body["error"], "脚本不属于当前项目");

    let missing_conversation_id = Uuid::new_v4();
    let missing_conversation_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/agent/conversations/{missing_conversation_id}/messages"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        missing_conversation_response.status(),
        StatusCode::NOT_FOUND
    );
    let missing_conversation_body = response_json(missing_conversation_response).await;
    assert_eq!(missing_conversation_body["error"], "会话不存在");
    assert_eq!(
        missing_conversation_body["conversation_id"],
        missing_conversation_id.to_string()
    );

    let script_id = insert_script(&test_pool, project_id).await;
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/conversations")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "agent_type": "script",
                        "project_id": project_id,
                        "subject_type": "script",
                        "subject_id": script_id,
                        "title": "脚本改稿"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let conversation = response_json(create_response).await;
    let conversation_id = conversation["conversation_id"].as_str().unwrap();

    let empty_message_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/agent/conversations/{conversation_id}/messages"
                ))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"content": "   ", "model_id": model_id}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(empty_message_response.status(), StatusCode::BAD_REQUEST);
    let empty_message_body = response_json(empty_message_response).await;
    assert_eq!(empty_message_body["error"], "消息不能为空");

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn sound_agent_recommends_only_catalog_voice_and_never_executes_speech_tools() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let text_model_id = insert_enabled_text_model(&test_pool).await;
    let speech_model_id = insert_tts_model_with_voice(&test_pool).await;
    insert_large_voice_catalog(&test_pool, speech_model_id).await;
    let requests = Arc::new(Mutex::new(Vec::new()));
    let openai_base_url = local_sound_openai_base_url(requests.clone()).await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone(), openai_base_url));

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/conversations")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "agent_type": "sound",
                        "project_id": project_id,
                        "title": "声音建议",
                        "metadata": {"speech_model_id": speech_model_id}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let conversation = response_json(create_response).await;
    let conversation_id = conversation["conversation_id"].as_str().unwrap();

    let missing_context_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/agent/conversations/{conversation_id}/messages"
                ))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "content": "给这段知识旁白推荐沉稳声音",
                        "model_id": text_model_id
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_context_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(missing_context_response).await["error"],
        "声音消息缺少当前编辑上下文"
    );
    assert!(requests.lock().unwrap().is_empty());

    let mismatched_context_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/agent/conversations/{conversation_id}/messages"
                ))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "content": "给这段知识旁白推荐沉稳声音",
                        "model_id": text_model_id,
                        "sound_context": {
                            "speech_model_id": Uuid::new_v4(),
                            "tts_text": "当前旁白内容",
                            "voice_type": "zh_female_fixture_mars_bigtts",
                            "language": "zh-cn",
                            "parameters": {"speed_ratio": 1.0},
                            "subtitle_segments": ["当前旁白内容"]
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        mismatched_context_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        response_json(mismatched_context_response).await["error"],
        "声音消息上下文与会话 TTS 模型不一致"
    );
    assert!(requests.lock().unwrap().is_empty());

    let send_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/agent/conversations/{conversation_id}/messages"
                ))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "content": "给这段知识旁白推荐沉稳声音",
                        "model_id": text_model_id,
                        "sound_context": {
                            "speech_model_id": speech_model_id,
                            "tts_text": "当前旁白内容",
                            "voice_type": "zh_female_fixture_mars_bigtts",
                            "language": "zh-cn",
                            "parameters": {"speed_ratio": 1.0},
                            "subtitle_segments": ["当前旁白", "内容"]
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = send_response.status();
    let turn = response_json(send_response).await;
    assert_eq!(status, StatusCode::OK, "{turn}");
    assert_eq!(
        turn["assistant_message"]["metadata"]["recommended_voice_type"],
        "zh_female_fixture_mars_bigtts"
    );
    assert_eq!(
        turn["assistant_message"]["metadata"]["requires_confirmation"],
        true
    );
    assert!(turn["assistant_message"]["metadata"]
        .get("emotion")
        .is_none());
    assert_eq!(
        turn["assistant_message"]["metadata"]["tool_execution"],
        false
    );

    let step_types = sqlx::query_scalar::<_, String>(
        r#"
        SELECT step.step_type
        FROM agent_steps step
        JOIN agent_runs run ON run.id = step.agent_run_id
        WHERE run.agent_type = 'sound'
        ORDER BY step.step_order
        "#,
    )
    .fetch_all(&test_pool)
    .await
    .unwrap();
    assert_eq!(step_types, vec!["read_voice_catalog", "recommend_sound"]);
    let task_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sound_subtitle_tasks")
        .fetch_one(&test_pool)
        .await
        .unwrap();
    assert_eq!(task_count, 0);
    assert_eq!(
        turn["user_message"]["metadata"]["sound_context"]["tts_text"],
        "当前旁白内容"
    );
    assert_eq!(
        turn["run"]["input"]["sound_context"]["speech_model_id"],
        speech_model_id.to_string()
    );
    let request_payload = requests.lock().unwrap()[0].clone();
    assert_eq!(request_payload["response_format"]["type"], "json_object");
    let prompt = request_payload["messages"][1]["content"]
        .as_str()
        .expect("声音 Agent 请求必须包含 user prompt");
    assert!(prompt.contains("当前编辑上下文"));
    assert!(prompt.contains("当前旁白内容"));
    assert!(prompt.contains("模型可调参数定义"));
    assert!(prompt.contains("声音建议 JSON 输出契约"));
    assert!(prompt.contains("recommended_voice_type"));
    assert!(prompt.contains("subtitle_segments"));
    assert!(prompt.contains("additionalProperties"));
    assert!(prompt.contains("\"minimum\":0.5"));
    assert!(prompt.contains("完整可用音色目录（共 82 项）"));
    assert!(prompt.contains("zh_female_fixture_mars_bigtts"));
    assert!(prompt.contains("catalog_voice_080"));
    assert!(prompt.contains("完整目录末尾音色"));
    assert!(!prompt.contains("目录试听文案不应进入Prompt"));
    assert!(!request_payload.to_string().contains("runtime-secret"));

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
