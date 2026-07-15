use axum::body::Body;
use axum::http::{Request, StatusCode};
use novex_api::application::material_upload::MAX_UPLOAD_BYTES;
use novex_api::bootstrap::{AppConfig, AppState};
use novex_api::build_app_with_state;
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::fs;
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
        .expect("temporary material route database should be created");
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
    let database_name = format!("video_agent_material_route_test_{}", suffix);
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
        .expect("temporary material route database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .expect("migrations should run for material route test database");

    (admin_pool, test_pool, database_name, test_url)
}

fn app_state(test_url: String, pool: PgPool) -> AppState {
    app_state_with_storage(test_url, pool, "/app/storage/assets".to_string())
}

fn app_state_with_storage(test_url: String, pool: PgPool, asset_storage_root: String) -> AppState {
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
            asset_storage_root,
            asset_generation_providers: vec!["gpt-image-2".to_string(), "jimeng".to_string()],
        },
        pool,
        None,
    )
    .unwrap()
}

const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00, 0x00, 0xb5, 0x1c, 0x0c,
    0x02, 0x00, 0x00, 0x00, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0xf8, 0x0f, 0x00,
    0x01, 0x05, 0x01, 0x01, 0x27, 0x18, 0xe3, 0x66, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
    0xae, 0x42, 0x60, 0x82,
];

fn multipart_upload(file_name: &str, content_type: &str, bytes: &[u8]) -> (String, Vec<u8>) {
    let boundary = "novex-material-upload-boundary";
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file_name\"\r\n\r\n封面素材\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"tags\"\r\n\r\n[\"封面\",\"办公\"]\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\nContent-Type: {content_type}\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (boundary.to_string(), body)
}

fn generated_material_upload(
    file_name: &str,
    content_type: &str,
    bytes: &[u8],
    generation: &Value,
) -> (String, Vec<u8>) {
    let boundary = "novex-generated-material-boundary";
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file_name\"\r\n\r\n作品配音.wav\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"tags\"\r\n\r\n[\"TTS\",\"作品生产\"]\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"generation\"\r\nContent-Type: application/json\r\n\r\n{}\r\n",
            generation
        )
        .as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\nContent-Type: {content_type}\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (boundary.to_string(), body)
}

