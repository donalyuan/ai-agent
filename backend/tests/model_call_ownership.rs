use novex_api::repositories::{ModelCallOwner, PostgresModelCallRepository};
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
    let name = format!("video_agent_model_call_owner_{}", Uuid::new_v4().simple());
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

async fn seed_model(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        r#"
        INSERT INTO ai_models (
            display_name, model_type, provider_name, api_protocol, auth_scheme,
            request_base_url, upstream_model, api_key, max_output_tokens, settings
        ) VALUES ('owner', 'text', 'test', 'openai_responses', 'bearer',
                  'https://example.invalid/v1', 'audit-1', 'secret', 4096,
                  '{"context_window":128000}') RETURNING id
        "#,
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_run(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO agent_runs (agent_type, status) VALUES ('script', 'running') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_call(
    pool: &PgPool,
    run_id: Uuid,
    model_id: Uuid,
    root: Option<Uuid>,
    attempt: i32,
) -> Uuid {
    sqlx::query_scalar(
        r#"
        INSERT INTO model_calls (
            agent_run_id, root_call_id, node_key, attempt, agent_key, agent_version,
            prompt_key, prompt_version, registry_digest, prompt_snapshot,
            model_id, behavior_fingerprint, model_snapshot
        ) VALUES ($1, $2, 'script.complete', $3, 'video.script', '1.0.0',
                  'script.complete', '1.0.0', $4, '{}', $5, $6, '{}')
        RETURNING id
        "#,
    )
    .bind(run_id)
    .bind(root)
    .bind(attempt)
    .bind("a".repeat(64))
    .bind(model_id)
    .bind("b".repeat(64))
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_eval_report(pool: &PgPool) -> Uuid {
    let eval_run_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO eval_runs (
            candidate_key, candidate_version, candidate_digest, case_set_version,
            evaluator_version, approved_real_calls, max_cases, max_input_tokens,
            max_output_tokens, max_retries, max_cost_micros, status
        ) VALUES ('video.script', '1.0.0', $1, 'cases-v1', 'eval-v1', false,
                  1, 0, 0, 0, 0, 'passed') RETURNING id
        "#,
    )
    .bind("c".repeat(64))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        r#"
        INSERT INTO eval_reports (
            eval_run_id, passed, gate_results, aggregate_metrics, redacted_case_results
        ) VALUES ($1, true, '{"safety":"passed"}', '{"score":1}',
                  '[{"case":"source","content":"redacted"}]') RETURNING id
        "#,
    )
    .bind(eval_run_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn explicit_run_deletion_cascades_owned_calls_marks_reports_and_preserves_adjacent_run() {
    let (admin, pool, database) = migrated_pool().await;
    let model_id = seed_model(&pool).await;
    let source_run = seed_run(&pool).await;
    let adjacent_run = seed_run(&pool).await;
    let root = seed_call(&pool, source_run, model_id, None, 1).await;
    let retry = seed_call(&pool, source_run, model_id, Some(root), 2).await;
    let adjacent_call = seed_call(&pool, adjacent_run, model_id, None, 1).await;
    let report_id = seed_eval_report(&pool).await;
    let repository = PostgresModelCallRepository::new(pool.clone());
    repository
        .attach_eval_report_source(report_id, root)
        .await
        .unwrap();
    repository
        .attach_eval_report_source(report_id, retry)
        .await
        .unwrap();

    repository
        .delete_owner(ModelCallOwner::AgentRun(source_run))
        .await
        .unwrap();

    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM model_calls WHERE agent_run_id=$1")
            .bind(source_run)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert!(
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM agent_runs WHERE id=$1")
            .bind(source_run)
            .fetch_optional(&pool)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM model_calls WHERE id=$1")
            .bind(adjacent_call)
            .fetch_one(&pool)
            .await
            .unwrap(),
        adjacent_call
    );
    let report: (bool, serde_json::Value, serde_json::Value) = sqlx::query_as(
        "SELECT source_deleted, aggregate_metrics, redacted_case_results FROM eval_reports WHERE id=$1",
    )
    .bind(report_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(report.0);
    assert_eq!(report.1, json!({"score":1}));
    assert_eq!(report.2, json!([]));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM eval_report_sources WHERE eval_report_id=$1"
        )
        .bind(report_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );

    pool.close().await;
    admin.close().await;
    drop(database);
}
