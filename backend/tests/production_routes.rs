use axum::body::Body;
use axum::http::{Request, StatusCode};
use novex_ai_core::{canonical_json, sha256_hex, DefinitionRegistry};
use novex_api::bootstrap::{AppConfig, AppState};
use novex_api::build_app_with_state;
use novex_production_crew::durable::canonical_digest;
use novex_production_crew::durable::package::{ArtifactPackageSnapshot, ArtifactRef, PackageType};
use novex_production_crew::durable::repository::DurableProductionRepository;
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{
    fs,
    path::Path,
    sync::{Arc, OnceLock},
};
use tower::ServiceExt;
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

async fn database() -> (PgPool, PgPool, TestDatabase, String) {
    let base_url = database_url();
    let database_name = format!("production_routes_{}", Uuid::new_v4().simple());
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
    (admin, pool, guard, test_url)
}

fn app_state(test_url: String, pool: PgPool) -> AppState {
    AppState::new(
        AppConfig {
            environment: "test".into(),
            database_url: test_url,
            redis_url: "redis://127.0.0.1:6379/15".into(),
            openai_api_key: "".into(),
            openai_base_url: "https://example.invalid/v1".into(),
            openai_model: "test-model".into(),
            openai_timeout_seconds: 5,
            openai_reasoning_effort: Some("low".into()),
            openai_max_output_tokens: 4096,
            asset_storage_root: "/app/storage/assets".into(),
            asset_generation_providers: vec![],
        },
        pool,
        None,
    )
    .unwrap()
}

fn app(test_url: String, pool: PgPool) -> axum::Router {
    build_app_with_state(
        app_state(test_url, pool).with_definition_registry(compatible_definition_registry()),
    )
}

fn production_registry_app(test_url: String, pool: PgPool) -> axum::Router {
    build_app_with_state(app_state(test_url, pool))
}

fn compatible_definition_registry() -> Arc<DefinitionRegistry> {
    static REGISTRY: OnceLock<Arc<DefinitionRegistry>> = OnceLock::new();
    REGISTRY
        .get_or_init(|| {
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("agent-definitions");
            let mut document: Value =
                serde_json::from_slice(&fs::read(root.join("registry.json")).unwrap()).unwrap();
            let prompts = document["prompts"].as_array_mut().unwrap();
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
                let prompt_key = format!("production.{role}.general");
                let compatible = prompts
                    .iter()
                    .find(|prompt| {
                        prompt["prompt_key"] == prompt_key && prompt["version"] == "3.0.0"
                    })
                    .unwrap()["output_schema"]
                    .clone();
                let active = prompts
                    .iter_mut()
                    .find(|prompt| {
                        prompt["prompt_key"] == prompt_key && prompt["version"] == "2.0.0"
                    })
                    .unwrap();
                active["output_schema"] = compatible;
            }

            let directory = std::env::temp_dir().join(format!(
                "novex-compatible-production-registry-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&directory);
            fs::create_dir_all(&directory).unwrap();
            for prompt in document["prompts"].as_array().unwrap() {
                for field in ["system_template", "user_template"] {
                    let relative = prompt[field].as_str().unwrap();
                    let target = directory.join(relative);
                    fs::create_dir_all(target.parent().unwrap()).unwrap();
                    fs::copy(root.join(relative), target).unwrap();
                }
            }
            fs::write(
                directory.join("registry.json"),
                serde_json::to_vec(&document).unwrap(),
            )
            .unwrap();
            fs::write(
                directory.join("release-index.json"),
                serde_json::to_vec(&json!({
                    "schema_version": document["schema_version"],
                    "registry_digest": sha256_hex(canonical_json(&document).as_bytes()),
                    "releases": []
                }))
                .unwrap(),
            )
            .unwrap();
            let registry = Arc::new(DefinitionRegistry::load(&directory).unwrap());
            fs::remove_dir_all(directory).unwrap();
            registry
        })
        .clone()
}

async fn source(pool: &PgPool, suffix: &str) -> (Uuid, Uuid) {
    let project_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO projects (name,positioning,status) VALUES ($1,'知识视频','active') RETURNING id",
    )
    .bind(format!("Full Crew API {suffix}"))
    .fetch_one(pool)
    .await
    .unwrap();
    let topic_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO content_topics (project_id,title,angle,target_audience,status)
        VALUES ($1,$2,'工程审计','开发者','approved') RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(format!("持久流程 {suffix}"))
    .fetch_one(pool)
    .await
    .unwrap();
    (project_id, topic_id)
}

