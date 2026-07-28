use async_trait::async_trait;
use novex_agent::{AuditedModelExecutor, BoundModelResolver, ResolvedBoundModel};
use novex_ai_core::{DefinitionRegistry, ModelCapabilities};
use novex_api::{
    application::production_workflow::ProductionWorkflowService,
    repositories::{
        PostgresContextAuditRepository, PostgresDefinitionReleaseRepository,
        PostgresModelCallRepository,
    },
};
use novex_model::{LLMClient, LLMError, LLMPrompt};
use novex_production_crew::{
    durable::{
        canonical_digest,
        package::{ArtifactPackageSnapshot, ArtifactRef, GateDecision, PackageType},
        plan::{FullCrewPlanRegistry, ResourceLimits},
        repository::{
            CreateIntentCommand, DurableProductionRepository, PackageDecisionCommand,
            ProductionActor, StartRunCommand,
        },
    },
    executor::role_executor::{
        PreparedRoleExecution, RoleExecutor, RoleFinalizeContext, RolePrepareContext,
    },
    roles::RoleRegistry,
};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use uuid::Uuid;

mod support;
use support::test_database::{insert_enabled_text_model, TestDatabase};

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@biga-postgres:5432/video_agent".into())
}

fn with_database_name(database_url: &str, database_name: &str) -> String {
    let (base, query) = database_url
        .split_once('?')
        .map_or((database_url, ""), |(base, _)| {
            (base, &database_url[base.len()..])
        });
    let slash = base.rfind('/').unwrap();
    format!("{}{}{}", &base[..=slash], database_name, query)
}

async fn database() -> (PgPool, PgPool, TestDatabase) {
    let base_url = database_url();
    let database_name = format!("role_prepare_{}", Uuid::new_v4().simple());
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

#[derive(Clone)]
struct CountingClient {
    calls: Arc<AtomicUsize>,
    response: Result<String, LLMError>,
}

#[async_trait]
impl LLMClient for CountingClient {
    async fn generate_script(&self, _prompt: LLMPrompt) -> Result<String, LLMError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.response.clone()
    }
}

struct FixedResolver {
    resolved: ResolvedBoundModel,
}

#[async_trait]
impl BoundModelResolver for FixedResolver {
    async fn resolve(&self, model_id: Uuid) -> Result<ResolvedBoundModel, novex_agent::BoxError> {
        if model_id != self.resolved.model_id {
            return Err("unexpected model id".into());
        }
        Ok(self.resolved.clone())
    }
}

async fn source(pool: &PgPool) -> (Uuid, Uuid) {
    let project_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO projects (name, positioning, status) VALUES ('测试账号', '知识视频', 'active') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let topic_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO content_topics (project_id, title, angle, target_audience, status)
        VALUES ($1, '持久角色准备', '审计边界', '工程师', 'approved') RETURNING id
        "#,
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (project_id, topic_id)
}

fn create_intent(project_id: Uuid, topic_id: Uuid, key: &str) -> CreateIntentCommand {
    CreateIntentCommand {
        project_id,
        topic_id,
        title: "Role prepare contract".into(),
        description: None,
        initial_input: json!({"instruction": "严格使用冻结来源和已批准 package"}),
        actor: ProductionActor::local_operator(),
        idempotency_key: key.into(),
    }
}

async fn frozen_bindings(
    registry: &DefinitionRegistry,
    executor: &AuditedModelExecutor,
    model_id: Uuid,
) -> serde_json::Value {
    let mut bindings = serde_json::Map::new();
    for role in [
        "producer",
        "screenwriter",
        "director",
        "cinematographer",
        "performance_director",
        "sound_director",
        "editor",
        "qc",
    ] {
        let binding =
            RoleExecutor::freeze_active_binding(role, "2.0.0", registry, executor, model_id)
                .await
                .unwrap();
        bindings.insert(role.into(), serde_json::to_value(binding).unwrap());
    }
    serde_json::Value::Object(bindings)
}

async fn executor(
    pool: &PgPool,
    model_id: Uuid,
    calls: Arc<AtomicUsize>,
) -> (Arc<DefinitionRegistry>, Arc<AuditedModelExecutor>) {
    executor_with_response(
        pool,
        model_id,
        calls,
        Ok("provider must not run during prepare".into()),
    )
    .await
}

