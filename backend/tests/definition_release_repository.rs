use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use novex_ai_core::{canonical_json, definition_digest, sha256_hex, DefinitionRegistry};
use novex_api::{
    bootstrap::{AppConfig, AppState},
    build_app_with_state,
    repositories::{DefinitionReleaseError, PostgresDefinitionReleaseRepository},
};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::fs;
use std::path::{Path, PathBuf};
use tower::ServiceExt;
use uuid::Uuid;

mod support;

use support::test_database::insert_enabled_text_model;
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
    let slash_index = base.rfind('/').expect("database URL must contain a name");
    format!("{}{}{}", &base[..=slash_index], database_name, query)
}

async fn test_pool() -> (PgPool, TestDatabase) {
    let base_url = database_url();
    let name = format!("video_agent_definition_release_{}", Uuid::new_v4().simple());
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
        .max_connections(2)
        .connect(&with_database_name(&base_url, &name))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (pool, TestDatabase::new(&admin_url, &name))
}

fn registry_fixture(document: &Value, suffix: &str) -> (DefinitionRegistry, PathBuf) {
    let definitions = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("agent-definitions");
    let directory = std::env::temp_dir().join(format!(
        "novex-release-registry-{}-{suffix}",
        Uuid::new_v4().simple()
    ));
    fs::create_dir_all(directory.join("templates")).unwrap();
    fs::write(
        directory.join("registry.json"),
        serde_json::to_vec(document).unwrap(),
    )
    .unwrap();
    for prompt in document["prompts"].as_array().unwrap() {
        for field in ["system_template", "user_template"] {
            let relative = prompt[field].as_str().unwrap();
            fs::copy(definitions.join(relative), directory.join(relative)).unwrap();
        }
    }
    let releases = ["agents", "prompts"]
        .into_iter()
        .flat_map(|collection| {
            document[collection]
                .as_array()
                .unwrap()
                .iter()
                .map(move |definition| {
                    let kind = if collection == "agents" {
                        "agent"
                    } else {
                        "prompt"
                    };
                    let key_field = if collection == "agents" {
                        "agent_key"
                    } else {
                        "prompt_key"
                    };
                    json!({
                        "definition_kind": kind,
                        "definition_key": definition[key_field],
                        "definition_version": definition["version"],
                        "definition_digest": definition_digest(definition).unwrap(),
                        "activation_evidence": {
                            "type": "golden_baseline",
                            "reference": "fixture-v1-golden",
                            "sha256": "0".repeat(64)
                        }
                    })
                })
        })
        .collect::<Vec<_>>();
    fs::write(
        directory.join("release-index.json"),
        serde_json::to_vec(&json!({
            "schema_version": "1",
            "registry_digest": sha256_hex(canonical_json(document).as_bytes()),
            "releases": releases
        }))
        .unwrap(),
    )
    .unwrap();
    (DefinitionRegistry::load(&directory).unwrap(), directory)
}

async fn passing_report(pool: &PgPool, key: &str, version: &str, digest: &str) -> Uuid {
    let run_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO eval_runs (
            candidate_key, candidate_version, candidate_digest, case_set_version,
            evaluator_version, validation_mode, approved_real_calls, approval_snapshot,
            max_cases, max_input_tokens, max_output_tokens, max_retries, max_cost_micros,
            status, actual_cases
        ) VALUES ($1, $2, $3, 'fixture@1', 'fixture@1', 'golden_baseline', FALSE,
                  '{"schema_version":"1"}', 1, 0, 0, 0, 0, 'passed', 1)
        RETURNING id
        "#,
    )
    .bind(key)
    .bind(version)
    .bind(digest)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        r#"
        INSERT INTO eval_reports (
            eval_run_id, passed, gate_results, aggregate_metrics, redacted_case_results
        ) VALUES ($1, TRUE, '{}', '{}', '[]')
        RETURNING id
        "#,
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn release_repository_is_idempotent_and_rejects_mutation_or_digest_conflict() {
    let (pool, _database) = test_pool().await;
    let fixture: Value = serde_json::from_str(include_str!(
        "../../agent-definitions/fixtures/registry-valid.json"
    ))
    .unwrap();
    let (registry, first_dir) = registry_fixture(&fixture, "first");
    let repository = PostgresDefinitionReleaseRepository::new(pool.clone());

    repository.publish_registry(&registry).await.unwrap();
    repository.publish_registry(&registry).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM definition_releases")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2, "idempotent publish must not duplicate evidence");

    let update = sqlx::query("UPDATE definition_releases SET initial_status = 'supported'")
        .execute(&pool)
        .await;
    assert!(update.is_err(), "published evidence must reject updates");
    let delete = sqlx::query("DELETE FROM definition_releases")
        .execute(&pool)
        .await;
    assert!(delete.is_err(), "published evidence must reject deletes");

    let mut modified = fixture;
    modified["agents"][0]["role"] = json!("changed in place");
    let (conflicting_registry, second_dir) = registry_fixture(&modified, "second");
    let error = repository
        .publish_registry(&conflicting_registry)
        .await
        .unwrap_err();
    assert!(matches!(error, DefinitionReleaseError::Conflict(_)));

    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns WHERE table_schema='public' AND table_name='definition_releases'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(columns.iter().all(|column| !column.contains("template")));

    fs::remove_dir_all(first_dir).unwrap();
    fs::remove_dir_all(second_dir).unwrap();
    pool.close().await;
}

