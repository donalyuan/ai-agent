use async_trait::async_trait;
use novex_agent::{AuditedModelExecutor, BoundModelResolver, ResolvedBoundModel};
use novex_ai_core::{DefinitionRegistry, DefinitionStatus, ModelCapabilities};
use novex_api::{
    application::evaluations::{
        build_production_crew_eval_authorization_plan, ProductionCrewEvalAuthorizationError,
        ProductionCrewEvalAuthorizationLimits,
    },
    repositories::{PostgresContextAuditRepository, PostgresModelCallRepository},
};
use novex_model::{LLMClient, LLMError, LLMPrompt};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use uuid::Uuid;

mod support;

use support::test_database::TestDatabase;

#[derive(Clone)]
struct CountingClient {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl LLMClient for CountingClient {
    async fn generate_script(&self, _prompt: LLMPrompt) -> Result<String, LLMError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok("provider must not run while building authorization".into())
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

async fn database() -> (PgPool, TestDatabase) {
    let base_url = database_url();
    let database_name = format!("production_eval_auth_{}", Uuid::new_v4().simple());
    let admin_url = with_database_name(&base_url, "postgres");
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .unwrap();
    sqlx::query(&format!(r#"CREATE DATABASE "{}""#, database_name))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&with_database_name(&base_url, &database_name))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (pool, TestDatabase::new(&admin_url, &database_name))
}

fn audited_executor(
    pool: &PgPool,
    registry: Arc<DefinitionRegistry>,
    calls: Arc<AtomicUsize>,
    vision: bool,
    max_output_tokens: u64,
) -> (Uuid, AuditedModelExecutor) {
    let model_id = Uuid::new_v4();
    let resolver = FixedResolver {
        resolved: ResolvedBoundModel {
            client: Arc::new(CountingClient { calls }),
            model_id,
            behavior_fingerprint: "b".repeat(64),
            capabilities: ModelCapabilities {
                text: true,
                tool_calling: false,
                structured_output: true,
                vision,
                reasoning: false,
                context_window: 128_000,
            },
            tokenizer_profile_key: "openai.o200k".into(),
            tokenizer_profile_version: "1.0.0".into(),
            max_output_tokens,
            model_snapshot: serde_json::json!({"provider":"fixture"}),
            known_secrets: vec![],
        },
    };
    (
        model_id,
        AuditedModelExecutor::new(
            registry,
            Arc::new(resolver),
            Arc::new(PostgresModelCallRepository::new(pool.clone())),
            Arc::new(PostgresContextAuditRepository::new(pool.clone())),
        ),
    )
}

#[tokio::test]
async fn unconfirmed_real_eval_plan_lists_exact_limits_without_calls_or_activation() {
    let (pool, _database) = database().await;
    let registry = Arc::new(DefinitionRegistry::load("/app/agent-definitions").unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    let (model_id, executor) =
        audited_executor(&pool, registry.clone(), calls.clone(), true, 4_000);
    let plan = build_production_crew_eval_authorization_plan(
        &registry,
        &executor,
        model_id,
        ProductionCrewEvalAuthorizationLimits::conservative_v3(),
    )
    .await
    .unwrap();

    assert!(plan.authorization_ready);
    assert_eq!(
        plan.authorization_state,
        "awaiting_explicit_user_confirmation"
    );
    assert_eq!(plan.authorization_digest.len(), 64);
    assert_eq!(plan.model_binding.behavior_fingerprint, "b".repeat(64));
    assert_eq!(plan.items.len(), 18);
    assert_eq!(plan.zero_cost_context_candidates.len(), 9);
    assert_eq!(plan.totals.eval_runs, 18);
    assert_eq!(plan.totals.max_cases, 18);
    assert_eq!(plan.totals.max_input_tokens, 294_912);
    assert_eq!(plan.totals.max_output_tokens, 50_000);
    assert_eq!(plan.totals.max_retries, 0);
    assert_eq!(plan.totals.max_cost_micros, 1_800_000);
    assert!(plan
        .items
        .iter()
        .all(|item| !item.budget.approved_real_calls && item.blockers.is_empty()));
    assert!(matches!(
        plan.approved_specs(false, &plan.authorization_digest),
        Err(ProductionCrewEvalAuthorizationError::ApprovalRequired)
    ));
    assert!(matches!(
        plan.approved_specs(true, &"0".repeat(64)),
        Err(ProductionCrewEvalAuthorizationError::ApprovalRequired)
    ));
    let approved = plan
        .approved_specs(true, &plan.authorization_digest)
        .unwrap();
    assert_eq!(approved.len(), 18);
    assert!(approved.iter().all(|spec| spec.budget.approved_real_calls));

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
        assert_eq!(
            registry
                .agent(&format!("production.{role}"), "3.0.0")
                .unwrap()
                .status,
            DefinitionStatus::Candidate
        );
        assert_eq!(
            registry
                .active_agent(&format!("production.{role}"))
                .unwrap()
                .version,
            "2.0.0"
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM eval_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn capability_blocked_plan_is_queryable_but_cannot_be_authorized() {
    let (pool, _database) = database().await;
    let registry = Arc::new(DefinitionRegistry::load("/app/agent-definitions").unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    let (model_id, executor) =
        audited_executor(&pool, registry.clone(), calls.clone(), false, 3_000);
    let plan = build_production_crew_eval_authorization_plan(
        &registry,
        &executor,
        model_id,
        ProductionCrewEvalAuthorizationLimits::conservative_v3(),
    )
    .await
    .unwrap();

    assert!(!plan.authorization_ready);
    assert!(plan
        .blockers
        .iter()
        .any(|blocker| blocker.contains("screenwriter") && blocker.contains("max_output_tokens")));
    assert!(plan
        .blockers
        .iter()
        .any(|blocker| blocker.contains("editor") && blocker.contains("vision")));
    assert!(plan
        .blockers
        .iter()
        .any(|blocker| blocker.contains("qc") && blocker.contains("vision")));
    assert!(matches!(
        plan.approved_specs(true, &plan.authorization_digest),
        Err(ProductionCrewEvalAuthorizationError::CapabilityBlocked(_))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM eval_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
