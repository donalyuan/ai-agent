//! Reusable candidate evaluation, budget, activation, and lifecycle rules.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;

pub const REQUIRED_GATES: [&str; 7] = [
    "static_validation",
    "dry_run",
    "safety",
    "structured_output",
    "core_quality",
    "token_budget",
    "cost_budget",
];

pub const REQUIRED_CONTEXT_GATES: [&str; 8] = [
    "schema",
    "cross_language_token",
    "determinism",
    "safety",
    "budget",
    "core_prompt",
    "business_output",
    "baseline_equivalence",
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalDefinitionKind {
    Agent,
    Prompt,
    ContextPolicy,
    TokenizerProfile,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalMode {
    GoldenBaseline,
    ZeroCost,
    RealModel,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CandidateRef {
    pub definition_kind: EvalDefinitionKind,
    pub key: String,
    pub version: String,
    pub digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ModelBinding {
    pub model_id: String,
    pub behavior_fingerprint: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvalBudget {
    pub approved_real_calls: bool,
    pub max_cases: u64,
    pub max_input_tokens: u64,
    pub max_output_tokens: u64,
    pub max_retries: u64,
    pub max_cost_micros: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvalRunSpec {
    pub candidate: CandidateRef,
    pub baseline: Option<CandidateRef>,
    pub case_set_version: String,
    pub evaluator_version: String,
    pub mode: EvalMode,
    pub context: Option<ContextEvalConfig>,
    pub model_binding: Option<ModelBinding>,
    pub budget: EvalBudget,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContextEvalConfig {
    pub schema_version: String,
    pub policy: CandidateRef,
    pub tokenizer_profile: CandidateRef,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ContextEvalCaseResult {
    pub case_id: String,
    pub node_key: String,
    pub schema_valid: bool,
    pub rust_tokens: u64,
    pub typescript_tokens: u64,
    pub first_digest: String,
    pub repeated_digest: String,
    pub shuffled_digest: String,
    pub safety_passed: bool,
    pub budget_passed: bool,
    pub core_prompt_passed: bool,
    pub business_output_passed: bool,
    pub equivalent: bool,
    pub selection_diff: Value,
    pub budget_ledger: Value,
    pub tokenizer_metrics: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ContextEvalEvidence {
    pub schema_version: String,
    pub policy: CandidateRef,
    pub tokenizer_profile: CandidateRef,
    pub node_results: Vec<ContextEvalCaseResult>,
    pub selection_diff: Vec<Value>,
    pub budget_ledgers: Vec<Value>,
    pub tokenizer_metrics: Vec<Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EvalCaseResult {
    pub case_id: String,
    pub static_validation: bool,
    pub dry_run: bool,
    pub safety: bool,
    pub structured_output: bool,
    pub core_quality: bool,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_micros: u64,
    pub redacted_details: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GateResult {
    pub name: String,
    pub passed: bool,
    pub evidence: Value,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvalUsage {
    pub cases: u64,
    pub real_model_calls: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub retries: u64,
    pub cost_micros: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EvalReport {
    pub schema_version: String,
    pub candidate: CandidateRef,
    pub baseline: Option<CandidateRef>,
    pub case_set_version: String,
    pub evaluator_version: String,
    pub mode: EvalMode,
    pub passed: bool,
    pub gates: Vec<GateResult>,
    pub usage: EvalUsage,
    pub actual_real_model_calls: u64,
    pub redacted_case_results: Vec<EvalCaseResult>,
    pub context: Option<ContextEvalEvidence>,
}

pub struct ZeroCostRunner;

impl ZeroCostRunner {
    pub fn run(spec: &EvalRunSpec, cases: &[EvalCaseResult]) -> Result<EvalReport, EvalError> {
        validate_zero_cost_spec(spec, cases)?;
        let usage = cases.iter().fold(EvalUsage::default(), |mut usage, case| {
            usage.cases += 1;
            usage.input_tokens = usage.input_tokens.saturating_add(case.input_tokens);
            usage.output_tokens = usage.output_tokens.saturating_add(case.output_tokens);
            usage.cost_micros = usage.cost_micros.saturating_add(case.cost_micros);
            usage
        });
        let boolean_gate = |name: &str, passed: bool| GateResult {
            name: name.into(),
            passed,
            evidence: json!({"case_count": cases.len()}),
        };
        let gates = vec![
            boolean_gate(
                "static_validation",
                cases.iter().all(|case| case.static_validation),
            ),
            boolean_gate("dry_run", cases.iter().all(|case| case.dry_run)),
            boolean_gate("safety", cases.iter().all(|case| case.safety)),
            boolean_gate(
                "structured_output",
                cases.iter().all(|case| case.structured_output),
            ),
            boolean_gate("core_quality", cases.iter().all(|case| case.core_quality)),
            GateResult {
                name: "token_budget".into(),
                passed: usage.input_tokens <= spec.budget.max_input_tokens
                    && usage.output_tokens <= spec.budget.max_output_tokens,
                evidence: json!({
                    "actual_input_tokens": usage.input_tokens,
                    "actual_output_tokens": usage.output_tokens,
                    "max_input_tokens": spec.budget.max_input_tokens,
                    "max_output_tokens": spec.budget.max_output_tokens,
                }),
            },
            GateResult {
                name: "cost_budget".into(),
                passed: usage.cost_micros <= spec.budget.max_cost_micros,
                evidence: json!({
                    "actual_cost_micros": usage.cost_micros,
                    "max_cost_micros": spec.budget.max_cost_micros,
                }),
            },
        ];
        Ok(EvalReport {
            schema_version: "1".into(),
            candidate: spec.candidate.clone(),
            baseline: spec.baseline.clone(),
            case_set_version: spec.case_set_version.clone(),
            evaluator_version: spec.evaluator_version.clone(),
            mode: spec.mode,
            passed: gates.iter().all(|gate| gate.passed),
            gates,
            usage,
            actual_real_model_calls: 0,
            redacted_case_results: cases.to_vec(),
            context: None,
        })
    }
}

pub struct ContextEvalRunner;

impl ContextEvalRunner {
    pub fn run(
        spec: &EvalRunSpec,
        cases: &[ContextEvalCaseResult],
    ) -> Result<EvalReport, EvalError> {
        let context = validate_context_spec(spec, cases)?;
        let usage = EvalUsage {
            cases: cases.len() as u64,
            input_tokens: cases.iter().map(|case| case.rust_tokens).sum(),
            ..EvalUsage::default()
        };
        let boolean_gate = |name: &str, passed: bool| GateResult {
            name: name.into(),
            passed,
            evidence: json!({"case_count": cases.len()}),
        };
        let valid_digest = |value: &str| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        };
        let gates = vec![
            boolean_gate("schema", cases.iter().all(|case| case.schema_valid)),
            boolean_gate(
                "cross_language_token",
                cases
                    .iter()
                    .all(|case| case.rust_tokens == case.typescript_tokens),
            ),
            boolean_gate(
                "determinism",
                cases.iter().all(|case| {
                    valid_digest(&case.first_digest)
                        && case.first_digest == case.repeated_digest
                        && case.first_digest == case.shuffled_digest
                }),
            ),
            boolean_gate("safety", cases.iter().all(|case| case.safety_passed)),
            GateResult {
                name: "budget".into(),
                passed: cases.iter().all(|case| case.budget_passed)
                    && usage.input_tokens <= spec.budget.max_input_tokens
                    && usage.output_tokens <= spec.budget.max_output_tokens,
                evidence: json!({
                    "actual_input_tokens": usage.input_tokens,
                    "actual_output_tokens": usage.output_tokens,
                    "max_input_tokens": spec.budget.max_input_tokens,
                    "max_output_tokens": spec.budget.max_output_tokens,
                }),
            },
            boolean_gate(
                "core_prompt",
                cases.iter().all(|case| case.core_prompt_passed),
            ),
            boolean_gate(
                "business_output",
                cases.iter().all(|case| case.business_output_passed),
            ),
            boolean_gate(
                "baseline_equivalence",
                spec.mode != EvalMode::GoldenBaseline || cases.iter().all(|case| case.equivalent),
            ),
        ];
        Ok(EvalReport {
            schema_version: "2".into(),
            candidate: spec.candidate.clone(),
            baseline: spec.baseline.clone(),
            case_set_version: spec.case_set_version.clone(),
            evaluator_version: spec.evaluator_version.clone(),
            mode: spec.mode,
            passed: gates.iter().all(|gate| gate.passed),
            gates,
            usage,
            actual_real_model_calls: 0,
            redacted_case_results: Vec::new(),
            context: Some(ContextEvalEvidence {
                schema_version: context.schema_version.clone(),
                policy: context.policy.clone(),
                tokenizer_profile: context.tokenizer_profile.clone(),
                node_results: cases.to_vec(),
                selection_diff: cases
                    .iter()
                    .map(|case| case.selection_diff.clone())
                    .collect(),
                budget_ledgers: cases
                    .iter()
                    .map(|case| case.budget_ledger.clone())
                    .collect(),
                tokenizer_metrics: cases
                    .iter()
                    .map(|case| case.tokenizer_metrics.clone())
                    .collect(),
            }),
        })
    }
}

fn validate_context_spec<'a>(
    spec: &'a EvalRunSpec,
    cases: &[ContextEvalCaseResult],
) -> Result<&'a ContextEvalConfig, EvalError> {
    let context = spec.context.as_ref().ok_or(EvalError::InvalidContextRun)?;
    let candidate_matches = match spec.candidate.definition_kind {
        EvalDefinitionKind::ContextPolicy => spec.candidate == context.policy,
        EvalDefinitionKind::TokenizerProfile => spec.candidate == context.tokenizer_profile,
        EvalDefinitionKind::Agent | EvalDefinitionKind::Prompt => false,
    };
    let valid_reference = |reference: &CandidateRef, expected: EvalDefinitionKind| {
        reference.definition_kind == expected
            && !reference.key.is_empty()
            && !reference.version.is_empty()
            && reference.digest.len() == 64
    };
    if !matches!(spec.mode, EvalMode::GoldenBaseline | EvalMode::ZeroCost)
        || spec.model_binding.is_some()
        || spec.budget.approved_real_calls
        || spec.budget.max_cost_micros != 0
        || context.schema_version != "1"
        || !candidate_matches
        || !valid_reference(&context.policy, EvalDefinitionKind::ContextPolicy)
        || !valid_reference(
            &context.tokenizer_profile,
            EvalDefinitionKind::TokenizerProfile,
        )
        || cases.is_empty()
        || cases.len() as u64 > spec.budget.max_cases
        || cases.iter().any(|case| {
            case.case_id.trim().is_empty()
                || case.node_key.trim().is_empty()
                || !case.selection_diff.is_array()
                || !case.budget_ledger.is_object()
                || !case.tokenizer_metrics.is_object()
        })
    {
        return Err(EvalError::InvalidContextRun);
    }
    Ok(context)
}

fn validate_zero_cost_spec(spec: &EvalRunSpec, cases: &[EvalCaseResult]) -> Result<(), EvalError> {
    if spec.mode == EvalMode::RealModel
        || spec.model_binding.is_some()
        || spec.budget.approved_real_calls
        || spec.budget.max_cost_micros != 0
    {
        return Err(EvalError::InvalidZeroCostRun);
    }
    if cases.is_empty() || cases.len() as u64 > spec.budget.max_cases {
        return Err(EvalError::InvalidCaseSet);
    }
    if spec.candidate.key.is_empty()
        || spec.candidate.version.is_empty()
        || spec.candidate.digest.len() != 64
        || spec.case_set_version.is_empty()
        || spec.evaluator_version.is_empty()
    {
        return Err(EvalError::InvalidCandidate);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetError {
    ApprovalRequired,
    InvalidBinding,
    FingerprintDrift,
    CaseLimit,
    InputTokenLimit,
    OutputTokenLimit,
    RetryLimit,
    CostLimit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BudgetTracker {
    budget: EvalBudget,
    binding: ModelBinding,
    usage: EvalUsage,
}

impl BudgetTracker {
    pub fn new(budget: EvalBudget, binding: ModelBinding) -> Result<Self, BudgetError> {
        if !budget.approved_real_calls {
            return Err(BudgetError::ApprovalRequired);
        }
        if binding.model_id.is_empty() || binding.behavior_fingerprint.len() != 64 {
            return Err(BudgetError::InvalidBinding);
        }
        Ok(Self {
            budget,
            binding,
            usage: EvalUsage::default(),
        })
    }

    pub fn authorize(&self, current_fingerprint: &str) -> Result<(), BudgetError> {
        if current_fingerprint != self.binding.behavior_fingerprint {
            return Err(BudgetError::FingerprintDrift);
        }
        if self.usage.cases >= self.budget.max_cases {
            return Err(BudgetError::CaseLimit);
        }
        if self.usage.input_tokens >= self.budget.max_input_tokens {
            return Err(BudgetError::InputTokenLimit);
        }
        if self.usage.output_tokens >= self.budget.max_output_tokens {
            return Err(BudgetError::OutputTokenLimit);
        }
        if self.usage.cost_micros >= self.budget.max_cost_micros {
            return Err(BudgetError::CostLimit);
        }
        Ok(())
    }

    pub fn record_attempt(
        &mut self,
        input_tokens: u64,
        output_tokens: u64,
        cost_micros: u64,
        retry: bool,
    ) -> Result<(), BudgetError> {
        if self.usage.cases.saturating_add(1) > self.budget.max_cases {
            return Err(BudgetError::CaseLimit);
        }
        if self.usage.input_tokens.saturating_add(input_tokens) > self.budget.max_input_tokens {
            return Err(BudgetError::InputTokenLimit);
        }
        if self.usage.output_tokens.saturating_add(output_tokens) > self.budget.max_output_tokens {
            return Err(BudgetError::OutputTokenLimit);
        }
        if self.usage.cost_micros.saturating_add(cost_micros) > self.budget.max_cost_micros {
            return Err(BudgetError::CostLimit);
        }
        if retry && self.usage.retries.saturating_add(1) > self.budget.max_retries {
            return Err(BudgetError::RetryLimit);
        }
        self.usage.cases += 1;
        self.usage.real_model_calls += 1;
        self.usage.input_tokens += input_tokens;
        self.usage.output_tokens += output_tokens;
        self.usage.cost_micros += cost_micros;
        if retry {
            self.usage.retries += 1;
        }
        Ok(())
    }

    pub fn usage(&self) -> &EvalUsage {
        &self.usage
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActivationEvidence {
    pub report_id: String,
    pub candidate: CandidateRef,
    pub report: EvalReport,
}

pub fn validate_activation(
    candidate: &CandidateRef,
    evidence: &ActivationEvidence,
) -> Result<(), EvalError> {
    let context_candidate = matches!(
        candidate.definition_kind,
        EvalDefinitionKind::ContextPolicy | EvalDefinitionKind::TokenizerProfile
    );
    let required_gates = if context_candidate {
        REQUIRED_CONTEXT_GATES.as_slice()
    } else {
        REQUIRED_GATES.as_slice()
    };
    let valid_mode = if context_candidate {
        matches!(
            evidence.report.mode,
            EvalMode::GoldenBaseline | EvalMode::ZeroCost | EvalMode::RealModel
        )
    } else {
        matches!(
            evidence.report.mode,
            EvalMode::GoldenBaseline | EvalMode::RealModel
        )
    };
    let valid_context = if context_candidate {
        evidence.report.context.as_ref().is_some_and(|context| {
            context.policy.definition_kind == EvalDefinitionKind::ContextPolicy
                && context.tokenizer_profile.definition_kind == EvalDefinitionKind::TokenizerProfile
                && match candidate.definition_kind {
                    EvalDefinitionKind::ContextPolicy => &context.policy == candidate,
                    EvalDefinitionKind::TokenizerProfile => &context.tokenizer_profile == candidate,
                    _ => false,
                }
        })
    } else {
        evidence.report.context.is_none()
    };
    if evidence.report_id.is_empty()
        || &evidence.candidate != candidate
        || evidence.report.candidate != *candidate
        || !evidence.report.passed
        || !valid_mode
        || !valid_context
        || evidence.report.gates.len() != required_gates.len()
        || !required_gates.iter().all(|required| {
            evidence
                .report
                .gates
                .iter()
                .any(|gate| gate.name == *required && gate.passed)
        })
    {
        return Err(EvalError::ActivationEvidenceRejected);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DefinitionStatus {
    Candidate,
    Active,
    Supported,
    Revoked,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RollbackPlan {
    pub new_active_version: String,
    pub previous_active_status: DefinitionStatus,
    pub preserve_bindings: bool,
    pub preserve_audit_and_reports: bool,
}

pub struct ManifestLifecycle;

impl ManifestLifecycle {
    pub fn can_start_new(status: DefinitionStatus) -> bool {
        status == DefinitionStatus::Active
    }

    pub fn can_continue_bound(status: DefinitionStatus) -> bool {
        matches!(
            status,
            DefinitionStatus::Active | DefinitionStatus::Supported
        )
    }

    pub fn rollback(
        current: (&str, DefinitionStatus),
        target: (&str, DefinitionStatus),
    ) -> Result<RollbackPlan, EvalError> {
        if current.1 != DefinitionStatus::Active
            || target.1 != DefinitionStatus::Supported
            || current.0 == target.0
        {
            return Err(EvalError::InvalidRollback);
        }
        Ok(RollbackPlan {
            new_active_version: target.0.into(),
            previous_active_status: DefinitionStatus::Supported,
            preserve_bindings: true,
            preserve_audit_and_reports: true,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvalError {
    InvalidZeroCostRun,
    InvalidCaseSet,
    InvalidCandidate,
    InvalidContextRun,
    ActivationEvidenceRejected,
    InvalidRollback,
}

impl fmt::Display for EvalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidZeroCostRun => "zero-cost evaluation cannot contain paid execution",
            Self::InvalidCaseSet => "evaluation case set is empty or exceeds the approved limit",
            Self::InvalidCandidate => "candidate evaluation identity is invalid",
            Self::InvalidContextRun => "context evaluation evidence is invalid",
            Self::ActivationEvidenceRejected => "candidate activation evidence is incomplete",
            Self::InvalidRollback => "rollback target must be a distinct supported version",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for EvalError {}
