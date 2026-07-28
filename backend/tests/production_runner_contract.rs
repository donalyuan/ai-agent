use novex_api::application::production_runner::ProductionWorkflowRunner;
use novex_production_crew::{
    durable::{
        plan::{FullCrewPlanRegistry, ResourceLimits},
        repository::{
            CreateIntentCommand, DurableProductionRepository, ProductionActor, StartRunCommand,
        },
    },
    gates::GateRegistry,
    orchestrator::ProductionOrchestrator,
    roles::RoleRegistry,
};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;
use tokio::time::{timeout, Duration as TokioDuration};
use uuid::Uuid;

mod support;
use support::test_database::TestDatabase;

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@biga-postgres:5432/video_agent".into())
}

fn with_database_name(url: &str, name: &str) -> String {
    let base = url.split_once('?').map_or(url, |(base, _)| base);
    let slash = base.rfind('/').expect("DATABASE_URL must include database");
    format!("{}{}", &base[..=slash], name)
}

async fn database() -> (PgPool, TestDatabase) {
    let base = database_url();
    let name = format!("full_crew_runner_{}", Uuid::new_v4().simple());
    let admin_url = with_database_name(&base, "postgres");
    let test_url = with_database_name(&base, &name);
    let admin = PgPoolOptions::new()
        .max_connections(2)
        .connect(&admin_url)
        .await
        .unwrap();
    sqlx::query(&format!(r#"CREATE DATABASE "{}""#, name))
        .execute(&admin)
        .await
        .unwrap();
    let guard = TestDatabase::new(&admin_url, &name);
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&test_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (pool, guard)
}

#[tokio::test]
async fn runner_recovery_executes_durable_domain_step_once() {
    let (pool, _guard) = database().await;
    let project_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO projects (name,positioning,status) VALUES ('Runner 项目','持久化','active') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let topic_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO content_topics (project_id,title,angle,target_audience,status) VALUES ($1,'Runner 选题','恢复','开发者','approved') RETURNING id",
    )
    .bind(project_id)
    .fetch_one(&pool)
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
            title: "Runner 恢复".into(),
            description: None,
            initial_input: json!({"brief":"只从 PostgreSQL 恢复"}),
            actor: ProductionActor::local_operator(),
            idempotency_key: "runner-create".into(),
        })
        .await
        .unwrap();
    let run = repository
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan,
            actor: ProductionActor::local_operator(),
            idempotency_key: "runner-start".into(),
        })
        .await
        .unwrap();
    let orchestrator = Arc::new(ProductionOrchestrator::new(
        pool.clone(),
        Arc::new(RoleRegistry::new()),
        Arc::new(GateRegistry::new()),
    ));
    let runner = ProductionWorkflowRunner::new(
        pool.clone(),
        orchestrator.clone(),
        None,
        "unused-runner-queue",
        "runner-contract",
        Duration::from_secs(30),
    )
    .unwrap();
    let competing_runner = ProductionWorkflowRunner::new(
        pool.clone(),
        orchestrator,
        None,
        "unused-runner-queue",
        "runner-contract-peer",
        Duration::from_secs(30),
    )
    .unwrap();

    let first = runner.tick().await.unwrap();
    assert!(first.completed >= 1);
    let validate = repository
        .get_run(run.id)
        .await
        .unwrap()
        .steps
        .into_iter()
        .find(|step| step.step_key == "validate_source")
        .unwrap();
    assert_eq!(validate.status, "succeeded");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_step_attempts WHERE step_id=$1",
        )
        .bind(validate.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );

    let second = runner.tick().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_step_attempts WHERE step_id=$1",
        )
        .bind(validate.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1,
        "重复恢复扫描不得为已完成 domain step 创建第二个 attempt"
    );
    assert!(second.completed == 0 || second.skipped > 0 || second.failed > 0);

    let view = repository.get_run(run.id).await.unwrap();
    let producer = view
        .steps
        .iter()
        .find(|step| step.step_key == "producer")
        .unwrap();
    let brief_gate = view
        .steps
        .iter()
        .find(|step| step.step_key == "brief_approval")
        .unwrap();
    let brief = json!({
        "target_audience": "开发者",
        "tone": ["严谨"],
        "key_messages": ["Gate 由 Runner 调度"],
        "constraints": {},
        "success_criteria": ["进入 package approval"]
    });
    let brief_digest = novex_production_crew::durable::canonical_digest(&brief).unwrap();
    sqlx::query(
        "UPDATE production_steps SET status='succeeded',attempt=1,output_digest=$2 WHERE id=$1",
    )
    .bind(producer.id)
    .bind(&brief_digest)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO creative_briefs (
            production_project_id,version,status,content,created_by,run_id,step_id,
            attempt,revision_epoch,content_digest,audit_status
        )
        SELECT production_project_id,1,'draft',$2,'producer',id,$3,1,0,$4,'complete'
        FROM production_runs WHERE id=$1
        "#,
    )
    .bind(run.id)
    .bind(brief)
    .bind(producer.id)
    .bind(&brief_digest)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE production_steps SET status='queued' WHERE id=$1")
        .bind(brief_gate.id)
        .execute(&pool)
        .await
        .unwrap();

    let (gate_tick, competing_gate_tick) = tokio::join!(runner.tick(), competing_runner.tick());
    let gate_tick = gate_tick.unwrap();
    let competing_gate_tick = competing_gate_tick.unwrap();
    assert_eq!(gate_tick.completed + competing_gate_tick.completed, 1);
    assert_eq!(gate_tick.failed + competing_gate_tick.failed, 0);
    assert_eq!(
        sqlx::query_as::<_, (String, i64, i64)>(
            r#"
            SELECT step.status,COUNT(DISTINCT package.id),COUNT(DISTINCT attempt.id)
            FROM production_steps step
            LEFT JOIN artifact_package_snapshots package
              ON package.run_id=step.run_id AND package.package_type='brief'
            LEFT JOIN production_step_attempts attempt ON attempt.step_id=step.id
            WHERE step.id=$1
            GROUP BY step.status
            "#,
        )
        .bind(brief_gate.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        ("waiting_approval".into(), 1, 1),
        "并发 Runner 必须只认领一次 Gate、构建一个不可变 BriefPackage 并等待审批"
    );
    runner.tick().await.unwrap();
    assert_eq!(
        sqlx::query_as::<_, (i64, i64)>(
            r#"
            SELECT
                (SELECT COUNT(*) FROM artifact_package_snapshots WHERE run_id=$1 AND package_type='brief'),
                (SELECT COUNT(*) FROM production_step_attempts WHERE step_id=$2)
            "#,
        )
        .bind(run.id)
        .bind(brief_gate.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        (1, 1),
        "等待审批的 Gate 不得被后续恢复扫描重复构建"
    );
}

#[tokio::test]
async fn runner_survives_redis_loss_and_stops_gracefully_without_duplicate_attempts() {
    let (pool, _guard) = database().await;
    let project_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO projects (name,positioning,status) VALUES ('Runner Redis','恢复','active') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let topic_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO content_topics (project_id,title,angle,target_audience,status) VALUES ($1,'Runner Redis 选题','恢复','开发者','approved') RETURNING id",
    )
    .bind(project_id)
    .fetch_one(&pool)
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
            title: "Runner Redis 恢复".into(),
            description: None,
            initial_input: json!({"brief":"Redis 不可用时仍从 PostgreSQL 恢复"}),
            actor: ProductionActor::local_operator(),
            idempotency_key: "runner-redis-create".into(),
        })
        .await
        .unwrap();
    let run = repository
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan,
            actor: ProductionActor::local_operator(),
            idempotency_key: "runner-redis-start".into(),
        })
        .await
        .unwrap();
    let orchestrator = Arc::new(ProductionOrchestrator::new(
        pool.clone(),
        Arc::new(RoleRegistry::new()),
        Arc::new(GateRegistry::new()),
    ));
    let runner = ProductionWorkflowRunner::new(
        pool.clone(),
        orchestrator,
        Some(redis::Client::open("redis://127.0.0.1:1/2").unwrap()),
        format!("novex:test:runner:{}", Uuid::new_v4()),
        "runner-redis-contract",
        Duration::from_secs(30),
    )
    .unwrap();

    // Redis 连接失败时 tick 仍必须完成 PostgreSQL 中的可执行 domain step。
    let first = runner.tick().await.unwrap();
    assert!(first.completed >= 1);
    let validate = repository
        .get_run(run.id)
        .await
        .unwrap()
        .steps
        .into_iter()
        .find(|step| step.step_key == "validate_source")
        .unwrap();
    assert_eq!(validate.status, "succeeded");

    // 长驻循环收到 shutdown 后应在当前 tick 边界退出，而不是遗留后台任务。
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let runner_task = tokio::spawn(async move { runner.run(shutdown_rx).await });
    shutdown_tx.send(true).unwrap();
    timeout(TokioDuration::from_secs(2), runner_task)
        .await
        .expect("Runner should stop after shutdown")
        .expect("Runner task should join")
        .unwrap();

    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_step_attempts WHERE step_id=$1",
        )
        .bind(validate.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1,
        "Redis 故障、重复 tick 和优雅停机都不得重复创建已完成步骤 attempt"
    );
}
