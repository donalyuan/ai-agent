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
    assert!(updated["thumbnail_url"].is_null());
    assert_eq!(updated["tags"], json!(["字幕", "已校对"]));

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
