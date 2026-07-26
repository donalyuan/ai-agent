use novex_ai_core::DefinitionRegistry;
use novex_api::model_context_inventory::{
    build_model_context_inventory, BindingBehaviorState, ContextReadiness,
};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::path::PathBuf;
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
    let database_name = format!("video_agent_context_inventory_{}", Uuid::new_v4().simple());
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
    let guard = TestDatabase::new(&admin_url, &database_name);
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(&test_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (admin_pool, pool, guard)
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

fn definitions() -> DefinitionRegistry {
    DefinitionRegistry::load(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../agent-definitions"))
        .unwrap()
}

#[tokio::test]
async fn inventory_is_read_only_secret_free_and_distinguishes_missing_stable_and_drifted_behavior()
{
    let (admin_pool, pool, database_name) = migrated_pool().await;
    let ready_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO ai_models (
            display_name, model_type, provider_name, api_protocol, auth_scheme,
            request_base_url, upstream_model, api_key, max_output_tokens,
            context_window, tokenizer_profile_key, tokenizer_profile_version, settings
        ) VALUES (
            'Ready', 'text', 'fixture', 'openai_responses', 'bearer',
            'https://example.invalid/v1', 'gpt-5.6-luna', 'canary-secret', 4096,
            128000, 'openai.o200k', '1.0.0', '{}'
        ) RETURNING id
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO ai_models (
            display_name, model_type, provider_name, api_protocol, auth_scheme,
            request_base_url, upstream_model, api_key, max_output_tokens, settings
        ) VALUES (
            'Missing', 'text', 'fixture', 'openai_responses', 'bearer',
            'https://example.invalid/v1', 'opaque-current-name', 'second-secret', 4096, '{}'
        )
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let initial = build_model_context_inventory(&pool, &definitions())
        .await
        .unwrap();
    assert!(!initial.credential_columns_selected);
    assert!(!serde_json::to_string(&initial)
        .unwrap()
        .contains("canary-secret"));
    let ready = initial
        .models
        .iter()
        .find(|item| item.model_id == ready_id)
        .unwrap();
    assert_eq!(ready.readiness, ContextReadiness::Ready);
    assert_eq!(ready.binding_behavior_state, BindingBehaviorState::Unbound);
    assert_eq!(ready.upstream_model_evidence, "opaque_not_inferred");
    assert_eq!(
        initial
            .models
            .iter()
            .find(|item| item.display_name == "Missing")
            .unwrap()
            .readiness,
        ContextReadiness::ConfigurationMissing
    );

    let conversation_id: Uuid = sqlx::query_scalar(
        "INSERT INTO agent_conversations (agent_type,title) VALUES ('script','inventory') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO agent_conversation_bindings (
            conversation_id, agent_key, agent_version, agent_digest, prompt_bindings,
            registry_digest, model_id, behavior_fingerprint, model_capabilities, binding_status
        ) VALUES ($1, 'fixture', '1.0.0', $2, '{"node":{"key":"fixture","version":"1.0.0"}}',
                  $2, $3, $4, '{"text":true}', 'executable')
        "#,
    )
    .bind(conversation_id)
    .bind("a".repeat(64))
    .bind(ready_id)
    .bind(ready.behavior_fingerprint.as_ref().unwrap())
    .execute(&pool)
    .await
    .unwrap();
    let stable = build_model_context_inventory(&pool, &definitions())
        .await
        .unwrap();
    assert_eq!(
        stable
            .models
            .iter()
            .find(|item| item.model_id == ready_id)
            .unwrap()
            .binding_behavior_state,
        BindingBehaviorState::Stable
    );

    sqlx::query("UPDATE ai_models SET context_window = context_window + 1 WHERE id = $1")
        .bind(ready_id)
        .execute(&pool)
        .await
        .unwrap();
    let drifted = build_model_context_inventory(&pool, &definitions())
        .await
        .unwrap();
    assert_eq!(
        drifted
            .models
            .iter()
            .find(|item| item.model_id == ready_id)
            .unwrap()
            .binding_behavior_state,
        BindingBehaviorState::BehaviorDrift
    );

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