async fn executor_with_response(
    pool: &PgPool,
    model_id: Uuid,
    calls: Arc<AtomicUsize>,
    response: Result<String, LLMError>,
) -> (Arc<DefinitionRegistry>, Arc<AuditedModelExecutor>) {
    let registry = Arc::new(DefinitionRegistry::load("/app/agent-definitions").unwrap());
    let resolved = ResolvedBoundModel {
        client: Arc::new(CountingClient { calls, response }),
        model_id,
        behavior_fingerprint: "b".repeat(64),
        capabilities: ModelCapabilities {
            text: true,
            tool_calling: false,
            structured_output: true,
            vision: false,
            reasoning: false,
            context_window: 128_000,
        },
        tokenizer_profile_key: "openai.o200k".into(),
        tokenizer_profile_version: "1.0.0".into(),
        max_output_tokens: 3_000,
        model_snapshot: json!({"provider": "fake", "upstream_model": "prepare-contract"}),
        known_secrets: vec![],
    };
    let audited = Arc::new(AuditedModelExecutor::new(
        registry.clone(),
        Arc::new(FixedResolver { resolved }),
        Arc::new(PostgresModelCallRepository::new(pool.clone())),
        Arc::new(PostgresContextAuditRepository::new(pool.clone())),
    ));
    (registry, audited)
}

async fn prepare_producer(
    pool: &PgPool,
    model_id: Uuid,
    registry: Arc<DefinitionRegistry>,
    audited: Arc<AuditedModelExecutor>,
    lease_owner: &str,
) -> PreparedRoleExecution {
    let durable = DurableProductionRepository::new(pool.clone());
    let role_registry =
        RoleRegistry::bootstrap(Path::new("/app/crates/novex-production-crew/roles")).unwrap();
    let (project_id, topic_id) = source(pool).await;
    let intent = durable
        .create_intent(create_intent(
            project_id,
            topic_id,
            "finalize-producer-create",
        ))
        .await
        .unwrap();
    let plan = FullCrewPlanRegistry::snapshot_v1(
        false,
        frozen_bindings(&registry, &audited, model_id).await,
        ResourceLimits::strict_default(),
    )
    .unwrap();
    let run = durable
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan,
            actor: ProductionActor::local_operator(),
            idempotency_key: "finalize-producer-start".into(),
        })
        .await
        .unwrap();
    let view = durable.get_run(run.id).await.unwrap();
    let validate_source = view
        .steps
        .iter()
        .find(|step| step.step_key == "validate_source")
        .unwrap();
    let producer = view
        .steps
        .iter()
        .find(|step| step.step_key == "producer")
        .unwrap();
    sqlx::query(
        "UPDATE production_steps SET status='succeeded', attempt=1, output_digest=$2 WHERE id=$1",
    )
    .bind(validate_source.id)
    .bind(canonical_digest(&run.source_snapshot).unwrap())
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("UPDATE production_steps SET status='queued' WHERE id=$1")
        .bind(producer.id)
        .execute(pool)
        .await
        .unwrap();
    let claim_digest = canonical_digest(&json!({"step_id": producer.id})).unwrap();
    let claimed = durable
        .claim_step(
            producer.id,
            lease_owner,
            Duration::from_secs(60),
            &claim_digest,
            "finalize-producer-claim",
        )
        .await
        .unwrap();
    RoleExecutor::prepare(
        RolePrepareContext {
            pool: pool.clone(),
            definition_registry: registry,
            audited_executor: audited,
            step_id: claimed.id,
            lease_owner: lease_owner.into(),
            attempt: claimed.attempt,
        },
        &role_registry,
    )
    .await
    .unwrap()
}

