use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use novex_api::{
    bootstrap::{AppConfig, AppState},
    build_app_with_state,
};
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
    let (base, query) = query_start
        .map(|index| (&database_url[..index], &database_url[index..]))
        .unwrap_or((database_url, ""));
    let slash = base
        .rfind('/')
        .expect("DATABASE_URL must include a database");
    format!("{}{}{}", &base[..=slash], database_name, query)
}

async fn migrated_pool() -> (PgPool, PgPool, TestDatabase, String) {
    let base_url = database_url();
    let name = format!("video_agent_model_call_api_{}", Uuid::new_v4().simple());
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
    let test_url = with_database_name(&base_url, &name);
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&test_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (admin, pool, TestDatabase::new(&admin_url, &name), test_url)
}

fn app_state(database_url: String, pool: PgPool) -> AppState {
    let mut config = AppConfig::from_env();
    config.database_url = database_url;
    AppState::new(config, pool, None).unwrap()
}

async fn seed_call(pool: &PgPool, drift_historical_output: bool) -> (Uuid, Uuid, Uuid) {
    let model_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO ai_models (
            display_name, model_type, provider_name, api_protocol, auth_scheme,
            request_base_url, upstream_model, api_key, max_output_tokens, settings
        ) VALUES ('audit api', 'text', 'test', 'openai_responses', 'bearer',
                  'https://example.invalid/v1', 'audit-1', 'secret', 4096,
                  '{"context_window":128000}') RETURNING id
        "#,
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO agent_runs (agent_type, status) VALUES ('script', 'running') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let registry = novex_ai_core::DefinitionRegistry::load("/app/agent-definitions").unwrap();
    let snapshot = novex_ai_core::PromptCompiler::new(&registry)
        .compile(
            "video.script",
            "1.0.0",
            "script.complete",
            novex_ai_core::PromptCompileInput {
                schema_version: "1".into(),
                variables: std::collections::BTreeMap::from([("scene_count".into(), json!(3))]),
                fragments: vec![novex_ai_core::DynamicFragment {
                    id: "request-1".into(),
                    trust: novex_ai_core::TrustLevel::UserInstruction,
                    source: "model_call_api_test".into(),
                    content: Some("generate a script".into()),
                    asset: None,
                }],
            },
            "chat",
            None,
        )
        .unwrap();
    let mut snapshot = serde_json::to_value(snapshot).unwrap();
    if drift_historical_output {
        snapshot["user"] = json!("historical output that no longer matches its compile input");
    }
    let call_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO model_calls (
            agent_run_id, node_key, attempt, status, agent_key, agent_version,
            prompt_key, prompt_version, registry_digest, prompt_snapshot,
            context_sources, memory_sources, model_id, behavior_fingerprint,
            model_snapshot, parameters, asset_references, prepared_at, completed_at
        ) VALUES ($1, 'script.complete', 1, 'succeeded', 'video.script', '1.0.0',
                  'script.complete', '1.0.0', $2, $3, '[]', '[]', $4, $5,
                  '{"provider":"test"}', '{"temperature":0}', '[]', NOW(), NOW())
        RETURNING id
        "#,
    )
    .bind(run_id)
    .bind(registry.digest())
    .bind(snapshot)
    .bind(model_id)
    .bind("b".repeat(64))
    .fetch_one(pool)
    .await
    .unwrap();
    (call_id, run_id, model_id)
}

fn contract() -> Value {
    serde_json::from_str(include_str!(
        "../../agent-definitions/fixtures/model-call-read-api.json"
    ))
    .unwrap()
}

