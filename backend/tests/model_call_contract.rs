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
    let (base, query) = query_start
        .map(|index| (&database_url[..index], &database_url[index..]))
        .unwrap_or((database_url, ""));
    let slash = base
        .rfind('/')
        .expect("DATABASE_URL must include a database");
    format!("{}{}{}", &base[..=slash], database_name, query)
}

async fn migrated_pool() -> (PgPool, PgPool, TestDatabase) {
    let base_url = database_url();
    let name = format!("video_agent_model_call_{}", Uuid::new_v4().simple());
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
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&with_database_name(&base_url, &name))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (admin, pool, TestDatabase::new(&admin_url, &name))
}

async fn model_and_owners(pool: &PgPool) -> (Uuid, Uuid, Uuid) {
    let model_id = sqlx::query_scalar(
        r#"
        INSERT INTO ai_models (
            display_name, model_type, provider_name, api_protocol, auth_scheme,
            request_base_url, upstream_model, api_key, max_output_tokens, settings
        ) VALUES ('audit', 'text', 'test', 'openai_responses', 'bearer',
                  'https://example.invalid/v1', 'audit-1', 'secret', 4096,
                  '{"context_window":128000}') RETURNING id
        "#,
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let run_id = sqlx::query_scalar(
        "INSERT INTO agent_runs (agent_type, status) VALUES ('script', 'running') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let conversation_id = sqlx::query_scalar(
        "INSERT INTO agent_conversations (agent_type, title) VALUES ('script', 'audit') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    (model_id, run_id, conversation_id)
}

async fn insert_call(
    pool: &PgPool,
    model_id: Uuid,
    run_id: Uuid,
    root_call_id: Option<Uuid>,
    attempt: i32,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        INSERT INTO model_calls (
            agent_run_id, root_call_id, node_key, attempt, agent_key, agent_version,
            prompt_key, prompt_version, registry_digest, prompt_snapshot,
            model_id, behavior_fingerprint, model_snapshot
        ) VALUES ($1, $2, 'script.complete', $3, 'video.script', '1.0.0',
                  'script.complete', '1.0.0', $4, '{"system":"s","user":"u"}',
                  $5, $6, '{"provider":"test"}') RETURNING id
        "#,
    )
    .bind(run_id)
    .bind(root_call_id)
    .bind(attempt)
    .bind("a".repeat(64))
    .bind(model_id)
    .bind("b".repeat(64))
    .fetch_one(pool)
    .await
}

#[tokio::test]
async fn postgres_model_call_enforces_owner_attempt_and_single_terminal_transition() {
    let (admin, pool, database) = migrated_pool().await;
    let (model_id, run_id, conversation_id) = model_and_owners(&pool).await;
    let root = insert_call(&pool, model_id, run_id, None, 1).await.unwrap();

    let prepared: (String, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as("SELECT status, completed_at FROM model_calls WHERE id = $1")
            .bind(root)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(prepared.0, "prepared");
    assert!(prepared.1.is_none());

    sqlx::query(
        "UPDATE model_calls SET status='succeeded', output_snapshot='{}', usage_snapshot='{}', completed_at=NOW() WHERE id=$1",
    )
    .bind(root)
    .execute(&pool)
    .await
    .unwrap();
    assert!(sqlx::query(
        "UPDATE model_calls SET status='failed', error_snapshot='{}', completed_at=NOW() WHERE id=$1",
    )
    .bind(root)
    .execute(&pool)
    .await
    .is_err());

    let retry = insert_call(&pool, model_id, run_id, Some(root), 2)
        .await
        .unwrap();
    assert_ne!(retry, root);
    assert!(insert_call(&pool, model_id, run_id, Some(root), 2)
        .await
        .is_err());

    let both_owners = sqlx::query(
        r#"
        INSERT INTO model_calls (
            conversation_id, agent_run_id, node_key, attempt, agent_key, agent_version,
            prompt_key, prompt_version, registry_digest, prompt_snapshot,
            model_id, behavior_fingerprint, model_snapshot
        ) VALUES ($1, $2, 'script.complete', 1, 'video.script', '1.0.0',
                  'script.complete', '1.0.0', $3, '{}', $4, $5, '{}')
        "#,
    )
    .bind(conversation_id)
    .bind(run_id)
    .bind("a".repeat(64))
    .bind(model_id)
    .bind("b".repeat(64))
    .execute(&pool)
    .await;
    assert!(both_owners.is_err());

    pool.close().await;
    admin.close().await;
    drop(database);
}