#[tokio::test]
async fn active_release_requires_exact_golden_or_passing_eval_report_evidence() {
    let (pool, _database) = test_pool().await;
    let fixture: Value = serde_json::from_str(include_str!(
        "../../agent-definitions/fixtures/registry-valid.json"
    ))
    .unwrap();
    let (_registry, directory) = registry_fixture(&fixture, "activation-evidence");
    let index_path = directory.join("release-index.json");
    let mut index: Value = serde_json::from_slice(&fs::read(&index_path).unwrap()).unwrap();
    index["releases"] = json!([]);
    fs::write(&index_path, serde_json::to_vec(&index).unwrap()).unwrap();
    let missing = DefinitionRegistry::load(&directory).unwrap();
    assert!(matches!(
        PostgresDefinitionReleaseRepository::new(pool.clone())
            .publish_registry(&missing)
            .await,
        Err(DefinitionReleaseError::ActivationEvidence(_))
    ));

    let agent = &fixture["agents"][0];
    let prompt = &fixture["prompts"][0];
    let agent_digest = definition_digest(agent).unwrap();
    let prompt_digest = definition_digest(prompt).unwrap();
    let agent_report = passing_report(
        &pool,
        agent["agent_key"].as_str().unwrap(),
        agent["version"].as_str().unwrap(),
        &agent_digest,
    )
    .await;
    index["releases"] = json!([
        {
            "definition_kind": "agent",
            "definition_key": agent["agent_key"],
            "definition_version": agent["version"],
            "definition_digest": agent_digest,
            "activation_evidence": {"type": "eval_report", "report_id": agent_report}
        },
        {
            "definition_kind": "prompt",
            "definition_key": prompt["prompt_key"],
            "definition_version": prompt["version"],
            "definition_digest": prompt_digest,
            "activation_evidence": {"type": "eval_report", "report_id": agent_report}
        }
    ]);
    fs::write(&index_path, serde_json::to_vec(&index).unwrap()).unwrap();
    let mismatched = DefinitionRegistry::load(&directory).unwrap();
    assert!(matches!(
        PostgresDefinitionReleaseRepository::new(pool.clone())
            .publish_registry(&mismatched)
            .await,
        Err(DefinitionReleaseError::ActivationEvidence(_))
    ));

    let prompt_report = passing_report(
        &pool,
        prompt["prompt_key"].as_str().unwrap(),
        prompt["version"].as_str().unwrap(),
        &prompt_digest,
    )
    .await;
    index["releases"][1]["activation_evidence"]["report_id"] = json!(prompt_report);
    fs::write(&index_path, serde_json::to_vec(&index).unwrap()).unwrap();
    let authorized = DefinitionRegistry::load(&directory).unwrap();
    PostgresDefinitionReleaseRepository::new(pool.clone())
        .publish_registry(&authorized)
        .await
        .unwrap();
    fs::remove_dir_all(directory).unwrap();
    pool.close().await;
}

#[tokio::test]
async fn api_has_no_online_registry_activation_route() {
    let (pool, _database) = test_pool().await;
    let mut config = AppConfig::from_env();
    config.database_url = database_url();
    let app = build_app_with_state(AppState::new(config, pool.clone(), None).unwrap());
    for method in [Method::POST, Method::PUT, Method::PATCH] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri("/api/agent-definitions/video.script/active")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    pool.close().await;
}