async fn request(
    app: &axum::Router,
    method: &str,
    uri: &str,
    idempotency_key: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(key) = idempotency_key {
        builder = builder.header("idempotency-key", key);
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()))
    };
    (status, value)
}

async fn create_and_start(
    app: &axum::Router,
    project_id: Uuid,
    topic_id: Uuid,
    suffix: &str,
) -> (Uuid, Uuid) {
    let (status, created) = request(
        app,
        "POST",
        "/api/v1/production/intents",
        Some(&format!("create-{suffix}")),
        json!({
            "project_id": project_id,
            "topic_id": topic_id,
            "title": format!("Full Crew {suffix}"),
            "description": "HTTP durable contract",
            "initial_input": {"brief": "只使用已确认来源"}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let intent_id = Uuid::parse_str(created["intent"]["id"].as_str().unwrap()).unwrap();
    let (status, started) = request(
        app,
        "POST",
        &format!("/api/v1/production/intents/{intent_id}/runs"),
        Some(&format!("start-{suffix}")),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{started}");
    let run_id = Uuid::parse_str(started["run"]["id"].as_str().unwrap()).unwrap();
    (intent_id, run_id)
}

async fn create_intent_request(
    app: &axum::Router,
    project_id: Uuid,
    topic_id: Uuid,
    key: &str,
) -> (StatusCode, Value) {
    request(
        app,
        "POST",
        "/api/v1/production/intents",
        Some(key),
        json!({
            "project_id": project_id,
            "topic_id": topic_id,
            "title": format!("Full Crew {key}"),
            "description": "HTTP source validation contract",
            "initial_input": {"brief": "只使用已确认来源"}
        }),
    )
    .await
}

async fn seed_brief_package(pool: &PgPool, run_id: Uuid, version: u32) -> ArtifactPackageSnapshot {
    let steps = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id,step_key FROM production_steps WHERE run_id=$1 AND revision_epoch=0 AND step_key IN ('validate_source','producer','brief_approval')",
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .map(|(id, key)| (key, id))
    .collect::<std::collections::BTreeMap<_, _>>();
    sqlx::query(
        "UPDATE production_steps SET status='succeeded',attempt=1,output_digest=$2 WHERE id IN ($1,$3)",
    )
    .bind(steps["validate_source"])
    .bind("1".repeat(64))
    .bind(steps["producer"])
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("UPDATE production_steps SET status='queued' WHERE id=$1")
        .bind(steps["brief_approval"])
        .execute(pool)
        .await
        .unwrap();
    let content = json!({"target_audience": "开发者", "key_messages": ["持久化"]});
    let artifact_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO creative_briefs (
            id,production_project_id,version,status,content,created_by,
            run_id,step_id,attempt,revision_epoch,content_digest,audit_status
        ) SELECT $1,production_project_id,1,'draft',$2,'producer',$3,$4,1,0,$5,'complete'
          FROM production_runs WHERE id=$3
        "#,
    )
    .bind(artifact_id)
    .bind(&content)
    .bind(run_id)
    .bind(steps["producer"])
    .bind(canonical_digest(&content).unwrap())
    .execute(pool)
    .await
    .unwrap();
    ArtifactPackageSnapshot::build(
        PackageType::Brief,
        run_id,
        steps["producer"],
        1,
        0,
        version,
        vec![ArtifactRef {
            run_id,
            artifact_type: "creative_brief".into(),
            artifact_id,
            version: 1,
            content_digest: canonical_digest(&content).unwrap(),
            source_step_id: steps["producer"],
            source_attempt: 1,
        }],
        json!({}),
    )
    .unwrap()
}

#[tokio::test]
async fn production_registry_rejects_incompatible_active_role_schemas_before_any_model_call() {
    let (_admin, pool, _guard, test_url) = database().await;
    insert_enabled_text_model(&pool).await;
    let app = production_registry_app(test_url, pool.clone());
    let (project_id, topic_id) = source(&pool, "incompatible-active-schema").await;
    let (status, created) =
        create_intent_request(&app, project_id, topic_id, "incompatible-create").await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let intent_id = Uuid::parse_str(created["intent"]["id"].as_str().unwrap()).unwrap();

    let (status, rejected) = request(
        &app,
        "POST",
        &format!("/api/v1/production/intents/{intent_id}/runs"),
        Some("incompatible-start"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{rejected}");
    assert_eq!(rejected["error"], "capability_mismatch");
    assert!(rejected["message"]
        .as_str()
        .unwrap()
        .contains("durable-role-output-contract@1"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_runs")
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
async fn full_crew_http_commands_create_query_decide_resume_and_retry_durable_state() {
    let (_admin, pool, _guard, test_url) = database().await;
    insert_enabled_text_model(&pool).await;
    let app = app(test_url, pool.clone());
    let repo = DurableProductionRepository::new(pool.clone());
    let (project_id, topic_id) = source(&pool, "approve").await;
    let (intent_id, run_id) = create_and_start(&app, project_id, topic_id, "approve").await;

    let (status, resumed) = request(
        &app,
        "POST",
        &format!("/api/v1/production/runs/{run_id}/resume"),
        Some("resume-initial"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{resumed}");
    assert_eq!(resumed["status"], "accepted");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_wakeups WHERE run_id=$1 AND status='pending'",
        )
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );

    let package = seed_brief_package(&pool, run_id, 1).await;
    repo.save_package(&package).await.unwrap();
    let (status, queried) = request(
        &app,
        "GET",
        &format!("/api/v1/production/runs/{run_id}"),
        None,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{queried}");
    assert_eq!(queried["run"]["id"], run_id.to_string());
    assert_eq!(queried["run"]["status"], "waiting_approval");
    assert!(queried["steps"].as_array().unwrap().len() > 10);
    assert_eq!(queried["packages"].as_array().unwrap().len(), 1);
    assert!(queried["gate_decisions"].as_array().unwrap().is_empty());
    assert!(queried["domain_links"].is_array());
    assert!(queried["allowed_commands"]
        .as_array()
        .unwrap()
        .iter()
        .any(|command| command["kind"] == "approve_package"
            && command["step_key"] == "brief_approval"));
    assert!(queried["allowed_commands"]
        .as_array()
        .unwrap()
        .iter()
        .any(|command| command["kind"] == "reject_package"
            && command["step_key"] == "brief_approval"));
    let public_json = queried.to_string().to_ascii_lowercase();
    for forbidden in [
        "test-key",
        "\"api_key\"",
        "\"authorization\"",
        "\"raw_response\"",
        "\"price\"",
        "\"currency\"",
        "\"amount_limit\"",
    ] {
        assert!(
            !public_json.contains(forbidden),
            "run query leaked forbidden field/value: {forbidden}"
        );
    }

    let (status, stale) = request(
        &app,
        "POST",
        &format!(
            "/api/v1/production/runs/{run_id}/packages/{}/approve",
            "f".repeat(64)
        ),
        Some("approve-stale"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{stale}");
    assert_eq!(stale["error"], "stale_package");

    let (status, missing_key) = request(
        &app,
        "POST",
        &format!("/api/v1/production/runs/{run_id}/resume"),
        None,
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{missing_key}");
    assert_eq!(missing_key["error"], "idempotency_key_required");

    let (status, approved) = request(
        &app,
        "POST",
        &format!(
            "/api/v1/production/runs/{run_id}/packages/{}/approve",
            package.package_digest
        ),
        Some("approve-brief"),
        json!({"note": "Brief 已确认"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{approved}");
    assert_eq!(approved["decision"]["decision"], "approved");

    let retry_step_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        UPDATE production_steps
        SET status='failed',retryable=TRUE,error_code='fixture_retryable'
        WHERE run_id=$1 AND revision_epoch=0 AND step_key='screenwriter'
        RETURNING id
        "#,
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE production_runs SET status='blocked' WHERE id=$1")
        .bind(run_id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, retry) = request(
        &app,
        "POST",
        &format!("/api/v1/production/runs/{run_id}/steps/{retry_step_id}/retry"),
        Some("retry-screenwriter"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{retry}");
    assert_eq!(retry["step_id"], retry_step_id.to_string());
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM production_steps WHERE id=$1")
            .bind(retry_step_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "queued"
    );

    let (reject_project, reject_topic) = source(&pool, "reject").await;
    let (_, reject_run) = create_and_start(&app, reject_project, reject_topic, "reject").await;
    let rejected_package = seed_brief_package(&pool, reject_run, 1).await;
    repo.save_package(&rejected_package).await.unwrap();
    let (status, rejected) = request(
        &app,
        "POST",
        &format!(
            "/api/v1/production/runs/{reject_run}/packages/{}/reject",
            rejected_package.package_digest
        ),
        Some("reject-brief"),
        json!({"reason": "目标受众需要重写", "affected_owners": ["producer"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{rejected}");
    assert_eq!(rejected["decision"]["decision"], "rejected");
    assert_eq!(
        sqlx::query_scalar::<_, i32>(
            "SELECT current_revision_epoch FROM production_runs WHERE id=$1",
        )
        .bind(reject_run)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM production_projects WHERE id=$1 AND project_id=$2 AND topic_id=$3",
        )
        .bind(intent_id)
        .bind(project_id)
        .bind(topic_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        intent_id
    );
}

#[tokio::test]
async fn full_crew_http_rejects_invalid_sources_plan_overrides_and_role_bypasses() {
    let (_admin, pool, _guard, test_url) = database().await;
    insert_enabled_text_model(&pool).await;
    let app = app(test_url, pool.clone());

    let (owner_project, owner_topic) = source(&pool, "owner").await;
    let (other_project, _) = source(&pool, "other-account").await;
    let (status, body) =
        create_intent_request(&app, other_project, owner_topic, "cross-account").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"], "source_invalid");

    let (deleted_project, deleted_topic) = source(&pool, "deleted").await;
    sqlx::query("UPDATE content_topics SET deleted_at=NOW() WHERE id=$1")
        .bind(deleted_topic)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) =
        create_intent_request(&app, deleted_project, deleted_topic, "deleted").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"], "source_invalid");

    let (idea_project, idea_topic) = source(&pool, "idea").await;
    sqlx::query("UPDATE content_topics SET status='idea' WHERE id=$1")
        .bind(idea_topic)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = create_intent_request(&app, idea_project, idea_topic, "idea").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"], "source_invalid");

    let (status, created) =
        create_intent_request(&app, owner_project, owner_topic, "fixed-plan").await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let intent_id = Uuid::parse_str(created["intent"]["id"].as_str().unwrap()).unwrap();
    for (index, payload) in [
        json!({"roles": ["producer", "director"]}),
        json!({"auto_approve": true}),
        json!({"skip_gates": ["brief_approval"]}),
        json!({"plan_version": "client-replacement"}),
    ]
    .into_iter()
    .enumerate()
    {
        let (status, _) = request(
            &app,
            "POST",
            &format!("/api/v1/production/intents/{intent_id}/runs"),
            Some(&format!("override-{index}")),
            payload,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_runs WHERE production_project_id=$1",
        )
        .bind(intent_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );

    let (status, started) = request(
        &app,
        "POST",
        &format!("/api/v1/production/intents/{intent_id}/runs"),
        Some("fixed-plan-start"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{started}");
    let run_id = Uuid::parse_str(started["run"]["id"].as_str().unwrap()).unwrap();
    let (status, _) = request(
        &app,
        "POST",
        &format!("/api/v1/production/productions/{intent_id}/roles/director/execute"),
        Some("bypass-director"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_steps WHERE run_id=$1 AND agent_run_id IS NOT NULL",
        )
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn production_run_status_exposes_waits_blockers_retryability_commands_and_audit_ids() {
    let (_admin, pool, _guard, test_url) = database().await;
    let model_id = insert_enabled_text_model(&pool).await;
    let app = app(test_url, pool.clone());
    let (project_id, topic_id) = source(&pool, "status").await;
    let (_, run_id) = create_and_start(&app, project_id, topic_id, "status").await;
    let steps = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id,step_key FROM production_steps WHERE run_id=$1 AND revision_epoch=0",
    )
    .bind(run_id)
    .fetch_all(&pool)
    .await
    .unwrap()
    .into_iter()
    .map(|(id, key)| (key, id))
    .collect::<std::collections::BTreeMap<_, _>>();
    let agent_run_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO agent_runs (project_id,agent_type,status) VALUES ($1,'production','failed') RETURNING id",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let model_call_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO model_calls (
            agent_run_id,node_key,attempt,status,agent_key,agent_version,
            prompt_key,prompt_version,registry_digest,prompt_snapshot,
            model_id,behavior_fingerprint,model_snapshot,error_snapshot,completed_at
        ) VALUES (
            $1,'production.producer.execute',1,'failed','production.producer','2.0.0',
            'production.producer.general','2.0.0',$2,'{"system":"fixture","user":"fixture"}',
            $3,$4,'{"provider":"test"}','{"code":"resource_limit"}',NOW()
        ) RETURNING id
        "#,
    )
    .bind(agent_run_id)
    .bind("1".repeat(64))
    .bind(model_id)
    .bind("2".repeat(64))
    .fetch_one(&pool)
    .await
    .unwrap();
    let context_snapshot_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO context_snapshots (
            agent_run_id,node_key,compiled_at,policy_key,policy_version,
            tokenizer_profile_key,tokenizer_profile_version,tokenizer_mode,
            model_context_window,budget_ledger,decisions,selected_order,
            logical_input,context_digest
        ) VALUES (
            $1,'production.producer.execute',NOW(),'production.producer.execute.baseline','1.0.0',
            'openai.o200k','1.0.0','exact',128000,'{}','[]','[]','{}',$2
        ) RETURNING id
        "#,
    )
    .bind(agent_run_id)
    .bind("3".repeat(64))
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("UPDATE production_steps SET status='succeeded',attempt=1 WHERE id=$1")
        .bind(steps["validate_source"])
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        r#"
        UPDATE production_steps
        SET status='failed',error_code='resource_limit',
            error_details='{"resource":"role_calls","current":16,"limit":16}',
            retryable=FALSE,side_effect_state='confirmed',attempt=1,
            agent_run_id=$2,model_call_id=$3,context_snapshot_id=$4
        WHERE id=$1
        "#,
    )
    .bind(steps["producer"])
    .bind(agent_run_id)
    .bind(model_call_id)
    .bind(context_snapshot_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        UPDATE production_runs SET status='blocked',error_code='resource_limit',
            error_details='{"resource":"role_calls","current":16,"limit":16}'
        WHERE id=$1
        "#,
    )
    .bind(run_id)
    .execute(&pool)
    .await
    .unwrap();
    let (status, resource) = request(
        &app,
        "GET",
        &format!("/api/v1/production/runs/{run_id}"),
        None,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resource}");
    let producer = resource["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["step_key"] == "producer")
        .unwrap();
    assert_eq!(producer["error_code"], "resource_limit");
    assert_eq!(producer["error_details"]["resource"], "role_calls");
    assert_eq!(producer["retryable"], false);
    assert_eq!(producer["agent_run_id"], agent_run_id.to_string());
    assert_eq!(producer["model_call_id"], model_call_id.to_string());
    assert_eq!(
        producer["context_snapshot_id"],
        context_snapshot_id.to_string()
    );

    sqlx::query(
        r#"
        UPDATE production_steps SET error_code='capability_mismatch',
            error_details='{"capability":"vision"}' WHERE id=$1
        "#,
    )
    .bind(steps["producer"])
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE production_runs SET error_code='capability_mismatch',error_details='{\"capability\":\"vision\"}' WHERE id=$1",
    )
    .bind(run_id)
    .execute(&pool)
    .await
    .unwrap();
    let (_, capability) = request(
        &app,
        "GET",
        &format!("/api/v1/production/runs/{run_id}"),
        None,
        Value::Null,
    )
    .await;
    assert!(capability["steps"].as_array().unwrap().iter().any(|step| {
        step["error_code"] == "capability_mismatch"
            && step["error_details"]["capability"] == "vision"
    }));

    sqlx::query(
        r#"
        UPDATE production_steps SET status='attention_required',
            error_code='attention_required',error_details='{"result":"unknown"}',
            retryable=FALSE,side_effect_state='unknown' WHERE id=$1
        "#,
    )
    .bind(steps["producer"])
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE production_runs SET status='attention_required',error_code='attention_required',error_details='{\"result\":\"unknown\"}' WHERE id=$1",
    )
    .bind(run_id)
    .execute(&pool)
    .await
    .unwrap();
    let (_, attention) = request(
        &app,
        "GET",
        &format!("/api/v1/production/runs/{run_id}"),
        None,
        Value::Null,
    )
    .await;
    assert_eq!(attention["run"]["status"], "attention_required");
    assert!(!attention["allowed_commands"]
        .as_array()
        .unwrap()
        .iter()
        .any(|command| command["kind"] == "retry_step"));

    sqlx::query(
        r#"
        UPDATE production_steps SET status='failed',error_code='transition_conflict',
            error_details='{"domain":"script_promotion"}',retryable=TRUE,
            side_effect_state='none' WHERE id=$1
        "#,
    )
    .bind(steps["producer"])
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE production_runs SET status='blocked',error_code='transition_conflict',error_details='{\"domain\":\"script_promotion\"}' WHERE id=$1",
    )
    .bind(run_id)
    .execute(&pool)
    .await
    .unwrap();
    let (_, domain_conflict) = request(
        &app,
        "GET",
        &format!("/api/v1/production/runs/{run_id}"),
        None,
        Value::Null,
    )
    .await;
    assert!(domain_conflict["allowed_commands"]
        .as_array()
        .unwrap()
        .iter()
        .any(|command| command["kind"] == "retry_step" && command["step_key"] == "producer"));

    sqlx::query(
        "UPDATE production_steps SET status='succeeded',error_code=NULL,error_details=NULL,retryable=FALSE,side_effect_state='confirmed' WHERE id=$1",
    )
    .bind(steps["producer"])
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE production_steps SET status='external_wait',waiting_reason='scene_visual_manifest' WHERE id=$1",
    )
    .bind(steps["wait_scene_visual_manifest"])
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE production_runs SET status='external_wait',error_code=NULL,error_details=NULL WHERE id=$1",
    )
    .bind(run_id)
    .execute(&pool)
    .await
    .unwrap();
    let (_, external_wait) = request(
        &app,
        "GET",
        &format!("/api/v1/production/runs/{run_id}"),
        None,
        Value::Null,
    )
    .await;
    assert_eq!(external_wait["run"]["status"], "external_wait");
    assert!(external_wait["steps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|step| {
            step["step_key"] == "wait_scene_visual_manifest"
                && step["waiting_reason"] == "scene_visual_manifest"
        }));
    assert!(external_wait["allowed_commands"]
        .as_array()
        .unwrap()
        .iter()
        .any(|command| command["kind"] == "resume"
            && command["step_key"] == "wait_scene_visual_manifest"));
}

#[tokio::test]
async fn production_routes_rebuild_from_postgres_and_replay_start_resume_and_retry_once() {
    let (_admin, pool, _guard, test_url) = database().await;
    insert_enabled_text_model(&pool).await;
    let first_app = app(test_url.clone(), pool.clone());
    let (project_id, topic_id) = source(&pool, "restart").await;
    let (status, created) =
        create_intent_request(&first_app, project_id, topic_id, "restart-create").await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let intent_id = Uuid::parse_str(created["intent"]["id"].as_str().unwrap()).unwrap();
    let (status, started) = request(
        &first_app,
        "POST",
        &format!("/api/v1/production/intents/{intent_id}/runs"),
        Some("restart-start"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{started}");
    let run_id = Uuid::parse_str(started["run"]["id"].as_str().unwrap()).unwrap();
    let (status, resumed) = request(
        &first_app,
        "POST",
        &format!("/api/v1/production/runs/{run_id}/resume"),
        Some("restart-resume"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{resumed}");

    drop(first_app);
    let restarted_app = app(test_url, pool.clone());
    let (status, queried) = request(
        &restarted_app,
        "GET",
        &format!("/api/v1/production/runs/{run_id}"),
        None,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{queried}");
    assert_eq!(queried["run"]["id"], run_id.to_string());

    sqlx::query("UPDATE ai_models SET status='disabled' WHERE model_type='text'")
        .execute(&pool)
        .await
        .unwrap();

    let (status, replayed_start) = request(
        &restarted_app,
        "POST",
        &format!("/api/v1/production/intents/{intent_id}/runs"),
        Some("restart-start"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{replayed_start}");
    assert_eq!(replayed_start["run"]["id"], run_id.to_string());
    let (status, replayed_resume) = request(
        &restarted_app,
        "POST",
        &format!("/api/v1/production/runs/{run_id}/resume"),
        Some("restart-resume"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{replayed_resume}");
    assert_eq!(replayed_resume, resumed);

    let step_ids = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id,step_key FROM production_steps WHERE run_id=$1 AND revision_epoch=0",
    )
    .bind(run_id)
    .fetch_all(&pool)
    .await
    .unwrap()
    .into_iter()
    .map(|(id, key)| (key, id))
    .collect::<std::collections::BTreeMap<_, _>>();
    for key in ["validate_source", "producer", "brief_approval"] {
        sqlx::query(
            "UPDATE production_steps SET status='succeeded',attempt=1,side_effect_state='confirmed' WHERE id=$1",
        )
        .bind(step_ids[key])
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "UPDATE production_steps SET status='failed',retryable=TRUE,error_code='fixture_retryable',side_effect_state='none' WHERE id=$1",
    )
    .bind(step_ids["screenwriter"])
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE production_runs SET status='blocked' WHERE id=$1")
        .bind(run_id)
        .execute(&pool)
        .await
        .unwrap();
    let retry_uri = format!(
        "/api/v1/production/runs/{run_id}/steps/{}/retry",
        step_ids["screenwriter"]
    );
    let (status, retried) = request(
        &restarted_app,
        "POST",
        &retry_uri,
        Some("restart-retry"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{retried}");
    let (status, replayed_retry) = request(
        &restarted_app,
        "POST",
        &retry_uri,
        Some("restart-retry"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{replayed_retry}");
    assert_eq!(replayed_retry, retried);

    for command_type in ["start_run", "resume_run", "retry_step"] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM production_commands WHERE command_type=$1",
            )
            .bind(command_type)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1,
            "{command_type} must have one durable command fact"
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_steps WHERE run_id=$1 AND attempt > 1",
        )
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM agent_runs WHERE agent_type='production'"
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
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM scripts WHERE production_run_id=$1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_generation_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn production_api_uses_stable_actor_and_scopes_idempotency_by_command_and_aggregate() {
    let (_admin, pool, _guard, test_url) = database().await;
    insert_enabled_text_model(&pool).await;
    let app = app(test_url, pool.clone());
    let (project_a, topic_a) = source(&pool, "idempotency-a").await;
    let (project_b, topic_b) = source(&pool, "idempotency-b").await;

    let (status, rejected_actor) = request(
        &app,
        "POST",
        "/api/v1/production/intents",
        Some("client-actor"),
        json!({
            "project_id": project_a,
            "topic_id": topic_a,
            "title": "客户端伪造 actor",
            "description": null,
            "initial_input": {},
            "user_id": Uuid::new_v4()
        }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{rejected_actor}");

    let (status, created_a) = create_intent_request(&app, project_a, topic_a, "shared-key").await;
    assert_eq!(status, StatusCode::CREATED, "{created_a}");
    let intent_a = Uuid::parse_str(created_a["intent"]["id"].as_str().unwrap()).unwrap();
    let (status, replayed_a) = create_intent_request(&app, project_a, topic_a, "shared-key").await;
    assert_eq!(status, StatusCode::CREATED, "{replayed_a}");
    assert_eq!(replayed_a["intent"]["id"], intent_a.to_string());

    let (status, conflict) = request(
        &app,
        "POST",
        "/api/v1/production/intents",
        Some("shared-key"),
        json!({
            "project_id": project_a,
            "topic_id": topic_a,
            "title": "相同 key 但不同 payload",
            "description": "必须冲突",
            "initial_input": {"brief": "不同摘要"}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{conflict}");
    assert_eq!(conflict["error"], "idempotency_conflict");

    let (status, created_b) = create_intent_request(&app, project_b, topic_b, "shared-key").await;
    assert_eq!(status, StatusCode::CREATED, "{created_b}");
    let intent_b = Uuid::parse_str(created_b["intent"]["id"].as_str().unwrap()).unwrap();
    assert_ne!(intent_a, intent_b);
    for intent_id in [intent_a, intent_b] {
        let (status, started) = request(
            &app,
            "POST",
            &format!("/api/v1/production/intents/{intent_id}/runs"),
            Some("shared-key"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED, "{started}");
        let run_id = Uuid::parse_str(started["run"]["id"].as_str().unwrap()).unwrap();
        let (status, resumed) = request(
            &app,
            "POST",
            &format!("/api/v1/production/runs/{run_id}/resume"),
            Some("shared-key"),
            json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED, "{resumed}");
    }

    let command_actors = sqlx::query_as::<_, (String, String)>(
        "SELECT DISTINCT actor_type,actor_id FROM production_commands",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        command_actors,
        vec![("local_operator".into(), "local_operator".into())]
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_commands WHERE idempotency_key='shared-key'",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        6
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM production_runs
            WHERE actor_type='local_operator' AND actor_id='local_operator'
            "#,
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );
}

#[tokio::test]
async fn production_cancel_delete_and_archive_preserve_auditable_history() {
    let (_admin, pool, _guard, test_url) = database().await;
    insert_enabled_text_model(&pool).await;
    let app = app(test_url, pool.clone());

    let (blank_project, blank_topic) = source(&pool, "blank-delete").await;
    let (status, blank) =
        create_intent_request(&app, blank_project, blank_topic, "blank-create").await;
    assert_eq!(status, StatusCode::CREATED, "{blank}");
    let blank_intent = Uuid::parse_str(blank["intent"]["id"].as_str().unwrap()).unwrap();
    let delete_uri = format!("/api/v1/production/intents/{blank_intent}");
    for _ in 0..2 {
        let (status, body) =
            request(&app, "DELETE", &delete_uri, Some("blank-delete"), json!({})).await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_projects WHERE id=$1")
            .bind(blank_intent)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_commands WHERE aggregate_id=$1 AND command_type='delete_intent'",
        )
        .bind(blank_intent)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );

    let (project_id, topic_id) = source(&pool, "archive-history").await;
    let (intent_id, run_id) = create_and_start(&app, project_id, topic_id, "archive-history").await;
    let (status, rejected_delete) = request(
        &app,
        "DELETE",
        &format!("/api/v1/production/intents/{intent_id}"),
        Some("history-delete"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{rejected_delete}");
    assert_eq!(rejected_delete["error"], "transition_conflict");

    let archive_uri = format!("/api/v1/production/intents/{intent_id}/archive");
    let (status, active_archive) = request(
        &app,
        "POST",
        &archive_uri,
        Some("active-archive"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{active_archive}");

    let (status, cancelled) = request(
        &app,
        "POST",
        &format!("/api/v1/production/runs/{run_id}/cancel"),
        Some("cancel-waiting-run"),
        json!({"reason": "操作者终止尚未产生外部副作用的流程"}),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{cancelled}");
    assert_eq!(cancelled["run"]["status"], "cancelled");

    let (status, archived) = request(
        &app,
        "POST",
        &archive_uri,
        Some("archive-terminal"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{archived}");
    assert_eq!(archived["intent"]["status"], "archived");
    let (status, intent_view) = request(
        &app,
        "GET",
        &format!("/api/v1/production/intents/{intent_id}"),
        None,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{intent_view}");
    assert_eq!(intent_view["intent"]["status"], "archived");
    let (status, run_view) = request(
        &app,
        "GET",
        &format!("/api/v1/production/runs/{run_id}"),
        None,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{run_view}");
    assert_eq!(run_view["run"]["status"], "cancelled");
    assert!(!run_view["steps"].as_array().unwrap().is_empty());
    let (status, legacy_delete) = request(
        &app,
        "DELETE",
        &format!("/api/v1/production/productions/{intent_id}"),
        None,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{legacy_delete}");
    assert_eq!(legacy_delete["error"], "durable_intent_lifecycle_required");

    let (uncertain_project, uncertain_topic) = source(&pool, "uncertain-cancel").await;
    let (uncertain_intent, uncertain_run) =
        create_and_start(&app, uncertain_project, uncertain_topic, "uncertain-cancel").await;
    sqlx::query(
        "UPDATE production_steps SET status='running',attempt=1,side_effect_state='unknown' WHERE run_id=$1 AND step_key='producer'",
    )
    .bind(uncertain_run)
    .execute(&pool)
    .await
    .unwrap();
    let (status, uncertain) = request(
        &app,
        "POST",
        &format!("/api/v1/production/runs/{uncertain_run}/cancel"),
        Some("cancel-uncertain"),
        json!({"reason": "上游提交结果未知，必须人工确认"}),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{uncertain}");
    assert_eq!(uncertain["run"]["status"], "attention_required");
    assert_ne!(uncertain["run"]["status"], "cancelled");
    let (status, _) = request(
        &app,
        "POST",
        &format!("/api/v1/production/intents/{uncertain_intent}/archive"),
        Some("archive-uncertain"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}
