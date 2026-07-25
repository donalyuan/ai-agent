use novex_api::domain::conversation::{
    AgentConversationDefinitionBindingInput, CreateAgentConversationInput, CreateAgentRunInput,
    ModelBindingEvidence,
};
use novex_api::repositories::{
    AgentBindingError, ConversationRepository, PostgresConversationRepository,
};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
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

async fn migrated_pool() -> (PgPool, PgPool, TestDatabase) {
    let base_url = database_url();
    let database_name = format!("video_agent_binding_test_{}", Uuid::new_v4().simple());
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .expect("admin database should be reachable");
    sqlx::query(&format!(r#"CREATE DATABASE "{database_name}""#))
        .execute(&admin_pool)
        .await
        .expect("temporary binding database should be created");
    let database = TestDatabase::new(&admin_url, &database_name);
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&test_url)
        .await
        .expect("temporary binding database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations should run");
    (admin_pool, pool, database)
}

async fn insert_text_model(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar(
        r#"
        INSERT INTO ai_models (
            display_name, model_type, provider_name, api_protocol, protocol_version,
            auth_scheme, request_base_url, upstream_model, api_key, timeout_seconds,
            max_output_tokens, settings, status, source
        ) VALUES ($1, 'text', 'test', 'openai_responses', 'v1', 'bearer',
                  'https://example.invalid/v1', $1, 'secret', 30, 4096,
                  '{"context_window":128000}', 'enabled', 'admin')
        RETURNING id
        "#,
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn conversation_input() -> CreateAgentConversationInput {
    CreateAgentConversationInput {
        project_id: None,
        agent_type: "script".into(),
        subject_type: None,
        subject_id: None,
        title: "binding contract".into(),
        metadata: json!({}),
    }
}

fn definition_binding() -> AgentConversationDefinitionBindingInput {
    AgentConversationDefinitionBindingInput {
        agent_key: "video.script".into(),
        agent_version: "1.0.0".into(),
        agent_digest: "a".repeat(64),
        prompt_bindings: json!({
            "script.complete": {
                "key": "script.complete",
                "version": "1.0.0",
                "digest": "b".repeat(64)
            }
        }),
        registry_digest: "c".repeat(64),
        migration_source: None,
        parent_conversation_id: None,
    }
}

fn model_evidence(model_id: Uuid, fingerprint: char) -> ModelBindingEvidence {
    ModelBindingEvidence {
        model_id,
        behavior_fingerprint: fingerprint.to_string().repeat(64),
        model_capabilities: json!({
            "text": true,
            "tool_calling": false,
            "structured_output": true,
            "vision": false,
            "reasoning": false,
            "context_window": 128000
        }),
    }
}

#[tokio::test]
async fn conversation_model_binding_is_atomic_and_rejects_rebinding_or_behavior_drift() {
    let (admin_pool, pool, database) = migrated_pool().await;
    let repository = PostgresConversationRepository::new(pool.clone());
    let conversation = repository
        .create_conversation_with_definition(conversation_input(), definition_binding())
        .await
        .unwrap();
    let model_a = insert_text_model(&pool, "model-a").await;
    let model_b = insert_text_model(&pool, "model-b").await;

    let first = repository
        .bind_or_validate_conversation_model(conversation.id, model_evidence(model_a, '1'))
        .await
        .unwrap();
    assert_eq!(first.model_id, Some(model_a));
    assert_eq!(first.binding_status, "executable");

    // Credential values never enter this repository contract, so rotation with the same
    // behavior fingerprint resolves to the existing immutable binding.
    let continued = repository
        .bind_or_validate_conversation_model(conversation.id, model_evidence(model_a, '1'))
        .await
        .unwrap();
    assert_eq!(continued, first);

    let different_model = repository
        .bind_or_validate_conversation_model(conversation.id, model_evidence(model_b, '1'))
        .await
        .unwrap_err();
    assert!(matches!(
        different_model,
        AgentBindingError::ModelRebindRequired { .. }
    ));

    let behavior_drift = repository
        .bind_or_validate_conversation_model(conversation.id, model_evidence(model_a, '2'))
        .await
        .unwrap_err();
    assert!(matches!(
        behavior_drift,
        AgentBindingError::ModelRebindRequired { .. }
    ));

    pool.close().await;
    admin_pool.close().await;
    drop(database);
}

#[tokio::test]
async fn concurrent_first_turns_choose_at_most_one_model_and_database_rejects_overwrite() {
    let (admin_pool, pool, database) = migrated_pool().await;
    let repository = PostgresConversationRepository::new(pool.clone());
    let conversation = repository
        .create_conversation_with_definition(conversation_input(), definition_binding())
        .await
        .unwrap();
    let model_a = insert_text_model(&pool, "concurrent-a").await;
    let model_b = insert_text_model(&pool, "concurrent-b").await;

    let (left, right) = tokio::join!(
        repository
            .bind_or_validate_conversation_model(conversation.id, model_evidence(model_a, '3'),),
        repository
            .bind_or_validate_conversation_model(conversation.id, model_evidence(model_b, '4'),)
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    assert!(left.is_ok() || matches!(left, Err(AgentBindingError::ModelRebindRequired { .. })));
    assert!(right.is_ok() || matches!(right, Err(AgentBindingError::ModelRebindRequired { .. })));

    let definition_overwrite = sqlx::query(
        "UPDATE agent_conversation_bindings SET agent_version = '2.0.0' WHERE conversation_id = $1",
    )
    .bind(conversation.id)
    .execute(&pool)
    .await;
    assert!(definition_overwrite.is_err());

    let model_overwrite = sqlx::query(
        "UPDATE agent_conversation_bindings SET behavior_fingerprint = $2 WHERE conversation_id = $1",
    )
    .bind(conversation.id)
    .bind("f".repeat(64))
    .execute(&pool)
    .await;
    assert!(model_overwrite.is_err());

    pool.close().await;
    admin_pool.close().await;
    drop(database);
}

#[tokio::test]
async fn non_session_run_binding_is_idempotent_and_immutable() {
    let (admin_pool, pool, database) = migrated_pool().await;
    let repository = PostgresConversationRepository::new(pool.clone());
    let conversation = repository
        .create_conversation(conversation_input())
        .await
        .unwrap();
    let model_id = insert_text_model(&pool, "run-model").await;
    let run = repository
        .create_run(CreateAgentRunInput {
            conversation_id: conversation.id,
            project_id: None,
            agent_type: "script".into(),
            input: json!({"intent": "direct_generation"}),
            model_id: Some(model_id),
            model_snapshot: Some(json!({"model_id": model_id})),
        })
        .await
        .unwrap();

    let first = repository
        .create_run_binding(
            run.id,
            definition_binding(),
            model_evidence(model_id, '5'),
            false,
        )
        .await
        .unwrap();
    let replay = repository
        .create_run_binding(
            run.id,
            definition_binding(),
            model_evidence(model_id, '5'),
            false,
        )
        .await
        .unwrap();
    assert_eq!(first, replay);

    let conflict = repository
        .create_run_binding(
            run.id,
            definition_binding(),
            model_evidence(model_id, '6'),
            false,
        )
        .await
        .unwrap_err();
    assert!(matches!(conflict, AgentBindingError::RunBindingConflict(id) if id == run.id));

    let overwrite = sqlx::query(
        "UPDATE agent_run_bindings SET agent_version = '2.0.0' WHERE agent_run_id = $1",
    )
    .bind(run.id)
    .execute(&pool)
    .await;
    assert!(overwrite.is_err());

    pool.close().await;
    admin_pool.close().await;
    drop(database);
}
