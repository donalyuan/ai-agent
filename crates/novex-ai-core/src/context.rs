use crate::{
    canonical_json, sha256_hex, AssetReference, DefinitionStatus, ExecutorOwner, TrustLevel,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};
use tiktoken_rs::{cl100k_base, o200k_base, CoreBPE};

pub const CONTEXT_SCHEMA_VERSION: &str = "2";
pub const ENCODING_CONTRACT_V1_DIGEST: &str =
    "cfcf757abd2fceb98ca75ea57e0153123447dab26375938154c909640505f8bf";
const TOKENIZER_CACHE_LIMIT: usize = 8;
static EXACT_TOKENIZER_CACHE: OnceLock<Mutex<BTreeMap<String, Arc<CoreBPE>>>> = OnceLock::new();

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextPriority {
    P0,
    P1,
    P2,
    P3,
    P4,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContextPayload {
    Text { text: String },
    Message { message: LogicalMessage },
    Asset { asset: AssetReference },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LogicalMessage {
    pub role: String,
    pub content: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextCandidate {
    pub candidate_id: String,
    pub source_kind: String,
    pub source_id: String,
    pub source_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fact_key: Option<String>,
    pub trust: TrustLevel,
    pub priority: ContextPriority,
    pub required: bool,
    /// Stable presentation position after budget selection; independent from retention priority.
    pub render_order: u32,
    pub observed_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<String>,
    #[serde(default)]
    pub supersedes: Vec<String>,
    pub content_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atomic_group_id: Option<String>,
    pub payload: ContextPayload,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextAtomicGroup {
    pub group_id: String,
    pub member_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FramingRules {
    pub per_message_tokens: u64,
    pub per_tool_tokens: u64,
    pub request_tokens: u64,
    pub reply_priming_tokens: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum TokenizerMode {
    Exact {
        encoding: String,
        asset_digest: String,
    },
    Conservative {
        algorithm: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TokenizerProfile {
    pub profile_key: String,
    pub version: String,
    pub status: DefinitionStatus,
    pub implementation_version: String,
    pub mode: TokenizerMode,
    pub applicable_protocols: Vec<String>,
    pub applicable_model_families: Vec<String>,
    pub framing: FramingRules,
    pub safety_reserve_tokens: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextPolicyDefinition {
    pub policy_key: String,
    pub version: String,
    pub status: DefinitionStatus,
    pub executor_owners: Vec<ExecutorOwner>,
    pub allowed_sources: Vec<String>,
    #[serde(default)]
    pub required_sources: Vec<String>,
    pub stable_sort: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedPromptEnvelope {
    pub system: String,
    pub user_template_fixed: String,
    pub tool_schema: Option<Value>,
    pub output_schema: Option<Value>,
    pub protocol_envelope_tokens: u64,
    pub max_output_tokens: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextCompileRequest {
    pub schema_version: String,
    pub owner: ExecutorOwner,
    pub owner_id: String,
    pub node_key: String,
    pub compiled_at: String,
    pub model_context_window: u64,
    pub policy: ContextPolicyDefinition,
    pub tokenizer_profile: TokenizerProfile,
    pub prepared_prompt: PreparedPromptEnvelope,
    pub candidates: Vec<ContextCandidate>,
    #[serde(default)]
    pub atomic_groups: Vec<ContextAtomicGroup>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetLedger {
    pub model_context_window: u64,
    pub system_prompt_tokens: u64,
    pub user_template_fixed_tokens: u64,
    pub tool_schema_tokens: u64,
    pub output_schema_tokens: u64,
    pub protocol_envelope_tokens: u64,
    pub max_output_tokens: u64,
    pub safety_reserve_tokens: u64,
    pub dynamic_context_budget: u64,
    pub selected_context_tokens: u64,
    pub final_input_tokens: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextDecisionCode {
    Selected,
    Expired,
    Superseded,
    DuplicateIdentity,
    DuplicateContent,
    AtomicGroupExcluded,
    BudgetExcluded,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextDecision {
    pub candidate_id: String,
    pub source_kind: String,
    pub source_id: String,
    pub source_version: String,
    pub trust: TrustLevel,
    pub priority: ContextPriority,
    pub required: bool,
    pub render_order: u32,
    pub content_hash: String,
    pub token_count: u64,
    pub decision: ContextDecisionCode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_payload: Option<ContextPayload>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LogicalModelInput {
    pub system: String,
    pub messages: Vec<LogicalMessage>,
    pub tool_schema: Option<Value>,
    pub output_schema: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledContext {
    pub schema_version: String,
    pub owner: ExecutorOwner,
    pub owner_id: String,
    pub node_key: String,
    pub compiled_at: String,
    pub policy_key: String,
    pub policy_version: String,
    pub tokenizer_profile_key: String,
    pub tokenizer_profile_version: String,
    pub tokenizer_mode: String,
    pub budget: BudgetLedger,
    pub decisions: Vec<ContextDecision>,
    pub selected_order: Vec<String>,
    pub logical_input: LogicalModelInput,
    pub digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextSnapshot {
    pub schema_version: String,
    pub owner: ExecutorOwner,
    pub owner_id: String,
    pub node_key: String,
    pub compiled_at: String,
    pub policy_key: String,
    pub policy_version: String,
    pub tokenizer_profile_key: String,
    pub tokenizer_profile_version: String,
    pub tokenizer_mode: String,
    pub budget: BudgetLedger,
    pub decisions: Vec<ContextDecision>,
    pub selected_order: Vec<String>,
    pub logical_input: LogicalModelInput,
    pub digest: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompileFailureStage {
    Schema,
    Eligibility,
    Conflict,
    Tokenizer,
    Budget,
    Finalize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextCompileAttempt {
    pub schema_version: String,
    pub owner: ExecutorOwner,
    pub owner_id: String,
    pub node_key: String,
    pub compiled_at: String,
    pub stage: CompileFailureStage,
    pub code: String,
    pub budget: Option<BudgetLedger>,
    pub decisions: Vec<ContextDecision>,
    pub digest: String,
}

impl ContextCompileAttempt {
    /// 构造不依赖 PreparedPrompt 的最小失败证据，供 binding/tokenizer 前置门禁复用。
    pub fn failure(
        owner: ExecutorOwner,
        owner_id: impl Into<String>,
        node_key: impl Into<String>,
        compiled_at: impl Into<String>,
        stage: CompileFailureStage,
        code: impl Into<String>,
    ) -> Self {
        let mut attempt = Self {
            schema_version: CONTEXT_SCHEMA_VERSION.into(),
            owner,
            owner_id: owner_id.into(),
            node_key: node_key.into(),
            compiled_at: compiled_at.into(),
            stage,
            code: code.into(),
            budget: None,
            decisions: Vec::new(),
            digest: String::new(),
        };
        attempt.digest = digest_without_digest(&attempt);
        attempt
    }
}

#[derive(Clone)]
pub enum ProfileTokenizer {
    Exact {
        profile: TokenizerProfile,
        bpe: Arc<CoreBPE>,
    },
    Conservative {
        profile: TokenizerProfile,
    },
}

impl fmt::Debug for ProfileTokenizer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProfileTokenizer")
            .field("profile", self.profile())
            .finish()
    }
}

impl ProfileTokenizer {
    pub fn from_profile(profile: TokenizerProfile) -> Result<Self, ContextCompileError> {
        if profile.status == DefinitionStatus::Revoked
            || profile.applicable_protocols.is_empty()
            || profile.safety_reserve_tokens > u32::MAX as u64
        {
            return Err(ContextCompileError::new(
                CompileFailureStage::Tokenizer,
                "tokenizer_profile_unavailable",
            ));
        }
        match &profile.mode {
            TokenizerMode::Exact {
                encoding,
                asset_digest,
            } if asset_digest == ENCODING_CONTRACT_V1_DIGEST => {
                let key = format!(
                    "{}:{}:{asset_digest}:{encoding}",
                    profile.profile_key, profile.version
                );
                let cache = EXACT_TOKENIZER_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
                let mut cache = cache.lock().map_err(|_| {
                    ContextCompileError::new(
                        CompileFailureStage::Tokenizer,
                        "tokenizer_profile_unavailable",
                    )
                })?;
                let bpe = if let Some(existing) = cache.get(&key) {
                    existing.clone()
                } else {
                    let built = match encoding.as_str() {
                        "cl100k_base" => cl100k_base(),
                        "o200k_base" => o200k_base(),
                        _ => {
                            return Err(ContextCompileError::new(
                                CompileFailureStage::Tokenizer,
                                "tokenizer_profile_unavailable",
                            ))
                        }
                    }
                    .map_err(|_| {
                        ContextCompileError::new(
                            CompileFailureStage::Tokenizer,
                            "tokenizer_profile_unavailable",
                        )
                    })?;
                    if cache.len() >= TOKENIZER_CACHE_LIMIT {
                        if let Some(oldest) = cache.keys().next().cloned() {
                            cache.remove(&oldest);
                        }
                    }
                    let built = Arc::new(built);
                    cache.insert(key, built.clone());
                    built
                };
                Ok(Self::Exact { profile, bpe })
            }
            TokenizerMode::Conservative { algorithm } if algorithm == "utf8-byte-upper-bound@1" => {
                Ok(Self::Conservative { profile })
            }
            _ => Err(ContextCompileError::new(
                CompileFailureStage::Tokenizer,
                "tokenizer_profile_unavailable",
            )),
        }
    }

    pub fn profile(&self) -> &TokenizerProfile {
        match self {
            Self::Exact { profile, .. } | Self::Conservative { profile } => profile,
        }
    }

    pub fn count_text(&self, text: &str) -> u64 {
        match self {
            Self::Exact { bpe, .. } => bpe.encode_with_special_tokens(text).len() as u64,
            Self::Conservative { .. } => text.len() as u64,
        }
    }

    pub fn count_json(&self, value: &Value) -> u64 {
        self.count_text(&canonical_json(value))
    }

    pub fn cache_size() -> usize {
        EXACT_TOKENIZER_CACHE
            .get()
            .and_then(|cache| cache.lock().ok().map(|items| items.len()))
            .unwrap_or(0)
    }

    fn count_payload(&self, payload: &ContextPayload) -> u64 {
        match payload {
            ContextPayload::Text { text } => self.count_text(text),
            ContextPayload::Message { message } => self.count_message(message, true),
            ContextPayload::Asset { asset } => self.count_text(&canonical_json(
                &serde_json::to_value(asset).expect("asset serialization"),
            )),
        }
    }

    fn count_message(&self, message: &LogicalMessage, include_framing: bool) -> u64 {
        let content = match &message.content {
            Value::String(value) => self.count_text(value),
            value => self.count_json(value),
        };
        self.count_text(&message.role)
            .saturating_add(content)
            .saturating_add(
                message
                    .thinking
                    .as_deref()
                    .map_or(0, |value| self.count_text(value)),
            )
            .saturating_add(
                message
                    .tool_call_id
                    .as_deref()
                    .map_or(0, |value| self.count_text(value)),
            )
            .saturating_add(if include_framing {
                self.profile().framing.per_message_tokens
            } else {
                0
            })
    }
}

pub struct ContextCompiler;

impl ContextCompiler {
    pub fn compile(request: ContextCompileRequest) -> Result<CompiledContext, ContextCompileError> {
        validate_request(&request)?;
        validate_required_sources(&request)?;
        let tokenizer = ProfileTokenizer::from_profile(request.tokenizer_profile.clone())?;
        let mut ledger = fixed_budget(&request, &tokenizer)?;
        let mut candidates = request.candidates.clone();
        candidates.sort_by(stable_candidate_order);
        let groups = validate_groups(&candidates, &request.atomic_groups)?;
        let mut decisions = Vec::with_capacity(candidates.len());
        let mut eligible = Vec::new();
        let mut identities = BTreeSet::new();
        let mut hashes = BTreeSet::new();
        let superseded = candidates
            .iter()
            .flat_map(|item| item.supersedes.iter().cloned())
            .collect::<BTreeSet<_>>();
        for candidate in candidates {
            let token_count = tokenizer.count_payload(&candidate.payload);
            let decision = if candidate
                .valid_until
                .as_deref()
                .is_some_and(|until| until < request.compiled_at.as_str())
            {
                Some(ContextDecisionCode::Expired)
            } else if superseded.contains(&candidate.candidate_id) {
                Some(ContextDecisionCode::Superseded)
            } else if !identities.insert((
                candidate.source_kind.clone(),
                candidate.source_id.clone(),
                candidate.source_version.clone(),
                candidate.candidate_id.clone(),
            )) {
                Some(ContextDecisionCode::DuplicateIdentity)
            } else if !hashes.insert(candidate.content_hash.clone()) {
                Some(ContextDecisionCode::DuplicateContent)
            } else {
                None
            };
            if let Some(code) = decision {
                if candidate.required
                    && matches!(
                        code,
                        ContextDecisionCode::Expired | ContextDecisionCode::Superseded
                    )
                {
                    return Err(ContextCompileError::new(
                        CompileFailureStage::Eligibility,
                        "required_context_unavailable",
                    ));
                }
                decisions.push(minimal_decision(&candidate, token_count, code, None));
            } else {
                eligible.push((candidate, token_count));
            }
        }
        exclude_incomplete_groups(&mut eligible, &mut decisions, &groups)?;
        detect_confirmed_fact_conflicts(&eligible)?;

        let mut selected_ids = BTreeSet::new();
        let mut selected_tokens = 0u64;
        for (candidate, token_count) in &eligible {
            if selected_ids.contains(&candidate.candidate_id) {
                continue;
            }
            let group_members = candidate
                .atomic_group_id
                .as_ref()
                .and_then(|id| groups.get(id));
            let group = if let Some(member_ids) = group_members {
                eligible
                    .iter()
                    .filter(|(item, _)| member_ids.contains(&item.candidate_id))
                    .map(|(item, tokens)| (item, *tokens))
                    .collect::<Vec<_>>()
            } else {
                vec![(candidate, *token_count)]
            };
            let group_tokens = group
                .iter()
                .map(|(_, tokens)| *tokens)
                .fold(0u64, u64::saturating_add);
            let group_required = group
                .iter()
                .any(|(item, _)| item.required || item.priority == ContextPriority::P0);
            if selected_tokens.saturating_add(group_tokens) <= ledger.dynamic_context_budget {
                selected_tokens += group_tokens;
                for (item, _) in group {
                    selected_ids.insert(item.candidate_id.clone());
                }
            } else if group_required {
                return Err(ContextCompileError::new(
                    CompileFailureStage::Budget,
                    "context_budget_exceeded",
                ));
            }
        }

        let mut selected = eligible
            .iter()
            .filter(|(item, _)| selected_ids.contains(&item.candidate_id))
            .collect::<Vec<_>>();
        selected.sort_by(|(left, _), (right, _)| render_candidate_order(left, right));
        for (candidate, token_count) in &eligible {
            let chosen = selected_ids.contains(&candidate.candidate_id);
            decisions.push(minimal_decision(
                candidate,
                *token_count,
                if chosen {
                    ContextDecisionCode::Selected
                } else {
                    ContextDecisionCode::BudgetExcluded
                },
                chosen.then_some(candidate.payload.clone()),
            ));
        }
        decisions.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
        let selected_order = selected
            .iter()
            .map(|(item, _)| item.candidate_id.clone())
            .collect::<Vec<_>>();
        let messages = selected
            .iter()
            .map(|(candidate, _)| payload_message(&candidate.payload))
            .collect();
        let logical_input = LogicalModelInput {
            system: request.prepared_prompt.system.clone(),
            messages,
            tool_schema: request.prepared_prompt.tool_schema.clone(),
            output_schema: request.prepared_prompt.output_schema.clone(),
        };
        ledger.selected_context_tokens = selected_tokens;
        let mut compiled = CompiledContext {
            schema_version: CONTEXT_SCHEMA_VERSION.into(),
            owner: request.owner,
            owner_id: request.owner_id,
            node_key: request.node_key,
            compiled_at: request.compiled_at,
            policy_key: request.policy.policy_key,
            policy_version: request.policy.version,
            tokenizer_profile_key: tokenizer.profile().profile_key.clone(),
            tokenizer_profile_version: tokenizer.profile().version.clone(),
            tokenizer_mode: match tokenizer.profile().mode {
                TokenizerMode::Exact { .. } => "exact",
                TokenizerMode::Conservative { .. } => "conservative",
            }
            .into(),
            budget: ledger,
            decisions,
            selected_order,
            logical_input,
            digest: String::new(),
        };
        compiled.digest = digest_without_digest(&compiled);
        Ok(compiled)
    }

    pub fn finalize(
        compiled: &CompiledContext,
        tokenizer_profile: &TokenizerProfile,
        logical_input: LogicalModelInput,
    ) -> Result<ContextSnapshot, ContextCompileError> {
        if tokenizer_profile.profile_key != compiled.tokenizer_profile_key
            || tokenizer_profile.version != compiled.tokenizer_profile_version
            || logical_input.system != compiled.logical_input.system
            || logical_input.tool_schema != compiled.logical_input.tool_schema
            || logical_input.output_schema != compiled.logical_input.output_schema
        {
            return Err(ContextCompileError::new(
                CompileFailureStage::Finalize,
                "context_finalize_mismatch",
            ));
        }
        let tokenizer = ProfileTokenizer::from_profile(tokenizer_profile.clone())?;
        let mut budget = compiled.budget.clone();
        budget.final_input_tokens =
            count_final_logical_input(&logical_input, &tokenizer, budget.protocol_envelope_tokens);
        if budget
            .final_input_tokens
            .saturating_add(budget.max_output_tokens)
            .saturating_add(budget.safety_reserve_tokens)
            > budget.model_context_window
        {
            return Err(ContextCompileError::new(
                CompileFailureStage::Finalize,
                "context_budget_exceeded",
            ));
        }
        let mut snapshot = ContextSnapshot {
            schema_version: compiled.schema_version.clone(),
            owner: compiled.owner,
            owner_id: compiled.owner_id.clone(),
            node_key: compiled.node_key.clone(),
            compiled_at: compiled.compiled_at.clone(),
            policy_key: compiled.policy_key.clone(),
            policy_version: compiled.policy_version.clone(),
            tokenizer_profile_key: compiled.tokenizer_profile_key.clone(),
            tokenizer_profile_version: compiled.tokenizer_profile_version.clone(),
            tokenizer_mode: compiled.tokenizer_mode.clone(),
            budget,
            decisions: compiled.decisions.clone(),
            selected_order: compiled.selected_order.clone(),
            logical_input,
            digest: String::new(),
        };
        snapshot.digest = digest_without_digest(&snapshot);
        Ok(snapshot)
    }
}

fn validate_required_sources(request: &ContextCompileRequest) -> Result<(), ContextCompileError> {
    let present_sources = request
        .candidates
        .iter()
        .map(|candidate| candidate.source_kind.as_str())
        .collect::<BTreeSet<_>>();
    if request
        .policy
        .required_sources
        .iter()
        .any(|source| !present_sources.contains(source.as_str()))
    {
        return Err(ContextCompileError::new(
            CompileFailureStage::Eligibility,
            "required_context_unavailable",
        ));
    }
    Ok(())
}

fn count_final_logical_input(
    logical_input: &LogicalModelInput,
    tokenizer: &ProfileTokenizer,
    protocol_envelope_tokens: u64,
) -> u64 {
    let messages = logical_input.messages.iter().fold(0u64, |total, message| {
        total.saturating_add(tokenizer.count_message(message, false))
    });
    let tool_schema = logical_input
        .tool_schema
        .as_ref()
        .map_or(0, |value| tokenizer.count_json(value));
    let output_schema = logical_input
        .output_schema
        .as_ref()
        .map_or(0, |value| tokenizer.count_json(value));
    tokenizer
        .count_text(&logical_input.system)
        .saturating_add(messages)
        .saturating_add(tool_schema)
        .saturating_add(output_schema)
        .saturating_add(protocol_envelope_tokens)
}

fn validate_request(request: &ContextCompileRequest) -> Result<(), ContextCompileError> {
    if request.schema_version != CONTEXT_SCHEMA_VERSION
        || request.owner_id.trim().is_empty()
        || request.node_key.trim().is_empty()
        || !valid_timestamp(&request.compiled_at)
        || request.model_context_window == 0
        || request.prepared_prompt.max_output_tokens == 0
        || !request.policy.executor_owners.contains(&request.owner)
        || request.policy.stable_sort
            != [
                "priority",
                "source_kind",
                "source_id",
                "source_version",
                "candidate_id",
            ]
        || !matches!(
            request.policy.status,
            DefinitionStatus::Active | DefinitionStatus::Supported
        )
    {
        return Err(ContextCompileError::new(
            CompileFailureStage::Schema,
            "context_schema_invalid",
        ));
    }
    let allowed = request
        .policy
        .allowed_sources
        .iter()
        .collect::<BTreeSet<_>>();
    let required_sources = request
        .policy
        .required_sources
        .iter()
        .collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.candidate_id.trim().is_empty()
            || candidate.source_kind.trim().is_empty()
            || candidate.source_id.trim().is_empty()
            || candidate.source_version.trim().is_empty()
            || !valid_timestamp(&candidate.observed_at)
            || candidate
                .valid_until
                .as_deref()
                .is_some_and(|value| !valid_timestamp(value))
            || candidate.content_hash.len() != 64
            || !candidate
                .content_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            || !ids.insert(&candidate.candidate_id)
            || !allowed.contains(&candidate.source_kind)
            || required_sources.contains(&candidate.source_kind) && !candidate.required
            || !valid_payload(&candidate.payload)
        {
            return Err(ContextCompileError::new(
                CompileFailureStage::Schema,
                "context_schema_invalid",
            ));
        }
        let actual = sha256_hex(payload_canonical(&candidate.payload).as_bytes());
        if actual != candidate.content_hash {
            return Err(ContextCompileError::new(
                CompileFailureStage::Schema,
                "context_content_hash_mismatch",
            ));
        }
    }
    for candidate in &request.candidates {
        let mut supersedes = BTreeSet::new();
        if candidate
            .supersedes
            .iter()
            .any(|id| id == &candidate.candidate_id || !ids.contains(id) || !supersedes.insert(id))
        {
            return Err(ContextCompileError::new(
                CompileFailureStage::Schema,
                "context_schema_invalid",
            ));
        }
    }
    Ok(())
}

fn valid_timestamp(value: &str) -> bool {
    value.ends_with('Z')
        && DateTime::parse_from_rfc3339(value)
            .map(|timestamp| {
                timestamp
                    .with_timezone(&Utc)
                    .timestamp_nanos_opt()
                    .is_some()
            })
            .unwrap_or(false)
}

fn valid_payload(payload: &ContextPayload) -> bool {
    match payload {
        ContextPayload::Text { text } => !text.is_empty(),
        ContextPayload::Message { message } => {
            matches!(message.role.as_str(), "user" | "assistant" | "tool")
                && !message.content.is_null()
                && message
                    .tool_call_id
                    .as_deref()
                    .is_none_or(|value| !value.trim().is_empty())
        }
        ContextPayload::Asset { asset } => {
            !asset.asset_id.trim().is_empty()
                && !asset.version.trim().is_empty()
                && !asset.mime.trim().is_empty()
                && asset.sha256.len() == 64
                && asset
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
                && asset.metadata.keys().all(|key| !key.trim().is_empty())
        }
    }
}

fn fixed_budget(
    request: &ContextCompileRequest,
    tokenizer: &ProfileTokenizer,
) -> Result<BudgetLedger, ContextCompileError> {
    let ledger = budget_evidence(request, tokenizer);
    let fixed = [
        ledger.system_prompt_tokens,
        ledger.user_template_fixed_tokens,
        ledger.tool_schema_tokens,
        ledger.output_schema_tokens,
        ledger.protocol_envelope_tokens,
        ledger.max_output_tokens,
        ledger.safety_reserve_tokens,
    ]
    .into_iter()
    .fold(0u64, u64::saturating_add);
    if fixed > ledger.model_context_window {
        return Err(ContextCompileError::new(
            CompileFailureStage::Budget,
            "context_budget_exceeded",
        ));
    }
    Ok(ledger)
}

fn budget_evidence(request: &ContextCompileRequest, tokenizer: &ProfileTokenizer) -> BudgetLedger {
    let tool_schema_tokens = request
        .prepared_prompt
        .tool_schema
        .as_ref()
        .map_or(0, |value| tokenizer.count_json(value));
    let output_schema_tokens = request
        .prepared_prompt
        .output_schema
        .as_ref()
        .map_or(0, |value| tokenizer.count_json(value));
    let system_prompt_tokens = tokenizer.count_text(&request.prepared_prompt.system);
    let user_template_fixed_tokens =
        tokenizer.count_text(&request.prepared_prompt.user_template_fixed);
    let framing = &tokenizer.profile().framing;
    let tool_count = request
        .prepared_prompt
        .tool_schema
        .as_ref()
        .map_or(0, |value| match value {
            Value::Array(items) => items.len() as u64,
            _ => 1,
        });
    let protocol_envelope_tokens = request
        .prepared_prompt
        .protocol_envelope_tokens
        .saturating_add(framing.request_tokens)
        .saturating_add(framing.reply_priming_tokens)
        .saturating_add(framing.per_message_tokens.saturating_mul(2))
        .saturating_add(framing.per_tool_tokens.saturating_mul(tool_count));
    let fixed = [
        system_prompt_tokens,
        user_template_fixed_tokens,
        tool_schema_tokens,
        output_schema_tokens,
        protocol_envelope_tokens,
        request.prepared_prompt.max_output_tokens,
        tokenizer.profile().safety_reserve_tokens,
    ]
    .into_iter()
    .fold(0u64, u64::saturating_add);
    BudgetLedger {
        model_context_window: request.model_context_window,
        system_prompt_tokens,
        user_template_fixed_tokens,
        tool_schema_tokens,
        output_schema_tokens,
        protocol_envelope_tokens,
        max_output_tokens: request.prepared_prompt.max_output_tokens,
        safety_reserve_tokens: tokenizer.profile().safety_reserve_tokens,
        dynamic_context_budget: request.model_context_window.saturating_sub(fixed),
        selected_context_tokens: 0,
        final_input_tokens: 0,
    }
}

fn validate_groups(
    candidates: &[ContextCandidate],
    groups: &[ContextAtomicGroup],
) -> Result<BTreeMap<String, BTreeSet<String>>, ContextCompileError> {
    let candidate_ids = candidates
        .iter()
        .map(|item| item.candidate_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut result = BTreeMap::new();
    for group in groups {
        let members = group.member_ids.iter().cloned().collect::<BTreeSet<_>>();
        if group.group_id.is_empty()
            || members.len() < 2
            || members.len() != group.member_ids.len()
            || members
                .iter()
                .any(|id| !candidate_ids.contains(id.as_str()))
            || result.insert(group.group_id.clone(), members).is_some()
        {
            return Err(ContextCompileError::new(
                CompileFailureStage::Schema,
                "context_atomic_group_invalid",
            ));
        }
    }
    for candidate in candidates {
        if candidate.atomic_group_id.as_ref().is_some_and(|id| {
            !result
                .get(id)
                .is_some_and(|members| members.contains(&candidate.candidate_id))
        }) {
            return Err(ContextCompileError::new(
                CompileFailureStage::Schema,
                "context_atomic_group_invalid",
            ));
        }
    }
    for (group_id, members) in &result {
        if members.iter().any(|member_id| {
            candidates
                .iter()
                .find(|candidate| &candidate.candidate_id == member_id)
                .and_then(|candidate| candidate.atomic_group_id.as_deref())
                != Some(group_id.as_str())
        }) {
            return Err(ContextCompileError::new(
                CompileFailureStage::Schema,
                "context_atomic_group_invalid",
            ));
        }
    }
    Ok(result)
}

fn exclude_incomplete_groups(
    eligible: &mut Vec<(ContextCandidate, u64)>,
    decisions: &mut Vec<ContextDecision>,
    groups: &BTreeMap<String, BTreeSet<String>>,
) -> Result<(), ContextCompileError> {
    let eligible_ids = eligible
        .iter()
        .map(|(candidate, _)| candidate.candidate_id.as_str())
        .collect::<BTreeSet<_>>();
    let incomplete = groups
        .values()
        .filter(|members| {
            members
                .iter()
                .any(|member| !eligible_ids.contains(member.as_str()))
        })
        .flat_map(|members| members.iter().cloned())
        .collect::<BTreeSet<_>>();
    if incomplete.is_empty() {
        return Ok(());
    }
    if eligible.iter().any(|(candidate, _)| {
        incomplete.contains(&candidate.candidate_id)
            && (candidate.required || candidate.priority == ContextPriority::P0)
    }) {
        return Err(ContextCompileError::new(
            CompileFailureStage::Eligibility,
            "required_context_unavailable",
        ));
    }
    for (candidate, tokens) in eligible
        .iter()
        .filter(|(candidate, _)| incomplete.contains(&candidate.candidate_id))
    {
        decisions.push(minimal_decision(
            candidate,
            *tokens,
            ContextDecisionCode::AtomicGroupExcluded,
            None,
        ));
    }
    eligible.retain(|(candidate, _)| !incomplete.contains(&candidate.candidate_id));
    Ok(())
}

fn detect_confirmed_fact_conflicts(
    eligible: &[(ContextCandidate, u64)],
) -> Result<(), ContextCompileError> {
    let mut facts: BTreeMap<&str, &str> = BTreeMap::new();
    for (candidate, _) in eligible
        .iter()
        .filter(|(item, _)| item.trust == TrustLevel::ConfirmedFact)
    {
        if let Some(key) = candidate.fact_key.as_deref() {
            if facts
                .insert(key, &candidate.content_hash)
                .is_some_and(|existing| existing != candidate.content_hash)
            {
                return Err(ContextCompileError::new(
                    CompileFailureStage::Conflict,
                    "context_conflict",
                ));
            }
        }
    }
    Ok(())
}

fn stable_candidate_order(left: &ContextCandidate, right: &ContextCandidate) -> std::cmp::Ordering {
    left.priority
        .cmp(&right.priority)
        .then_with(|| right.required.cmp(&left.required))
        .then_with(|| left.source_kind.cmp(&right.source_kind))
        .then_with(|| left.source_id.cmp(&right.source_id))
        .then_with(|| left.source_version.cmp(&right.source_version))
        .then_with(|| left.candidate_id.cmp(&right.candidate_id))
}

fn render_candidate_order(left: &ContextCandidate, right: &ContextCandidate) -> std::cmp::Ordering {
    left.render_order
        .cmp(&right.render_order)
        .then_with(|| stable_candidate_order(left, right))
}

fn payload_canonical(payload: &ContextPayload) -> String {
    canonical_json(&serde_json::to_value(payload).expect("context payload serialization"))
}

fn payload_message(payload: &ContextPayload) -> LogicalMessage {
    match payload {
        ContextPayload::Message { message } => message.clone(),
        ContextPayload::Text { text } => LogicalMessage {
            role: "user".into(),
            content: Value::String(text.clone()),
            thinking: None,
            tool_call_id: None,
        },
        ContextPayload::Asset { asset } => LogicalMessage {
            role: "user".into(),
            content: serde_json::to_value(asset).expect("asset serialization"),
            thinking: None,
            tool_call_id: None,
        },
    }
}

fn minimal_decision(
    candidate: &ContextCandidate,
    token_count: u64,
    decision: ContextDecisionCode,
    selected_payload: Option<ContextPayload>,
) -> ContextDecision {
    ContextDecision {
        candidate_id: candidate.candidate_id.clone(),
        source_kind: candidate.source_kind.clone(),
        source_id: candidate.source_id.clone(),
        source_version: candidate.source_version.clone(),
        trust: candidate.trust,
        priority: candidate.priority,
        required: candidate.required,
        render_order: candidate.render_order,
        content_hash: candidate.content_hash.clone(),
        token_count,
        decision,
        selected_payload,
    }
}

fn digest_without_digest(record: &impl Serialize) -> String {
    let mut value = serde_json::to_value(record).expect("context record serialization");
    value
        .as_object_mut()
        .expect("context record object")
        .remove("digest");
    sha256_hex(canonical_json(&value).as_bytes())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextCompileError {
    pub stage: CompileFailureStage,
    pub code: &'static str,
}

impl ContextCompileError {
    fn new(stage: CompileFailureStage, code: &'static str) -> Self {
        Self { stage, code }
    }

    pub fn attempt(&self, request: &ContextCompileRequest) -> ContextCompileAttempt {
        let mut attempt = ContextCompileAttempt::failure(
            request.owner,
            request.owner_id.clone(),
            request.node_key.clone(),
            request.compiled_at.clone(),
            self.stage,
            self.code,
        );
        if !matches!(
            self.stage,
            CompileFailureStage::Schema | CompileFailureStage::Tokenizer
        ) {
            if let Ok(tokenizer) = ProfileTokenizer::from_profile(request.tokenizer_profile.clone())
            {
                attempt.budget = Some(budget_evidence(request, &tokenizer));
            }
        }
        attempt.digest = digest_without_digest(&attempt);
        attempt
    }
}

impl fmt::Display for ContextCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at {:?}", self.code, self.stage)
    }
}

impl std::error::Error for ContextCompileError {}