async fn prepare_screenwriter(
    pool: &PgPool,
    model_id: Uuid,
    registry: Arc<DefinitionRegistry>,
    audited: Arc<AuditedModelExecutor>,
    lease_owner: &str,
) -> PreparedRoleExecution {
    let durable = DurableProductionRepository::new(pool.clone());
    let role_registry =
        RoleRegistry::bootstrap(Path::new("/app/crates/novex-production-crew/roles")).unwrap();
    let (project_id, topic_id) = source(pool).await;
    let intent = durable
        .create_intent(create_intent(
            project_id,
            topic_id,
            "finalize-screenwriter-create",
        ))
        .await
        .unwrap();
    let plan = FullCrewPlanRegistry::snapshot_v1(
        false,
        frozen_bindings(&registry, &audited, model_id).await,
        ResourceLimits::strict_default(),
    )
    .unwrap();
    let run = durable
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan,
            actor: ProductionActor::local_operator(),
            idempotency_key: "finalize-screenwriter-start".into(),
        })
        .await
        .unwrap();
    let view = durable.get_run(run.id).await.unwrap();
    let producer = view
        .steps
        .iter()
        .find(|step| step.step_key == "producer")
        .unwrap();
    let validate_source = view
        .steps
        .iter()
        .find(|step| step.step_key == "validate_source")
        .unwrap();
    sqlx::query(
        "UPDATE production_steps SET status='succeeded', attempt=1, output_digest=$2 WHERE id=$1",
    )
    .bind(validate_source.id)
    .bind(canonical_digest(&run.source_snapshot).unwrap())
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE production_steps SET status='succeeded', attempt=1, output_digest=$2 WHERE id=$1",
    )
    .bind(producer.id)
    .bind(canonical_digest(&json!({"producer": "output"})).unwrap())
    .execute(pool)
    .await
    .unwrap();
    let package = ArtifactPackageSnapshot::build(
        PackageType::Brief,
        run.id,
        producer.id,
        1,
        0,
        1,
        vec![ArtifactRef {
            run_id: run.id,
            artifact_type: "creative_brief".into(),
            artifact_id: Uuid::new_v4(),
            version: 1,
            content_digest: canonical_digest(&json!({"creative_brief": "v1"})).unwrap(),
            source_step_id: producer.id,
            source_attempt: 1,
        }],
        json!({}),
    )
    .unwrap();
    durable.save_package(&package).await.unwrap();
    durable
        .decide_package(PackageDecisionCommand {
            run_id: run.id,
            package_digest: package.package_digest,
            decision: GateDecision::Approve,
            reason: None,
            affected_owners: vec![],
            actor: ProductionActor::local_operator(),
            idempotency_key: "finalize-screenwriter-approve".into(),
        })
        .await
        .unwrap();
    let screenwriter = durable
        .get_run(run.id)
        .await
        .unwrap()
        .steps
        .into_iter()
        .find(|step| step.step_key == "screenwriter")
        .unwrap();
    let claim_digest = canonical_digest(&json!({"step_id": screenwriter.id})).unwrap();
    let claimed = durable
        .claim_step(
            screenwriter.id,
            lease_owner,
            Duration::from_secs(60),
            &claim_digest,
            "finalize-screenwriter-claim",
        )
        .await
        .unwrap();
    RoleExecutor::prepare(
        RolePrepareContext {
            pool: pool.clone(),
            definition_registry: registry,
            audited_executor: audited,
            step_id: claimed.id,
            lease_owner: lease_owner.into(),
            attempt: claimed.attempt,
        },
        &role_registry,
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn prepare_uses_exact_package_and_persists_all_pre_provider_anchors() {
    let (_admin, pool, _guard) = database().await;
    let model_id = insert_enabled_text_model(&pool).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let (registry, audited) = executor(&pool, model_id, calls.clone()).await;
    let role_registry =
        RoleRegistry::bootstrap(Path::new("/app/crates/novex-production-crew/roles")).unwrap();
    let durable = DurableProductionRepository::new(pool.clone());
    let (project_id, topic_id) = source(&pool).await;
    let intent = durable
        .create_intent(create_intent(project_id, topic_id, "create"))
        .await
        .unwrap();
    let plan = FullCrewPlanRegistry::snapshot_v1(
        false,
        frozen_bindings(&registry, &audited, model_id).await,
        ResourceLimits::strict_default(),
    )
    .unwrap();
    let run = durable
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan,
            actor: ProductionActor::local_operator(),
            idempotency_key: "start".into(),
        })
        .await
        .unwrap();
    let view = durable.get_run(run.id).await.unwrap();
    let producer = view
        .steps
        .iter()
        .find(|step| step.step_key == "producer")
        .unwrap();
    let validate_source = view
        .steps
        .iter()
        .find(|step| step.step_key == "validate_source")
        .unwrap();
    sqlx::query(
        "UPDATE production_steps SET status='succeeded', attempt=1, output_digest=$2 WHERE id=$1",
    )
    .bind(validate_source.id)
    .bind(canonical_digest(&run.source_snapshot).unwrap())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE production_steps SET status='succeeded', attempt=1, output_digest=$2 WHERE id=$1",
    )
    .bind(producer.id)
    .bind(canonical_digest(&json!({"producer": "output"})).unwrap())
    .execute(&pool)
    .await
    .unwrap();

    let artifact_digest = canonical_digest(&json!({"creative_brief": "v1"})).unwrap();
    let package = ArtifactPackageSnapshot::build(
        PackageType::Brief,
        run.id,
        producer.id,
        1,
        0,
        1,
        vec![ArtifactRef {
            run_id: run.id,
            artifact_type: "creative_brief".into(),
            artifact_id: Uuid::new_v4(),
            version: 1,
            content_digest: artifact_digest,
            source_step_id: producer.id,
            source_attempt: 1,
        }],
        json!({}),
    )
    .unwrap();
    durable.save_package(&package).await.unwrap();
    durable
        .decide_package(PackageDecisionCommand {
            run_id: run.id,
            package_digest: package.package_digest.clone(),
            decision: GateDecision::Approve,
            reason: None,
            affected_owners: vec![],
            actor: ProductionActor::local_operator(),
            idempotency_key: "approve-brief".into(),
        })
        .await
        .unwrap();
    let screenwriter = durable
        .get_run(run.id)
        .await
        .unwrap()
        .steps
        .into_iter()
        .find(|step| step.step_key == "screenwriter")
        .unwrap();
    let claim_digest = canonical_digest(&json!({
        "run_id": run.id,
        "step_id": screenwriter.id,
        "command": "prepare"
    }))
    .unwrap();
    let claimed = durable
        .claim_step(
            screenwriter.id,
            "worker-role-prepare",
            Duration::from_secs(60),
            &claim_digest,
            "claim-screenwriter",
        )
        .await
        .unwrap();

    let wrong_lease = match RoleExecutor::prepare(
        RolePrepareContext {
            pool: pool.clone(),
            definition_registry: registry.clone(),
            audited_executor: audited.clone(),
            step_id: claimed.id,
            lease_owner: "other-worker".into(),
            attempt: claimed.attempt,
        },
        &role_registry,
    )
    .await
    {
        Ok(_) => panic!("wrong lease must not prepare a role call"),
        Err(error) => error,
    };
    assert_eq!(wrong_lease.code(), "transition_conflict");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM agent_runs WHERE agent_type='production'",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );

    let prepared = RoleExecutor::prepare(
        RolePrepareContext {
            pool: pool.clone(),
            definition_registry: registry,
            audited_executor: audited,
            step_id: claimed.id,
            lease_owner: "worker-role-prepare".into(),
            attempt: claimed.attempt,
        },
        &role_registry,
    )
    .await
    .unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(prepared.run_id, run.id);
    assert_eq!(prepared.role_key, "screenwriter");
    assert_eq!(prepared.input_packages.len(), 1);
    assert_eq!(prepared.input_packages[0].id, package.id);
    assert_eq!(prepared.input_packages[0].digest, package.package_digest);

    let links = sqlx::query_as::<_, (Option<Uuid>, Option<Uuid>, Option<Uuid>, String)>(
        "SELECT agent_run_id, model_call_id, context_snapshot_id, side_effect_state FROM production_steps WHERE id=$1",
    )
    .bind(claimed.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(links.0, Some(prepared.agent_run_id));
    assert_eq!(links.1, Some(prepared.model_call_id));
    assert_eq!(links.2, Some(prepared.context_snapshot_id));
    assert_eq!(links.3, "prepared");
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM model_calls WHERE id=$1")
            .bind(prepared.model_call_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "prepared"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_resource_reservations WHERE step_id=$1 AND attempt_no=$2 AND status='reserved'",
        )
        .bind(claimed.id)
        .bind(claimed.attempt)
        .fetch_one(&pool)
        .await
        .unwrap(),
        3
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM agent_run_bindings WHERE agent_run_id=$1 AND context_binding_status='executable'",
        )
        .bind(prepared.agent_run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
async fn missing_exact_package_blocks_and_closes_the_owned_attempt_before_provider() {
    let (_admin, pool, _guard) = database().await;
    let model_id = insert_enabled_text_model(&pool).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let (registry, audited) = executor(&pool, model_id, calls.clone()).await;
    let role_registry =
        RoleRegistry::bootstrap(Path::new("/app/crates/novex-production-crew/roles")).unwrap();
    let durable = DurableProductionRepository::new(pool.clone());
    let (project_id, topic_id) = source(&pool).await;
    let intent = durable
        .create_intent(create_intent(
            project_id,
            topic_id,
            "missing-package-create",
        ))
        .await
        .unwrap();
    let plan = FullCrewPlanRegistry::snapshot_v1(
        false,
        frozen_bindings(&registry, &audited, model_id).await,
        ResourceLimits::strict_default(),
    )
    .unwrap();
    let run = durable
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan,
            actor: ProductionActor::local_operator(),
            idempotency_key: "missing-package-start".into(),
        })
        .await
        .unwrap();
    let view = durable.get_run(run.id).await.unwrap();
    let digest = canonical_digest(&json!({"completed": true})).unwrap();
    for key in ["validate_source", "producer", "brief_approval"] {
        let step = view.steps.iter().find(|step| step.step_key == key).unwrap();
        sqlx::query(
            "UPDATE production_steps SET status='succeeded', attempt=1, output_digest=$2 WHERE id=$1",
        )
        .bind(step.id)
        .bind(&digest)
        .execute(&pool)
        .await
        .unwrap();
    }
    let screenwriter = view
        .steps
        .iter()
        .find(|step| step.step_key == "screenwriter")
        .unwrap();
    sqlx::query("UPDATE production_steps SET status='queued' WHERE id=$1")
        .bind(screenwriter.id)
        .execute(&pool)
        .await
        .unwrap();
    let claim_digest = canonical_digest(&json!({"step_id": screenwriter.id})).unwrap();
    let claimed = durable
        .claim_step(
            screenwriter.id,
            "worker-missing-package",
            Duration::from_secs(60),
            &claim_digest,
            "claim-missing-package",
        )
        .await
        .unwrap();

    let error = match RoleExecutor::prepare(
        RolePrepareContext {
            pool: pool.clone(),
            definition_registry: registry,
            audited_executor: audited,
            step_id: claimed.id,
            lease_owner: "worker-missing-package".into(),
            attempt: claimed.attempt,
        },
        &role_registry,
    )
    .await
    {
        Ok(_) => panic!("missing package must block role prepare"),
        Err(error) => error,
    };

    assert_eq!(error.code(), "transition_conflict");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let persisted =
        sqlx::query_as::<
            _,
            (
                String,
                Option<String>,
                Option<chrono::DateTime<chrono::Utc>>,
                Option<String>,
                Option<serde_json::Value>,
            ),
        >("SELECT status, lease_owner, lease_expires_at, error_code, error_details FROM production_steps WHERE id=$1")
        .bind(claimed.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(persisted.0, "failed");
    assert!(persisted.1.is_none());
    assert!(persisted.2.is_none());
    assert_eq!(persisted.3.as_deref(), Some("transition_conflict"));
    assert_eq!(
        persisted
            .4
            .as_ref()
            .and_then(|value| value.get("code"))
            .and_then(serde_json::Value::as_str),
        Some("transition_conflict"),
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM production_runs WHERE id=$1")
            .bind(run.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "blocked"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM model_calls call JOIN agent_runs run ON run.id=call.agent_run_id WHERE run.agent_type='production'",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn editor_without_media_evidence_closes_the_owned_attempt_before_model_call() {
    let (_admin, pool, _guard) = database().await;
    let model_id = insert_enabled_text_model(&pool).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let (registry, audited) = executor(&pool, model_id, calls.clone()).await;
    let role_registry =
        RoleRegistry::bootstrap(Path::new("/app/crates/novex-production-crew/roles")).unwrap();
    let durable = DurableProductionRepository::new(pool.clone());
    let (project_id, topic_id) = source(&pool).await;
    let intent = durable
        .create_intent(create_intent(project_id, topic_id, "editor-media-create"))
        .await
        .unwrap();
    let plan = FullCrewPlanRegistry::snapshot_v1(
        false,
        frozen_bindings(&registry, &audited, model_id).await,
        ResourceLimits::strict_default(),
    )
    .unwrap();
    let run = durable
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan,
            actor: ProductionActor::local_operator(),
            idempotency_key: "editor-media-start".into(),
        })
        .await
        .unwrap();
    let editor = durable
        .get_run(run.id)
        .await
        .unwrap()
        .steps
        .into_iter()
        .find(|step| step.step_key == "editor")
        .unwrap();
    sqlx::query(
        r#"
        UPDATE production_steps
        SET status='running',dependencies='[]',attempt=1,
            lease_owner='worker-editor-media',
            lease_expires_at=NOW()+INTERVAL '60 seconds',side_effect_state='none'
        WHERE id=$1
        "#,
    )
    .bind(editor.id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO production_step_attempts (
            step_id,attempt_no,status,request_digest,idempotency_key,lease_owner
        ) VALUES ($1,1,'running',$2,'claim-editor-media','worker-editor-media')
        "#,
    )
    .bind(editor.id)
    .bind(canonical_digest(&json!({"step_id": editor.id})).unwrap())
    .execute(&pool)
    .await
    .unwrap();

    let error = match RoleExecutor::prepare(
        RolePrepareContext {
            pool: pool.clone(),
            definition_registry: registry,
            audited_executor: audited,
            step_id: editor.id,
            lease_owner: "worker-editor-media".into(),
            attempt: 1,
        },
        &role_registry,
    )
    .await
    {
        Ok(_) => panic!("Editor 缺少媒体证据时不得完成 prepare"),
        Err(error) => error,
    };

    assert_eq!(error.code(), "evidence_blocker");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT status,error_code FROM production_steps WHERE id=$1",
        )
        .bind(editor.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        ("failed".into(), Some("evidence_blocker".into()))
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM agent_runs WHERE agent_type='production'",
        )
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
}

