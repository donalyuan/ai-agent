use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{DateTime, Utc};
use novex_api::bootstrap::{AppConfig, AppState};
use novex_api::build_app_with_state;
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
    let slash_index = base.rfind('/').unwrap();
    format!("{}{}{}", &base[..=slash_index], database_name, query)
}

async fn migrated_pool() -> (PgPool, PgPool, TestDatabase, String) {
    let base_url = database_url();
    let database_name = format!("sound_routes_{}", Uuid::new_v4().simple());
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .unwrap();
    sqlx::query(&format!(r#"CREATE DATABASE "{database_name}""#))
        .execute(&admin_pool)
        .await
        .unwrap();
    let database = TestDatabase::new(&admin_url, &database_name);
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&test_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (admin_pool, pool, database, test_url)
}

fn app_state(test_url: String, pool: PgPool) -> AppState {
    AppState::new(
        AppConfig {
            environment: "test".to_string(),
            database_url: test_url,
            redis_url: "redis://127.0.0.1:6379/15".to_string(),
            openai_api_key: String::new(),
            openai_base_url: "https://example.invalid/v1".to_string(),
            openai_model: "unused".to_string(),
            openai_timeout_seconds: 5,
            openai_reasoning_effort: None,
            openai_max_output_tokens: 3000,
            asset_storage_root: "/app/storage/assets".to_string(),
            asset_generation_providers: vec![],
        },
        pool,
        None,
    )
    .unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap_or(Value::Null)
}

async fn send(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    idempotency_key: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    if let Some(key) = idempotency_key {
        builder = builder.header("idempotency-key", key);
    }
    let response = app
        .clone()
        .oneshot(
            builder
                .body(body.map_or_else(Body::empty, |value| Body::from(value.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    (status, response_json(response).await)
}

async fn seed_project(pool: &PgPool) -> Uuid {
    sqlx::query_scalar("INSERT INTO projects (name) VALUES ('声音测试') RETURNING id")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn seed_source_script(
    pool: &PgPool,
    project_id: Uuid,
    status: &str,
) -> (Uuid, DateTime<Utc>, Vec<Uuid>) {
    let (script_id, updated_at): (Uuid, DateTime<Utc>) = sqlx::query_as(
        r#"
        INSERT INTO scripts (project_id, title, hook, content, status)
        VALUES ($1, '别硬扛：稳定前进的方法', '停止内耗', $2, $3)
        RETURNING id, updated_at
        "#,
    )
    .bind(project_id)
    .bind(json!({"topic_snapshot": {"title": "停止内耗，从拆小目标开始"}}))
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap();
    let second_scene_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO scenes (script_id, sequence, narration, visual_description, emotion, duration_sec)
        VALUES ($1, 2, '把目标拆小，让每一步都能真正完成。', '拆分目标', '平静', 8)
        RETURNING id
        "#,
    )
    .bind(script_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let first_scene_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO scenes (script_id, sequence, narration, visual_description, emotion, duration_sec)
        VALUES ($1, 1, '允许自己停一停，并不是放弃目标。', '短暂停顿', '温暖', 8)
        RETURNING id
        "#,
    )
    .bind(script_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (script_id, updated_at, vec![first_scene_id, second_scene_id])
}

async fn seed_tts_model(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
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
        "supported_audio_formats": ["mp3", "wav"],
        "default_audio_format": "mp3",
        "supported_sample_rates": [24000],
        "default_sample_rate": 24000,
        "max_input_characters": 3000,
        "max_audio_duration_seconds": null,
        "supports_word_timestamps": true,
        "word_timestamp_languages": ["zh-cn", "en-us"],
        "catalog_sync_interval_minutes": 1440,
        "parameters": {
            "speed_ratio": {"type": "number", "minimum": 0.5, "maximum": 2.0}
        }
    }))
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_shared_tts_model(pool: &PgPool, source_model_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        r#"
        INSERT INTO ai_models (
            display_name, model_type, provider_name, api_protocol, protocol_version,
            auth_scheme, request_base_url, upstream_model, api_key,
            voice_catalog_source_model_id, timeout_seconds, settings, status
        )
        SELECT
            'Doubao TTS Gateway', 'speech', '中转服务', 'volcengine_tts_v3', 'v3',
            'api_key', 'https://speech-gateway.example.com/api/v3', upstream_model,
            'gateway-runtime-secret', id, 120, settings, 'enabled'
        FROM ai_models
        WHERE id = $1
        RETURNING id
        "#,
    )
    .bind(source_model_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_openai_audio_speech_model(pool: &PgPool, source_model_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        r#"
        INSERT INTO ai_models (
            display_name, model_type, provider_name, api_protocol, protocol_version,
            auth_scheme, request_base_url, upstream_model, api_key,
            voice_catalog_source_model_id, timeout_seconds, settings, status
        )
        SELECT
            'Doubao TTS OpenAI Gateway', 'speech', 'ZeekAI', 'openai_audio_speech', 'v1',
            'bearer', 'https://speech-gateway.example.com/v1', upstream_model,
            'gateway-runtime-secret', id, 120,
            jsonb_set(
                jsonb_set(settings, '{supports_word_timestamps}', 'false'::jsonb),
                '{word_timestamp_languages}', '[]'::jsonb
            ) || '{"catalog_sync_interval_minutes": null}'::jsonb,
            'enabled'
        FROM ai_models
        WHERE id = $1
        RETURNING id
        "#,
    )
    .bind(source_model_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_voice(pool: &PgPool, model_id: Uuid, available: bool) -> String {
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
    let voice_type = "zh_female_fixture_mars_bigtts";
    sqlx::query(
        r#"
        INSERT INTO voice_catalog_entries (
            model_id, voice_type, resource_id, name, languages, emotions,
            is_available, first_seen_sync_id, last_seen_sync_id
        )
        VALUES ($1, $2, 'seed-tts-2.0', '测试音色', $3, $4, $5, $6, $6)
        "#,
    )
    .bind(model_id)
    .bind(voice_type)
    .bind(json!([{"Language": "zh-cn", "Text": "你好", "Flag": "CN"}]))
    .bind(json!([{"Label": "", "Value": "", "Icon": ""}]))
    .bind(available)
    .bind(sync_id)
    .execute(pool)
    .await
    .unwrap();
    voice_type.to_string()
}

async fn seed_asr_model(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        r#"
        INSERT INTO ai_models (
            display_name, model_type, provider_name, api_protocol, protocol_version,
            auth_scheme, request_base_url, upstream_model, api_key, timeout_seconds,
            settings, status
        )
        VALUES (
            'Doubao ASR', 'speech', '火山引擎', 'volcengine_asr_v3', 'v3',
            'api_key', 'https://openspeech.bytedance.com/api/v3',
            'doubao-seed-asr-2.0', 'runtime-secret', 120, $1, 'enabled'
        )
        RETURNING id
        "#,
    )
    .bind(json!({
        "resource_id": "volc.seedasr.auc",
        "supported_audio_formats": ["mp3", "wav"],
        "default_audio_format": "mp3",
        "supported_sample_rates": [16000, 24000, 48000],
        "default_sample_rate": 16000,
        "max_input_characters": null,
        "max_audio_duration_seconds": 3600,
        "supports_word_timestamps": true,
        "word_timestamp_languages": ["*"],
        "catalog_sync_interval_minutes": null,
        "parameters": {}
    }))
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_tos_staging_config(pool: &PgPool, enabled: bool) -> (Uuid, i64) {
    sqlx::query_as(
        r#"
        INSERT INTO tos_staging_tool_configs (
            version, is_current, is_enabled, storage_provider, endpoint, region,
            bucket, object_prefix, access_key, secret_key, signed_url_ttl_seconds,
            max_file_bytes, max_audio_duration_seconds, last_check_status,
            last_check_requested_at, last_checked_at
        ) VALUES (
            1, TRUE, $1, 'volcengine_tos',
            'https://tos-cn-beijing.volces.com', 'cn-beijing', 'private-bucket',
            'novex/asr', 'tos-ak', 'tos-sk', 600, 10485760, 3600,
            CASE WHEN $1 THEN 'succeeded' ELSE 'never' END,
            CASE WHEN $1 THEN NOW() ELSE NULL END,
            CASE WHEN $1 THEN NOW() ELSE NULL END
        ) RETURNING id, version
        "#,
    )
    .bind(enabled)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn tts_intent(model_id: Uuid, voice_type: &str) -> Value {
    json!({
        "task_type": "tts",
        "model_id": model_id,
        "text_content": "你好世界",
        "voice_type": voice_type,
        "language": "zh-cn",
        "parameters": {
            "audio_format": "mp3",
            "sample_rate": 24000,
            "speed_ratio": 1.0
        },
        "generate_subtitle": true,
        "subtitle_segments": ["你好世界"]
    })
}

#[tokio::test]
async fn tts_preflight_and_creation_validate_catalog_confirmation_and_idempotency() {
    let (_admin_pool, pool, _database, test_url) = migrated_pool().await;
    let project_id = seed_project(&pool).await;
    let catalog_source_model_id = seed_tts_model(&pool).await;
    let model_id = seed_shared_tts_model(&pool, catalog_source_model_id).await;
    let voice_type = seed_voice(&pool, catalog_source_model_id, true).await;
    let app = build_app_with_state(app_state(test_url, pool.clone()));
    let base = format!("/api/projects/{project_id}/sound-subtitle/tasks");

    let (status, preflight) = send(
        &app,
        "POST",
        &format!("{base}/preflight"),
        Some(tts_intent(model_id, &voice_type)),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preflight}");
    assert_eq!(preflight["resource_usage"]["character_count"], 4);
    assert_eq!(preflight["resource_usage"]["task_count"], 2);
    assert_eq!(preflight["voice_snapshot"]["emotion"], Value::Null);
    assert_eq!(
        preflight["voice_snapshot"]["catalog_source_model_id"],
        catalog_source_model_id.to_string()
    );
    assert!(preflight["confirmation_token"].as_str().unwrap().len() == 64);
    assert!(preflight.to_string().find("runtime-secret").is_none());

    let mut creation = tts_intent(model_id, &voice_type);
    creation["confirmation_token"] = preflight["confirmation_token"].clone();
    let (status, created) = send(
        &app,
        "POST",
        &base,
        Some(creation.clone()),
        Some("tts-request-1"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["task_type"], "tts");
    assert_eq!(created["status"], "queued");
    assert_eq!(created["resource_usage"]["character_count"], 4);
    assert_eq!(created["generate_subtitle"], true);
    assert_eq!(created["subtitle_segments"], json!(["你好世界"]));

    let (status, reused) = send(&app, "POST", &base, Some(creation), Some("tts-request-1")).await;
    assert_eq!(status, StatusCode::OK, "{reused}");
    assert_eq!(reused["task_id"], created["task_id"]);

    let stored_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sound_subtitle_tasks WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored_count, 1);
}

#[tokio::test]
async fn openai_audio_speech_preflight_allows_audio_and_rejects_tts_subtitles() {
    let (_admin_pool, pool, _database, test_url) = migrated_pool().await;
    let project_id = seed_project(&pool).await;
    let catalog_source_model_id = seed_tts_model(&pool).await;
    let model_id = seed_openai_audio_speech_model(&pool, catalog_source_model_id).await;
    let voice_type = seed_voice(&pool, catalog_source_model_id, true).await;
    let app = build_app_with_state(app_state(test_url, pool.clone()));
    let preflight_url = format!("/api/projects/{project_id}/sound-subtitle/tasks/preflight");

    let subtitle_intent = tts_intent(model_id, &voice_type);
    let (status, error) = send(
        &app,
        "POST",
        &preflight_url,
        Some(subtitle_intent.clone()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{error}");
    assert_eq!(error["error"]["code"], "timestamps_unsupported");

    let mut audio_only = subtitle_intent;
    audio_only["generate_subtitle"] = json!(false);
    audio_only["subtitle_segments"] = json!([]);
    let (status, preflight) = send(&app, "POST", &preflight_url, Some(audio_only), None).await;
    assert_eq!(status, StatusCode::OK, "{preflight}");
    assert_eq!(preflight["resource_usage"]["task_count"], 1);
    assert_eq!(
        preflight["voice_snapshot"]["catalog_source_model_id"],
        catalog_source_model_id.to_string()
    );
}

#[tokio::test]
async fn tts_script_source_is_server_validated_persisted_and_inherited_by_retry() {
    let (_admin_pool, pool, _database, test_url) = migrated_pool().await;
    let project_id = seed_project(&pool).await;
    let model_id = seed_tts_model(&pool).await;
    let voice_type = seed_voice(&pool, model_id, true).await;
    let (script_id, script_updated_at, scene_ids) =
        seed_source_script(&pool, project_id, "approved").await;
    let app = build_app_with_state(app_state(test_url, pool.clone()));
    let base = format!("/api/projects/{project_id}/sound-subtitle/tasks");

    let mut intent = tts_intent(model_id, &voice_type);
    intent["text_content"] = json!("导入后人工编辑的最终旁白");
    intent["subtitle_segments"] = json!(["导入后人工编辑的最终旁白"]);
    intent["source_script_id"] = json!(script_id);
    intent["source_script_updated_at"] = json!(script_updated_at);
    intent["source_script_scene_ids"] = json!([scene_ids[1], scene_ids[0]]);
    let (status, preflight) = send(
        &app,
        "POST",
        &format!("{base}/preflight"),
        Some(intent.clone()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preflight}");
    assert_eq!(
        preflight["source_script_snapshot"]["script_id"],
        script_id.to_string()
    );
    assert_eq!(
        preflight["source_script_snapshot"]["scenes"][0]["scene_id"],
        scene_ids[0].to_string()
    );
    assert_eq!(
        preflight["source_script_snapshot"]["scenes"][1]["scene_id"],
        scene_ids[1].to_string()
    );

    intent["confirmation_token"] = preflight["confirmation_token"].clone();
    let (status, created) =
        send(&app, "POST", &base, Some(intent), Some("tts-script-source")).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["text_content"], "导入后人工编辑的最终旁白");
    assert_eq!(created["source_script_id"], script_id.to_string());
    assert_eq!(
        created["source_script_snapshot"]["title"],
        "别硬扛：稳定前进的方法"
    );
    assert_eq!(
        created["source_script_snapshot"]["scenes"][0]["narration"],
        "允许自己停一停，并不是放弃目标。"
    );

    let failed_task_id = Uuid::parse_str(created["task_id"].as_str().unwrap()).unwrap();
    sqlx::query(
        "UPDATE sound_subtitle_tasks SET status = 'failed', completed_at = NOW() WHERE id = $1",
    )
    .bind(failed_task_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE scripts SET title = '脚本后续已修改' WHERE id = $1")
        .bind(script_id)
        .execute(&pool)
        .await
        .unwrap();

    let mut retry_intent = tts_intent(model_id, &voice_type);
    retry_intent["text_content"] = json!("导入后人工编辑的最终旁白");
    retry_intent["subtitle_segments"] = json!(["导入后人工编辑的最终旁白"]);
    let (_, retry_preflight) = send(
        &app,
        "POST",
        &format!("{base}/preflight"),
        Some(retry_intent.clone()),
        None,
    )
    .await;
    retry_intent["confirmation_token"] = retry_preflight["confirmation_token"].clone();
    let (status, retried) = send(
        &app,
        "POST",
        &format!("{base}/{failed_task_id}/retry"),
        Some(retry_intent),
        Some("tts-script-source-retry"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{retried}");
    assert_eq!(retried["source_script_id"], script_id.to_string());
    assert_eq!(
        retried["source_script_snapshot"],
        created["source_script_snapshot"]
    );
}

#[tokio::test]
async fn tts_script_source_rejects_changed_archived_cross_project_and_unknown_scenes() {
    let (_admin_pool, pool, _database, test_url) = migrated_pool().await;
    let project_id = seed_project(&pool).await;
    let other_project_id = seed_project(&pool).await;
    let model_id = seed_tts_model(&pool).await;
    let voice_type = seed_voice(&pool, model_id, true).await;
    let (script_id, original_updated_at, scene_ids) =
        seed_source_script(&pool, project_id, "draft").await;
    let (other_script_id, other_updated_at, other_scene_ids) =
        seed_source_script(&pool, other_project_id, "draft").await;
    let app = build_app_with_state(app_state(test_url, pool.clone()));
    let preflight_url = format!("/api/projects/{project_id}/sound-subtitle/tasks/preflight");

    sqlx::query("UPDATE scripts SET updated_at = updated_at + INTERVAL '1 second' WHERE id = $1")
        .bind(script_id)
        .execute(&pool)
        .await
        .unwrap();
    let mut changed = tts_intent(model_id, &voice_type);
    changed["source_script_id"] = json!(script_id);
    changed["source_script_updated_at"] = json!(original_updated_at);
    changed["source_script_scene_ids"] = json!(scene_ids);
    let (status, body) = send(&app, "POST", &preflight_url, Some(changed), None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "source_script_changed");

    sqlx::query("UPDATE scripts SET status = 'archived' WHERE id = $1")
        .bind(script_id)
        .execute(&pool)
        .await
        .unwrap();
    let archived_updated_at: DateTime<Utc> =
        sqlx::query_scalar("SELECT updated_at FROM scripts WHERE id = $1")
            .bind(script_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let mut archived = tts_intent(model_id, &voice_type);
    archived["source_script_id"] = json!(script_id);
    archived["source_script_updated_at"] = json!(archived_updated_at);
    archived["source_script_scene_ids"] = json!(scene_ids);
    let (status, body) = send(&app, "POST", &preflight_url, Some(archived), None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "source_script_unavailable");

    let mut cross_project = tts_intent(model_id, &voice_type);
    cross_project["source_script_id"] = json!(other_script_id);
    cross_project["source_script_updated_at"] = json!(other_updated_at);
    cross_project["source_script_scene_ids"] = json!(other_scene_ids);
    let (status, body) = send(&app, "POST", &preflight_url, Some(cross_project), None).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"]["code"], "source_script_project_mismatch");

    sqlx::query("UPDATE scripts SET status = 'draft' WHERE id = $1")
        .bind(script_id)
        .execute(&pool)
        .await
        .unwrap();
    let current_updated_at: DateTime<Utc> =
        sqlx::query_scalar("SELECT updated_at FROM scripts WHERE id = $1")
            .bind(script_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let mut unknown_scene = tts_intent(model_id, &voice_type);
    unknown_scene["source_script_id"] = json!(script_id);
    unknown_scene["source_script_updated_at"] = json!(current_updated_at);
    unknown_scene["source_script_scene_ids"] = json!([Uuid::new_v4()]);
    let (status, body) = send(&app, "POST", &preflight_url, Some(unknown_scene), None).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"]["code"], "source_scene_invalid");
}

#[tokio::test]
async fn tts_preflight_rejects_unsupported_structured_emotion() {
    let (_admin_pool, pool, _database, test_url) = migrated_pool().await;
    let project_id = seed_project(&pool).await;
    let model_id = seed_tts_model(&pool).await;
    let voice_type = seed_voice(&pool, model_id, true).await;
    let app = build_app_with_state(app_state(test_url, pool));
    let mut intent = tts_intent(model_id, &voice_type);
    intent["emotion"] = json!("neutral");

    let (status, body) = send(
        &app,
        "POST",
        &format!("/api/projects/{project_id}/sound-subtitle/tasks/preflight"),
        Some(intent),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"]["code"], "emotion_unsupported");
}

#[tokio::test]
async fn unavailable_voice_and_stale_confirmation_are_blocked() {
    let (_admin_pool, pool, _database, test_url) = migrated_pool().await;
    let project_id = seed_project(&pool).await;
    let model_id = seed_tts_model(&pool).await;
    let voice_type = seed_voice(&pool, model_id, false).await;
    let app = build_app_with_state(app_state(test_url, pool));
    let base = format!("/api/projects/{project_id}/sound-subtitle/tasks");

    let (status, body) = send(
        &app,
        "POST",
        &format!("{base}/preflight"),
        Some(tts_intent(model_id, &voice_type)),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "voice_unavailable");

    let mut creation = tts_intent(model_id, &voice_type);
    creation["confirmation_token"] = json!("0".repeat(64));
    let (status, body) = send(
        &app,
        "POST",
        &base,
        Some(creation),
        Some("stale-confirmation"),
    )
    .await;
    assert_ne!(status, StatusCode::CREATED, "{body}");
}

#[tokio::test]
async fn audio_inspection_is_idempotent_and_asr_uses_server_duration_snapshot() {
    let (_admin_pool, pool, _database, test_url) = migrated_pool().await;
    let project_id = seed_project(&pool).await;
    let model_id = seed_asr_model(&pool).await;
    let material_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO materials (project_id, material_type, file_url, file_name, status)
        VALUES ($1, 'audio', '/assets/uploads/fixture.mp3', 'fixture.mp3', 'active')
        RETURNING id
        "#,
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let app = build_app_with_state(app_state(test_url, pool.clone()));
    let inspection_url =
        format!("/api/projects/{project_id}/audio-materials/{material_id}/inspection");
    let (status, queued) = send(&app, "POST", &inspection_url, None, Some("inspect-fixture")).await;
    assert_eq!(status, StatusCode::CREATED, "{queued}");
    let inspection_id = Uuid::parse_str(queued["inspection_id"].as_str().unwrap()).unwrap();
    let (status, reused) = send(&app, "POST", &inspection_url, None, Some("inspect-fixture")).await;
    assert_eq!(status, StatusCode::OK, "{reused}");
    assert_eq!(reused["inspection_id"], queued["inspection_id"]);

    sqlx::query(
        r#"
        UPDATE audio_material_inspections
        SET status = 'succeeded', source_sha256 = $2, file_size_bytes = 1024,
            duration_ms = 12500, container_format = 'mp3', audio_codec = 'mp3',
            sample_rate_hz = 24000, channel_count = 1, completed_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(inspection_id)
    .bind("a".repeat(64))
    .execute(&pool)
    .await
    .unwrap();

    let intent = json!({
        "task_type": "asr",
        "model_id": model_id,
        "source_audio_material_id": material_id,
        "audio_inspection_id": inspection_id,
        "parameters": {}
    });
    let task_url = format!("/api/projects/{project_id}/sound-subtitle/tasks");
    let (status, body) = send(
        &app,
        "POST",
        &format!("{task_url}/preflight"),
        Some(intent.clone()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"]["code"], "tos_staging_not_configured");

    let (tos_config_id, tos_config_version) = seed_tos_staging_config(&pool, false).await;
    let (status, body) = send(
        &app,
        "POST",
        &format!("{task_url}/preflight"),
        Some(intent.clone()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"]["code"], "tos_staging_disabled");

    sqlx::query("UPDATE tos_staging_tool_configs SET is_enabled = TRUE WHERE id = $1")
        .bind(tos_config_id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = send(
        &app,
        "POST",
        &format!("{task_url}/preflight"),
        Some(intent.clone()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "tos_staging_check_required");

    sqlx::query(
        r#"
        UPDATE tos_staging_tool_configs
        SET last_check_status = 'succeeded', last_check_requested_at = NOW(),
            last_checked_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(tos_config_id)
    .execute(&pool)
    .await
    .unwrap();
    let (status, preflight) = send(
        &app,
        "POST",
        &format!("{task_url}/preflight"),
        Some(intent.clone()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preflight}");
    assert_eq!(preflight["resource_usage"]["audio_duration_ms"], 12500);
    assert_eq!(preflight["resource_usage"]["task_count"], 1);

    let mut creation = intent;
    creation["confirmation_token"] = preflight["confirmation_token"].clone();
    let (status, created) =
        send(&app, "POST", &task_url, Some(creation), Some("asr-fixture")).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["task_type"], "asr");
    assert_eq!(created["audio_inspection_id"], inspection_id.to_string());
    assert_eq!(created["resource_usage"]["audio_duration_ms"], 12500);
    assert_eq!(created["generate_subtitle"], true);
    assert_eq!(created["subtitle_segments"], json!([]));

    let locked_config: (Uuid, i64) = sqlx::query_as(
        r#"
        SELECT tos_staging_config_id, tos_staging_config_version
        FROM sound_subtitle_tasks
        WHERE id = $1
        "#,
    )
    .bind(Uuid::parse_str(created["task_id"].as_str().unwrap()).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(locked_config, (tos_config_id, tos_config_version));
}

#[tokio::test]
async fn failed_task_retry_keeps_parent_audit_and_queued_task_can_be_cancelled() {
    let (_admin_pool, pool, _database, test_url) = migrated_pool().await;
    let project_id = seed_project(&pool).await;
    let model_id = seed_tts_model(&pool).await;
    let voice_type = seed_voice(&pool, model_id, true).await;
    let app = build_app_with_state(app_state(test_url, pool.clone()));
    let base = format!("/api/projects/{project_id}/sound-subtitle/tasks");
    let intent = tts_intent(model_id, &voice_type);
    let (_, preflight) = send(
        &app,
        "POST",
        &format!("{base}/preflight"),
        Some(intent.clone()),
        None,
    )
    .await;
    let mut creation = intent;
    creation["confirmation_token"] = preflight["confirmation_token"].clone();
    let (_, created) = send(
        &app,
        "POST",
        &base,
        Some(creation.clone()),
        Some("retry-parent"),
    )
    .await;
    let failed_task_id = Uuid::parse_str(created["task_id"].as_str().unwrap()).unwrap();
    sqlx::query(
        r#"
        UPDATE sound_subtitle_tasks
        SET status = 'failed', error_code = 'tts_http_error',
            error_summary = '语音供应商返回 HTTP 403',
            error_details = $2,
            upstream_log_id = '20260717150632A1B2C3D4E5F60789',
            attempt_count = 1,
            completed_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(failed_task_id)
    .bind(json!({
        "http_status": 403,
        "provider_error_code": "45000020",
        "provider_error_message": "Permission denied"
    }))
    .execute(&pool)
    .await
    .unwrap();

    let (status, failed) = send(
        &app,
        "GET",
        &format!("{base}/{failed_task_id}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{failed}");
    assert_eq!(failed["error_code"], "tts_http_error");
    assert_eq!(failed["error_details"]["http_status"], 403);
    assert_eq!(
        failed["error_details"]["provider_error_code"],
        "45000020"
    );
    assert_eq!(
        failed["error_details"]["provider_error_message"],
        "Permission denied"
    );
    assert_eq!(
        failed["upstream_log_id"],
        "20260717150632A1B2C3D4E5F60789"
    );
    assert_eq!(failed["request_id"], created["request_id"]);
    assert_eq!(failed["attempt_count"], 1);
    assert!(!failed.to_string().contains("runtime-secret"));

    let (status, retried) = send(
        &app,
        "POST",
        &format!("{base}/{failed_task_id}/retry"),
        Some(creation),
        Some("retry-child"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{retried}");
    assert_eq!(retried["parent_task_id"], failed_task_id.to_string());
    assert_eq!(retried["status"], "queued");

    let retried_id = retried["task_id"].as_str().unwrap();
    let (status, cancelled) = send(
        &app,
        "POST",
        &format!("{base}/{retried_id}/cancel"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cancelled}");
    assert_eq!(cancelled["status"], "cancelled");
}

#[tokio::test]
async fn project_in_flight_limit_is_enforced_under_distinct_idempotency_keys() {
    let (_admin_pool, pool, _database, test_url) = migrated_pool().await;
    let project_id = seed_project(&pool).await;
    let model_id = seed_tts_model(&pool).await;
    let voice_type = seed_voice(&pool, model_id, true).await;
    let app = build_app_with_state(app_state(test_url, pool));
    let base = format!("/api/projects/{project_id}/sound-subtitle/tasks");

    for index in 1..=3 {
        let mut intent = tts_intent(model_id, &voice_type);
        intent["text_content"] = json!(format!("第{index}条"));
        intent["subtitle_segments"] = json!([format!("第{index}条")]);
        let (_, preflight) = send(
            &app,
            "POST",
            &format!("{base}/preflight"),
            Some(intent.clone()),
            None,
        )
        .await;
        intent["confirmation_token"] = preflight["confirmation_token"].clone();
        let (status, body) = send(
            &app,
            "POST",
            &base,
            Some(intent),
            Some(&format!("concurrency-{index}")),
        )
        .await;
        if index <= 2 {
            assert_eq!(status, StatusCode::CREATED, "{body}");
        } else {
            assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
            assert_eq!(body["error"]["code"], "concurrency_limit");
        }
    }
}
