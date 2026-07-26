use novex_api::repositories::{EvalRepositoryError, PostgresEvalRepository};
use novex_eval::{
    CandidateRef, ContextEvalCaseResult, ContextEvalConfig, ContextEvalRunner, EvalBudget,
    EvalCaseResult, EvalDefinitionKind, EvalMode, EvalRunSpec, ModelBinding, ZeroCostRunner,
};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

mod support;

use support::test_database::TestDatabase;

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
    let name = format!("video_agent_eval_repository_{}", Uuid::new_v4().simple());
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
        .max_connections(3)
        .connect(&with_database_name(&base_url, &name))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (pool, TestDatabase::new(&admin_url, &name))
}

fn candidate() -> CandidateRef {
    CandidateRef {
        definition_kind: EvalDefinitionKind::Agent,
        key: "video.script".into(),
        version: "1.0.0".into(),
        digest: "a".repeat(64),
    }
}

fn spec(mode: EvalMode) -> EvalRunSpec {
    EvalRunSpec {
        candidate: candidate(),
        baseline: None,
        case_set_version: "rust-v1-golden@1".into(),
        evaluator_version: "novex-eval@1".into(),
        mode,
        context: None,
        model_binding: None,
        budget: EvalBudget {
            approved_real_calls: false,
            max_cases: 14,
            max_input_tokens: 100_000,
            max_output_tokens: 100_000,
            max_retries: 0,
            max_cost_micros: 0,
        },
    }
}

fn passing_case() -> EvalCaseResult {
    EvalCaseResult {
        case_id: "all-rust-v1-nodes".into(),
        static_validation: true,
        dry_run: true,
        safety: true,
        structured_output: true,
        core_quality: true,
        input_tokens: 100,
        output_tokens: 50,
        cost_micros: 0,
        redacted_details: json!({"nodes":14,"equivalent":true}),
    }
}

fn context_spec() -> EvalRunSpec {
    let policy = CandidateRef {
        definition_kind: EvalDefinitionKind::ContextPolicy,
        key: "personal.turn.baseline".into(),
        version: "1.0.0".into(),
        digest: "d".repeat(64),
    };
    EvalRunSpec {
        candidate: policy.clone(),
        baseline: None,
        case_set_version: "context-production-nodes@1".into(),
        evaluator_version: "novex-context-eval@1".into(),
        mode: EvalMode::ZeroCost,
        context: Some(ContextEvalConfig {
            schema_version: "1".into(),
            policy,
            tokenizer_profile: CandidateRef {
                definition_kind: EvalDefinitionKind::TokenizerProfile,
                key: "openai.o200k".into(),
                version: "1.0.0".into(),
                digest: "e".repeat(64),
            },
        }),
        model_binding: None,
        budget: EvalBudget {
            approved_real_calls: false,
            max_cases: 18,
            max_input_tokens: 100_000,
            max_output_tokens: 0,
            max_retries: 0,
            max_cost_micros: 0,
        },
    }
}

fn passing_context_case() -> ContextEvalCaseResult {
    ContextEvalCaseResult {
        case_id: "personal.turn:golden".into(),
        node_key: "personal.turn".into(),
        schema_valid: true,
        rust_tokens: 100,
        typescript_tokens: 100,
        first_digest: "f".repeat(64),
        repeated_digest: "f".repeat(64),
        shuffled_digest: "f".repeat(64),
        safety_passed: true,
        budget_passed: true,
        core_prompt_passed: true,
        business_output_passed: true,
        equivalent: false,
        selection_diff: json!([{"candidate_id":"reference","change":"excluded"}]),
        budget_ledger: json!({"dynamic_context_budget":4096,"selected_context_tokens":100}),
        tokenizer_metrics: json!({"rust_tokens":100,"typescript_tokens":100,"mode":"exact"}),
    }
}

