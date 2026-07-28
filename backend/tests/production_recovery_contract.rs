use novex_api::application::production_wakeups::{
    ProductionWakeupMessage, RedisProductionWakeupDispatcher,
};
use novex_production_crew::durable::{
    plan::{FullCrewPlanRegistry, ResourceLimits},
    repository::{
        CreateIntentCommand, DurableProductionRepository, ProductionActor, StartRunCommand,
    },
};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;
use uuid::Uuid;

mod support;
use support::test_database::TestDatabase;

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@biga-postgres:5432/video_agent".into())
}

fn with_database_name(database_url: &str, database_name: &str) -> String {
    let base = database_url
        .split_once('?')
        .map_or(database_url, |(base, _)| base);
    let slash = base.rfind('/').unwrap();
    format!("{}{}", &base[..=slash], database_name)
}

async fn database() -> (PgPool, PgPool, TestDatabase) {
    let base_url = database_url();
    let database_name = format!("full_crew_recovery_{}", Uuid::new_v4().simple());
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);
    let admin = PgPoolOptions::new()
        .max_connections(2)
        .connect(&admin_url)
        .await
        .unwrap();
    sqlx::query(&format!(r#"CREATE DATABASE "{}""#, database_name))
        .execute(&admin)
        .await
        .unwrap();
    let guard = TestDatabase::new(&admin_url, &database_name);
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&test_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (admin, pool, guard)
}

async fn create_run(pool: &PgPool) -> (DurableProductionRepository, Uuid) {
    let project_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO projects (name, positioning, status) VALUES ('恢复测试', '工程审计', 'active') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let topic_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO content_topics (project_id, title, angle, target_audience, status)
        VALUES ($1, '恢复工作流', '可靠派发', '开发者', 'approved') RETURNING id
        "#,
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let bindings = [
        "producer",
        "screenwriter",
        "director",
        "cinematographer",
        "performance_director",
        "sound_director",
        "editor",
        "qc",
    ]
    .into_iter()
    .map(|role| {
        (
            role.to_string(),
            json!({"definition_key": format!("production.{role}"), "lifecycle": "active"}),
        )
    })
    .collect::<serde_json::Map<_, _>>();
    let plan = FullCrewPlanRegistry::snapshot_v1(
        false,
        Value::Object(bindings),
        ResourceLimits::strict_default(),
    )
    .unwrap();
    let repository = DurableProductionRepository::new(pool.clone());
    let intent = repository
        .create_intent(CreateIntentCommand {
            project_id,
            topic_id,
            title: "恢复测试".into(),
            description: None,
            initial_input: json!({"brief": "只依赖 PostgreSQL"}),
            actor: ProductionActor::local_operator(),
            idempotency_key: "create".into(),
        })
        .await
        .unwrap();
    let run = repository
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan,
            actor: ProductionActor::local_operator(),
            idempotency_key: "start".into(),
        })
        .await
        .unwrap();
    (repository, run.id)
}

#[tokio::test]
async fn redis_loss_duplicate_delivery_and_process_restart_preserve_postgres_truth() {
    let (_admin, pool, _guard) = database().await;
    let (repository, run_id) = create_run(&pool).await;
    let step_id = repository.get_run(run_id).await.unwrap().steps[0].id;
    let queue_key = format!("novex:test:production_wakeups:{}", Uuid::new_v4());
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://bs-redis:6379/2".into());
    let redis_client = redis::Client::open(redis_url).unwrap();
    let mut connection = redis_client
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    redis::cmd("DEL")
        .arg(&queue_key)
        .query_async::<i64>(&mut connection)
        .await
        .unwrap();

    let unavailable = RedisProductionWakeupDispatcher::new(
        redis::Client::open("redis://127.0.0.1:1/2").unwrap(),
        queue_key.clone(),
    );
    let failed = unavailable
        .recover_and_dispatch(&repository, 100)
        .await
        .unwrap();
    assert_eq!(failed.recovered, 1);
    assert_eq!(failed.failed, 1);
    assert_eq!(repository.pending_wakeups(100).await.unwrap().len(), 1);

    let restarted_repository = DurableProductionRepository::new(pool.clone());
    let dispatcher = RedisProductionWakeupDispatcher::new(redis_client.clone(), queue_key.clone());
    let recovered = dispatcher
        .recover_and_dispatch(&restarted_repository, 100)
        .await
        .unwrap();
    assert_eq!(recovered.delivered, 1);
    let first_payload = redis::cmd("LPOP")
        .arg(&queue_key)
        .query_async::<String>(&mut connection)
        .await
        .unwrap();
    let first_json: Value = serde_json::from_str(&first_payload).unwrap();
    assert_eq!(first_json.as_object().unwrap().len(), 2);
    assert_eq!(
        serde_json::from_str::<ProductionWakeupMessage>(&first_payload).unwrap(),
        ProductionWakeupMessage { run_id, step_id }
    );

    restarted_repository
        .enqueue_wakeup(run_id, step_id)
        .await
        .unwrap();
    dispatcher
        .recover_and_dispatch(&restarted_repository, 100)
        .await
        .unwrap();
    let duplicate_payload = redis::cmd("LPOP")
        .arg(&queue_key)
        .query_async::<String>(&mut connection)
        .await
        .unwrap();
    assert_eq!(duplicate_payload, first_payload);

    let digest = "f".repeat(64);
    restarted_repository
        .claim_step(
            step_id,
            "worker-a",
            Duration::from_secs(30),
            &digest,
            "claim-a",
        )
        .await
        .unwrap();
    assert!(restarted_repository
        .claim_step(
            step_id,
            "worker-b",
            Duration::from_secs(30),
            &digest,
            "claim-b",
        )
        .await
        .is_err());

    redis::cmd("DEL")
        .arg(&queue_key)
        .query_async::<i64>(&mut connection)
        .await
        .unwrap();
}

#[tokio::test]
async fn approval_and_external_wait_states_are_reconstructed_without_redis_payload() {
    let (_admin, pool, _guard) = database().await;
    let (repository, run_id) = create_run(&pool).await;
    let view = repository.get_run(run_id).await.unwrap();
    let approval = view
        .steps
        .iter()
        .find(|step| step.step_key == "brief_approval")
        .unwrap()
        .id;
    let external = view
        .steps
        .iter()
        .find(|step| step.step_key == "wait_work_generation")
        .unwrap()
        .id;
    sqlx::query(
        "UPDATE production_steps SET status = 'waiting_approval', waiting_reason = 'package_approval' WHERE id = $1",
    )
    .bind(approval)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE production_steps SET status = 'external_wait', waiting_reason = 'work_generation' WHERE id = $1",
    )
    .bind(external)
    .execute(&pool)
    .await
    .unwrap();

    let restarted = DurableProductionRepository::new(pool);
    let restored = restarted.get_run(run_id).await.unwrap();
    assert_eq!(
        restored
            .steps
            .iter()
            .find(|step| step.id == approval)
            .unwrap()
            .status,
        "waiting_approval"
    );
    assert_eq!(
        restored
            .steps
            .iter()
            .find(|step| step.id == external)
            .unwrap()
            .status,
        "external_wait"
    );
}
