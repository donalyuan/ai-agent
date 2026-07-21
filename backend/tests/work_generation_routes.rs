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
    let database_name = format!("work_generation_routes_{}", Uuid::new_v4().simple());
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
    let database = TestDatabase::new(&admin_url, &database_name);
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&test_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (admin_pool, pool, database, test_url)
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

async fn send(
    app: &axum::Router,
    method: &str,
    uri: &str,
    idempotency_key: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(key) = idempotency_key {
        builder = builder.header("idempotency-key", key);
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

struct SeededRun {
    project_id: Uuid,
    run_id: Uuid,
    successful_step_id: Uuid,
    target_step_id: Uuid,
    downstream_step_id: Uuid,
}

async fn seed_run(pool: &PgPool, suffix: &str) -> SeededRun {
    let project_id: Uuid =
        sqlx::query_scalar("INSERT INTO projects (name) VALUES ($1) RETURNING id")
            .bind(format!("作品任务测试-{suffix}"))
            .fetch_one(pool)
            .await
            .unwrap();
    let script_id: Uuid = sqlx::query_scalar(
        "INSERT INTO scripts (project_id, title, hook, content) VALUES ($1,$2,'测试钩子',$3) RETURNING id",
    )
    .bind(project_id)
    .bind(format!("作品-{suffix}"))
    .bind(json!({"topic_snapshot": {"title": "作品任务测试"}}))
    .fetch_one(pool)
    .await
    .unwrap();
    let work_id: Uuid = sqlx::query_scalar(
        "INSERT INTO works (project_id, script_id, title, status) VALUES ($1,$2,$3,'running') RETURNING id",
    )
    .bind(project_id)
    .bind(script_id)
    .bind(format!("作品-{suffix}"))
    .fetch_one(pool)
    .await
    .unwrap();
    let version_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_versions (work_id, version_no, source_manifest_version, input_snapshot, model_snapshot, parameter_snapshot) VALUES ($1,1,'test', '{}','{}','{}') RETURNING id",
    )
    .bind(work_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let plan_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_plans (work_id, work_version_id, plan_version, status, input_fingerprint, capability_snapshot, output_snapshot, prompt_snapshot, timeline_snapshot) VALUES ($1,$2,1,'confirmed',$3,'{}','{}','{}','{}') RETURNING id",
    )
    .bind(work_id)
    .bind(version_id)
    .bind("0".repeat(64))
    .fetch_one(pool)
    .await
    .unwrap();
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_generation_runs (work_id, work_version_id, work_plan_id, idempotency_key, model_snapshot, capability_snapshot, prompt_snapshot, timeline_snapshot, parameter_snapshot) VALUES ($1,$2,$3,$4,'{}','{}','{}','{}','{}') RETURNING id",
    )
    .bind(work_id)
    .bind(version_id)
    .bind(plan_id)
    .bind(format!("run-{suffix}"))
    .fetch_one(pool)
    .await
    .unwrap();
    let successful_step_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_generation_steps (run_id, step_no, step_type, status, result_material_ids) VALUES ($1,1,'video_segment','succeeded',$2) RETURNING id",
    )
    .bind(run_id)
    .bind(json!(["material-success"]))
    .fetch_one(pool)
    .await
    .unwrap();
    let target_step_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_generation_steps (run_id, step_no, step_type, status, error_category, error_summary) VALUES ($1,2,'video_segment','failed','provider','分段失败') RETURNING id",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let downstream_step_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_generation_steps (run_id, step_no, step_type, status, depends_on, result_material_ids, error_category, error_summary) VALUES ($1,3,'compose','blocked',$2,$3,'dependency','等待失败分段') RETURNING id",
    )
    .bind(run_id)
    .bind(json!([successful_step_id, target_step_id]))
    .bind(json!(["stale-compose-material"]))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO work_generation_attempts (step_id, attempt_no, status, upstream_task_id, output_snapshot) VALUES ($1,1,'succeeded','upstream-success',$2)",
    )
    .bind(successful_step_id)
    .bind(json!(["material-success"]))
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO work_generation_attempts (step_id, attempt_no, status, upstream_task_id, error_category, error_summary) VALUES ($1,1,'failed','upstream-failed','provider','分段失败')",
    )
    .bind(target_step_id)
    .execute(pool)
    .await
    .unwrap();
    SeededRun {
        project_id,
        run_id,
        successful_step_id,
        target_step_id,
        downstream_step_id,
    }
}

