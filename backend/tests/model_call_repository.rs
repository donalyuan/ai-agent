use novex_api::repositories::{
    FinishModelCall, ModelCallOwner, ModelCallRepositoryError, ModelCallTerminalStatus,
    PostgresModelCallRepository, PrepareModelCall,
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
    let name = format!("video_agent_model_call_repo_{}", Uuid::new_v4().simple());
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

async fn seed_model_and_run(pool: &PgPool) -> (Uuid, Uuid) {
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
    (model_id, run_id)
}

fn prepared(
    run_id: Uuid,
    model_id: Uuid,
    attempt: i32,
    root_call_id: Option<Uuid>,
) -> PrepareModelCall {
    PrepareModelCall {
        owner: ModelCallOwner::AgentRun(run_id),
        root_call_id,
        parent_call_id: None,
        node_key: "script.complete".into(),
        attempt,
        agent_key: "video.script".into(),
        agent_version: "1.0.0".into(),
        prompt_key: "script.complete".into(),
        prompt_version: "1.0.0".into(),
        registry_digest: "a".repeat(64),
        prompt_snapshot: json!({
            "schema_version":"1",
            "system":"s",
            "user":"u repository-secret",
            "api_key":"must-not-persist",
            "url":"https://provider.invalid/v1?token=must-not-persist"
        }),
        context_sources: json!([{"id":"input-1","source":"test","trust":"user_instruction"}]),
        memory_sources: json!([]),
        tool_schema: None,
        model_id,
        behavior_fingerprint: "b".repeat(64),
        model_snapshot: json!({"provider":"test","upstream_model":"audit-1"}),
        parameters: json!({"max_output_tokens":4096}),
        asset_references: json!([]),
        known_secrets: vec!["repository-secret".into()],
    }
}

#[tokio::test]
async fn repository_preserves_attempts_terminal_evidence_and_run_step_links() {
    let (admin, pool, database) = migrated_pool().await;
    let (model_id, run_id) = seed_model_and_run(&pool).await;
    let repository = PostgresModelCallRepository::new(pool.clone());

    let root = repository
        .prepare(prepared(run_id, model_id, 1, None))
        .await
        .unwrap();
    assert_eq!(root.status.as_str(), "prepared");
    assert_eq!(root.owner, ModelCallOwner::AgentRun(run_id));
    let persisted_prompt = serde_json::to_string(&root.prompt_snapshot).unwrap();
    assert!(!persisted_prompt.contains("repository-secret"));
    assert!(!persisted_prompt.contains("must-not-persist"));
    assert!(persisted_prompt.contains("[REDACTED]"));

    let step_id: Uuid = sqlx::query_scalar(
        "INSERT INTO agent_steps (agent_run_id, step_order, step_type, status) VALUES ($1, 1, 'model_call', 'succeeded') RETURNING id",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    repository.associate_step(root.id, step_id).await.unwrap();
    let links: (Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        "SELECT mc.agent_step_id, step.model_call_id FROM model_calls mc JOIN agent_steps step ON step.id=$2 WHERE mc.id=$1",
    )
    .bind(root.id)
    .bind(step_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(links, (Some(step_id), Some(root.id)));

    let succeeded = repository
        .finish(FinishModelCall {
            id: root.id,
            status: ModelCallTerminalStatus::Succeeded,
            output_snapshot: Some(json!({"text":"done"})),
            usage_snapshot: Some(json!({"total_tokens":15})),
            error_snapshot: None,
            structured_parse_status: Some("valid".into()),
            known_secrets: vec![],
        })
        .await
        .unwrap();
    assert_eq!(succeeded.status.as_str(), "succeeded");
    assert!(succeeded.completed_at.is_some());
    assert_eq!(repository.get(root.id).await.unwrap(), succeeded);

    let duplicate = repository
        .finish(FinishModelCall {
            id: root.id,
            status: ModelCallTerminalStatus::Failed,
            output_snapshot: None,
            usage_snapshot: None,
            error_snapshot: Some(json!({"code":"late"})),
            structured_parse_status: None,
            known_secrets: vec![],
        })
        .await
        .unwrap_err();
    assert!(matches!(
        duplicate,
        ModelCallRepositoryError::TerminalConflict(_)
    ));

    let retry = repository
        .prepare(prepared(run_id, model_id, 2, Some(root.id)))
        .await
        .unwrap();
    assert_ne!(retry.id, root.id);
    assert_eq!(retry.root_call_id, Some(root.id));
    assert_eq!(
        repository.get(root.id).await.unwrap().output_snapshot,
        Some(json!({"text":"done"}))
    );

    pool.close().await;
    admin.close().await;
    drop(database);
}

#[tokio::test]
async fn repository_rejects_non_incrementing_retry_and_cross_run_step_link() {
    let (admin, pool, database) = migrated_pool().await;
    let (model_id, run_id) = seed_model_and_run(&pool).await;
    let (_, other_run_id) = seed_model_and_run(&pool).await;
    let repository = PostgresModelCallRepository::new(pool.clone());
    let mut binary = prepared(run_id, model_id, 1, None);
    binary.prompt_snapshot = json!({"image":"data:image/png;base64,AAAA"});
    assert!(matches!(
        repository.prepare(binary).await.unwrap_err(),
        ModelCallRepositoryError::UnsafeAudit(_)
    ));
    let mut signed = prepared(run_id, model_id, 1, None);
    signed.context_sources =
        json!([{"url":"https://assets.invalid/a.png?X-Amz-Signature=secret&X-Amz-Expires=60"}]);
    assert!(matches!(
        repository.prepare(signed).await.unwrap_err(),
        ModelCallRepositoryError::UnsafeAudit(_)
    ));
    let mut invalid_asset = prepared(run_id, model_id, 1, None);
    invalid_asset.asset_references =
        json!([{"asset_id":"bad","version":"1","sha256":"short","mime":"image/png"}]);
    assert!(matches!(
        repository.prepare(invalid_asset).await.unwrap_err(),
        ModelCallRepositoryError::UnsafeAudit(_)
    ));
    let root = repository
        .prepare(prepared(run_id, model_id, 1, None))
        .await
        .unwrap();

    let skipped = repository
        .prepare(prepared(run_id, model_id, 3, Some(root.id)))
        .await
        .unwrap_err();
    assert!(matches!(
        skipped,
        ModelCallRepositoryError::InvalidAttempt(_)
    ));

    let other_step: Uuid = sqlx::query_scalar(
        "INSERT INTO agent_steps (agent_run_id, step_order, step_type, status) VALUES ($1, 1, 'model_call', 'succeeded') RETURNING id",
    )
    .bind(other_run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let mismatch = repository
        .associate_step(root.id, other_step)
        .await
        .unwrap_err();
    assert!(matches!(mismatch, ModelCallRepositoryError::OwnerMismatch));
    let linked: Option<Uuid> =
        sqlx::query_scalar("SELECT model_call_id FROM agent_steps WHERE id=$1")
            .bind(other_step)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(linked.is_none());

    pool.close().await;
    admin.close().await;
    drop(database);
}
