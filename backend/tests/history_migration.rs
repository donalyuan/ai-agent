use novex_ai_core::{sha256_hex, DefinitionRegistry};
use novex_api::application::history_migration::{
    HistoryMigrationBackupEvidence, MigrationDisposition, PostgresHistoryMigrator,
};
use novex_model::{ApiProtocol, ModelExecutionSnapshot, ModelType};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{fs, path::Path, sync::Arc};
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

async fn test_pool() -> (PgPool, TestDatabase) {
    let base_url = database_url();
    let name = format!("video_agent_history_migration_{}", Uuid::new_v4().simple());
    let admin_url = with_database_name(&base_url, "postgres");
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .unwrap();
    sqlx::query(&format!(r#"CREATE DATABASE "{name}""#))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&with_database_name(&base_url, &name))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (pool, TestDatabase::new(&admin_url, &name))
}

async fn conversation(pool: &PgPool, agent_type: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO agent_conversations (agent_type,title) VALUES ($1,$2) RETURNING id",
    )
    .bind(agent_type)
    .bind(format!("{agent_type} history"))
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn migration_plan_is_dry_run_and_apply_is_idempotent_without_fabricated_calls() {
    let (pool, _database) = test_pool().await;
    let model_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO ai_models (
            display_name, model_type, provider_name, api_protocol, auth_scheme,
            request_base_url, upstream_model, api_key, max_output_tokens, settings
        ) VALUES ('history', 'text', 'fixture', 'openai_responses', 'bearer',
                  'https://example.invalid/v1', 'history-model', 'secret', 100,
                  '{"context_window":8192}') RETURNING id
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let trusted = conversation(&pool, "script").await;
    let pending = conversation(&pool, "topic").await;
    let message_id: Uuid = sqlx::query_scalar(
        "INSERT INTO agent_messages (conversation_id,role,content) VALUES ($1,'user','history') RETURNING id",
    )
    .bind(trusted)
    .fetch_one(&pool)
    .await
    .unwrap();
    let snapshot = ModelExecutionSnapshot {
        model_id,
        display_name: "history".into(),
        model_type: ModelType::Text,
        provider_name: "fixture".into(),
        api_protocol: ApiProtocol::OpenAiResponses,
        protocol_version: "v1".into(),
        request_base_url: "https://example.invalid/v1".into(),
        upstream_model: "history-model".into(),
        reasoning_effort: None,
        timeout_seconds: 5,
        max_output_tokens: Some(100),
        settings: json!({"context_window":8192}),
    };
    let trusted_run: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO agent_runs (agent_type,status,input,model_id,model_snapshot)
        VALUES ('script','succeeded',$1,$2,$3) RETURNING id
        "#,
    )
    .bind(json!({"conversation_id":trusted}))
    .bind(model_id)
    .bind(serde_json::to_value(snapshot).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    let partial_run: Uuid = sqlx::query_scalar(
        "INSERT INTO agent_runs (agent_type,status,input) VALUES ('topic','succeeded','{}') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let definitions = Arc::new(
        DefinitionRegistry::load(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("agent-definitions"),
        )
        .unwrap(),
    );
    let migrator = PostgresHistoryMigrator::new(pool.clone(), definitions);
    let plan = migrator.plan().await.unwrap();
    assert!(plan.dry_run);
    assert_eq!(plan.items.len(), 4);
    assert!(plan.items.iter().any(|item| {
        item.entity_id == trusted && item.disposition == MigrationDisposition::AutoBindWithModel
    }));
    assert!(plan.items.iter().any(|item| {
        item.entity_id == pending
            && item.disposition == MigrationDisposition::AwaitFirstModelBinding
    }));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM agent_conversation_bindings")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );

    let backup_path =
        std::env::temp_dir().join(format!("novex-history-backup-{}.json", Uuid::new_v4()));
    let backup_bytes = serde_json::to_vec(&json!({
        "conversations":[trusted,pending],
        "messages":[message_id],
        "runs":[trusted_run,partial_run]
    }))
    .unwrap();
    fs::write(&backup_path, &backup_bytes).unwrap();
    let backup = HistoryMigrationBackupEvidence {
        reference: backup_path.display().to_string(),
        sha256: sha256_hex(&backup_bytes),
    };
    let applied = migrator.apply(&backup).await.unwrap();
    assert!(!applied.dry_run);
    let trusted_binding: (String, Option<Uuid>, String) = sqlx::query_as(
        "SELECT agent_key,model_id,binding_status FROM agent_conversation_bindings WHERE conversation_id=$1",
    )
    .bind(trusted)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        trusted_binding,
        ("video.script".into(), Some(model_id), "executable".into())
    );
    let pending_binding: (String, Option<Uuid>, String) = sqlx::query_as(
        "SELECT agent_key,model_id,binding_status FROM agent_conversation_bindings WHERE conversation_id=$1",
    )
    .bind(pending)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        pending_binding,
        ("video.topic".into(), None, "definition_bound".into())
    );
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT legacy_partial_audit FROM agent_runs WHERE id=$1",
    )
    .bind(trusted_run)
    .fetch_one(&pool)
    .await
    .unwrap());
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT legacy_partial_audit FROM agent_runs WHERE id=$1",
    )
    .bind(partial_run)
    .fetch_one(&pool)
    .await
    .unwrap());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM model_calls")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM agent_messages WHERE id=$1")
            .bind(message_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        message_id
    );
    assert!(migrator.plan().await.unwrap().items.is_empty());
    assert!(migrator.apply(&backup).await.unwrap().items.is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM agent_history_migration_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        4
    );
    fs::remove_file(backup_path).unwrap();
    pool.close().await;
}
