use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use novex_agent::{AuditedCallOwner, PersistContextSnapshot};
use novex_ai_core::{
    canonical_json, sha256_hex, ContextCompileRequest, ContextCompiler, ContextPayload,
    LogicalModelInput,
};
use novex_api::{
    bootstrap::{AppConfig, AppState},
    build_app_with_state,
    repositories::PostgresContextAuditRepository,
};
use serde::Deserialize;
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, PgPool};
use tower::ServiceExt;
use uuid::Uuid;

mod support;
use support::test_database::TestDatabase;

#[derive(Deserialize)]
struct CompileFixture {
    request: ContextCompileRequest,
    final_logical_input: LogicalModelInput,
}

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@biga-postgres:5432/video_agent".into())
}

fn with_database_name(url: &str, name: &str) -> String {
    let query_start = url.find('?');
    let (base, query) = query_start
        .map(|index| (&url[..index], &url[index..]))
        .unwrap_or((url, ""));
    let slash = base.rfind('/').unwrap();
    format!("{}{}{}", &base[..=slash], name, query)
}

async fn migrated_pool() -> (PgPool, PgPool, TestDatabase, String) {
    let base = database_url();
    let name = format!("video_agent_context_api_{}", Uuid::new_v4().simple());
    let admin_url = with_database_name(&base, "postgres");
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .unwrap();
    sqlx::query(&format!(r#"CREATE DATABASE "{name}""#))
        .execute(&admin)
        .await
        .unwrap();
    let test_url = with_database_name(&base, &name);
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&test_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (admin, pool, TestDatabase::new(&admin_url, &name), test_url)
}

async fn response(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    (status, body)
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

fn fields(contract: &Value, key: &str) -> Vec<String> {
    let mut values = contract[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().into())
        .collect::<Vec<_>>();
    values.sort();
    values
}

#[tokio::test]
async fn rust_context_read_api_matches_shared_contract() {
    let (admin, pool, database, test_url) = migrated_pool().await;
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO agent_runs (agent_type,status) VALUES ('script','running') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let fixture: CompileFixture = serde_json::from_str(include_str!(
        "../../agent-definitions/fixtures/context-compile-contract-v1.json"
    ))
    .unwrap();
    let mut request = fixture.request.clone();
    request.owner_id = run_id.to_string();
    request.node_key = "personal.turn".into();
    let registry = novex_ai_core::DefinitionRegistry::load("/app/agent-definitions").unwrap();
    request.policy = registry
        .context_policy("script.complete.baseline", "1.0.0")
        .unwrap()
        .clone();
    request.tokenizer_profile = registry
        .tokenizer_profile("openai.o200k", "1.0.0")
        .unwrap()
        .clone();
    request.model_context_window = 128_000;
    for (index, candidate) in request.candidates.iter_mut().enumerate() {
        candidate.source_kind = if index == 0 {
            "user_instruction".into()
        } else {
            "pi_branch_entry".into()
        };
    }
    let compiled = ContextCompiler::compile(request.clone()).unwrap();
    let snapshot = ContextCompiler::finalize(
        &compiled,
        &request.tokenizer_profile,
        fixture.final_logical_input.clone(),
    )
    .unwrap();
    let stored = PostgresContextAuditRepository::new(pool.clone())
        .persist_snapshot_record(PersistContextSnapshot {
            owner: AuditedCallOwner::AgentRun(run_id),
            snapshot,
            known_secrets: vec![],
        })
        .await
        .unwrap();
    let model_id: Uuid = sqlx::query_scalar(r#"
        INSERT INTO ai_models (display_name,model_type,provider_name,api_protocol,auth_scheme,
          request_base_url,upstream_model,api_key,max_output_tokens,context_window,
          tokenizer_profile_key,tokenizer_profile_version,settings)
        VALUES ('context replay','text','test','openai_responses','bearer','https://example.invalid/v1',
          'fixture','secret',4096,128000,'openai.o200k','1.0.0','{}') RETURNING id
    "#).fetch_one(&pool).await.unwrap();
    let prompt_snapshot = serde_json::json!({
        "schema_version":"2", "context_snapshot_id":stored.id,
        "context_digest":stored.snapshot.digest, "logical_input":stored.snapshot.logical_input,
    });
    let call_id: Uuid = sqlx::query_scalar(r#"
        INSERT INTO model_calls (schema_version,agent_run_id,node_key,attempt,status,agent_key,agent_version,
          prompt_key,prompt_version,registry_digest,prompt_snapshot,context_sources,memory_sources,
          model_id,behavior_fingerprint,model_snapshot,parameters,asset_references,
          context_snapshot_id,context_digest,context_policy_key,context_policy_version,
          tokenizer_profile_key,tokenizer_profile_version,context_budget_summary,completed_at)
        VALUES ('2',$1,'personal.turn',1,'succeeded','personal.general','2.0.0','personal.general','2.0.0',
          $2,$3,'[]','[]',$4,$5,'{}','{}','[]',$6,$7,$8,$9,$10,$11,$12,NOW()) RETURNING id
    "#)
        .bind(run_id).bind(registry.digest()).bind(prompt_snapshot).bind(model_id)
        .bind("f".repeat(64)).bind(stored.id).bind(&stored.snapshot.digest)
        .bind(&stored.snapshot.policy_key).bind(&stored.snapshot.policy_version)
        .bind(&stored.snapshot.tokenizer_profile_key).bind(&stored.snapshot.tokenizer_profile_version)
        .bind(serde_json::to_value(&stored.snapshot.budget).unwrap())
        .fetch_one(&pool).await.unwrap();
    let mut config = AppConfig::from_env();
    config.database_url = test_url;
    let app = build_app_with_state(AppState::new(config, pool.clone(), None).unwrap());
    let contract: Value = serde_json::from_str(include_str!(
        "../../agent-definitions/fixtures/context-audit-read-api.json"
    ))
    .unwrap();

    let (status, list) = response(&app, &format!(
        "/contexts?owner_type=agent_run&owner_id={run_id}&record_type=snapshot&node_key=personal.turn&limit=20&offset=0"
    )).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        sorted_keys(&list),
        fields(&contract, "list_envelope_fields")
    );
    assert_eq!(
        sorted_keys(&list["items"][0]),
        fields(&contract, "summary_fields")
    );
    assert!(list["items"][0].get("decisions").is_none());

    let (status, detail) = response(&app, &format!("/contexts/{}", stored.id)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        sorted_keys(&detail),
        fields(&contract, "detail_envelope_fields")
    );
    assert_eq!(
        sorted_keys(&detail["record"]),
        fields(&contract, "snapshot_record_fields")
    );
    assert_eq!(detail["record_hash"].as_str().unwrap().len(), 64);
    let (_, exported) = response(&app, &format!("/contexts/{}/export", stored.id)).await;
    assert_eq!(exported, detail);
    let replay_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/model-calls/{call_id}/replay"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"mode":"dry_run"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replay_response.status(), StatusCode::OK);
    let replay: Value = serde_json::from_slice(
        &to_bytes(replay_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        replay["validation_order"],
        serde_json::json!(["context", "prompt", "model_call"])
    );
    assert_eq!(replay["compile_succeeded"], true);
    assert_eq!(replay["diff"], serde_json::json!([]));
    assert_eq!(
        replay["side_effects"],
        serde_json::json!({
            "model_calls":0,"tools":0,"session_writes":0,"run_writes":0,"domain_writes":0
        })
    );

    let canary = "NOVEX_CANARY_SECRET_DO_NOT_PERSIST_context_export";
    let mut safety_request = request;
    safety_request.candidates[0].payload = ContextPayload::Text {
        text: canary.into(),
    };
    let safety_payload = serde_json::to_value(&safety_request.candidates[0].payload).unwrap();
    safety_request.candidates[0].content_hash =
        sha256_hex(canonical_json(&safety_payload).as_bytes());
    let safety_compiled = ContextCompiler::compile(safety_request.clone()).unwrap();
    let mut safety_input = fixture.final_logical_input;
    let original_content = safety_input.messages[0].content.as_str().unwrap();
    safety_input.messages[0].content =
        Value::String(original_content.replacen("current user instruction", canary, 1));
    let safety_snapshot = ContextCompiler::finalize(
        &safety_compiled,
        &safety_request.tokenizer_profile,
        safety_input,
    )
    .unwrap();
    let safety_record = PostgresContextAuditRepository::new(pool.clone())
        .persist_snapshot_record(PersistContextSnapshot {
            owner: AuditedCallOwner::AgentRun(run_id),
            snapshot: safety_snapshot,
            known_secrets: vec![],
        })
        .await
        .unwrap();
    let (status, safety_export) =
        response(&app, &format!("/contexts/{}/export", safety_record.id)).await;
    assert_eq!(status, StatusCode::OK);
    let safety_json = serde_json::to_string(&safety_export).unwrap();
    assert!(safety_json.contains("[REDACTED]"));
    assert!(!safety_json.contains(canary));

    pool.close().await;
    admin.close().await;
    drop(database);
}
