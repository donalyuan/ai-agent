use novex_agent::{
    AuditedCallOwner, AuditedModelError, AuditedModelExecutor, AuditedModelRequest,
    AuditedModelResponse,
};
use novex_ai_core::{
    canonical_json, definition_digest, sha256_hex, validate_model_capabilities, DefinitionRegistry,
    DefinitionStatus,
};
use novex_eval::{
    CandidateRef, EvalBudget, EvalDefinitionKind, EvalMode, EvalRunSpec, ModelBinding,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::{fmt, sync::Arc};
use uuid::Uuid;

const PRODUCTION_CREW_CANDIDATE_ROLES: [&str; 9] = [
    "producer",
    "screenwriter",
    "character_critic",
    "director",
    "cinematographer",
    "performance_director",
    "sound_director",
    "editor",
    "qc",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionCrewEvalAuthorizationLimits {
    pub case_ids: Vec<String>,
    pub max_input_tokens_per_candidate: u64,
    pub max_retries_per_candidate: u64,
    pub max_cost_micros_per_candidate: u64,
}

impl ProductionCrewEvalAuthorizationLimits {
    pub fn conservative_v3() -> Self {
        Self {
            case_ids: vec!["full-crew-golden-happy-path-v3".into()],
            max_input_tokens_per_candidate: 16_384,
            max_retries_per_candidate: 0,
            max_cost_micros_per_candidate: 100_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProductionCrewEvalAuthorizationItem {
    pub role_key: String,
    pub candidate: CandidateRef,
    pub baseline: CandidateRef,
    pub case_ids: Vec<String>,
    pub model_binding: ModelBinding,
    pub budget: EvalBudget,
    pub blockers: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProductionCrewEvalAuthorizationTotals {
    pub eval_runs: u64,
    pub max_cases: u64,
    pub max_input_tokens: u64,
    pub max_output_tokens: u64,
    pub max_retries: u64,
    pub max_cost_micros: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProductionCrewEvalAuthorizationPlan {
    pub schema_version: String,
    pub authorization_state: String,
    pub authorization_ready: bool,
    pub authorization_digest: String,
    pub case_set_version: String,
    pub evaluator_version: String,
    pub model_binding: ModelBinding,
    pub model_capabilities: Value,
    pub items: Vec<ProductionCrewEvalAuthorizationItem>,
    pub zero_cost_context_candidates: Vec<CandidateRef>,
    pub totals: ProductionCrewEvalAuthorizationTotals,
    pub blockers: Vec<String>,
    pub external_effects: Value,
}

impl ProductionCrewEvalAuthorizationPlan {
    /// 只有操作者确认精确 digest 且部署能力完整时，才生成可持久化的 real_model EvalRunSpec。
    pub fn approved_specs(
        &self,
        explicitly_confirmed: bool,
        confirmed_digest: &str,
    ) -> Result<Vec<EvalRunSpec>, ProductionCrewEvalAuthorizationError> {
        if !explicitly_confirmed || confirmed_digest != self.authorization_digest {
            return Err(ProductionCrewEvalAuthorizationError::ApprovalRequired);
        }
        if !self.authorization_ready {
            return Err(ProductionCrewEvalAuthorizationError::CapabilityBlocked(
                self.blockers.join("; "),
            ));
        }
        Ok(self
            .items
            .iter()
            .map(|item| {
                let mut budget = item.budget.clone();
                budget.approved_real_calls = true;
                EvalRunSpec {
                    candidate: item.candidate.clone(),
                    baseline: Some(item.baseline.clone()),
                    case_set_version: self.case_set_version.clone(),
                    evaluator_version: self.evaluator_version.clone(),
                    mode: EvalMode::RealModel,
                    context: None,
                    model_binding: Some(item.model_binding.clone()),
                    budget,
                }
            })
            .collect())
    }
}

/// 生成 Full Crew v3 真实评测授权清单；只解析本地定义和模型 binding，不创建 EvalRun。
pub async fn build_production_crew_eval_authorization_plan(
    definitions: &DefinitionRegistry,
    audited_executor: &AuditedModelExecutor,
    model_id: Uuid,
    limits: ProductionCrewEvalAuthorizationLimits,
) -> Result<ProductionCrewEvalAuthorizationPlan, ProductionCrewEvalAuthorizationError> {
    if limits.case_ids.is_empty()
        || limits.case_ids.iter().any(|case| case.trim().is_empty())
        || limits.max_input_tokens_per_candidate == 0
        || limits.max_cost_micros_per_candidate == 0
    {
        return Err(ProductionCrewEvalAuthorizationError::InvalidPlan(
            "case、input token 和成本上限必须为正值".into(),
        ));
    }
    let evidence = audited_executor
        .build_binding_evidence("production.screenwriter", "3.0.0", model_id)
        .await
        .map_err(|error| ProductionCrewEvalAuthorizationError::ModelBinding(error.to_string()))?;
    let model_binding = ModelBinding {
        model_id: evidence.binding.model_id.to_string(),
        behavior_fingerprint: evidence.binding.behavior_fingerprint.clone(),
    };
    let mut items = Vec::new();
    let mut context_candidates = Vec::new();
    let mut plan_blockers = Vec::new();

    for role in PRODUCTION_CREW_CANDIDATE_ROLES {
        let agent_key = format!("production.{role}");
        let candidate_agent = definitions
            .agent(&agent_key, "3.0.0")
            .map_err(|error| invalid_definition(role, error.to_string()))?;
        if candidate_agent.status != DefinitionStatus::Candidate {
            return Err(invalid_definition(
                role,
                "3.0.0 AgentDefinition 不是 candidate".into(),
            ));
        }
        let baseline_agent = definitions
            .active_agent(&agent_key)
            .map_err(|error| invalid_definition(role, error.to_string()))?;
        let node = candidate_agent.nodes.values().next().ok_or_else(|| {
            invalid_definition(role, "candidate AgentDefinition 没有执行节点".into())
        })?;
        if candidate_agent.nodes.len() != 1 {
            return Err(invalid_definition(
                role,
                "production candidate 必须只有一个固定执行节点".into(),
            ));
        }
        let candidate_prompt = definitions
            .prompts()
            .iter()
            .find(|prompt| prompt.prompt_key == node.key && prompt.version == node.version)
            .ok_or_else(|| invalid_definition(role, "candidate PromptDefinition 不存在".into()))?;
        if candidate_prompt.status != DefinitionStatus::Candidate {
            return Err(invalid_definition(
                role,
                "PromptDefinition 不是 candidate".into(),
            ));
        }
        let baseline_prompt = definitions
            .prompts()
            .iter()
            .find(|prompt| {
                prompt.prompt_key == node.key && prompt.status == DefinitionStatus::Active
            })
            .ok_or_else(|| invalid_definition(role, "active PromptDefinition 不存在".into()))?;
        let context_ref = node.context_policy.as_ref().ok_or_else(|| {
            invalid_definition(role, "candidate Context Policy 引用不存在".into())
        })?;
        let candidate_context = definitions
            .context_policy(&context_ref.key, &context_ref.version)
            .map_err(|error| invalid_definition(role, error.to_string()))?;
        if candidate_context.status != DefinitionStatus::Candidate {
            return Err(invalid_definition(
                role,
                "ContextPolicyDefinition 不是 candidate".into(),
            ));
        }
        context_candidates.push(CandidateRef {
            definition_kind: EvalDefinitionKind::ContextPolicy,
            key: context_ref.key.clone(),
            version: context_ref.version.clone(),
            digest: definition_digest(candidate_context)
                .map_err(|error| invalid_definition(role, error.to_string()))?,
        });

        let mut blockers = Vec::new();
        if let Err(error) =
            validate_model_capabilities(&candidate_agent.model_requirements, &evidence.capabilities)
        {
            blockers.push(error.to_string());
        }
        let prompt_output_limit = candidate_prompt.max_output_tokens.ok_or_else(|| {
            invalid_definition(role, "candidate Prompt 缺少 max_output_tokens".into())
        })? as u64;
        if prompt_output_limit > evidence.max_output_tokens {
            blockers.push(format!(
                "模型 max_output_tokens={} 低于 Prompt 要求 {}",
                evidence.max_output_tokens, prompt_output_limit
            ));
        }
        plan_blockers.extend(blockers.iter().map(|blocker| format!("{role}: {blocker}")));
        let budget = EvalBudget {
            approved_real_calls: false,
            max_cases: limits.case_ids.len() as u64,
            max_input_tokens: limits.max_input_tokens_per_candidate,
            max_output_tokens: prompt_output_limit,
            max_retries: limits.max_retries_per_candidate,
            max_cost_micros: limits.max_cost_micros_per_candidate,
        };
        for (candidate, baseline) in [
            (
                CandidateRef {
                    definition_kind: EvalDefinitionKind::Agent,
                    key: candidate_agent.agent_key.clone(),
                    version: candidate_agent.version.clone(),
                    digest: definition_digest(candidate_agent)
                        .map_err(|error| invalid_definition(role, error.to_string()))?,
                },
                CandidateRef {
                    definition_kind: EvalDefinitionKind::Agent,
                    key: baseline_agent.agent_key.clone(),
                    version: baseline_agent.version.clone(),
                    digest: definition_digest(baseline_agent)
                        .map_err(|error| invalid_definition(role, error.to_string()))?,
                },
            ),
            (
                CandidateRef {
                    definition_kind: EvalDefinitionKind::Prompt,
                    key: candidate_prompt.prompt_key.clone(),
                    version: candidate_prompt.version.clone(),
                    digest: definition_digest(candidate_prompt)
                        .map_err(|error| invalid_definition(role, error.to_string()))?,
                },
                CandidateRef {
                    definition_kind: EvalDefinitionKind::Prompt,
                    key: baseline_prompt.prompt_key.clone(),
                    version: baseline_prompt.version.clone(),
                    digest: definition_digest(baseline_prompt)
                        .map_err(|error| invalid_definition(role, error.to_string()))?,
                },
            ),
        ] {
            items.push(ProductionCrewEvalAuthorizationItem {
                role_key: role.into(),
                candidate,
                baseline,
                case_ids: limits.case_ids.clone(),
                model_binding: model_binding.clone(),
                budget: budget.clone(),
                blockers: blockers.clone(),
            });
        }
    }

    context_candidates.sort_by(|left, right| left.key.cmp(&right.key));
    context_candidates.dedup_by(|left, right| left == right);
    let totals = ProductionCrewEvalAuthorizationTotals {
        eval_runs: items.len() as u64,
        max_cases: items.iter().map(|item| item.budget.max_cases).sum(),
        max_input_tokens: items.iter().map(|item| item.budget.max_input_tokens).sum(),
        max_output_tokens: items.iter().map(|item| item.budget.max_output_tokens).sum(),
        max_retries: items.iter().map(|item| item.budget.max_retries).sum(),
        max_cost_micros: items.iter().map(|item| item.budget.max_cost_micros).sum(),
    };
    let digest_payload = json!({
        "schema_version": "1",
        "case_set_version": "durable-production-crew-real-v3@1",
        "evaluator_version": "novex-production-crew-eval@1",
        "model_binding": model_binding,
        "items": items,
        "zero_cost_context_candidates": context_candidates,
        "totals": totals,
        "blockers": plan_blockers,
    });
    let authorization_digest = sha256_hex(canonical_json(&digest_payload).as_bytes());
    Ok(ProductionCrewEvalAuthorizationPlan {
        schema_version: "1".into(),
        authorization_state: "awaiting_explicit_user_confirmation".into(),
        authorization_ready: plan_blockers.is_empty(),
        authorization_digest,
        case_set_version: "durable-production-crew-real-v3@1".into(),
        evaluator_version: "novex-production-crew-eval@1".into(),
        model_binding,
        model_capabilities: serde_json::to_value(evidence.capabilities).map_err(|error| {
            ProductionCrewEvalAuthorizationError::InvalidPlan(error.to_string())
        })?,
        items,
        zero_cost_context_candidates: context_candidates,
        totals,
        blockers: plan_blockers,
        external_effects: json!({
            "real_model_calls": 0,
            "eval_runs_created": 0,
            "candidate_activations": 0,
        }),
    })
}

fn invalid_definition(role: &str, reason: String) -> ProductionCrewEvalAuthorizationError {
    ProductionCrewEvalAuthorizationError::InvalidDefinition {
        role: role.into(),
        reason,
    }
}

#[derive(Debug)]
pub enum ProductionCrewEvalAuthorizationError {
    InvalidPlan(String),
    InvalidDefinition { role: String, reason: String },
    ModelBinding(String),
    ApprovalRequired,
    CapabilityBlocked(String),
}

impl fmt::Display for ProductionCrewEvalAuthorizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPlan(reason) => write!(formatter, "评测授权计划无效: {reason}"),
            Self::InvalidDefinition { role, reason } => {
                write!(formatter, "{role} candidate 定义无效: {reason}")
            }
            Self::ModelBinding(reason) => write!(formatter, "评测模型 binding 无效: {reason}"),
            Self::ApprovalRequired => formatter.write_str("真实评测需要确认精确授权 digest"),
            Self::CapabilityBlocked(reason) => write!(formatter, "真实评测能力阻断: {reason}"),
        }
    }
}

impl std::error::Error for ProductionCrewEvalAuthorizationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvalBudgetCharge {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_micros: u64,
    pub retry: bool,
}

pub struct RealEvalRunner {
    executor: Arc<AuditedModelExecutor>,
}

impl RealEvalRunner {
    pub fn new(executor: Arc<AuditedModelExecutor>) -> Self {
        Self { executor }
    }

    /// Executes only through the audited path; the audit repository atomically reserves budget.
    pub async fn execute_attempt(
        &self,
        eval_run_id: Uuid,
        mut request: AuditedModelRequest,
        charge: EvalBudgetCharge,
    ) -> Result<AuditedModelResponse, AuditedModelError> {
        request.owner = AuditedCallOwner::EvalRun(eval_run_id);
        let mut parameters = match request.parameters {
            Value::Object(parameters) => parameters,
            _ => {
                return Err(AuditedModelError::Compile(
                    "eval parameters must be a JSON object".into(),
                ))
            }
        };
        parameters.insert(
            "eval_budget_charge".into(),
            json!({
                "input_tokens": charge.input_tokens,
                "output_tokens": charge.output_tokens,
                "cost_micros": charge.cost_micros,
                "retry": charge.retry,
            }),
        );
        request.parameters = Value::Object(parameters);
        self.executor.execute(request).await
    }
}
