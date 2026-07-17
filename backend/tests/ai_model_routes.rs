use axum::body::Body;
use axum::http::{Request, StatusCode};
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
    let database_name = format!(
        "video_agent_ai_model_route_test_{}",
        Uuid::new_v4().simple()
    );
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
    let database_name = TestDatabase::new(&admin_url, &database_name);
    let test_pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&test_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .unwrap();
    (admin_pool, test_pool, database_name, test_url)
}

async fn drop_database(admin_pool: &PgPool, database_name: &str) {
    let _ = sqlx::query(&format!(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{database_name}'"
    ))
    .execute(admin_pool)
    .await;
    let _ = sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{database_name}""#))
        .execute(admin_pool)
        .await;
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
    if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body).unwrap_or_else(|error| {
            json!({
                "raw": String::from_utf8_lossy(&body),
                "parse_error": error.to_string()
            })
        })
    }
}

fn text_payload(name: &str) -> Value {
    json!({
        "display_name": name,
        "model_type": "text",
        "provider_name": "OpenAI",
        "api_protocol": "openai_responses",
        "protocol_version": "v1",
        "auth_scheme": "bearer",
        "request_base_url": "https://api.example.com/v1",
        "upstream_model": "gpt-test",
        "api_key": "secret-key-1234",
        "api_secret": null,
        "timeout_seconds": 120,
        "reasoning_effort": "high",
        "max_output_tokens": 4096,
        "settings": {},
        "sort_order": 10,
        "remark": "测试模型",
        "is_default": false
    })
}

fn image_responses_payload(name: &str) -> Value {
    json!({
        "display_name": name,
        "model_type": "image",
        "provider_name": "zeek-ai",
        "api_protocol": "openai_responses",
        "protocol_version": "v1",
        "auth_scheme": "bearer",
        "request_base_url": "https://api.example.com/v1",
        "upstream_model": "gpt-image-2",
        "api_key": "secret-key-1234",
        "api_secret": null,
        "timeout_seconds": 120,
        "reasoning_effort": null,
        "max_output_tokens": null,
        "settings": {
            "supported_sizes": ["1024x1024"],
            "default_size": "1024x1024",
            "max_images_per_request": 4
        },
        "sort_order": 10,
        "remark": "Responses 图片模型",
        "is_default": false
    })
}

fn ark_image_payload(name: &str, request_base_url: &str) -> Value {
    json!({
        "display_name": name,
        "model_type": "image",
        "provider_name": "火山引擎",
        "api_protocol": "volcengine_ark_images",
        "protocol_version": "v3",
        "auth_scheme": "bearer",
        "request_base_url": request_base_url,
        "upstream_model": "doubao-seedream-5-0-260128",
        "api_key": "ark-secret-key-1234",
        "api_secret": null,
        "timeout_seconds": 120,
        "reasoning_effort": null,
        "max_output_tokens": null,
        "settings": {
            "supported_sizes": [],
            "default_size": null,
            "max_images_per_request": 1
        },
        "sort_order": 10,
        "remark": "Seedream Ark",
        "is_default": false
    })
}

fn speech_payload(name: &str, protocol: &str) -> Value {
    let is_tts = protocol == "volcengine_tts_v3";
    json!({
        "display_name": name,
        "model_type": "speech",
        "provider_name": "火山引擎",
        "api_protocol": protocol,
        "protocol_version": "v3",
        "auth_scheme": "api_key",
        "request_base_url": "https://openspeech.bytedance.com/api/v3",
        "upstream_model": if is_tts { "doubao-seed-tts-2.0" } else { "doubao-seed-asr-2.0" },
        "api_key": "speech-secret-key-1234",
        "api_secret": null,
        "catalog_access_key": if is_tts { Some("catalog-access-1234") } else { None },
        "catalog_secret_key": if is_tts { Some("catalog-secret-1234") } else { None },
        "timeout_seconds": 120,
        "reasoning_effort": null,
        "max_output_tokens": null,
        "settings": {
            "resource_id": if is_tts { "seed-tts-2.0" } else { "volc.seedasr.auc" },
            "supported_audio_formats": ["mp3", "wav"],
            "default_audio_format": "mp3",
            "supported_sample_rates": [24000],
            "default_sample_rate": 24000,
            "max_input_characters": if is_tts { Some(3000) } else { None },
            "max_audio_duration_seconds": if is_tts { None } else { Some(7200) },
            "supports_word_timestamps": true,
            "word_timestamp_languages": if is_tts { json!(["zh-cn", "en-us"]) } else { json!(["*"]) },
            "catalog_sync_interval_minutes": if is_tts { Some(1440) } else { None },
            "parameters": {}
        },
        "sort_order": 10,
        "remark": "语音模型",
        "is_default": false
    })
}

