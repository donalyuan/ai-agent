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

async fn conversation_with_metadata(
    pool: &PgPool,
    agent_type: &str,
    metadata: serde_json::Value,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO agent_conversations (agent_type,title,metadata) VALUES ($1,$2,$3) RETURNING id",
    )
    .bind(agent_type)
    .bind(format!("{agent_type} history"))
    .bind(metadata)
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
    let pending =
        conversation_with_metadata(&pool, "topic", json!({"parent_conversation_id":trusted})).await;
    let unmapped = conversation(&pool, "material").await;
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
        context_window: Some(8192),
        tokenizer_profile_key: Some("byte-upper-bound".into()),
        tokenizer_profile_version: Some("1.0.0".into()),
        settings: json!({}),
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
    assert_eq!(plan.schema_version, "2");
    assert_eq!(plan.items.len(), 5);
    assert!(plan.items.iter().any(|item| {
        item.entity_id == trusted
            && item.disposition == MigrationDisposition::Equivalent
            && item.reason_code == "baseline_equivalent"
            && item.node_keys
                == [
                    "script.complete",
                    "script.generation_intent",
                    "script.metadata",
                    "script.scene_patch",
                    "script.single_scene",
                ]
    }));
    assert!(plan.items.iter().any(|item| {
        item.entity_id == pending
            && item.disposition == MigrationDisposition::ModelConfigurationMissing
            && item.parent_entity_id == Some(trusted)
    }));
    assert!(plan.items.iter().any(|item| {
        item.entity_id == unmapped && item.disposition == MigrationDisposition::Unmappable
    }));
    assert_eq!(plan.summary["equivalent"], 1);
    assert_eq!(plan.summary["model_configuration_missing"], 1);
    assert_eq!(plan.summary["unmappable"], 1);
    assert_eq!(plan.summary["legacy_partial_audit"], 2);
    let without_baseline =
        PostgresHistoryMigrator::with_baseline_evidence(pool.clone(), migrator_registry(), None)
            .plan()
            .await
            .unwrap();
    assert!(without_baseline.items.iter().any(|item| {
        item.entity_id == trusted
            && item.disposition == MigrationDisposition::ContextMigrationRequired
            && item.reason_code == "baseline_equivalence_evidence_missing"
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
        "conversations":[trusted,pending,unmapped],
        "messages":[message_id],
        "runs":[trusted_run,partial_run]
    }))
    .unwrap();
    fs::write(&backup_path, &backup_bytes).unwrap();
    let backup = HistoryMigrationBackupEvidence {
        reference: backup_path.display().to_string(),
        sha256: sha256_hex(&backup_bytes),
    };
    let invalid_backup = HistoryMigrationBackupEvidence {
        reference: backup.reference.clone(),
        sha256: "0".repeat(64),
    };
    assert!(migrator.apply(&invalid_backup).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM agent_history_migration_events")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let applied = migrator.apply(&backup).await.unwrap();
    assert!(!applied.dry_run);
    let trusted_binding: (
        String,
        Option<Uuid>,
        String,
        serde_json::Value,
        String,
        String,
        String,
    ) = sqlx::query_as(
        r#"SELECT agent_key,model_id,binding_status,context_policy_bindings,
                  tokenizer_profile_key,tokenizer_profile_version,tokenizer_profile_digest
           FROM agent_conversation_bindings WHERE conversation_id=$1"#,
    )
    .bind(trusted)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(trusted_binding.0, "video.script");
    assert_eq!(trusted_binding.1, Some(model_id));
    assert_eq!(trusted_binding.2, "executable");
    assert!(trusted_binding.3.is_object());
    assert_eq!(trusted_binding.4, "byte-upper-bound");
    assert_eq!(trusted_binding.5, "1.0.0");
    assert!(trusted_binding
        .6
        .chars()
        .all(|value| value.is_ascii_hexdigit()));
    let pending_binding: (
        String,
        Option<Uuid>,
        String,
        serde_json::Value,
        Option<String>,
        Option<Uuid>,
    ) = sqlx::query_as(
        r#"SELECT agent_key,model_id,binding_status,context_policy_bindings,
                  tokenizer_profile_key,parent_conversation_id
           FROM agent_conversation_bindings WHERE conversation_id=$1"#,
    )
    .bind(pending)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pending_binding.0, "video.topic");
    assert_eq!(pending_binding.1, None);
    assert_eq!(pending_binding.2, "definition_bound");
    assert!(pending_binding.3.is_object());
    assert_eq!(pending_binding.4, None);
    assert_eq!(pending_binding.5, Some(trusted));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM agent_conversation_bindings WHERE conversation_id=$1",
        )
        .bind(unmapped)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
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
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM context_snapshots")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM context_compile_attempts")
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
        5
    );
    fs::remove_file(backup_path).unwrap();
    pool.close().await;
}

#[tokio::test]
async fn missing_baseline_evidence_migrates_conversation_to_read_only_without_context_audit() {
    let (pool, _database) = test_pool().await;
    let conversation_id = conversation(&pool, "script").await;
    let message_id: Uuid = sqlx::query_scalar(
        "INSERT INTO agent_messages (conversation_id,role,content) VALUES ($1,'user','history') RETURNING id",
    )
    .bind(conversation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let migrator =
        PostgresHistoryMigrator::with_baseline_evidence(pool.clone(), migrator_registry(), None);
    let plan = migrator.plan().await.unwrap();
    assert_eq!(plan.items.len(), 1);
    assert_eq!(
        plan.items[0].disposition,
        MigrationDisposition::ContextMigrationRequired
    );

    let backup_path = std::env::temp_dir().join(format!(
        "novex-context-read-only-backup-{}.json",
        Uuid::new_v4()
    ));
    let backup_bytes = serde_json::to_vec(&json!({
        "conversations":[conversation_id],
        "messages":[message_id]
    }))
    .unwrap();
    fs::write(&backup_path, &backup_bytes).unwrap();
    migrator
        .apply(&HistoryMigrationBackupEvidence {
            reference: backup_path.display().to_string(),
            sha256: sha256_hex(&backup_bytes),
        })
        .await
        .unwrap();

    let binding: (String, Option<serde_json::Value>, Option<String>) = sqlx::query_as(
        "SELECT binding_status,context_policy_bindings,tokenizer_profile_key FROM agent_conversation_bindings WHERE conversation_id=$1",
    )
    .bind(conversation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(binding, ("read_only".into(), None, None));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM context_snapshots")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
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
    fs::remove_file(backup_path).unwrap();
    pool.close().await;
}

fn migrator_registry() -> Arc<DefinitionRegistry> {
    Arc::new(
        DefinitionRegistry::load(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("agent-definitions"),
        )
        .unwrap(),
    )
}