#[tokio::test]
async fn zero_cost_golden_report_is_immutable_and_is_valid_activation_evidence() {
    let (pool, _database) = test_pool().await;
    let repository = PostgresEvalRepository::new(pool.clone());
    let spec = spec(EvalMode::GoldenBaseline);
    let run = repository.create_run(&spec).await.unwrap();
    assert_eq!(run.status, "pending");
    assert_eq!(run.validation_mode, "golden_baseline");
    assert_eq!(run.actual_real_model_calls, 0);
    assert_eq!(run.approval_snapshot["approved_real_calls"], false);

    let report = ZeroCostRunner::run(&spec, &[passing_case()]).unwrap();
    let stored = repository.complete_run(run.id, &report).await.unwrap();
    assert!(stored.passed);
    assert!(!stored.source_deleted);
    assert_eq!(stored.aggregate_metrics["real_model_calls"], 0);
    assert_eq!(
        repository
            .activation_report_id(&spec.candidate)
            .await
            .unwrap(),
        stored.id
    );

    assert!(matches!(
        repository.complete_run(run.id, &report).await,
        Err(EvalRepositoryError::Immutable)
    ));
    assert!(
        sqlx::query("UPDATE eval_reports SET passed = FALSE WHERE id = $1")
            .bind(stored.id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query(
            "UPDATE eval_reports SET source_deleted = TRUE, redacted_case_results = '[]', passed = FALSE WHERE id = $1"
        )
        .bind(stored.id)
        .execute(&pool)
        .await
        .is_err()
    );
    assert!(sqlx::query("DELETE FROM eval_reports WHERE id = $1")
        .bind(stored.id)
        .execute(&pool)
        .await
        .is_err());
    assert!(
        sqlx::query("UPDATE eval_runs SET max_cases = max_cases + 1 WHERE id = $1")
            .bind(run.id)
            .execute(&pool)
            .await
            .is_err()
    );
    pool.close().await;
}

#[tokio::test]
async fn zero_cost_or_failed_reports_cannot_activate_behavior_changes() {
    let (pool, _database) = test_pool().await;
    let repository = PostgresEvalRepository::new(pool.clone());
    let zero_spec = spec(EvalMode::ZeroCost);
    let run = repository.create_run(&zero_spec).await.unwrap();
    let report = ZeroCostRunner::run(&zero_spec, &[passing_case()]).unwrap();
    repository.complete_run(run.id, &report).await.unwrap();
    assert!(matches!(
        repository.activation_report_id(&zero_spec.candidate).await,
        Err(EvalRepositoryError::ActivationEvidenceMissing)
    ));

    let golden_spec = spec(EvalMode::GoldenBaseline);
    let run = repository.create_run(&golden_spec).await.unwrap();
    let mut failed_case = passing_case();
    failed_case.safety = false;
    let report = ZeroCostRunner::run(&golden_spec, &[failed_case]).unwrap();
    repository.complete_run(run.id, &report).await.unwrap();
    assert!(matches!(
        repository
            .activation_report_id(&golden_spec.candidate)
            .await,
        Err(EvalRepositoryError::ActivationEvidenceMissing)
    ));
    pool.close().await;
}

#[tokio::test]
async fn real_model_run_requires_fixed_explicit_approval_and_budget_snapshot() {
    let (pool, _database) = test_pool().await;
    let repository = PostgresEvalRepository::new(pool.clone());
    let model_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO ai_models (
            display_name, model_type, provider_name, api_protocol, protocol_version,
            auth_scheme, request_base_url, upstream_model, api_key, timeout_seconds,
            max_output_tokens, settings, status, source
        ) VALUES (
            'Eval Model', 'text', 'fixture', 'openai_responses', 'v1', 'bearer',
            'https://example.invalid/v1', 'fixture-model', 'secret', 5, 100,
            '{"context_window":8192}', 'enabled', 'admin'
        ) RETURNING id
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut real = spec(EvalMode::RealModel);
    real.model_binding = Some(ModelBinding {
        model_id: model_id.to_string(),
        behavior_fingerprint: "c".repeat(64),
    });
    real.budget = EvalBudget {
        approved_real_calls: false,
        max_cases: 2,
        max_input_tokens: 100,
        max_output_tokens: 100,
        max_retries: 1,
        max_cost_micros: 1000,
    };
    assert!(matches!(
        repository.create_run(&real).await,
        Err(EvalRepositoryError::ApprovalRequired)
    ));

    real.budget.approved_real_calls = true;
    let run = repository.create_run(&real).await.unwrap();
    assert_eq!(run.validation_mode, "real_model");
    assert_eq!(run.approval_snapshot["budget"]["max_cost_micros"], 1000);
    assert_eq!(
        run.approval_snapshot["model_binding"]["behavior_fingerprint"],
        "c".repeat(64)
    );
    assert!(
        sqlx::query("UPDATE eval_runs SET behavior_fingerprint = $2 WHERE id = $1")
            .bind(run.id)
            .bind("d".repeat(64))
            .execute(&pool)
            .await
            .is_err()
    );
    pool.close().await;
}

#[tokio::test]
async fn context_report_persists_versioned_evidence_and_authorizes_only_exact_kind() {
    let (pool, _database) = test_pool().await;
    let repository = PostgresEvalRepository::new(pool.clone());
    let spec = context_spec();
    let run = repository.create_run(&spec).await.unwrap();
    assert_eq!(run.definition_kind, "context_policy");
    assert_eq!(run.approval_snapshot["schema_version"], "2");
    assert_eq!(
        run.context_case_set.as_ref().unwrap()["version"],
        spec.case_set_version
    );
    assert_eq!(
        run.context_policy.as_ref().unwrap()["key"],
        spec.candidate.key
    );
    assert_eq!(
        run.tokenizer_profile.as_ref().unwrap()["definition_kind"],
        "tokenizer_profile"
    );

    let report = ContextEvalRunner::run(&spec, &[passing_context_case()]).unwrap();
    assert!(report.passed, "zero-cost 行为变化应由明确 Context 门禁决定");
    let stored = repository.complete_run(run.id, &report).await.unwrap();
    assert!(stored.passed);
    assert_eq!(
        stored
            .context_node_results
            .as_ref()
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        stored.context_selection_diff.as_ref().unwrap()[0][0]["change"],
        "excluded"
    );
    assert_eq!(
        stored.context_budget_ledgers.as_ref().unwrap()[0]["dynamic_context_budget"],
        4096
    );
    assert_eq!(
        stored.tokenizer_metrics.as_ref().unwrap()[0]["rust_tokens"],
        100
    );
    assert_eq!(
        repository
            .activation_report_id(&spec.candidate)
            .await
            .unwrap(),
        stored.id
    );

    let wrong_kind = CandidateRef {
        definition_kind: EvalDefinitionKind::Prompt,
        ..spec.candidate.clone()
    };
    assert!(matches!(
        repository.activation_report_id(&wrong_kind).await,
        Err(EvalRepositoryError::ActivationEvidenceMissing)
    ));
    assert!(
        sqlx::query("UPDATE eval_reports SET tokenizer_metrics = '[]' WHERE id = $1")
            .bind(stored.id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE eval_runs SET context_policy = '{}' WHERE id = $1")
            .bind(run.id)
            .execute(&pool)
            .await
            .is_err()
    );
    pool.close().await;
}