fn openai_audio_speech_payload(name: &str, source_model_id: &str) -> Value {
    json!({
        "display_name": name,
        "model_type": "speech",
        "provider_name": "ZeekAI",
        "api_protocol": "openai_audio_speech",
        "protocol_version": "v1",
        "auth_scheme": "bearer",
        "request_base_url": "https://speech-gateway.example.com/v1/audio/speech/",
        "upstream_model": "doubao-seed-tts-2.0",
        "api_key": "gateway-speech-key-1234",
        "api_secret": null,
        "catalog_access_key": null,
        "catalog_secret_key": null,
        "voice_catalog_mode": "shared",
        "voice_catalog_source_model_id": source_model_id,
        "timeout_seconds": 120,
        "reasoning_effort": null,
        "max_output_tokens": null,
        "settings": {
            "resource_id": "seed-tts-2.0",
            "supported_audio_formats": ["mp3", "wav"],
            "default_audio_format": "mp3",
            "supported_sample_rates": [24000],
            "default_sample_rate": 24000,
            "max_input_characters": 3000,
            "max_audio_duration_seconds": null,
            "supports_word_timestamps": false,
            "word_timestamp_languages": [],
            "catalog_sync_interval_minutes": null,
            "parameters": {
                "speed_ratio": {"type": "number", "minimum": 0.25, "maximum": 4.0}
            }
        },
        "sort_order": 10,
        "remark": "OpenAI Audio Speech 中转",
        "is_default": false
    })
}

fn tos_tool_payload(version: Option<i64>) -> Value {
    json!({
        "version": version,
        "enabled": false,
        "storage_provider": "volcengine_tos",
        "endpoint": "https://tos-cn-beijing.volces.com",
        "region": "cn-beijing",
        "bucket": "novex-private-staging",
        "object_prefix": "novex/asr",
        "access_key": "tos-access-key-1234",
        "secret_key": "tos-secret-key-1234",
        "signed_url_ttl_seconds": 600,
        "max_file_bytes": 104857600,
        "max_audio_duration_seconds": 7200
    })
}

