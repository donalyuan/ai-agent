use async_trait::async_trait;
use novex_agent::{
    text_context_candidate, BoundModelResolver, FixedModelBinding, ResolvedBoundModel,
    TextContextCandidateInput,
};
use novex_ai_core::{
    definition_digest, ContextPriority, DefinitionRegistry, ModelCapabilities, TrustLevel,
};
use novex_api::application::evaluations::{EvalBudgetCharge, RealEvalRunner};
use novex_api::repositories::{
    PostgresContextAuditRepository, PostgresEvalRepository, PostgresModelCallRepository,
};
use novex_eval::{CandidateRef, EvalBudget, EvalMode, EvalRunSpec, ModelBinding};
use novex_model::{LLMClient, LLMError, LLMPrompt};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use uuid::Uuid;

mod support;

use support::test_database::TestDatabase;

#[derive(Clone)]
struct FakeClient {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl LLMClient for FakeClient {
    async fn generate_script(&self, _prompt: LLMPrompt) -> Result<String, LLMError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(r#"{"title":"fixture","hook":"fixture","scenes":[{"sequence":1,"narration":"fixture","visual_description":"fixture","emotion":"calm","duration_sec":8},{"sequence":2,"narration":"fixture","visual_description":"fixture","emotion":"calm","duration_sec":8},{"sequence":3,"narration":"fixture","visual_description":"fixture","emotion":"calm","duration_sec":8}]}"#.into())
    }
}

struct StaticResolver {
    resolved: ResolvedBoundModel,
}

#[async_trait]
impl BoundModelResolver for StaticResolver {
    async fn resolve(&self, _model_id: Uuid) -> Result<ResolvedBoundModel, novex_agent::BoxError> {
        Ok(self.resolved.clone())
    }
}

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@biga-postgres:5432/video_agent".to_string()
    })
}

fn with_database_name(database_url: &str, database_name: &str) -> String {
    let slash = database_url.rfind('/').unwrap();
    format!("{}{}", &database_url[..=slash], database_name)
}

async fn test_pool() -> (PgPool, TestDatabase) {
    let base_url = database_url();
    let name = format!("video_agent_eval_execution_{}", Uuid::new_v4().simple());
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
        .max_connections(4)
        .connect(&with_database_name(&base_url, &name))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (pool, TestDatabase::new(&admin_url, &name))
}

fn spec(model_id: Uuid, fingerprint: &str) -> EvalRunSpec {
    EvalRunSpec {
        candidate: CandidateRef {
            definition_kind: novex_eval::EvalDefinitionKind::Agent,
            key: "video.script".into(),
            version: "candidate".into(),
            digest: "a".repeat(64),
        },
        baseline: None,
        case_set_version: "fixture@1".into(),
        evaluator_version: "fixture-evaluator@1".into(),
        mode: EvalMode::RealModel,
        context: None,
        model_binding: Some(ModelBinding {
            model_id: model_id.to_string(),
            behavior_fingerprint: fingerprint.into(),
        }),
        budget: EvalBudget {
            approved_real_calls: true,
            max_cases: 1,
            max_input_tokens: 100,
            max_output_tokens: 100,
            max_retries: 0,
            max_cost_micros: 1000,
        },
    }
}

fn request(model_id: Uuid, fingerprint: &str) -> novex_agent::AuditedModelRequest {
    novex_agent::AuditedModelRequest {
        owner: novex_agent::AuditedCallOwner::EvalRun(Uuid::nil()),
        step_id: None,
        root_call_id: None,
        parent_call_id: None,
        attempt: 1,
        agent_key: "video.script".into(),
        agent_version: "2.0.0".into(),
        node_key: "script.complete".into(),
        variables: BTreeMap::from([("scene_count".into(), json!(3))]),
        context_candidates: vec![text_context_candidate(TextContextCandidateInput {
            candidate_id: "eval-input".into(),
            source_kind: "user_instruction".into(),
            source_id: "eval_case".into(),
            source_version: "1".into(),
            trust: TrustLevel::UserInstruction,
            priority: ContextPriority::P0,
            required: true,
            render_order: 0,
            observed_at: "2026-01-01T00:00:00Z".into(),
            text: "fixture input".into(),
        })],
        context_atomic_groups: Vec::new(),
        compiled_at: "2026-01-01T00:00:00Z".into(),
        tool_profile: "chat".into(),
        tool_schema: None,
        binding: fixed_binding(model_id, fingerprint),
        context_sources: json!([{ "id":"eval-input", "source":"eval_case", "trust":"user_instruction" }]),
        memory_sources: json!([]),
        parameters: json!({ "max_output_tokens": 100 }),
        asset_references: json!([]),
    }
}