async fn seed_running_attempt(pool: &PgPool, suffix: &str, cancel_supported: bool) -> SeededRun {
    let seeded = seed_run(pool, suffix).await;
    sqlx::query("UPDATE work_generation_steps SET status='running', error_category=NULL, error_summary=NULL WHERE id=$1")
        .bind(seeded.target_step_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("UPDATE work_generation_attempts SET status='running', provider_cancel_supported=$2 WHERE step_id=$1")
        .bind(seeded.target_step_id)
        .bind(cancel_supported)
        .execute(pool)
        .await
        .unwrap();
    seeded
}

#[tokio::test]
async fn task_routes_aggregate_retry_and_cancel_without_repeating_successful_nodes() {
    let (admin_pool, pool, _database, test_url) = migrated_pool().await;
    let app = build_app_with_state(app_state(test_url, pool.clone()));

    let retry_run = seed_run(&pool, "retry").await;
    let (status, before) = send(
        &app,
        "GET",
        &format!("/api/work-generation/runs/{}", retry_run.run_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(before["task"]["status"], "failed");
    assert_eq!(before["task"]["progress_percent"], 33);
    assert_eq!(before["task"]["can_cancel"], false);

    let (status, attention_list) = send(
        &app,
        "GET",
        &format!(
            "/api/projects/{}/work-generation/tasks?view=attention",
            retry_run.project_id
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(attention_list["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(attention_list["counts"]["attention"], 1);
    assert_eq!(attention_list["counts"]["total"], 1);

    let (status, first_retry) = send(
        &app,
        "POST",
        &format!(
            "/api/work-generation/steps/{}/retry",
            retry_run.target_step_id
        ),
        Some("retry-failed-segment"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first_retry["attempt_no"], 2);
    assert_eq!(first_retry["status"], "queued");
    let (status, repeated_retry) = send(
        &app,
        "POST",
        &format!(
            "/api/work-generation/steps/{}/retry",
            retry_run.target_step_id
        ),
        Some("retry-failed-segment"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(repeated_retry["id"], first_retry["id"]);

    let successful: (String, Value, i64) = sqlx::query_as(
        "SELECT status, result_material_ids, (SELECT COUNT(*) FROM work_generation_attempts WHERE step_id=$1) FROM work_generation_steps WHERE id=$1",
    )
    .bind(retry_run.successful_step_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(successful.0, "succeeded");
    assert_eq!(successful.1, json!(["material-success"]));
    assert_eq!(successful.2, 1);
    let downstream: (String, Value) =
        sqlx::query_as("SELECT status, result_material_ids FROM work_generation_steps WHERE id=$1")
            .bind(retry_run.downstream_step_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(downstream.0, "queued");
    assert_eq!(downstream.1, json!([]));

    let cancellable = seed_running_attempt(&pool, "cancel-supported", true).await;
    let (status, cancelling) = send(
        &app,
        "POST",
        &format!("/api/work-generation/runs/{}/cancel", cancellable.run_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cancelling["task"]["status"], "cancelling");
    assert_eq!(cancelling["task"]["can_cancel"], false);

    let unsupported = seed_running_attempt(&pool, "cancel-unsupported", false).await;
    let (status, conflict) = send(
        &app,
        "POST",
        &format!("/api/work-generation/runs/{}/cancel", unsupported.run_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(conflict["details"]
        .as_str()
        .unwrap()
        .contains("不支持运行中取消"));

    let queued = seed_run(&pool, "cancel-queued").await;
    sqlx::query("DELETE FROM work_generation_attempts WHERE step_id IN (SELECT id FROM work_generation_steps WHERE run_id=$1)")
        .bind(queued.run_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE work_generation_steps SET status='queued', error_category=NULL, error_summary=NULL WHERE id=$1")
        .bind(queued.target_step_id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, cancelled) = send(
        &app,
        "POST",
        &format!("/api/work-generation/runs/{}/cancel", queued.run_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cancelled["task"]["status"], "cancelled");
    assert_eq!(cancelled["task"]["running_steps"], 0);
    let (status, cancelled_list) = send(
        &app,
        "GET",
        &format!(
            "/api/projects/{}/work-generation/tasks?view=cancelled",
            queued.project_id
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cancelled_list["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(cancelled_list["counts"]["cancelled"], 1);
    assert_eq!(cancelled_list["counts"]["total"], 1);

    pool.close().await;
    admin_pool.close().await;
}