fn audio_material_upload(audio_usage: &str, bytes: &[u8]) -> (String, Vec<u8>) {
    let boundary = "novex-audio-material-boundary";
    let mut body = Vec::new();
    for (name, value) in [
        ("file_name", "背景音乐.wav"),
        ("tags", "[\"背景音乐\"]"),
        ("audio_usage", audio_usage),
    ] {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"bgm.wav\"\r\nContent-Type: audio/wav\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (boundary.to_string(), body)
}

fn one_second_wav() -> Vec<u8> {
    let sample_rate = 8_000_u32;
    let data_size = sample_rate * 2;
    let mut bytes = Vec::with_capacity(44 + data_size as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());
    bytes.resize(44 + data_size as usize, 0);
    bytes
}

fn stored_file_count(storage_root: &std::path::Path, project_id: Uuid) -> usize {
    let project_directory = storage_root.join("uploads").join(project_id.to_string());
    match fs::read_dir(project_directory) {
        Ok(entries) => entries.count(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => panic!("读取上传目录失败: {error}"),
    }
}

fn stored_generated_file_count(storage_root: &std::path::Path, project_id: Uuid) -> usize {
    let project_directory = storage_root
        .join("generated")
        .join("artifacts")
        .join(project_id.to_string());
    match fs::read_dir(project_directory) {
        Ok(entries) => entries.count(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => panic!("读取生成素材目录失败: {error}"),
    }
}

async fn insert_project(pool: &PgPool) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO projects (name, positioning, description)
        VALUES ('素材账号', '', '')
        RETURNING id
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("project fixture should be inserted")
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn response_text(response: axum::response::Response) -> String {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(body.to_vec()).unwrap()
}

fn material_payload() -> Value {
    json!({
        "material_type": "subtitle",
        "file_url": "https://cdn.example.com/subtitles/demo.vtt",
        "thumbnail_url": "https://cdn.example.com/covers/demo.jpg",
        "file_name": "demo.vtt",
        "tags": ["字幕", "中英双语"],
        "metadata": {
            "language": "zh-CN",
            "subtitle_format": "vtt",
            "source_note": "人工整理",
            "license_note": "内部可用"
        }
    })
}

#[tokio::test]
async fn material_upload_route_persists_png_metadata_and_serves_file() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let storage_root =
        std::env::temp_dir().join(format!("novex-material-route-{}", Uuid::new_v4()));
    let app = build_app_with_state(app_state_with_storage(
        test_url,
        test_pool.clone(),
        storage_root.to_string_lossy().into_owned(),
    ));
    let (boundary, body) = multipart_upload("cover.png", "image/png", PNG_1X1);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/materials/upload"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let uploaded = response_json(response).await;
    assert_eq!(uploaded["file_name"], "封面素材");
    assert_eq!(uploaded["material_type"], "image");
    assert_eq!(uploaded["tags"], json!(["封面", "办公"]));
    assert_eq!(uploaded["metadata"]["source"], "user_upload");
    assert_eq!(uploaded["metadata"]["storage_provider"], "local");
    assert_eq!(uploaded["metadata"]["mime_type"], "image/png");
    assert_eq!(uploaded["metadata"]["format"], "png");
    assert_eq!(uploaded["metadata"]["width"], 1);
    assert_eq!(uploaded["metadata"]["height"], 1);
    let file_url = uploaded["file_url"].as_str().unwrap();
    assert!(file_url.starts_with(&format!("/assets/uploads/{project_id}/")));

    let file_response = app
        .oneshot(
            Request::builder()
                .uri(file_url)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(file_response.status(), StatusCode::OK);
    let file_bytes = axum::body::to_bytes(file_response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(file_bytes.as_ref(), PNG_1X1);

    let _ = fs::remove_dir_all(storage_root);
    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn material_upload_route_classifies_existing_audio_for_work_mixing() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let storage_root =
        std::env::temp_dir().join(format!("novex-audio-material-route-{}", Uuid::new_v4()));
    let app = build_app_with_state(app_state_with_storage(
        test_url,
        test_pool.clone(),
        storage_root.to_string_lossy().into_owned(),
    ));
    let (boundary, body) = audio_material_upload("bgm", &one_second_wav());

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/materials/upload"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let uploaded = response_json(response).await;
    assert_eq!(uploaded["material_type"], "audio");
    assert_eq!(uploaded["source"], "user_upload");
    assert_eq!(uploaded["audio_usage"], "bgm");
    assert_eq!(uploaded["metadata"]["audio_usage"], "bgm");

    let list_response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/projects/{project_id}/materials?type=audio&audio_usage=bgm&source=user_upload"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    assert_eq!(
        response_json(list_response).await["materials"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let _ = fs::remove_dir_all(storage_root);
    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn generated_material_route_validates_stores_filters_and_exposes_read_only_snapshot() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let work_id = Uuid::new_v4();
    let work_version_id = Uuid::new_v4();
    let storage_root =
        std::env::temp_dir().join(format!("novex-generated-material-route-{}", Uuid::new_v4()));
    let app = build_app_with_state(app_state_with_storage(
        test_url,
        test_pool.clone(),
        storage_root.to_string_lossy().into_owned(),
    ));
    let generation = json!({
        "work_id": work_id,
        "work_version_id": work_version_id,
        "generation_run_id": Uuid::new_v4(),
        "generation_step_id": Uuid::new_v4(),
        "artifact_role": "tts_audio",
        "audio_usage": "tts",
        "model_snapshot": {
            "model_id": Uuid::new_v4(),
            "provider": "volcengine",
            "upstream_model": "doubao-tts",
            "capability_version": "2026-07"
        },
        "voice_snapshot": {
            "voice_id": "zh_female_1",
            "language": "zh-CN",
            "style": "自然"
        },
        "prompt_snapshot": {"text_summary": "测试配音文本", "character_count": 6},
        "timeline_snapshot": {"version": "timeline-v1"},
        "resource_usage": {"duration_sec": 1.0, "character_count": 6},
        "request_trace_id": "trace-safe-1"
    });
    let wav = one_second_wav();
    let (boundary, body) = generated_material_upload("voice.wav", "audio/wav", &wav, &generation);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/materials/generated"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let created = response_json(response).await;
    let material_id = created["material_id"].as_str().unwrap();
    assert_eq!(created["material_type"], "audio");
    assert_eq!(created["audio_usage"], "tts");
    assert_eq!(created["source"], "work_generation");
    assert_eq!(created["work_id"], work_id.to_string());
    assert_eq!(created["work_version_id"], work_version_id.to_string());
    assert_eq!(created["generation"]["artifact_role"], "tts_audio");
    assert_eq!(
        created["generation"]["model_snapshot"]["provider"],
        "volcengine"
    );
    let file_url = created["file_url"].as_str().unwrap();
    assert!(file_url.starts_with(&format!("/assets/generated/artifacts/{project_id}/")));

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/projects/{project_id}/materials?type=audio&audio_usage=tts&source=work_generation&work_id={work_id}&work_version_id={work_version_id}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let listed = response_json(list_response).await;
    assert_eq!(listed["materials"].as_array().unwrap().len(), 1);
    assert_eq!(listed["materials"][0]["material_id"], material_id);

    let archive_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/materials/{material_id}/status"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"status": "archived"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(archive_response.status(), StatusCode::OK);

    let selectable_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/projects/{project_id}/materials?type=audio&audio_usage=tts"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response_json(selectable_response).await["materials"]
            .as_array()
            .unwrap()
            .len(),
        0,
        "archived audio must not be selectable for new work"
    );

    let historical_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/materials/{material_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(historical_response.status(), StatusCode::OK);
    assert_eq!(
        response_json(historical_response).await["generation"]["request_trace_id"],
        "trace-safe-1",
        "archiving must preserve historical generation snapshots"
    );

    let (second_boundary, second_body) =
        generated_material_upload("voice.wav", "audio/wav", &wav, &generation);
    let regenerated_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/materials/generated"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={second_boundary}"),
                )
                .body(Body::from(second_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(regenerated_response.status(), StatusCode::CREATED);
    let regenerated = response_json(regenerated_response).await;
    assert_ne!(regenerated["material_id"].as_str(), Some(material_id));
    assert_ne!(regenerated["file_url"].as_str(), Some(file_url));
    assert_eq!(stored_generated_file_count(&storage_root, project_id), 2);

    let regenerated_id = regenerated["material_id"].as_str().unwrap();
    let mut subtitle_generation = generation.clone();
    subtitle_generation["artifact_role"] = json!("subtitle");
    subtitle_generation
        .as_object_mut()
        .unwrap()
        .remove("audio_usage");
    subtitle_generation["alignment_source"] = json!("tts_timestamp");
    subtitle_generation["source_audio_material_id"] = json!(regenerated_id);
    subtitle_generation["generation_step_id"] = json!(Uuid::new_v4());
    let (subtitle_boundary, subtitle_body) = generated_material_upload(
        "captions.srt",
        "application/x-subrip",
        b"1\n00:00:00,000 --> 00:00:01,000\ntest\n",
        &subtitle_generation,
    );
    let subtitle_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/materials/generated"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={subtitle_boundary}"),
                )
                .body(Body::from(subtitle_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(subtitle_response.status(), StatusCode::CREATED);
    let subtitle = response_json(subtitle_response).await;
    assert_eq!(subtitle["material_type"], "subtitle");
    assert_eq!(subtitle["generation"]["alignment_source"], "tts_timestamp");
    assert_eq!(
        subtitle["generation"]["source_audio_material_id"],
        regenerated_id
    );
    assert_eq!(stored_generated_file_count(&storage_root, project_id), 3);

    let _ = fs::remove_dir_all(storage_root);
    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn material_detail_recursively_redacts_historical_credentials() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    sqlx::query("ALTER TABLE materials DROP CONSTRAINT materials_metadata_no_credentials_check")
        .execute(&test_pool)
        .await
        .unwrap();
    let material_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO materials (project_id, material_type, file_url, file_name, metadata)
        VALUES (
            $1,
            'audio',
            'https://cdn.example.com/historical.mp3',
            '历史生成音频.mp3',
            '{"source":"ai_generated","model_snapshot":{"api_key":"hidden","headers":{"Authorization":"Bearer hidden"}}}'
        )
        RETURNING id
        "#,
    )
    .bind(project_id)
    .fetch_one(&test_pool)
    .await
    .unwrap();
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/materials/{material_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let material = response_json(response).await;
    assert!(material["metadata"]["model_snapshot"]
        .get("api_key")
        .is_none());
    assert!(material["metadata"]["model_snapshot"]["headers"]
        .get("Authorization")
        .is_none());

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn generated_material_route_rejects_invalid_media_and_credentials_without_side_effects() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let storage_root =
        std::env::temp_dir().join(format!("novex-generated-material-route-{}", Uuid::new_v4()));
    let app = build_app_with_state(app_state_with_storage(
        test_url,
        test_pool.clone(),
        storage_root.to_string_lossy().into_owned(),
    ));
    let base_generation = json!({
        "work_id": Uuid::new_v4(),
        "work_version_id": Uuid::new_v4(),
        "generation_run_id": Uuid::new_v4(),
        "generation_step_id": Uuid::new_v4(),
        "artifact_role": "tts_audio",
        "audio_usage": "tts",
        "model_snapshot": {"model_id": Uuid::new_v4()},
        "voice_snapshot": {},
        "prompt_snapshot": {},
        "timeline_snapshot": {},
        "resource_usage": {}
    });
    let mut monetary_generation = base_generation.clone();
    monetary_generation["resource_usage"] = json!({"duration_sec": 1, "estimated_cost": 0.1});

    let invalid_cases = [
        (base_generation.clone(), b"not-a-wav".to_vec()),
        (
            json!({
                "work_id": Uuid::new_v4(),
                "work_version_id": Uuid::new_v4(),
                "generation_run_id": Uuid::new_v4(),
                "generation_step_id": Uuid::new_v4(),
                "artifact_role": "tts_audio",
                "audio_usage": "tts",
                "model_snapshot": {"headers": {"Authorization": "Bearer secret"}},
                "voice_snapshot": {},
                "prompt_snapshot": {},
                "timeline_snapshot": {},
                "resource_usage": {}
            }),
            one_second_wav(),
        ),
        (monetary_generation, one_second_wav()),
    ];

    for (generation, bytes) in invalid_cases {
        let (boundary, body) =
            generated_material_upload("voice.wav", "audio/wav", &bytes, &generation);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/projects/{project_id}/materials/generated"))
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    let count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM materials WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&test_pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    assert_eq!(stored_generated_file_count(&storage_root, project_id), 0);

    let _ = fs::remove_dir_all(storage_root);
    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn material_upload_route_rejects_unsupported_file_without_side_effects() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let storage_root =
        std::env::temp_dir().join(format!("novex-material-route-{}", Uuid::new_v4()));
    let app = build_app_with_state(app_state_with_storage(
        test_url,
        test_pool.clone(),
        storage_root.to_string_lossy().into_owned(),
    ));
    let (boundary, body) = multipart_upload("payload.exe", "application/octet-stream", b"MZ");

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/materials/upload"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM materials WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&test_pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    assert!(!storage_root.join("uploads").exists());

    let _ = fs::remove_dir_all(storage_root);
    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn material_upload_route_rejects_missing_file_without_side_effects() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let storage_root =
        std::env::temp_dir().join(format!("novex-material-route-{}", Uuid::new_v4()));
    let app = build_app_with_state(app_state_with_storage(
        test_url,
        test_pool.clone(),
        storage_root.to_string_lossy().into_owned(),
    ));
    let boundary = "novex-material-upload-boundary";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file_name\"\r\n\r\n封面素材\r\n--{boundary}--\r\n"
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/materials/upload"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(stored_file_count(&storage_root, project_id), 0);

    let _ = fs::remove_dir_all(storage_root);
    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn material_upload_route_returns_stable_error_for_malformed_multipart() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let storage_root =
        std::env::temp_dir().join(format!("novex-material-route-{}", Uuid::new_v4()));
    let app = build_app_with_state(app_state_with_storage(
        test_url,
        test_pool.clone(),
        storage_root.to_string_lossy().into_owned(),
    ));
    let boundary = "novex-material-upload-boundary";
    let malformed_requests = [
        ("multipart/form-data".to_string(), Body::empty()),
        (
            format!("multipart/form-data; boundary={boundary}"),
            Body::from(format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"broken.png\"\r\n\r\ntruncated"
            )),
        ),
    ];

    for (content_type, body) in malformed_requests {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/projects/{project_id}/materials/upload"))
                    .header("content-type", content_type)
                    .body(body)
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response_text(response).await,
            r#"{"error":"上传请求格式无效"}"#
        );
    }
    assert_eq!(stored_file_count(&storage_root, project_id), 0);

    let _ = fs::remove_dir_all(storage_root);
    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn material_upload_route_rejects_missing_project_before_writing_file() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = Uuid::new_v4();
    let storage_root =
        std::env::temp_dir().join(format!("novex-material-route-{}", Uuid::new_v4()));
    let app = build_app_with_state(app_state_with_storage(
        test_url,
        test_pool.clone(),
        storage_root.to_string_lossy().into_owned(),
    ));
    let (boundary, body) = multipart_upload("cover.png", "image/png", PNG_1X1);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/materials/upload"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(stored_file_count(&storage_root, project_id), 0);

    let _ = fs::remove_dir_all(storage_root);
    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn material_upload_route_rejects_oversized_request_with_413() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let storage_root =
        std::env::temp_dir().join(format!("novex-material-route-{}", Uuid::new_v4()));
    let app = build_app_with_state(app_state_with_storage(
        test_url,
        test_pool.clone(),
        storage_root.to_string_lossy().into_owned(),
    ));
    let boundary = "novex-material-upload-boundary";

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/materials/upload"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .header("content-length", MAX_UPLOAD_BYTES + 1024 * 1024 + 1)
                .body(Body::from(format!("--{boundary}--\r\n")))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(stored_file_count(&storage_root, project_id), 0);

    let _ = fs::remove_dir_all(storage_root);
    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn material_upload_route_removes_file_when_database_insert_fails() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let storage_root =
        std::env::temp_dir().join(format!("novex-material-route-{}", Uuid::new_v4()));
    let app = build_app_with_state(app_state_with_storage(
        test_url,
        test_pool.clone(),
        storage_root.to_string_lossy().into_owned(),
    ));
    sqlx::query("ALTER TABLE materials RENAME TO materials_unavailable")
        .execute(&test_pool)
        .await
        .unwrap();
    let (boundary, body) = multipart_upload("cover.png", "image/png", PNG_1X1);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/materials/upload"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(stored_file_count(&storage_root, project_id), 0);

    let _ = fs::remove_dir_all(storage_root);
    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn material_routes_create_list_update_archive_and_restore() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/materials"))
                .header("content-type", "application/json")
                .body(Body::from(material_payload().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = response_json(create_response).await;
    let material_id = created["material_id"].as_str().unwrap().to_string();
    assert_eq!(created["project_id"], project_id.to_string());
    assert_eq!(created["material_type"], "subtitle");
    assert_eq!(
        created["thumbnail_url"],
        "https://cdn.example.com/covers/demo.jpg"
    );
    assert_eq!(created["status"], "active");

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/projects/{project_id}/materials"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let listed = response_json(list_response).await;
    assert_eq!(listed["materials"].as_array().unwrap().len(), 1);
    assert_eq!(listed["materials"][0]["material_id"], material_id);

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/materials/{material_id}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "material_type": "subtitle",
                        "file_url": "https://cdn.example.com/subtitles/demo-updated.vtt",
                        "thumbnail_url": "",
                        "file_name": "demo-updated.vtt",
                        "tags": ["字幕", "已校对"],
                        "metadata": {
                            "language": "zh-CN",
                            "subtitle_format": "vtt",
                            "source_note": "人工校对"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);
    let updated = response_json(update_response).await;
    assert_eq!(updated["file_name"], "demo-updated.vtt");
    assert_eq!(
        updated["file_url"],
        "https://cdn.example.com/subtitles/demo.vtt"
    );
    assert_eq!(
        updated["thumbnail_url"],
        "https://cdn.example.com/covers/demo.jpg"
    );
    assert_eq!(updated["tags"], json!(["字幕", "已校对"]));
    assert_eq!(updated["metadata"]["source_note"], "人工整理");

    let archive_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/materials/{material_id}/status"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "archived" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(archive_response.status(), StatusCode::OK);
    assert_eq!(response_json(archive_response).await["status"], "archived");

    let active_after_archive = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/projects/{project_id}/materials"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response_json(active_after_archive).await["materials"]
            .as_array()
            .unwrap()
            .len(),
        0
    );

    let archived_list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/projects/{project_id}/materials?status=archived"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response_json(archived_list).await["materials"][0]["material_id"],
        material_id
    );

    let restore_response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/materials/{material_id}/status"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "active" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(restore_response.status(), StatusCode::OK);
    assert_eq!(response_json(restore_response).await["status"], "active");

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn material_routes_reject_invalid_payloads() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));
    let unknown_material_id = Uuid::parse_str("99999999-9999-4999-8999-999999999999").unwrap();

    for payload in [
        json!({ "material_type": "video", "file_url": "https://cdn.example.com/video.mp4", "file_name": "" }),
        json!({ "material_type": "video", "file_url": "ftp://cdn.example.com/video.mp4", "file_name": "video.mp4" }),
        json!({ "material_type": "document", "file_url": "https://cdn.example.com/doc.pdf", "file_name": "doc.pdf" }),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/projects/{project_id}/materials"))
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    let invalid_status = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/materials/{unknown_material_id}/status"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "deleted" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_status.status(), StatusCode::BAD_REQUEST);

    let unknown_get = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/materials/{unknown_material_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unknown_get.status(), StatusCode::NOT_FOUND);

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
