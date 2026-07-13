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
