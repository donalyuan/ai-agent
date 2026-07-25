use novex_eval::{
    validate_activation, ActivationEvidence, BudgetError, BudgetTracker, CandidateRef,
    DefinitionStatus, EvalBudget, EvalCaseResult, EvalMode, EvalRunSpec, ManifestLifecycle,
    ModelBinding, ZeroCostRunner, REQUIRED_GATES,
};

fn candidate() -> CandidateRef {
    CandidateRef {
        key: "video.script".into(),
        version: "1.0.0".into(),
        digest: "a".repeat(64),
    }
}

fn zero_cost_spec(mode: EvalMode) -> EvalRunSpec {
    EvalRunSpec {
        candidate: candidate(),
        baseline: Some(CandidateRef {
            key: "video.script".into(),
            version: "legacy".into(),
            digest: "b".repeat(64),
        }),
        case_set_version: "rust-v1-golden@1".into(),
        evaluator_version: "novex-eval@1".into(),
        mode,
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

fn passing_case(id: &str) -> EvalCaseResult {
    EvalCaseResult {
        case_id: id.into(),
        static_validation: true,
        dry_run: true,
        safety: true,
        structured_output: true,
        core_quality: true,
        input_tokens: 100,
        output_tokens: 50,
        cost_micros: 0,
        redacted_details: serde_json::json!({"result":"equivalent"}),
    }
}

#[test]
fn candidate_requires_every_gate_and_respects_token_and_cost_thresholds() {
    let spec = zero_cost_spec(EvalMode::ZeroCost);
    let passed = ZeroCostRunner::run(&spec, &[passing_case("case-1")]).unwrap();
    assert!(passed.passed);
    assert_eq!(passed.gates.len(), REQUIRED_GATES.len());
    assert!(passed.gates.iter().all(|gate| gate.passed));
    assert_eq!(passed.actual_real_model_calls, 0);

    for gate in [
        "static_validation",
        "dry_run",
        "safety",
        "structured_output",
        "core_quality",
    ] {
        let mut failed = passing_case(gate);
        match gate {
            "static_validation" => failed.static_validation = false,
            "dry_run" => failed.dry_run = false,
            "safety" => failed.safety = false,
            "structured_output" => failed.structured_output = false,
            "core_quality" => failed.core_quality = false,
            _ => unreachable!(),
        }
        let report = ZeroCostRunner::run(&spec, &[failed]).unwrap();
        assert!(!report.passed, "failed gate {gate} must block activation");
    }

    let mut over_tokens = passing_case("over-token-budget");
    over_tokens.output_tokens = spec.budget.max_output_tokens + 1;
    let report = ZeroCostRunner::run(&spec, &[over_tokens]).unwrap();
    assert!(!report.passed);
    assert!(
        !report
            .gates
            .iter()
            .find(|gate| gate.name == "token_budget")
            .unwrap()
            .passed
    );

    let mut paid = passing_case("unexpected-cost");
    paid.cost_micros = 1;
    let report = ZeroCostRunner::run(&spec, &[paid]).unwrap();
    assert!(!report.passed);
    assert!(
        !report
            .gates
            .iter()
            .find(|gate| gate.name == "cost_budget")
            .unwrap()
            .passed
    );
}

#[test]
fn v1_golden_baseline_is_zero_cost_and_records_zero_real_calls() {
    let spec = zero_cost_spec(EvalMode::GoldenBaseline);
    let cases = (1..=14)
        .map(|index| passing_case(&format!("rust-node-{index}")))
        .collect::<Vec<_>>();
    let report = ZeroCostRunner::run(&spec, &cases).unwrap();

    assert!(report.passed);
    assert_eq!(report.mode, EvalMode::GoldenBaseline);
    assert_eq!(report.actual_real_model_calls, 0);
    assert_eq!(report.usage.cases, 14);
    assert_eq!(report.usage.cost_micros, 0);
}

#[test]
fn real_evaluation_requires_immutable_approval_and_stops_at_every_budget() {
    let binding = ModelBinding {
        model_id: "11111111-1111-4111-8111-111111111111".into(),
        behavior_fingerprint: "c".repeat(64),
    };
    let budget = EvalBudget {
        approved_real_calls: true,
        max_cases: 2,
        max_input_tokens: 100,
        max_output_tokens: 50,
        max_retries: 1,
        max_cost_micros: 1000,
    };
    let mut tracker = BudgetTracker::new(budget.clone(), binding.clone()).unwrap();
    tracker.authorize(&binding.behavior_fingerprint).unwrap();
    tracker.record_attempt(40, 20, 300, false).unwrap();
    tracker.record_attempt(40, 20, 300, true).unwrap();

    assert_eq!(
        tracker.authorize(&binding.behavior_fingerprint),
        Err(BudgetError::CaseLimit)
    );
    assert_eq!(
        tracker.authorize(&"d".repeat(64)),
        Err(BudgetError::FingerprintDrift)
    );

    let unapproved = EvalBudget {
        approved_real_calls: false,
        ..budget
    };
    assert_eq!(
        BudgetTracker::new(unapproved, binding),
        Err(BudgetError::ApprovalRequired)
    );
}

#[test]
fn budget_tracker_rejects_usage_that_would_cross_token_cost_or_retry_limits() {
    let binding = ModelBinding {
        model_id: "11111111-1111-4111-8111-111111111111".into(),
        behavior_fingerprint: "c".repeat(64),
    };
    let budget = EvalBudget {
        approved_real_calls: true,
        max_cases: 3,
        max_input_tokens: 10,
        max_output_tokens: 10,
        max_retries: 0,
        max_cost_micros: 10,
    };
    let mut tracker = BudgetTracker::new(budget.clone(), binding.clone()).unwrap();
    assert_eq!(
        tracker.record_attempt(11, 1, 1, false),
        Err(BudgetError::InputTokenLimit)
    );
    assert_eq!(
        tracker.record_attempt(1, 11, 1, false),
        Err(BudgetError::OutputTokenLimit)
    );
    assert_eq!(
        tracker.record_attempt(1, 1, 11, false),
        Err(BudgetError::CostLimit)
    );
    assert_eq!(
        tracker.record_attempt(1, 1, 1, true),
        Err(BudgetError::RetryLimit)
    );
    assert_eq!(
        tracker.usage().real_model_calls,
        0,
        "rejected attempts must not consume or call"
    );
}

#[test]
fn activation_requires_matching_immutable_passed_report_or_v1_golden_evidence() {
    let spec = zero_cost_spec(EvalMode::GoldenBaseline);
    let report = ZeroCostRunner::run(&spec, &[passing_case("v1")]).unwrap();
    let evidence = ActivationEvidence {
        report_id: "report-1".into(),
        candidate: candidate(),
        report,
    };
    validate_activation(&candidate(), &evidence).unwrap();

    let mut failed = evidence.clone();
    failed.report.passed = false;
    assert!(validate_activation(&candidate(), &failed).is_err());
    let mut mismatch = evidence;
    mismatch.candidate.digest = "f".repeat(64);
    assert!(validate_activation(&candidate(), &mismatch).is_err());
}

#[test]
fn supported_rollback_and_revocation_preserve_history_without_silent_upgrade() {
    assert!(ManifestLifecycle::can_start_new(DefinitionStatus::Active));
    assert!(!ManifestLifecycle::can_start_new(
        DefinitionStatus::Supported
    ));
    assert!(ManifestLifecycle::can_continue_bound(
        DefinitionStatus::Supported
    ));
    assert!(!ManifestLifecycle::can_continue_bound(
        DefinitionStatus::Revoked
    ));

    let rollback = ManifestLifecycle::rollback(
        ("2.0.0", DefinitionStatus::Active),
        ("1.0.0", DefinitionStatus::Supported),
    )
    .unwrap();
    assert_eq!(rollback.new_active_version, "1.0.0");
    assert_eq!(rollback.previous_active_status, DefinitionStatus::Supported);
    assert!(rollback.preserve_bindings);
    assert!(rollback.preserve_audit_and_reports);
}