fn sorted_keys(value: &Value) -> Vec<String> {
    let mut keys = value
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

fn contract_fields(contract: &Value, key: &str) -> Vec<String> {
    let mut fields = contract[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    fields.sort();
    fields
}

async fn json_response(
    app: &axum::Router,
    uri: &str,
    method: &str,
    body: Value,
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, body)
}

#[tokio::test]
async fn rust_model_call_read_api_matches_shared_contract_and_filters() {
    let (admin, pool, database, test_url) = migrated_pool().await;
    let (call_id, run_id, model_id) = seed_call(&pool, false).await;
    let app = build_app_with_state(app_state(test_url, pool.clone()));
    let fixture = contract();
    let prepared_at: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT prepared_at FROM model_calls WHERE id=$1")
            .bind(call_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let prepared_from = prepared_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let prepared_to = (prepared_at + chrono::Duration::minutes(1))
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let (status, list) = json_response(
        &app,
        &format!(
            "/model-calls?owner_type=agent_run&owner_id={run_id}&node_key=script.complete&agent_version=1.0.0&model_id={model_id}&status=succeeded&prepared_from={prepared_from}&prepared_to={prepared_to}&limit=20&offset=0"
        ),
        "GET",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        sorted_keys(&list),
        contract_fields(&fixture, "list_envelope_fields")
    );
    assert_eq!(list["source_runtime"], "rust");
    assert_eq!(list["total"], 1);
    assert_eq!(
        sorted_keys(&list["items"][0]),
        contract_fields(&fixture, "summary_fields")
    );
    assert_eq!(
        sorted_keys(&list["items"][0]["owner"]),
        contract_fields(&fixture, "owner_fields")
    );
    assert_eq!(
        sorted_keys(&list["items"][0]["execution"]),
        contract_fields(&fixture, "execution_fields")
    );
    assert_eq!(
        sorted_keys(&list["items"][0]["definition"]),
        contract_fields(&fixture, "definition_fields")
    );
    assert_eq!(
        sorted_keys(&list["items"][0]["model"]),
        contract_fields(&fixture, "summary_model_fields")
    );
    assert_eq!(
        sorted_keys(&list["items"][0]["usage"]),
        contract_fields(&fixture, "usage_fields")
    );
    assert_eq!(list["items"][0]["usage"], fixture["default_usage"]);

    let (status, detail) =
        json_response(&app, &format!("/model-calls/{call_id}"), "GET", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        sorted_keys(&detail),
        contract_fields(&fixture, "detail_envelope_fields")
    );
    assert_eq!(
        sorted_keys(&detail["record"]),
        contract_fields(&fixture, "record_fields")
    );
    assert_eq!(
        sorted_keys(&detail["record"]["model"]),
        contract_fields(&fixture, "detail_model_fields")
    );
    assert_eq!(detail["record_hash"].as_str().unwrap().len(), 64);

    let (status, exported) = json_response(
        &app,
        &format!("/model-calls/{call_id}/export"),
        "GET",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(exported, detail);

    pool.close().await;
    admin.close().await;
    drop(database);
}

#[tokio::test]
async fn rust_dry_run_replay_recompiles_without_model_tool_run_or_domain_writes() {
    let (admin, pool, database, test_url) = migrated_pool().await;
    let (call_id, _, _) = seed_call(&pool, true).await;
    let app = build_app_with_state(app_state(test_url, pool.clone()));
    let before: (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM model_calls), (SELECT COUNT(*) FROM agent_runs), (SELECT COUNT(*) FROM agent_steps), (SELECT COUNT(*) FROM scripts)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let (status, replay) = json_response(
        &app,
        &format!("/model-calls/{call_id}/replay"),
        "POST",
        json!({"mode":"dry_run"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        sorted_keys(&replay),
        contract_fields(&contract(), "replay_fields")
    );
    assert_eq!(replay["definition_resolved"], true);
    assert_eq!(replay["compile_succeeded"], true);
    assert!(replay["diff"].as_array().is_some_and(|diff| diff
        .iter()
        .any(|item| { item["path"] == "user" && item["kind"] == "changed" })));
    assert_eq!(
        replay["side_effects"],
        json!({"model_calls":0,"tools":0,"session_writes":0,"run_writes":0,"domain_writes":0})
    );
    let after: (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM model_calls), (SELECT COUNT(*) FROM agent_runs), (SELECT COUNT(*) FROM agent_steps), (SELECT COUNT(*) FROM scripts)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after, before);

    let (status, rejected) = json_response(
        &app,
        &format!("/model-calls/{call_id}/replay"),
        "POST",
        json!({"mode":"real"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(rejected["error"]["code"], "bad_request");

    pool.close().await;
    admin.close().await;
    drop(database);
}