async fn send(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    let request_body = match body {
        Some(value) => {
            builder = builder.header("content-type", "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    let response = app
        .clone()
        .oneshot(builder.body(request_body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    (status, response_json(response).await)
}

#[tokio::test]
async fn admin_crud_masks_credentials_and_options_omit_sensitive_configuration() {
    let (admin_pool, pool, database_name, test_url) = migrated_pool().await;
    let app = build_app_with_state(app_state(test_url, pool.clone()));
    let (status, created) = send(
        &app,
        "POST",
        "/api/admin/models",
        Some(text_payload("Text A")),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "unexpected response: {created}"
    );
    assert_eq!(created["api_key_masked"], "secr****1234");
    assert_eq!(created["api_key_configured"], true);
    assert!(created.get("api_key").is_none());
    assert!(created.get("api_secret").is_none());
    assert_eq!(
        created["is_default"], true,
        "first enabled model is default"
    );
    let model_id = created["model_id"].as_str().unwrap();

    let (status, detail) = send(&app, "GET", &format!("/api/admin/models/{model_id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["api_key_masked"], "secr****1234");

    let (status, listed) = send(
        &app,
        "GET",
        "/api/admin/models?type=text&status=enabled&provider=OpenAI&protocol=openai_responses&q=Text",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["models"].as_array().unwrap().len(), 1);

    let (status, options) = send(&app, "GET", "/api/model-options?type=text", None).await;
    assert_eq!(status, StatusCode::OK);
    let option = &options["models"][0];
    assert_eq!(option["model_id"], model_id);
    assert_eq!(option["is_default"], true);
    for forbidden in [
        "request_base_url",
        "api_key",
        "api_key_masked",
        "api_secret_masked",
        "settings",
        "timeout_seconds",
    ] {
        assert!(
            option.get(forbidden).is_none(),
            "options leaked {forbidden}"
        );
    }

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn admin_rejects_image_responses_protocol() {
    let (admin_pool, pool, database_name, test_url) = migrated_pool().await;
    let app = build_app_with_state(app_state(test_url, pool.clone()));

    let (status, body) = send(
        &app,
        "POST",
        "/api/admin/models",
        Some(image_responses_payload("Responses Image")),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "invalid_model_config");

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn admin_accepts_and_normalizes_volcengine_ark_images_protocol() {
    let (admin_pool, pool, database_name, test_url) = migrated_pool().await;
    let app = build_app_with_state(app_state(test_url, pool.clone()));

    let mut payload = ark_image_payload(
        "Seedream Ark",
        "https://ark.cn-beijing.volces.com/api/v3/images/generations/",
    );
    payload["api_secret"] = json!("legacy-secret-must-not-be-stored");
    let (status, created) = send(&app, "POST", "/api/admin/models", Some(payload)).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "unexpected response: {created}"
    );
    assert_eq!(created["api_protocol"], "volcengine_ark_images");
    assert_eq!(created["auth_scheme"], "bearer");
    assert_eq!(
        created["request_base_url"],
        "https://ark.cn-beijing.volces.com/api/v3"
    );
    assert_eq!(created["api_secret_configured"], false);

    let (invalid_status, invalid) = send(
        &app,
        "POST",
        "/api/admin/models",
        Some(ark_image_payload(
            "Invalid Ark URL",
            "https://ark.cn-beijing.volces.com/api/v3?region=cn-beijing",
        )),
    )
    .await;
    assert_eq!(invalid_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(invalid["error"]["code"], "invalid_model_config");

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn admin_manages_tts_and_asr_models_without_leaking_credentials() {
    let (admin_pool, pool, database_name, test_url) = migrated_pool().await;
    let app = build_app_with_state(app_state(test_url, pool.clone()));

    let (tts_status, tts) = send(
        &app,
        "POST",
        "/api/admin/models",
        Some(speech_payload("Doubao TTS", "volcengine_tts_v3")),
    )
    .await;
    assert_eq!(
        tts_status,
        StatusCode::CREATED,
        "unexpected response: {tts}"
    );
    assert_eq!(tts["model_type"], "speech");
    assert_eq!(tts["auth_scheme"], "api_key");
    assert_eq!(tts["catalog_access_key_masked"], "cata****1234");
    assert_eq!(tts["catalog_secret_key_masked"], "cata****1234");
    assert_eq!(tts["voice_catalog_mode"], "official_sync");
    assert_eq!(tts["voice_catalog_source_model_id"], Value::Null);
    for forbidden in ["api_key", "catalog_access_key", "catalog_secret_key"] {
        assert!(tts.get(forbidden).is_none(), "response leaked {forbidden}");
    }

    let (asr_status, asr) = send(
        &app,
        "POST",
        "/api/admin/models",
        Some(speech_payload("Doubao ASR", "volcengine_asr_v3")),
    )
    .await;
    assert_eq!(
        asr_status,
        StatusCode::CREATED,
        "unexpected response: {asr}"
    );
    assert_eq!(tts["is_default"], true);
    assert_eq!(
        asr["is_default"], true,
        "ASR must have an independent default scope"
    );
    assert!(asr.get("staging_config").is_none());

    let tts_id = tts["model_id"].as_str().unwrap();
    let mut update = speech_payload("Doubao TTS Updated", "volcengine_tts_v3");
    update["version"] = tts["version"].clone();
    update["api_key"] = json!("");
    update["catalog_access_key"] = json!("");
    update["catalog_secret_key"] = json!("");
    update["is_default"] = json!(true);
    let (update_status, updated) = send(
        &app,
        "PUT",
        &format!("/api/admin/models/{tts_id}"),
        Some(update),
    )
    .await;
    assert_eq!(
        update_status,
        StatusCode::OK,
        "unexpected response: {updated}"
    );
    assert_eq!(updated["display_name"], "Doubao TTS Updated");
    assert_eq!(updated["api_key_masked"], "spee****1234");
    assert_eq!(updated["catalog_access_key_masked"], "cata****1234");

    let (options_status, options) = send(&app, "GET", "/api/model-options?type=speech", None).await;
    assert_eq!(options_status, StatusCode::OK);
    assert_eq!(options["models"].as_array().unwrap().len(), 2);

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn tos_cleanup_pending_blocks_only_tool_changes() {
    let (admin_pool, pool, database_name, test_url) = migrated_pool().await;
    let app = build_app_with_state(app_state(test_url, pool.clone()));
    let (_, asr) = send(
        &app,
        "POST",
        "/api/admin/models",
        Some(speech_payload("Doubao ASR", "volcengine_asr_v3")),
    )
    .await;
    let model_id = Uuid::parse_str(asr["model_id"].as_str().unwrap()).unwrap();
    let mut premature_enable = tos_tool_payload(None);
    premature_enable["enabled"] = json!(true);
    let (enable_status, enable_error) = send(
        &app,
        "PUT",
        "/api/tools/tos-staging",
        Some(premature_enable),
    )
    .await;
    assert_eq!(enable_status, StatusCode::CONFLICT, "{enable_error}");
    assert_eq!(enable_error["error"]["code"], "tos_staging_check_required");
    let (tool_status, tool) = send(
        &app,
        "PUT",
        "/api/tools/tos-staging",
        Some(tos_tool_payload(None)),
    )
    .await;
    assert_eq!(tool_status, StatusCode::OK, "{tool}");
    assert_eq!(tool["configured"], true);
    assert_eq!(tool["version"], 1);
    assert_eq!(tool["access_key_masked"], "tos-****1234");
    assert_eq!(tool["secret_key_masked"], "tos-****1234");
    assert!(tool.get("access_key").is_none());
    assert!(tool.get("secret_key").is_none());
    let (check_status, check) = send(
        &app,
        "POST",
        "/api/tools/tos-staging/check",
        Some(json!({ "version": 1 })),
    )
    .await;
    assert_eq!(check_status, StatusCode::ACCEPTED, "{check}");
    assert_eq!(check["last_check_status"], "queued");
    assert!(check["last_check_requested_at"].is_string());
    assert!(check.get("access_key").is_none());
    assert!(check.get("secret_key").is_none());

    let (stale_check_status, stale_check) = send(
        &app,
        "POST",
        "/api/tools/tos-staging/check",
        Some(json!({ "version": 0 })),
    )
    .await;
    assert_eq!(stale_check_status, StatusCode::CONFLICT, "{stale_check}");
    assert_eq!(stale_check["error"]["code"], "tos_staging_version_conflict");

    sqlx::query(
        r#"
        UPDATE tos_staging_tool_configs
        SET last_check_status = 'succeeded', last_checked_at = NOW(),
            check_locked_at = NULL, check_worker_id = NULL
        WHERE version = 1
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();
    let mut enable_tool = tos_tool_payload(Some(1));
    enable_tool["enabled"] = json!(true);
    enable_tool["access_key"] = json!("");
    enable_tool["secret_key"] = json!("");
    let (enable_status, enabled_tool) =
        send(&app, "PUT", "/api/tools/tos-staging", Some(enable_tool)).await;
    assert_eq!(enable_status, StatusCode::OK, "{enabled_tool}");
    assert_eq!(enabled_tool["version"], 2);
    assert_eq!(enabled_tool["enabled"], true);
    assert_eq!(enabled_tool["last_check_status"], "succeeded");
    let tos_config_id = Uuid::parse_str(enabled_tool["config_id"].as_str().unwrap()).unwrap();
    let tos_config_version = enabled_tool["version"].as_i64().unwrap();
    let project_id: Uuid =
        sqlx::query_scalar("INSERT INTO projects (name) VALUES ('ASR cleanup guard') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let material_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO materials (project_id, material_type, file_url, file_name, status)
        VALUES ($1, 'audio', '/assets/guard.mp3', 'guard.mp3', 'active')
        RETURNING id
        "#,
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let inspection_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO audio_material_inspections (
            project_id, material_id, status, idempotency_key, source_sha256,
            file_size_bytes, duration_ms, container_format, audio_codec,
            sample_rate_hz, channel_count, completed_at
        )
        VALUES ($1, $2, 'succeeded', 'guard', $3, 100, 1000, 'mp3', 'mp3', 24000, 1, NOW())
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(material_id)
    .bind("a".repeat(64))
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO sound_subtitle_tasks (
            project_id, task_type, status, model_id, audio_inspection_id,
            source_audio_material_id, confirmation_snapshot, idempotency_key,
            tos_staging_config_id, tos_staging_config_version,
            staging_object_key, staging_source_sha256, staging_status, completed_at
        )
        VALUES ($1, 'asr', 'failed', $2, $3, $4, '{}', 'guard', $5, $6, $7, $8, 'cleanup_pending', NOW())
        "#,
    )
    .bind(project_id)
    .bind(model_id)
    .bind(inspection_id)
    .bind(material_id)
    .bind(tos_config_id)
    .bind(tos_config_version)
    .bind(format!("novex/asr/{project_id}/guard.mp3"))
    .bind("a".repeat(64))
    .execute(&pool)
    .await
    .unwrap();

    let mut update = speech_payload("Doubao ASR updated", "volcengine_asr_v3");
    update["version"] = asr["version"].clone();
    update["api_key"] = json!("");
    update["is_default"] = json!(true);
    let (status, body) = send(
        &app,
        "PUT",
        &format!("/api/admin/models/{model_id}"),
        Some(update),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let updated_version = body["version"].as_i64().unwrap();

    let (status, body) = send(
        &app,
        "PUT",
        &format!("/api/admin/models/{model_id}/status"),
        Some(json!({
            "version": updated_version,
            "status": "disabled",
            "allow_no_default": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let mut tool_update = tos_tool_payload(Some(2));
    tool_update["bucket"] = json!("novex-private-staging-v2");
    tool_update["access_key"] = json!("");
    tool_update["secret_key"] = json!("");
    let (status, body) = send(&app, "PUT", "/api/tools/tos-staging", Some(tool_update)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "tos_staging_cleanup_pending");

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn voice_catalog_sync_is_idempotent_and_observable_from_admin_and_workspace() {
    let (admin_pool, pool, database_name, test_url) = migrated_pool().await;
    let app = build_app_with_state(app_state(test_url, pool.clone()));
    let (_, model) = send(
        &app,
        "POST",
        "/api/admin/models",
        Some(speech_payload("Doubao TTS", "volcengine_tts_v3")),
    )
    .await;
    let model_id = model["model_id"].as_str().unwrap();

    let (sync_status, first_sync) = send(
        &app,
        "POST",
        &format!("/api/admin/models/{model_id}/voice-catalog/sync"),
        None,
    )
    .await;
    assert_eq!(
        sync_status,
        StatusCode::CREATED,
        "unexpected response: {first_sync}"
    );
    assert_eq!(first_sync["status"], "queued");
    assert_eq!(first_sync["trigger_source"], "admin");

    let (duplicate_status, duplicate) = send(
        &app,
        "POST",
        &format!("/api/speech/models/{model_id}/voice-catalog/check"),
        None,
    )
    .await;
    assert_eq!(duplicate_status, StatusCode::OK);
    assert_eq!(duplicate["sync_id"], first_sync["sync_id"]);

    let sync_id = Uuid::parse_str(first_sync["sync_id"].as_str().unwrap()).unwrap();
    sqlx::query(
        "UPDATE voice_catalog_syncs SET status = 'succeeded', completed_at = NOW(), page_count = 1, speaker_count = 1 WHERE id = $1",
    )
    .bind(sync_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO voice_catalog_entries (
            model_id, voice_type, resource_id, name, languages, emotions,
            first_seen_sync_id, last_seen_sync_id
        )
        VALUES (
            $1, 'fixture_voice', 'seed-tts-2.0', 'Fixture Voice',
            '[{"Language":"zh-cn","Text":"hello","Flag":"CN"}]',
            '[{"Label":"neutral","Value":"neutral","Icon":""}]', $2, $2
        )
        "#,
    )
    .bind(Uuid::parse_str(model_id).unwrap())
    .bind(sync_id)
    .execute(&pool)
    .await
    .unwrap();

    let (catalog_status, catalog) = send(
        &app,
        "GET",
        &format!("/api/speech/models/{model_id}/voice-catalog"),
        None,
    )
    .await;
    assert_eq!(
        catalog_status,
        StatusCode::OK,
        "unexpected response: {catalog}"
    );
    assert_eq!(catalog["last_sync"]["status"], "succeeded");
    assert_eq!(catalog["voices"][0]["voice_type"], "fixture_voice");
    assert_eq!(catalog["voices"][0]["is_available"], true);
    for forbidden in ["api_key", "catalog_access_key", "catalog_secret_key"] {
        assert!(
            catalog.get(forbidden).is_none(),
            "catalog leaked {forbidden}"
        );
    }

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn gateway_tts_reuses_matching_official_catalog_and_protects_its_source() {
    let (admin_pool, pool, database_name, test_url) = migrated_pool().await;
    let app = build_app_with_state(app_state(test_url, pool.clone()));
    let (source_status, source) = send(
        &app,
        "POST",
        "/api/admin/models",
        Some(speech_payload("Seed TTS 官方目录", "volcengine_tts_v3")),
    )
    .await;
    assert_eq!(source_status, StatusCode::CREATED, "{source}");
    let source_id = source["model_id"].as_str().unwrap();

    let mut gateway_payload = speech_payload("Seed TTS 中转", "volcengine_tts_v3");
    gateway_payload["provider_name"] = json!("中转服务");
    gateway_payload["request_base_url"] = json!("https://speech-gateway.example.com/api/v3");
    gateway_payload["api_key"] = json!("gateway-speech-key-1234");
    gateway_payload["catalog_access_key"] = Value::Null;
    gateway_payload["catalog_secret_key"] = Value::Null;
    gateway_payload["voice_catalog_mode"] = json!("shared");
    gateway_payload["voice_catalog_source_model_id"] = json!(source_id);
    let (gateway_status, gateway) = send(
        &app,
        "POST",
        "/api/admin/models",
        Some(gateway_payload.clone()),
    )
    .await;
    assert_eq!(gateway_status, StatusCode::CREATED, "{gateway}");
    assert_eq!(gateway["voice_catalog_mode"], "shared");
    assert_eq!(gateway["voice_catalog_source_model_id"], source_id);
    assert_eq!(
        gateway["voice_catalog_source_display_name"],
        "Seed TTS 官方目录"
    );
    assert_eq!(gateway["catalog_access_key_configured"], false);
    assert_eq!(gateway["catalog_secret_key_configured"], false);
    let gateway_id = gateway["model_id"].as_str().unwrap();

    let mut mismatched = gateway_payload.clone();
    mismatched["display_name"] = json!("错误上游模型");
    mismatched["upstream_model"] = json!("doubao-seed-tts-other");
    let (mismatch_status, mismatch_body) =
        send(&app, "POST", "/api/admin/models", Some(mismatched)).await;
    assert_eq!(mismatch_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(mismatch_body["error"]["code"], "invalid_model_config");

    let mut chained = gateway_payload.clone();
    chained["display_name"] = json!("错误共享链");
    chained["voice_catalog_source_model_id"] = json!(gateway_id);
    let (chain_status, chain_body) = send(&app, "POST", "/api/admin/models", Some(chained)).await;
    assert_eq!(chain_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(chain_body["error"]["code"], "invalid_model_config");

    let (sync_status, sync) = send(
        &app,
        "POST",
        &format!("/api/admin/models/{gateway_id}/voice-catalog/sync"),
        None,
    )
    .await;
    assert_eq!(sync_status, StatusCode::CREATED, "{sync}");
    assert_eq!(sync["model_id"], source_id);
    let sync_id = Uuid::parse_str(sync["sync_id"].as_str().unwrap()).unwrap();
    let source_uuid = Uuid::parse_str(source_id).unwrap();
    sqlx::query(
        "UPDATE voice_catalog_syncs SET status = 'succeeded', completed_at = NOW(), page_count = 1, speaker_count = 1 WHERE id = $1",
    )
    .bind(sync_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO voice_catalog_entries (
            model_id, voice_type, resource_id, name, languages, emotions,
            first_seen_sync_id, last_seen_sync_id
        )
        VALUES (
            $1, 'shared_fixture_voice', 'seed-tts-2.0', 'Shared Fixture Voice',
            '[{"Language":"zh-cn","Text":"你好"}]', '[]', $2, $2
        )
        "#,
    )
    .bind(source_uuid)
    .bind(sync_id)
    .execute(&pool)
    .await
    .unwrap();

    let (catalog_status, catalog) = send(
        &app,
        "GET",
        &format!("/api/speech/models/{gateway_id}/voice-catalog"),
        None,
    )
    .await;
    assert_eq!(catalog_status, StatusCode::OK, "{catalog}");
    assert_eq!(catalog["model_id"], gateway_id);
    assert_eq!(catalog["source_model_id"], source_id);
    assert_eq!(catalog["voices"][0]["voice_type"], "shared_fixture_voice");

    let (disable_status, disable_body) = send(
        &app,
        "PUT",
        &format!("/api/admin/models/{source_id}/status"),
        Some(json!({
            "version": source["version"],
            "status": "disabled",
            "replacement_model_id": gateway_id,
            "allow_no_default": false
        })),
    )
    .await;
    assert_eq!(disable_status, StatusCode::CONFLICT);
    assert_eq!(disable_body["error"]["code"], "voice_catalog_source_in_use");

    let mut source_update = speech_payload("Seed TTS 官方目录", "volcengine_tts_v3");
    source_update["version"] = source["version"].clone();
    source_update["upstream_model"] = json!("doubao-seed-tts-other");
    source_update["is_default"] = json!(true);
    let (update_status, update_body) = send(
        &app,
        "PUT",
        &format!("/api/admin/models/{source_id}"),
        Some(source_update),
    )
    .await;
    assert_eq!(update_status, StatusCode::CONFLICT);
    assert_eq!(update_body["error"]["code"], "voice_catalog_source_in_use");

    let (delete_status, delete_body) = send(
        &app,
        "DELETE",
        &format!("/api/admin/models/{source_id}"),
        Some(json!({
            "version": source["version"],
            "replacement_model_id": gateway_id,
            "allow_no_default": false
        })),
    )
    .await;
    assert_eq!(delete_status, StatusCode::CONFLICT);
    assert_eq!(delete_body["error"]["code"], "voice_catalog_source_in_use");

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn openai_audio_speech_gateway_uses_bearer_and_reuses_official_catalog() {
    let (admin_pool, pool, database_name, test_url) = migrated_pool().await;
    let app = build_app_with_state(app_state(test_url, pool.clone()));
    let (source_status, source) = send(
        &app,
        "POST",
        "/api/admin/models",
        Some(speech_payload("Seed TTS 官方目录", "volcengine_tts_v3")),
    )
    .await;
    assert_eq!(source_status, StatusCode::CREATED, "{source}");
    let source_id = source["model_id"].as_str().unwrap();

    let payload = openai_audio_speech_payload("Seed TTS OpenAI 中转", source_id);
    let (status, created) = send(&app, "POST", "/api/admin/models", Some(payload.clone())).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["api_protocol"], "openai_audio_speech");
    assert_eq!(created["auth_scheme"], "bearer");
    assert_eq!(
        created["request_base_url"],
        "https://speech-gateway.example.com/v1"
    );
    assert_eq!(created["voice_catalog_mode"], "shared");
    assert_eq!(created["voice_catalog_source_model_id"], source_id);
    assert_eq!(created["settings"]["supports_word_timestamps"], false);

    let mut update = openai_audio_speech_payload("Seed TTS OpenAI 中转", source_id);
    update["version"] = created["version"].clone();
    update["api_key"] = json!("replacement-gateway-speech-key");
    update["is_default"] = json!(true);
    let (update_status, updated) = send(
        &app,
        "PUT",
        &format!(
            "/api/admin/models/{}",
            created["model_id"].as_str().unwrap()
        ),
        Some(update),
    )
    .await;
    assert_eq!(update_status, StatusCode::OK, "{updated}");
    assert_eq!(updated["version"], created["version"].as_i64().unwrap() + 1);
    assert_eq!(updated["api_key_masked"], "repl****-key");
    assert_eq!(updated["is_default"], true);
    assert_eq!(updated["voice_catalog_source_model_id"], source_id);

    let mut missing_source = payload;
    missing_source["display_name"] = json!("缺少目录来源");
    missing_source["voice_catalog_mode"] = json!("official_sync");
    missing_source["voice_catalog_source_model_id"] = Value::Null;
    let (invalid_status, invalid) =
        send(&app, "POST", "/api/admin/models", Some(missing_source)).await;
    assert_eq!(
        invalid_status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{invalid}"
    );
    assert_eq!(invalid["error"]["code"], "invalid_model_config");
    assert_eq!(
        invalid["error"]["message"],
        "模型配置无效：OpenAI Audio Speech 中转必须选择官方音色目录来源"
    );

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn update_keeps_blank_credentials_and_returns_stable_version_errors() {
    let (admin_pool, pool, database_name, test_url) = migrated_pool().await;
    let app = build_app_with_state(app_state(test_url, pool.clone()));
    let (_, created) = send(
        &app,
        "POST",
        "/api/admin/models",
        Some(text_payload("Text A")),
    )
    .await;
    let model_id = created["model_id"].as_str().unwrap();
    let version = created["version"].as_i64().unwrap();
    let mut update = text_payload("Text A Updated");
    update["version"] = json!(version);
    update["api_key"] = json!("");
    update["api_secret"] = json!("");
    update["is_default"] = json!(true);
    let (status, updated) = send(
        &app,
        "PUT",
        &format!("/api/admin/models/{model_id}"),
        Some(update.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["display_name"], "Text A Updated");
    assert_eq!(updated["api_key_masked"], "secr****1234");

    let (status, conflict) = send(
        &app,
        "PUT",
        &format!("/api/admin/models/{model_id}"),
        Some(update),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(conflict["error"]["code"], "model_version_conflict");

    let mut invalid = text_payload("Invalid");
    invalid["model_type"] = json!("image");
    let (status, invalid_body) = send(&app, "POST", "/api/admin/models", Some(invalid)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(invalid_body["error"]["code"], "invalid_model_config");

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn status_and_delete_routes_preserve_default_and_history_rules() {
    let (admin_pool, pool, database_name, test_url) = migrated_pool().await;
    let app = build_app_with_state(app_state(test_url, pool.clone()));
    let (_, first) = send(
        &app,
        "POST",
        "/api/admin/models",
        Some(text_payload("Text A")),
    )
    .await;
    let (_, second) = send(
        &app,
        "POST",
        "/api/admin/models",
        Some(text_payload("Text B")),
    )
    .await;
    let first_id = first["model_id"].as_str().unwrap();
    let second_id = second["model_id"].as_str().unwrap();

    let (status, required) = send(
        &app,
        "PUT",
        &format!("/api/admin/models/{first_id}/status"),
        Some(json!({
            "version": first["version"],
            "status": "disabled",
            "allow_no_default": false
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(required["error"]["code"], "replacement_model_required");

    let (status, disabled) = send(
        &app,
        "PUT",
        &format!("/api/admin/models/{first_id}/status"),
        Some(json!({
            "version": first["version"],
            "status": "disabled",
            "replacement_model_id": second_id,
            "allow_no_default": false
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(disabled["status"], "disabled");

    sqlx::query(
        "INSERT INTO agent_runs (agent_type, status, model_id) VALUES ('script', 'succeeded', $1)",
    )
    .bind(Uuid::parse_str(first_id).unwrap())
    .execute(&pool)
    .await
    .unwrap();
    let (status, deleted) = send(
        &app,
        "DELETE",
        &format!("/api/admin/models/{first_id}"),
        Some(json!({
            "version": disabled["version"],
            "allow_no_default": false
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(deleted["deletion"], "logical");
    assert_eq!(deleted["model"]["status"], "deleted");
    assert!(deleted["model"].get("api_key").is_none());

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
