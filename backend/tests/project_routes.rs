use axum::body::Body;
use axum::http::{Request, StatusCode};
use novex_api::{build_app_with_state, AppConfig, AppState};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::{SystemTime, UNIX_EPOCH};
use tower::ServiceExt;

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

async fn create_database(admin_pool: &PgPool, database_name: &str) {
    let query = format!(r#"CREATE DATABASE "{}""#, database_name);
    sqlx::query(&query)
        .execute(admin_pool)
        .await
        .expect("temporary project route database should be created");
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

async fn migrated_pool() -> (PgPool, PgPool, String, String) {
    let base_url = database_url();
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let database_name = format!("video_agent_project_route_test_{}", suffix);
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .expect("admin database should be reachable");
    create_database(&admin_pool, &database_name).await;

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
                        "description": "面向程序员的知识短视频"
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