fn fixed_binding(model_id: Uuid, fingerprint: &str) -> FixedModelBinding {
    let registry = DefinitionRegistry::load(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("agent-definitions"),
    )
    .unwrap();
    let agent = registry.agent("video.script", "2.0.0").unwrap();
    let context_policy_bindings = agent
        .nodes
        .iter()
        .map(|(node_key, reference)| {
            let policy = reference.context_policy.as_ref().unwrap();
            let definition = registry
                .context_policy(&policy.key, &policy.version)
                .unwrap();
            (
                node_key.clone(),
                novex_agent::FixedDefinitionBinding {
                    key: policy.key.clone(),
                    version: policy.version.clone(),
                    digest: definition_digest(definition).unwrap(),
                },
            )
        })
        .collect();
    let profile = registry
        .tokenizer_profile("byte-upper-bound", "1.0.0")
        .unwrap();
    FixedModelBinding {
        model_id,
        behavior_fingerprint: fingerprint.into(),
        context_policy_bindings,
        tokenizer_profile: novex_agent::FixedDefinitionBinding {
            key: profile.profile_key.clone(),
            version: profile.version.clone(),
            digest: definition_digest(profile).unwrap(),
        },
    }
}

#[tokio::test]
async fn real_eval_attempt_reserves_budget_and_uses_eval_owned_model_call() {
    let (pool, _database) = test_pool().await;
    let model_id = sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO ai_models (display_name, model_type, provider_name, api_protocol, protocol_version, auth_scheme, request_base_url, upstream_model, api_key, timeout_seconds, max_output_tokens, settings, status, source)
           VALUES ('eval', 'text', 'fixture', 'openai_responses', 'v1', 'bearer', 'https://example.invalid/v1', 'fixture', 'secret', 5, 100, '{"context_window":8192}', 'enabled', 'admin') RETURNING id"#,
    ).fetch_one(&pool).await.unwrap();
    let fingerprint = "b".repeat(64);
    let eval_repo = PostgresEvalRepository::new(pool.clone());
    let run = eval_repo
        .create_run(&spec(model_id, &fingerprint))
        .await
        .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let resolver = StaticResolver {
        resolved: ResolvedBoundModel {
            client: Arc::new(FakeClient {
                calls: calls.clone(),
            }),
            model_id,
            behavior_fingerprint: fingerprint.clone(),
            capabilities: ModelCapabilities {
                text: true,
                tool_calling: false,
                structured_output: true,
                vision: false,
                reasoning: false,
                context_window: 8192,
            },
            tokenizer_profile_key: "byte-upper-bound".into(),
            tokenizer_profile_version: "1.0.0".into(),
            max_output_tokens: 100,
            model_snapshot: json!({"provider":"fixture"}),
            known_secrets: vec!["secret".into()],
        },
    };
    let registry = Arc::new(
        DefinitionRegistry::load(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("agent-definitions"),
        )
        .unwrap(),
    );
    let executor = Arc::new(novex_agent::AuditedModelExecutor::new(
        registry,
        Arc::new(resolver),
        Arc::new(PostgresModelCallRepository::new(pool.clone())),
        Arc::new(PostgresContextAuditRepository::new(pool.clone())),
    ));
    let runner = RealEvalRunner::new(executor);
    let first = runner
        .execute_attempt(
            run.id,
            request(model_id, &fingerprint),
            EvalBudgetCharge {
                input_tokens: 20,
                output_tokens: 20,
                cost_micros: 100,
                retry: false,
            },
        )
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let owner: String = sqlx::query_scalar("SELECT CASE WHEN eval_run_id IS NOT NULL THEN 'eval_run' ELSE 'other' END FROM model_calls WHERE id=$1")
        .bind(first.model_call_id).fetch_one(&pool).await.unwrap();
    assert_eq!(owner, "eval_run");
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT actual_real_model_calls FROM eval_runs WHERE id=$1")
            .bind(run.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );

    let second = runner
        .execute_attempt(
            run.id,
            request(model_id, &fingerprint),
            EvalBudgetCharge {
                input_tokens: 20,
                output_tokens: 20,
                cost_micros: 100,
                retry: false,
            },
        )
        .await;
    assert!(matches!(
        second,
        Err(novex_agent::AuditedModelError::PrepareAudit(_))
    ));
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "budget rejection must precede provider"
    );
    pool.close().await;
}
