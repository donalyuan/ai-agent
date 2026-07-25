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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalMode {
    GoldenBaseline,
    ZeroCost,
    RealModel,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CandidateRef {
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
    pub model_binding: Option<ModelBinding>,
    pub budget: EvalBudget,
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
        })
    }
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
    if evidence.report_id.is_empty()
        || &evidence.candidate != candidate
        || evidence.report.candidate != *candidate
        || !evidence.report.passed
        || !matches!(
            evidence.report.mode,
            EvalMode::GoldenBaseline | EvalMode::RealModel
        )
        || evidence.report.gates.len() != REQUIRED_GATES.len()
        || !REQUIRED_GATES.iter().all(|required| {
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
    ActivationEvidenceRejected,
    InvalidRollback,
}

impl fmt::Display for EvalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidZeroCostRun => "zero-cost evaluation cannot contain paid execution",
            Self::InvalidCaseSet => "evaluation case set is empty or exceeds the approved limit",
            Self::InvalidCandidate => "candidate evaluation identity is invalid",
            Self::ActivationEvidenceRejected => "candidate activation evidence is incomplete",
            Self::InvalidRollback => "rollback target must be a distinct supported version",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for EvalError {}
