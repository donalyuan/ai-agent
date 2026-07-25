use axum::{response::IntoResponse, routing::post, Router};
use novex_agent::BoundModelResolver;
use novex_api::model_routing::{
    ModelClientResolver, ModelResolveError, PostgresModelClientResolver,
};
use novex_api::repositories::{
    AiModelRepository, AiModelStatus, CreateAiModelInput, PostgresAiModelRepository,
};
use novex_model::{ApiProtocol, AuthScheme, LLMPrompt, ModelType};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use tokio::net::TcpListener;
use uuid::Uuid;

mod support;

use support::test_database::TestDatabase;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@biga-postgres:5432/video_agent".to_string()
    })
}

fn with_database_name(database_url: &str, database_name: &str) -> String {
    let slash = database_url.rfind('/').unwrap();
    format!("{}{}", &database_url[..=slash], database_name)
}

async fn migrated_pool() -> (PgPool, PgPool, TestDatabase) {
    let base_url = database_url();
    let database_name = format!("video_agent_model_routing_test_{}", Uuid::new_v4().simple());
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
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(&test_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (admin_pool, pool, database_name)
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

fn model_input(base_url: String, model_type: ModelType) -> CreateAiModelInput {
    let (api_protocol, settings, reasoning_effort, max_output_tokens) = match model_type {
        ModelType::Text => (
            ApiProtocol::OpenAiResponses,
            json!({"context_window": 128000}),
            Some("high".to_string()),
            Some(2048),
        ),
        ModelType::Image => (
            ApiProtocol::OpenAiImages,
            json!({"supported_sizes": ["1024x1024"], "default_size": "1024x1024"}),
            None,
            None,
        ),
        ModelType::Video | ModelType::Speech => unreachable!(),
    };
    CreateAiModelInput {
        display_name: "Resolved Model".to_string(),
        model_type,
        provider_name: "Fake Provider".to_string(),
        api_protocol,
        protocol_version: "v1".to_string(),
        auth_scheme: AuthScheme::Bearer,
        request_base_url: base_url,
        upstream_model: "fake-upstream".to_string(),
        api_key: "resolver-secret-key".to_string(),
        api_secret: None,
        catalog_access_key: None,
        catalog_secret_key: None,
        voice_catalog_source_model_id: None,
        timeout_seconds: 5,
        reasoning_effort,
        max_output_tokens,
        settings,
        sort_order: 0,
        remark: String::new(),
        status: AiModelStatus::Enabled,
        source: "admin".to_string(),
        source_key: None,
    }
}

#[tokio::test]
async fn audited_resolver_reloads_credentials_and_behavior_from_ai_models() {
    let (admin_pool, pool, database_name) = migrated_pool().await;
    let repository = PostgresAiModelRepository::new(pool.clone());
    let model = repository
        .create(model_input(
            "https://example.invalid/v1".to_string(),
            ModelType::Text,
        ))
        .await
        .unwrap();
    let resolver = PostgresModelClientResolver::new(repository);

    let initial = BoundModelResolver::resolve(&resolver, model.id)
        .await
        .unwrap();
    sqlx::query("UPDATE ai_models SET api_key = 'rotated-secret-key' WHERE id = $1")
        .bind(model.id)
        .execute(&pool)
        .await
        .unwrap();
    let rotated = BoundModelResolver::resolve(&resolver, model.id)
        .await
        .unwrap();

    assert_eq!(initial.behavior_fingerprint, rotated.behavior_fingerprint);
    assert_eq!(initial.capabilities, rotated.capabilities);
    assert_eq!(rotated.known_secrets, vec!["rotated-secret-key"]);
    assert!(!serde_json::to_string(&rotated.model_snapshot)
        .unwrap()
        .contains("rotated-secret-key"));

    sqlx::query("UPDATE ai_models SET upstream_model = 'behavior-drifted' WHERE id = $1")
        .bind(model.id)
        .execute(&pool)
        .await
        .unwrap();
    let drifted = BoundModelResolver::resolve(&resolver, model.id)
        .await
        .unwrap();
    assert_ne!(rotated.behavior_fingerprint, drifted.behavior_fingerprint);

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn resolver_builds_protocol_driven_client_and_non_sensitive_snapshot() {
    async fn handler() -> impl IntoResponse {
        let body = concat!(
            "event: response.output_text.delta\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\n\n",
            "event: response.completed\n",
            "data: {\"type\":\"response.completed\"}\n\n"
        );
        (
            [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
            body,
        )
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route("/v1/responses", post(handler)),
        )
        .await
        .unwrap();
    });
    let (admin_pool, pool, database_name) = migrated_pool().await;
    let repository = PostgresAiModelRepository::new(pool.clone());
    let model = repository
        .create(model_input(format!("http://{address}/v1"), ModelType::Text))
        .await
        .unwrap();
    let resolver = PostgresModelClientResolver::new(repository.clone());

    let resolved = resolver.text_client(model.id).await.unwrap();
    assert_eq!(resolved.snapshot.model_id, model.id);
    assert_eq!(resolved.snapshot.api_protocol, ApiProtocol::OpenAiResponses);
    let snapshot_json = serde_json::to_value(&resolved.snapshot).unwrap();
    assert!(snapshot_json.get("api_key").is_none());
    assert!(snapshot_json.get("api_secret").is_none());
    let output = resolved
        .client
        .generate_script(LLMPrompt {
            system: "system".to_string(),
            user: "user".to_string(),
            max_output_tokens: None,
            output_schema: None,
        })
        .await
        .unwrap();
    assert_eq!(output, "ok");

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn resolver_rejects_disabled_and_non_text_models_before_client_creation() {
    let (admin_pool, pool, database_name) = migrated_pool().await;
    let repository = PostgresAiModelRepository::new(pool.clone());
    let text = repository
        .create(model_input(
            "https://example.invalid/v1".to_string(),
            ModelType::Text,
        ))
        .await
        .unwrap();
    let deleted = repository
        .create(model_input(
            "https://example.invalid/v1".to_string(),
            ModelType::Text,
        ))
        .await
        .unwrap();
    let image = repository
        .create(model_input(
            "https://example.invalid/v1".to_string(),
            ModelType::Image,
        ))
        .await
        .unwrap();
    sqlx::query("UPDATE ai_models SET status = 'disabled', is_default = FALSE WHERE id = $1")
        .bind(text.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE ai_models SET status = 'deleted', deleted_at = NOW(), is_default = FALSE WHERE id = $1",
    )
    .bind(deleted.id)
    .execute(&pool)
    .await
    .unwrap();
    let resolver = PostgresModelClientResolver::new(repository);

    assert!(matches!(
        resolver.text_client(text.id).await,
        Err(ModelResolveError::Disabled(id)) if id == text.id
    ));
    assert!(matches!(
        resolver.text_client(image.id).await,
        Err(ModelResolveError::TypeMismatch { id, .. }) if id == image.id
    ));
    assert!(matches!(
        resolver.text_client(deleted.id).await,
        Err(ModelResolveError::NotFound(id) | ModelResolveError::Disabled(id)) if id == deleted.id
    ));

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn resolver_preserves_requested_model_id_for_invalid_stored_config() {
    let (admin_pool, pool, database_name) = migrated_pool().await;
    let repository = PostgresAiModelRepository::new(pool.clone());
    let model = repository
        .create(model_input(
            "https://example.invalid/v1".to_string(),
            ModelType::Text,
        ))
        .await
        .unwrap();
    sqlx::query("UPDATE ai_models SET api_key = '' WHERE id = $1")
        .bind(model.id)
        .execute(&pool)
        .await
        .unwrap();
    let resolver = PostgresModelClientResolver::new(repository);

    assert!(matches!(
        resolver.text_client(model.id).await,
        Err(ModelResolveError::InvalidConfig(id)) if id == model.id
    ));

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