#[tokio::test]
async fn candidate_binding_cannot_start_a_normal_production_run() {
    let (_admin, pool, _guard) = database().await;
    let model_id = insert_enabled_text_model(&pool).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let (registry, audited) = executor(&pool, model_id, calls.clone()).await;
    let durable = DurableProductionRepository::new(pool.clone());
    let (project_id, topic_id) = source(&pool).await;
    let intent = durable
        .create_intent(create_intent(project_id, topic_id, "candidate-create"))
        .await
        .unwrap();
    let mut bindings = frozen_bindings(&registry, &audited, model_id).await;
    bindings["screenwriter"]["lifecycle"] = json!("candidate");
    let plan = FullCrewPlanRegistry::snapshot_v1(false, bindings, ResourceLimits::strict_default())
        .unwrap();

    let error = durable
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan,
            actor: ProductionActor::local_operator(),
            idempotency_key: "candidate-start".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), "transition_conflict");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM agent_runs WHERE agent_type='production'",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn current_candidate_registry_never_enters_normal_runs_or_mutates_frozen_bindings() {
    let (_admin, pool, _guard) = database().await;
    let model_id = insert_enabled_text_model(&pool).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let (registry, audited) = executor(&pool, model_id, calls.clone()).await;
    let durable = DurableProductionRepository::new(pool.clone());

    // 模拟兼容性预检上线前已经冻结的 v2 Run；后续 registry 发布不得改写历史 binding。
    let (first_project_id, first_topic_id) = source(&pool).await;
    let first_intent = durable
        .create_intent(create_intent(
            first_project_id,
            first_topic_id,
            "candidate-manifest-before",
        ))
        .await
        .unwrap();
    let mut legacy_bindings = frozen_bindings(&registry, &audited, model_id).await;
    legacy_bindings.as_object_mut().unwrap().insert(
        "character_critic".into(),
        serde_json::to_value(
            RoleExecutor::freeze_active_binding(
                "character_critic",
                "2.0.0",
                &registry,
                &audited,
                model_id,
            )
            .await
            .unwrap(),
        )
        .unwrap(),
    );
    let first_run = durable
        .start_run(StartRunCommand {
            intent_id: first_intent.id,
            plan: FullCrewPlanRegistry::snapshot_v1(
                true,
                legacy_bindings,
                ResourceLimits::strict_default(),
            )
            .unwrap(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "candidate-manifest-first-run".into(),
        })
        .await
        .unwrap();
    assert_active_v2_full_crew_bindings(&first_run.binding_snapshot);
    let frozen_before_publish = first_run.binding_snapshot.clone();
    let service = ProductionWorkflowService::new(pool.clone(), registry.clone(), audited);

    // v2 active Prompt 仍是旧宽松 schema；v3 未通过 Eval 前不得被普通 Run 使用，
    // 因此新 Run 必须在任何模型调用前稳定 fail-closed。
    let (blocked_project_id, blocked_topic_id) = source(&pool).await;
    let blocked_intent = durable
        .create_intent(create_intent(
            blocked_project_id,
            blocked_topic_id,
            "candidate-manifest-blocked-before-publish",
        ))
        .await
        .unwrap();
    let error = service
        .start_run(
            blocked_intent.id,
            "candidate-manifest-blocked-before-publish-run".into(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), "capability_mismatch");
    assert!(error.to_string().contains("durable-role-output-contract@1"));

    PostgresDefinitionReleaseRepository::new(pool.clone())
        .publish_registry(&registry)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM definition_release_manifest_entries
            WHERE definition_kind = 'agent'
              AND definition_key LIKE 'production.%'
              AND definition_version = '3.0.0'
              AND lifecycle_status = 'candidate'
            "#,
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        9
    );

    let persisted_first = durable.get_run(first_run.id).await.unwrap().run;
    assert_eq!(persisted_first.binding_snapshot, frozen_before_publish);

    let (second_project_id, second_topic_id) = source(&pool).await;
    let second_intent = durable
        .create_intent(create_intent(
            second_project_id,
            second_topic_id,
            "candidate-manifest-after",
        ))
        .await
        .unwrap();
    let error = service
        .start_run(second_intent.id, "candidate-manifest-second-run".into())
        .await
        .unwrap_err();
    assert_eq!(error.code(), "capability_mismatch");
    assert!(error.to_string().contains("durable-role-output-contract@1"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1,
        "发布 candidate 前后都不得为不兼容 active schema 创建新 Run"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

fn assert_active_v2_full_crew_bindings(bindings: &serde_json::Value) {
    let bindings = bindings.as_object().unwrap();
    assert_eq!(bindings.len(), 9);
    for role in [
        "producer",
        "screenwriter",
        "character_critic",
        "director",
        "cinematographer",
        "performance_director",
        "sound_director",
        "editor",
        "qc",
    ] {
        let binding = &bindings[role];
        assert_eq!(binding["definition_version"], "2.0.0", "role={role}");
        assert_eq!(binding["lifecycle"], "active", "role={role}");
    }
}

fn producer_output() -> String {
    json!({
        "creative_brief": {
            "target_audience": "工程师",
            "tone": ["严谨"],
            "key_messages": ["事务必须原子"],
            "constraints": {},
            "success_criteria": ["审计闭合"]
        }
    })
    .to_string()
}

fn screenwriter_output() -> String {
    json!({
        "story_bible": {
            "premise": "一次原子提交",
            "theme": "一致性",
            "narrative_structure": "linear",
            "world": "工程系统"
        },
        "character_bibles": [{
            "character_id": "operator",
            "name": "操作者",
            "role": "protagonist",
            "personality": "严谨",
            "motivation": "闭合审计",
            "arc": "从准备到提交"
        }],
        "script_draft": {
            "title": "原子执行",
            "hook": "任何部分写入都不可接受",
            "scenes": [
                {"sequence": 1, "narration": "准备", "visual_description": "加载固定输入", "emotion": "专注", "duration_sec": 5, "character_ids": ["operator"]},
                {"sequence": 2, "narration": "执行", "visual_description": "模型返回", "emotion": "紧张", "duration_sec": 5, "character_ids": ["operator"]},
                {"sequence": 3, "narration": "提交", "visual_description": "事务完成", "emotion": "平静", "duration_sec": 5, "character_ids": ["operator"]}
            ]
        }
    })
    .to_string()
}

#[tokio::test]
async fn provider_parse_and_schema_failures_close_every_audit_anchor_without_artifacts() {
    for (case, response, expected_code) in [
        (
            "provider",
            Err(LLMError::Provider("rejected".into())),
            "agent_execution_failed",
        ),
        ("parse", Ok("not-json".into()), "invalid_artifact_schema"),
        (
            "schema",
            Ok(json!({}).to_string()),
            "invalid_artifact_schema",
        ),
    ] {
        let (_admin, pool, _guard) = database().await;
        let model_id = insert_enabled_text_model(&pool).await;
        let calls = Arc::new(AtomicUsize::new(0));
        let (registry, audited) =
            executor_with_response(&pool, model_id, calls.clone(), response).await;
        let lease_owner = format!("worker-{case}");
        let prepared = prepare_producer(&pool, model_id, registry, audited, &lease_owner).await;
        let step_id = prepared.step_id;
        let model_call_id = prepared.model_call_id;
        let agent_run_id = prepared.agent_run_id;

        let executed = RoleExecutor::execute_prepared(prepared).await;
        let error = RoleExecutor::finalize(
            RoleFinalizeContext {
                pool: pool.clone(),
                lease_owner,
            },
            &executed,
        )
        .await
        .unwrap_err();

        assert_eq!(error.code(), expected_code, "case={case}");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "case={case}");
        assert_eq!(
            sqlx::query_as::<_, (String, String, String)>(
                r#"
                SELECT step.status, agent.status, call.status
                FROM production_steps step
                JOIN agent_runs agent ON agent.id=step.agent_run_id
                JOIN model_calls call ON call.id=step.model_call_id
                WHERE step.id=$1 AND agent.id=$2 AND call.id=$3
                "#,
            )
            .bind(step_id)
            .bind(agent_run_id)
            .bind(model_call_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            ("failed".into(), "failed".into(), "failed".into()),
            "case={case}",
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM creative_briefs WHERE step_id=$1",)
                .bind(step_id)
                .fetch_one(&pool)
                .await
                .unwrap(),
            0,
            "case={case}",
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM production_resource_reservations WHERE step_id=$1 AND status='reserved'",
            )
            .bind(step_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            0,
            "case={case}",
        );
    }
}

#[tokio::test]
async fn database_failure_rolls_back_every_screenwriter_artifact_and_closes_audit() {
    let (_admin, pool, _guard) = database().await;
    let model_id = insert_enabled_text_model(&pool).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let (registry, audited) =
        executor_with_response(&pool, model_id, calls.clone(), Ok(screenwriter_output())).await;
    let lease_owner = "worker-screenwriter-db-failure";
    let prepared = prepare_screenwriter(&pool, model_id, registry, audited, lease_owner).await;
    let step_id = prepared.step_id;
    let model_call_id = prepared.model_call_id;
    let agent_run_id = prepared.agent_run_id;
    sqlx::query(
        r#"
        CREATE FUNCTION reject_character_bible_finalize() RETURNS TRIGGER AS $$
        BEGIN
            RAISE EXCEPTION 'injected_character_bible_failure';
        END;
        $$ LANGUAGE plpgsql
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        CREATE TRIGGER reject_character_bible_finalize
        BEFORE INSERT ON character_bibles
        FOR EACH ROW EXECUTE FUNCTION reject_character_bible_finalize()
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let executed = RoleExecutor::execute_prepared(prepared).await;
    let error = RoleExecutor::finalize(
        RoleFinalizeContext {
            pool: pool.clone(),
            lease_owner: lease_owner.into(),
        },
        &executed,
    )
    .await
    .unwrap_err();

    assert_eq!(error.code(), "database_error");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    for table in ["story_bibles", "character_bibles", "script_drafts"] {
        let count =
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table} WHERE step_id=$1"))
                .bind(step_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 0, "table={table}");
    }
    assert_eq!(
        sqlx::query_as::<_, (String, String, String)>(
            r#"
            SELECT step.status, agent.status, call.status
            FROM production_steps step
            JOIN agent_runs agent ON agent.id=step.agent_run_id
            JOIN model_calls call ON call.id=step.model_call_id
            WHERE step.id=$1 AND agent.id=$2 AND call.id=$3
            "#,
        )
        .bind(step_id)
        .bind(agent_run_id)
        .bind(model_call_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        ("failed".into(), "failed".into(), "failed".into()),
    );
}

#[tokio::test]
async fn successful_finalize_is_atomic_and_replays_the_same_attempt_result() {
    let (_admin, pool, _guard) = database().await;
    let model_id = insert_enabled_text_model(&pool).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let (registry, audited) =
        executor_with_response(&pool, model_id, calls.clone(), Ok(producer_output())).await;
    let lease_owner = "worker-producer-success";
    let prepared = prepare_producer(&pool, model_id, registry, audited, lease_owner).await;
    let step_id = prepared.step_id;
    let attempt = prepared.attempt;
    let executed = RoleExecutor::execute_prepared(prepared).await;
    let context = RoleFinalizeContext {
        pool: pool.clone(),
        lease_owner: lease_owner.into(),
    };

    let first = RoleExecutor::finalize(context.clone(), &executed)
        .await
        .unwrap();
    let replay = RoleExecutor::finalize(context, &executed).await.unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(first.output_artifacts.len(), 1);
    assert_eq!(first.output_artifacts[0].id, replay.output_artifacts[0].id);
    assert_eq!(first.model_call_id, replay.model_call_id);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM creative_briefs WHERE step_id=$1 AND attempt=$2",
        )
        .bind(step_id)
        .bind(attempt)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1,
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM production_steps WHERE id=$1")
            .bind(step_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "succeeded",
    );
    let audit = sqlx::query_as::<_, (String, String, Option<Uuid>, i64)>(
        r#"
        SELECT agent.status, call.status, step.context_snapshot_id,
               (SELECT COUNT(*) FROM creative_briefs artifact
                WHERE artifact.step_id=step.id AND artifact.attempt=step.attempt)
        FROM production_steps step
        JOIN agent_runs agent ON agent.id=step.agent_run_id
        JOIN model_calls call ON call.id=step.model_call_id
        WHERE step.id=$1
        "#,
    )
    .bind(step_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit.0, "succeeded");
    assert_eq!(audit.1, "succeeded");
    assert!(audit.2.is_some());
    assert_eq!(audit.3, 1);
}

#[tokio::test]
async fn uncertain_provider_result_closes_audit_and_holds_resources_for_manual_attention() {
    let (_admin, pool, _guard) = database().await;
    let model_id = insert_enabled_text_model(&pool).await;
    let calls = Arc::new(AtomicUsize::new(0));
    let (registry, audited) =
        executor_with_response(&pool, model_id, calls.clone(), Err(LLMError::Timeout)).await;
    let lease_owner = "worker-provider-timeout";
    let prepared = prepare_producer(&pool, model_id, registry, audited, lease_owner).await;
    let step_id = prepared.step_id;
    let agent_run_id = prepared.agent_run_id;
    let model_call_id = prepared.model_call_id;
    let context_snapshot_id = prepared.context_snapshot_id;

    let executed = RoleExecutor::execute_prepared(prepared).await;
    let error = RoleExecutor::finalize(
        RoleFinalizeContext {
            pool: pool.clone(),
            lease_owner: lease_owner.into(),
        },
        &executed,
    )
    .await
    .unwrap_err();

    assert_eq!(error.code(), "attention_required");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let audit = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            Option<Uuid>,
            Option<Uuid>,
            Option<Uuid>,
        ),
    >(
        r#"
        SELECT step.status, attempt.status, agent.status, call.status,
               step.agent_run_id, step.model_call_id, step.context_snapshot_id
        FROM production_steps step
        JOIN production_step_attempts attempt
          ON attempt.step_id=step.id AND attempt.attempt_no=step.attempt
        JOIN agent_runs agent ON agent.id=step.agent_run_id
        JOIN model_calls call ON call.id=step.model_call_id
        WHERE step.id=$1
        "#,
    )
    .bind(step_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit.0, "attention_required");
    assert_eq!(audit.1, "attention_required");
    assert_eq!(audit.2, "failed");
    assert_eq!(audit.3, "failed");
    assert_eq!(audit.4, Some(agent_run_id));
    assert_eq!(audit.5, Some(model_call_id));
    assert_eq!(audit.6, Some(context_snapshot_id));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_resource_reservations WHERE step_id=$1 AND status='held_uncertain'",
        )
        .bind(step_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        3,
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            r#"
            SELECT
                (SELECT COUNT(*) FROM production_steps WHERE id=$1 AND status='running') +
                (SELECT COUNT(*) FROM production_step_attempts WHERE step_id=$1 AND status IN ('running','prepared')) +
                (SELECT COUNT(*) FROM agent_runs WHERE id=$2 AND status='running') +
                (SELECT COUNT(*) FROM model_calls WHERE id=$3 AND status='prepared')
            "#,
        )
        .bind(step_id)
        .bind(agent_run_id)
        .bind(model_call_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0,
    );
}