#[tokio::test]
async fn lifecycle_manifests_and_code_rollback_are_immutable_and_preserve_history() {
    let (pool, _database) = test_pool().await;
    let fixture: Value = serde_json::from_str(include_str!(
        "../../agent-definitions/fixtures/registry-valid.json"
    ))
    .unwrap();
    let mut registries = Vec::new();
    let mut directories = Vec::new();
    for status in ["candidate", "active", "supported", "revoked"] {
        let mut document = fixture.clone();
        document["agents"][0]["status"] = json!(status);
        document["prompts"][0]["status"] = json!(status);
        let (registry, directory) = registry_fixture(&document, status);
        registries.push(registry);
        directories.push(directory);
    }
    let repository = PostgresDefinitionReleaseRepository::new(pool.clone());

    repository.publish_registry(&registries[0]).await.unwrap();
    repository.publish_registry(&registries[1]).await.unwrap();
    repository.publish_registry(&registries[2]).await.unwrap();

    let conversation_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO agent_conversations (agent_type, title) VALUES ('script', 'rollback fixture') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO agent_conversation_bindings (
            conversation_id, agent_key, agent_version, agent_digest,
            prompt_bindings, registry_digest, binding_status
        ) VALUES ($1, 'fixture.agent', '1.0.0', $2, $3, $4, 'definition_bound')
        "#,
    )
    .bind(conversation_id)
    .bind(definition_digest(&fixture["agents"][0]).unwrap())
    .bind(json!({"fixture.node":{"key":"fixture.prompt","version":"1.0.0"}}))
    .bind(registries[1].digest())
    .execute(&pool)
    .await
    .unwrap();
    let model_id = insert_enabled_text_model(&pool).await;
    let run_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO agent_runs (agent_type, status) VALUES ('script', 'succeeded') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let model_call_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO model_calls (
            agent_run_id, node_key, attempt, agent_key, agent_version,
            prompt_key, prompt_version, registry_digest, prompt_snapshot,
            model_id, behavior_fingerprint, model_snapshot
        ) VALUES (
            $1, 'fixture.node', 1, 'fixture.agent', '1.0.0',
            'fixture.prompt', '1.0.0', $2, '{"schema_version":"1"}',
            $3, $4, '{"provider":"fixture"}'
        ) RETURNING id
        "#,
    )
    .bind(run_id)
    .bind(registries[1].digest())
    .bind(model_id)
    .bind("f".repeat(64))
    .fetch_one(&pool)
    .await
    .unwrap();
    let eval_report_id = passing_report(
        &pool,
        "fixture.agent",
        "1.0.0",
        &definition_digest(&fixture["agents"][0]).unwrap(),
    )
    .await;

    // 回滚只重新发布既有 active manifest；不得改写 supported 快照或清理前向兼容数据。
    repository.publish_registry(&registries[1]).await.unwrap();
    repository.publish_registry(&registries[3]).await.unwrap();

    let manifest_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM definition_release_manifests")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(manifest_count, 4);
    let entry_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM definition_release_manifest_entries")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(entry_count, 8);
    let statuses: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT lifecycle_status FROM definition_release_manifest_entries ORDER BY lifecycle_status",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(statuses, ["active", "candidate", "revoked", "supported"]);
    let release_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM definition_releases")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        release_count, 2,
        "lifecycle changes must not rewrite content releases"
    );

    for (table, id) in [
        ("agent_conversation_bindings", conversation_id),
        ("model_calls", model_call_id),
        ("eval_reports", eval_report_id),
    ] {
        let exists: bool = sqlx::query_scalar(&format!(
            "SELECT EXISTS (SELECT 1 FROM {table} WHERE {} = $1)",
            if table == "agent_conversation_bindings" {
                "conversation_id"
            } else {
                "id"
            }
        ))
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            exists,
            "code manifest rollback/revoke must preserve {table}"
        );
    }

    let update =
        sqlx::query("UPDATE definition_release_manifest_entries SET lifecycle_status = 'active'")
            .execute(&pool)
            .await;
    assert!(
        update.is_err(),
        "manifest lifecycle snapshots must reject updates"
    );
    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns WHERE table_schema='public' AND table_name IN ('definition_release_manifests','definition_release_manifest_entries')",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(columns.iter().all(|column| !column.contains("template")));

    for directory in directories {
        fs::remove_dir_all(directory).unwrap();
    }
    pool.close().await;
}

#[test]
fn versioned_execution_migrations_have_no_destructive_rollback_path() {
    let migrations = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let source = fs::read_dir(migrations)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().as_ref() >= "20260724010000")
        .map(|entry| fs::read_to_string(entry.path()).unwrap())
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_uppercase();
    for table in [
        "DEFINITION_RELEASES",
        "DEFINITION_RELEASE_MANIFESTS",
        "AGENT_CONVERSATION_BINDINGS",
        "AGENT_RUN_BINDINGS",
        "MODEL_CALLS",
        "EVAL_RUNS",
        "EVAL_REPORTS",
    ] {
        assert!(
            !source.contains(&format!("DROP TABLE {table}")),
            "forward migrations must not contain destructive rollback for {table}"
        );
    }
}
