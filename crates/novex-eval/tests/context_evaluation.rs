use novex_eval::{
    validate_activation, ActivationEvidence, CandidateRef, ContextEvalCaseResult,
    ContextEvalConfig, ContextEvalRunner, EvalBudget, EvalDefinitionKind, EvalMode, EvalRunSpec,
    REQUIRED_CONTEXT_GATES,
};
use serde_json::json;

const PRODUCTION_NODES: [&str; 18] = [
    "personal.branch_summary",
    "personal.compaction",
    "personal.tool_followup",
    "personal.turn",
    "project.strategy_draft",
    "script.complete",
    "script.generation_intent",
    "script.metadata",
    "script.scene_patch",
    "script.single_scene",
    "sound.recommend",
    "topic.generate",
    "topic.group_review",
    "topic.quality_review",
    "topic.rewrite",
    "topic.supplement",
    "work.patch",
    "work.plan",
];

fn reference(kind: EvalDefinitionKind, key: &str, digest: char) -> CandidateRef {
    CandidateRef {
        definition_kind: kind,
        key: key.into(),
        version: "1.0.0".into(),
        digest: digest.to_string().repeat(64),
    }
}

fn spec(mode: EvalMode, candidate_kind: EvalDefinitionKind) -> EvalRunSpec {
    let policy = reference(
        EvalDefinitionKind::ContextPolicy,
        "personal.turn.baseline",
        'a',
    );
    let profile = reference(EvalDefinitionKind::TokenizerProfile, "openai.o200k", 'b');
    let candidate = match candidate_kind {
        EvalDefinitionKind::ContextPolicy => policy.clone(),
        EvalDefinitionKind::TokenizerProfile => profile.clone(),
        _ => panic!("Context Eval 只接受 Policy/Profile candidate"),
    };
    EvalRunSpec {
        candidate,
        baseline: None,
        case_set_version: "context-production-nodes@1".into(),
        evaluator_version: "novex-context-eval@1".into(),
        mode,
        context: Some(ContextEvalConfig {
            schema_version: "1".into(),
            policy,
            tokenizer_profile: profile,
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

fn passing_case(node_key: &str) -> ContextEvalCaseResult {
    ContextEvalCaseResult {
        case_id: format!("{node_key}:golden"),
        node_key: node_key.into(),
        schema_valid: true,
        rust_tokens: 100,
        typescript_tokens: 100,
        first_digest: "c".repeat(64),
        repeated_digest: "c".repeat(64),
        shuffled_digest: "c".repeat(64),
        safety_passed: true,
        budget_passed: true,
        core_prompt_passed: true,
        business_output_passed: true,
        equivalent: true,
        selection_diff: json!([]),
        budget_ledger: json!({"dynamic_context_budget":4096,"selected_context_tokens":100}),
        tokenizer_metrics: json!({"mode":"exact","wasted_tokens":0}),
    }
}

#[test]
fn every_context_gate_failure_blocks_candidate_activation() {
    let passing_spec = spec(EvalMode::ZeroCost, EvalDefinitionKind::ContextPolicy);
    let passed = ContextEvalRunner::run(&passing_spec, &[passing_case("personal.turn")]).unwrap();
    assert!(passed.passed);
    assert_eq!(passed.actual_real_model_calls, 0);
    assert_eq!(passed.gates.len(), REQUIRED_CONTEXT_GATES.len());
    validate_activation(
        &passing_spec.candidate,
        &ActivationEvidence {
            report_id: "context-report".into(),
            candidate: passing_spec.candidate.clone(),
            report: passed,
        },
    )
    .unwrap();

    for gate in REQUIRED_CONTEXT_GATES {
        let mut case = passing_case("personal.turn");
        match gate {
            "schema" => case.schema_valid = false,
            "cross_language_token" => case.typescript_tokens += 1,
            "determinism" => case.shuffled_digest = "d".repeat(64),
            "safety" => case.safety_passed = false,
            "budget" => case.budget_passed = false,
            "core_prompt" => case.core_prompt_passed = false,
            "business_output" => case.business_output_passed = false,
            "baseline_equivalence" => case.equivalent = false,
            _ => unreachable!(),
        }
        let mode = if gate == "baseline_equivalence" {
            EvalMode::GoldenBaseline
        } else {
            EvalMode::ZeroCost
        };
        let failed_spec = spec(mode, EvalDefinitionKind::ContextPolicy);
        let report = ContextEvalRunner::run(&failed_spec, &[case]).unwrap();
        assert!(!report.passed, "失败门禁 {gate} 必须阻止激活");
        assert!(validate_activation(
            &failed_spec.candidate,
            &ActivationEvidence {
                report_id: format!("failed-{gate}"),
                candidate: failed_spec.candidate.clone(),
                report,
            },
        )
        .is_err());
    }
}

#[test]
fn all_production_nodes_have_zero_cost_equivalent_baseline_evidence() {
    let contract: serde_json::Value = serde_json::from_str(include_str!(
        "../../../agent-definitions/fixtures/context-eval-contract.json"
    ))
    .unwrap();
    assert_eq!(
        contract["required_gates"],
        serde_json::to_value(REQUIRED_CONTEXT_GATES).unwrap()
    );
    assert_eq!(
        contract["production_nodes"],
        serde_json::to_value(PRODUCTION_NODES).unwrap()
    );
    assert_eq!(
        contract["baseline_report"]["report_id"],
        "context-production-baseline@1"
    );
    assert_eq!(contract["baseline_report"]["mode"], "golden_baseline");
    assert_eq!(contract["baseline_report"]["passed"], true);
    assert_eq!(contract["baseline_report"]["actual_real_model_calls"], 0);
    assert_eq!(
        contract["baseline_report"]["node_results"]
            .as_array()
            .unwrap()
            .len(),
        18
    );
    assert_eq!(contract["tokenizer_metrics"]["rust_tokens"], 100);
    assert_eq!(contract["tokenizer_metrics"]["typescript_tokens"], 100);
    let spec = spec(EvalMode::GoldenBaseline, EvalDefinitionKind::ContextPolicy);
    let cases = PRODUCTION_NODES
        .into_iter()
        .map(passing_case)
        .collect::<Vec<_>>();
    let report = ContextEvalRunner::run(&spec, &cases).unwrap();

    assert!(report.passed);
    assert_eq!(report.actual_real_model_calls, 0);
    assert_eq!(report.usage.cases, 18);
    assert_eq!(report.usage.cost_micros, 0);
    let context = report.context.unwrap();
    assert_eq!(context.node_results.len(), 18);
    assert!(context.node_results.iter().all(|result| result.equivalent));
    assert_eq!(context.selection_diff.len(), 18);
    assert_eq!(context.budget_ledgers.len(), 18);
    assert_eq!(context.tokenizer_metrics.len(), 18);
    assert_eq!(contract["external_effects"]["real_model_calls"], 0);
}

#[test]
fn intentional_context_change_can_pass_zero_cost_but_not_baseline_equivalence() {
    let mut changed = passing_case("personal.turn");
    changed.equivalent = false;
    changed.selection_diff = json!([{"candidate_id":"reference","change":"excluded"}]);

    let candidate = spec(EvalMode::ZeroCost, EvalDefinitionKind::TokenizerProfile);
    let report = ContextEvalRunner::run(&candidate, &[changed.clone()]).unwrap();
    assert!(report.passed);
    assert!(!report.context.unwrap().node_results[0].equivalent);

    let baseline = spec(
        EvalMode::GoldenBaseline,
        EvalDefinitionKind::TokenizerProfile,
    );
    assert!(
        !ContextEvalRunner::run(&baseline, &[changed])
            .unwrap()
            .passed
    );
}

#[test]
fn context_runner_rejects_paid_or_mismatched_candidates() {
    let mut paid = spec(EvalMode::RealModel, EvalDefinitionKind::ContextPolicy);
    paid.budget.approved_real_calls = true;
    assert!(ContextEvalRunner::run(&paid, &[passing_case("personal.turn")]).is_err());

    let mut mismatch = spec(EvalMode::ZeroCost, EvalDefinitionKind::ContextPolicy);
    mismatch.candidate = reference(EvalDefinitionKind::ContextPolicy, "other.policy", 'd');
    assert!(ContextEvalRunner::run(&mismatch, &[passing_case("personal.turn")]).is_err());
}
