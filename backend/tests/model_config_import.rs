use novex_api::model_config_import::{import_legacy_model_config, LegacyModelImportConfig};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
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
    let database_name = format!("video_agent_model_import_test_{}", Uuid::new_v4().simple());
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

fn complete_config() -> LegacyModelImportConfig {
    LegacyModelImportConfig {
        text_api_key: Some("text-secret".to_string()),
        text_base_url: Some("https://text.example/responses".to_string()),
        text_model: Some("text-upstream".to_string()),
        text_timeout_seconds: 120,
        text_reasoning_effort: Some("high".to_string()),
        text_max_output_tokens: Some(3000),
        image_api_key: Some("image-secret".to_string()),
        image_base_url: Some("https://images.example/v1/images/generations".to_string()),
        image_model: Some("image-upstream".to_string()),
        jimeng_access_key: Some("jimeng-ak".to_string()),
        jimeng_secret_key: Some("jimeng-sk".to_string()),
        jimeng_request_key: Some("jimeng-request".to_string()),
        jimeng_width: 1328,
        jimeng_height: 1328,
    }
}

#[tokio::test]
async fn import_creates_three_models_once_and_never_overwrites_admin_edits() {
    let (admin_pool, pool, database_name) = migrated_pool().await;

    let first = import_legacy_model_config(&pool, complete_config())
        .await
        .unwrap();
    assert_eq!(first.created.len(), 3);
    assert!(first.skipped.is_empty());
    let rows = sqlx::query(
        "SELECT source_key, api_protocol, request_base_url, api_key, api_secret FROM ai_models ORDER BY source_key",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 3);
    let text = rows
        .iter()
        .find(|row| row.get::<String, _>("source_key") == "legacy:text-openai")
        .unwrap();
    assert_eq!(text.get::<String, _>("api_protocol"), "openai_responses");
    assert_eq!(
        text.get::<String, _>("request_base_url"),
        "https://text.example/v1"
    );
    let image = rows
        .iter()
        .find(|row| row.get::<String, _>("source_key") == "legacy:image-openai")
        .unwrap();
    assert_eq!(
        image.get::<String, _>("request_base_url"),
        "https://images.example/v1"
    );

    sqlx::query("UPDATE ai_models SET request_base_url = 'https://admin-edited.example/v1' WHERE source_key = 'legacy:text-openai'")
        .execute(&pool)
        .await
        .unwrap();
    let second = import_legacy_model_config(&pool, complete_config())
        .await
        .unwrap();
    assert!(second.created.is_empty());
    assert_eq!(second.skipped.len(), 3);
    let edited = sqlx::query_scalar::<_, String>(
        "SELECT request_base_url FROM ai_models WHERE source_key = 'legacy:text-openai'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(edited, "https://admin-edited.example/v1");

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn import_skips_incomplete_credentials_instead_of_creating_enabled_models() {
    let (admin_pool, pool, database_name) = migrated_pool().await;
    let mut config = complete_config();
    config.text_api_key = None;
    config.image_api_key = None;
    config.jimeng_secret_key = None;

    let outcome = import_legacy_model_config(&pool, config).await.unwrap();

    assert!(outcome.created.is_empty());
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ai_models")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
