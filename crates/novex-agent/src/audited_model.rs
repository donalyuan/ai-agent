use async_trait::async_trait;
use novex_ai_core::{
    canonical_json, definition_digest, redact_audit_value, sha256_hex, validate_model_capabilities,
    CompileFailureStage, ContextAtomicGroup, ContextCandidate, ContextCompileAttempt,
    ContextCompileRequest, ContextCompiler, ContextPayload, ContextPriority, DefinitionRegistry,
    ExecutorOwner, ModelCapabilities, PromptCompiler, PromptPrepareInput, PromptSnapshot,
    TrustLevel,
};
use novex_model::{LLMClient, LLMError, LLMJsonSchema, LLMPrompt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use uuid::Uuid;

use crate::{BoxError, ContextAuditStore, PersistContextSnapshot};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditedCallOwner {
    Conversation(Uuid),
    AgentRun(Uuid),
    EvalRun(Uuid),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FixedModelBinding {
    pub model_id: Uuid,
    pub behavior_fingerprint: String,
    /// 每个 Rust node 固定到其 owner binding 创建时的 Context Policy 证据。
    pub context_policy_bindings: BTreeMap<String, FixedDefinitionBinding>,
    /// Tokenizer Profile 属于模型行为，同时需要独立保存其不可变定义证据。
    pub tokenizer_profile: FixedDefinitionBinding,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FixedDefinitionBinding {
    pub key: String,
    pub version: String,
    pub digest: String,
}

/// Run 创建时可持久化的完整模型 binding 证据，不包含 client、凭据或原始认证配置。
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedBindingEvidence {
    pub binding: FixedModelBinding,
    pub capabilities: ModelCapabilities,
    pub max_output_tokens: u64,
}

#[derive(Clone)]
pub struct ResolvedBoundModel {
    pub client: Arc<dyn LLMClient>,
    pub model_id: Uuid,
    pub behavior_fingerprint: String,
    pub capabilities: ModelCapabilities,
    pub tokenizer_profile_key: String,
    pub tokenizer_profile_version: String,
    pub max_output_tokens: u64,
    pub model_snapshot: Value,
    pub known_secrets: Vec<String>,
}

#[async_trait]
pub trait BoundModelResolver: Send + Sync {
    async fn resolve(&self, model_id: Uuid) -> Result<ResolvedBoundModel, BoxError>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrepareAuditedCall {
    pub owner: AuditedCallOwner,
    pub root_call_id: Option<Uuid>,
    pub parent_call_id: Option<Uuid>,
    pub attempt: i32,
    pub snapshot: PromptSnapshot,
    pub model_id: Uuid,
    pub behavior_fingerprint: String,
    pub model_snapshot: Value,
    pub context_sources: Value,
    pub memory_sources: Value,
    pub parameters: Value,
    pub asset_references: Value,
    pub known_secrets: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrepareAuditedCallWithContext {
    pub model_call: PrepareAuditedCall,
    pub context: PersistContextSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditedTerminalStatus {
    Succeeded,
    Failed,
    Aborted,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FinishAuditedCall {
    pub id: Uuid,
    pub status: AuditedTerminalStatus,
    pub output_snapshot: Option<Value>,
    pub usage_snapshot: Option<Value>,
    pub error_snapshot: Option<Value>,
    pub structured_parse_status: Option<String>,
    pub known_secrets: Vec<String>,
}

#[async_trait]
pub trait ModelCallAuditStore: Send + Sync {
    async fn prepare_with_context(
        &self,
        input: PrepareAuditedCallWithContext,
    ) -> Result<Uuid, BoxError>;
    async fn associate_step(&self, model_call_id: Uuid, step_id: Uuid) -> Result<(), BoxError>;
    async fn finish(&self, input: FinishAuditedCall) -> Result<(), BoxError>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuditedModelRequest {
    pub owner: AuditedCallOwner,
    pub step_id: Option<Uuid>,
    pub root_call_id: Option<Uuid>,
    pub parent_call_id: Option<Uuid>,
    pub attempt: i32,
    pub agent_key: String,
    pub agent_version: String,
    pub node_key: String,
    pub variables: BTreeMap<String, Value>,
    pub context_candidates: Vec<ContextCandidate>,
    pub context_atomic_groups: Vec<ContextAtomicGroup>,
    /// 固定一次模型调用的编译时钟，保证 Context 决策和审计可重放。
    pub compiled_at: String,
    pub tool_profile: String,
    pub tool_schema: Option<Value>,
    pub binding: FixedModelBinding,
    pub context_sources: Value,
    pub memory_sources: Value,
    pub parameters: Value,
    pub asset_references: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuditedModelResponse {
    pub model_call_id: Uuid,
    pub output: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuditedParsedModelResponse<T> {
    pub model_call_id: Uuid,
    pub output: T,
}

struct ProviderOutput {
    model_call_id: Uuid,
    output: String,
    known_secrets: Vec<String>,
}

/// 已完成 Context 编译和审计持久化、但尚未调用 provider 的请求。
pub struct PreparedAuditedModelCall {
    owner: AuditedCallOwner,
    model_call_id: Uuid,
    context_snapshot_id: Uuid,
    prompt: LLMPrompt,
    client: Arc<dyn LLMClient>,
    known_secrets: Vec<String>,
    context_audit: Arc<dyn ContextAuditStore>,
}

impl PreparedAuditedModelCall {
    pub fn model_call_id(&self) -> Uuid {
        self.model_call_id
    }

    pub fn context_snapshot_id(&self) -> Uuid {
        self.context_snapshot_id
    }

    /// 调用 provider，但把 ModelCall 终态留给业务事务统一提交。
    pub async fn execute(self) -> PreparedAuditedModelOutcome {
        match self.client.generate_script(self.prompt).await {
            Ok(output) => PreparedAuditedModelOutcome::Succeeded(PreparedAuditedModelSuccess {
                model_call_id: self.model_call_id,
                output,
                known_secrets: self.known_secrets,
            }),
            Err(error) => {
                let profile_incompatible = is_provider_context_overflow(&error);
                let result_uncertain = matches!(error, LLMError::Timeout | LLMError::Transport(_));
                let error_kind = if profile_incompatible {
                    "tokenizer_profile_incompatible"
                } else {
                    llm_error_kind(&error)
                };
                let raw_message = error.to_string();
                let message = redact_audit_value(&json!(raw_message), &self.known_secrets)
                    .as_str()
                    .unwrap_or("provider error was redacted")
                    .to_string();
                if profile_incompatible {
                    let _ = self
                        .context_audit
                        .block_tokenizer_profile_binding(self.owner)
                        .await;
                }
                PreparedAuditedModelOutcome::Failed(PreparedAuditedModelFailure {
                    model_call_id: self.model_call_id,
                    error_kind: error_kind.into(),
                    message: message.clone(),
                    result_uncertain,
                    finish: FinishAuditedCall {
                        id: self.model_call_id,
                        status: AuditedTerminalStatus::Failed,
                        output_snapshot: None,
                        usage_snapshot: None,
                        error_snapshot: Some(json!({
                            "kind": error_kind,
                            "message": message,
                            "result_uncertain": result_uncertain,
                        })),
                        structured_parse_status: None,
                        known_secrets: self.known_secrets,
                    },
                })
            }
        }
    }
}

/// provider 已成功返回，但 ModelCall 尚未写入终态。
pub struct PreparedAuditedModelSuccess {
    pub model_call_id: Uuid,
    pub output: String,
    known_secrets: Vec<String>,
}

impl PreparedAuditedModelSuccess {
    pub fn contains_known_secret(&self) -> bool {
        self.known_secrets
            .iter()
            .any(|secret| !secret.is_empty() && self.output.contains(secret))
    }

    pub fn finish(
        self,
        status: AuditedTerminalStatus,
        usage_snapshot: Option<Value>,
        error_snapshot: Option<Value>,
        structured_parse_status: Option<String>,
    ) -> FinishAuditedCall {
        FinishAuditedCall {
            id: self.model_call_id,
            status,
            output_snapshot: Some(json!({"text": self.output})),
            usage_snapshot,
            error_snapshot,
            structured_parse_status,
            known_secrets: self.known_secrets,
        }
    }
}

/// provider 已失败，调用方必须把该证据与业务失败终态一起提交。
pub struct PreparedAuditedModelFailure {
    pub model_call_id: Uuid,
    pub error_kind: String,
    pub message: String,
    pub result_uncertain: bool,
    finish: FinishAuditedCall,
}

impl PreparedAuditedModelFailure {
    pub fn into_finish(self) -> FinishAuditedCall {
        self.finish
    }
}

pub enum PreparedAuditedModelOutcome {
    Succeeded(PreparedAuditedModelSuccess),
    Failed(PreparedAuditedModelFailure),
}

pub struct AuditedModelExecutor {
    registry: Arc<DefinitionRegistry>,
    models: Arc<dyn BoundModelResolver>,
    audit: Arc<dyn ModelCallAuditStore>,
    context_audit: Arc<dyn ContextAuditStore>,
}

impl AuditedModelExecutor {
    pub fn new(
        registry: Arc<DefinitionRegistry>,
        models: Arc<dyn BoundModelResolver>,
        audit: Arc<dyn ModelCallAuditStore>,
        context_audit: Arc<dyn ContextAuditStore>,
    ) -> Self {
        Self {
            registry,
            models,
            audit,
            context_audit,
        }
    }

    pub async fn associate_step(
        &self,
        model_call_id: Uuid,
        step_id: Uuid,
    ) -> Result<(), AuditedModelError> {
        self.audit
            .associate_step(model_call_id, step_id)
            .await
            .map_err(AuditedModelError::FinishAudit)
    }

    /// 为指定 agent 和模型 ID 构建不可漂移的 `FixedModelBinding`。
    ///
    /// 角色执行管道在发起 `execute()` 之前调用此方法一次，将当前的 Context Policy
    /// 摘要和 Tokenizer Profile 摘要固定下来，确保整次执行可重放。
    pub async fn build_binding(
        &self,
        agent_key: &str,
        agent_version: &str,
        model_id: Uuid,
    ) -> Result<FixedModelBinding, AuditedModelError> {
        Ok(self
            .build_binding_evidence(agent_key, agent_version, model_id)
            .await?
            .binding)
    }

    /// 解析并校验 Run 创建时需要冻结的 Definition/model binding 证据。
    pub async fn build_binding_evidence(
        &self,
        agent_key: &str,
        agent_version: &str,
        model_id: Uuid,
    ) -> Result<ResolvedBindingEvidence, AuditedModelError> {
        let resolved = self
            .models
            .resolve(model_id)
            .await
            .map_err(AuditedModelError::ModelResolution)?;

        // 获取 agent 的节点定义，计算各节点的 context policy 摘要
        let agent = self
            .registry
            .agent(agent_key, agent_version)
            .map_err(|e| AuditedModelError::Compile(e.to_string()))?;
        validate_model_capabilities(&agent.model_requirements, &resolved.capabilities)
            .map_err(|_| AuditedModelError::ModelCapabilityMismatch)?;

        let context_policy_bindings: std::collections::BTreeMap<String, FixedDefinitionBinding> =
            agent
                .nodes
                .iter()
                .map(|(node_key, reference)| {
                    let policy_ref = reference.context_policy.as_ref().ok_or_else(|| {
                        AuditedModelError::Compile(format!(
                            "governed Context Policy binding is missing at node {node_key}"
                        ))
                    })?;
                    let policy = self
                        .registry
                        .context_policy(&policy_ref.key, &policy_ref.version)
                        .map_err(|e| AuditedModelError::Compile(e.to_string()))?;
                    let digest = definition_digest(policy)
                        .map_err(|e| AuditedModelError::Compile(e.to_string()))?;
                    Ok((
                        node_key.clone(),
                        FixedDefinitionBinding {
                            key: policy_ref.key.clone(),
                            version: policy_ref.version.clone(),
                            digest,
                        },
                    ))
                })
                .collect::<Result<_, AuditedModelError>>()?;

        // 获取并摘要 tokenizer profile
        let profile = self
            .registry
            .tokenizer_profile(
                &resolved.tokenizer_profile_key,
                &resolved.tokenizer_profile_version,
            )
            .map_err(|_| AuditedModelError::TokenizerProfileUnavailable)?;
        let profile_digest =
            definition_digest(profile).map_err(|e| AuditedModelError::Compile(e.to_string()))?;

        Ok(ResolvedBindingEvidence {
            binding: FixedModelBinding {
                model_id: resolved.model_id,
                behavior_fingerprint: resolved.behavior_fingerprint,
                context_policy_bindings,
                tokenizer_profile: FixedDefinitionBinding {
                    key: resolved.tokenizer_profile_key,
                    version: resolved.tokenizer_profile_version,
                    digest: profile_digest,
                },
            },
            capabilities: resolved.capabilities,
            max_output_tokens: resolved.max_output_tokens,
        })
    }

    pub async fn execute(
        &self,
        request: AuditedModelRequest,
    ) -> Result<AuditedModelResponse, AuditedModelError> {
        let provider = self.execute_provider(request).await?;
        self.audit
            .finish(FinishAuditedCall {
                id: provider.model_call_id,
                status: AuditedTerminalStatus::Succeeded,
                output_snapshot: Some(json!({ "text": provider.output })),
                usage_snapshot: None,
                error_snapshot: None,
                structured_parse_status: None,
                known_secrets: provider.known_secrets,
            })
            .await
            .map_err(AuditedModelError::FinishAudit)?;
        Ok(AuditedModelResponse {
            model_call_id: provider.model_call_id,
            output: provider.output,
        })
    }

    pub async fn execute_parsed<T, Parse>(
        &self,
        request: AuditedModelRequest,
        parse: Parse,
    ) -> Result<AuditedParsedModelResponse<T>, AuditedModelError>
    where
        Parse: FnOnce(&str) -> Result<T, String>,
    {
        let provider = self.execute_provider(request).await?;
        match parse(&provider.output) {
            Ok(output) => {
                self.audit
                    .finish(FinishAuditedCall {
                        id: provider.model_call_id,
                        status: AuditedTerminalStatus::Succeeded,
                        output_snapshot: Some(json!({ "text": provider.output })),
                        usage_snapshot: None,
                        error_snapshot: None,
                        structured_parse_status: Some("succeeded".into()),
                        known_secrets: provider.known_secrets,
                    })
                    .await
                    .map_err(AuditedModelError::FinishAudit)?;
                Ok(AuditedParsedModelResponse {
                    model_call_id: provider.model_call_id,
                    output,
                })
            }
            Err(message) => {
                self.audit
                    .finish(FinishAuditedCall {
                        id: provider.model_call_id,
                        status: AuditedTerminalStatus::Failed,
                        output_snapshot: Some(json!({ "text": provider.output })),
                        usage_snapshot: None,
                        error_snapshot: Some(json!({
                            "kind": "structured_parse",
                            "message": message,
                        })),
                        structured_parse_status: Some("failed".into()),
                        known_secrets: provider.known_secrets,
                    })
                    .await
                    .map_err(AuditedModelError::FinishAudit)?;
                Err(AuditedModelError::StructuredParse {
                    model_call_id: provider.model_call_id,
                    message,
                })
            }
        }
    }

    /// 完成绑定、Prompt、Context 与 prepared ModelCall 持久化，但不调用模型 provider。
    pub async fn prepare(
        &self,
        request: AuditedModelRequest,
    ) -> Result<PreparedAuditedModelCall, AuditedModelError> {
        if !self
            .context_audit
            .binding_is_executable(request.owner)
            .await
            .map_err(AuditedModelError::BindingAudit)?
        {
            return Err(AuditedModelError::ContextBindingRebindRequired);
        }
        let resolved = self
            .models
            .resolve(request.binding.model_id)
            .await
            .map_err(AuditedModelError::ModelResolution)?;
        if resolved.model_id != request.binding.model_id
            || resolved.behavior_fingerprint != request.binding.behavior_fingerprint
        {
            return Err(AuditedModelError::ModelRebindRequired);
        }
        let agent = self
            .registry
            .agent(&request.agent_key, &request.agent_version)
            .map_err(|error| AuditedModelError::Compile(error.to_string()))?;
        validate_model_capabilities(&agent.model_requirements, &resolved.capabilities)
            .map_err(|_| AuditedModelError::ModelCapabilityMismatch)?;
        let node = agent.nodes.get(&request.node_key).ok_or_else(|| {
            AuditedModelError::Compile(format!("node {} is not declared", request.node_key))
        })?;
        let policy_reference = node.context_policy.as_ref().ok_or_else(|| {
            AuditedModelError::Compile("governed Context Policy binding is missing".into())
        })?;
        let bound_policy = request
            .binding
            .context_policy_bindings
            .get(&request.node_key)
            .ok_or(AuditedModelError::ContextBindingRebindRequired)?;
        if bound_policy.key != policy_reference.key
            || bound_policy.version != policy_reference.version
        {
            return Err(AuditedModelError::ContextBindingRebindRequired);
        }
        let policy = self
            .registry
            .context_policy(&policy_reference.key, &policy_reference.version)
            .map_err(|error| AuditedModelError::Compile(error.to_string()))?
            .clone();
        if definition_digest(&policy)
            .map_err(|error| AuditedModelError::Compile(error.to_string()))?
            != bound_policy.digest
        {
            return Err(AuditedModelError::ContextBindingRebindRequired);
        }
        if request.binding.tokenizer_profile.key != resolved.tokenizer_profile_key
            || request.binding.tokenizer_profile.version != resolved.tokenizer_profile_version
        {
            return Err(AuditedModelError::ContextBindingRebindRequired);
        }
        let tokenizer_profile = match self.registry.tokenizer_profile(
            &resolved.tokenizer_profile_key,
            &resolved.tokenizer_profile_version,
        ) {
            Ok(profile) => profile.clone(),
            Err(_) => {
                self.persist_precompile_failure(
                    &request,
                    &resolved,
                    CompileFailureStage::Tokenizer,
                    "tokenizer_profile_unavailable",
                )
                .await?;
                return Err(AuditedModelError::TokenizerProfileUnavailable);
            }
        };
        if definition_digest(&tokenizer_profile)
            .map_err(|error| AuditedModelError::Compile(error.to_string()))?
            != request.binding.tokenizer_profile.digest
        {
            return Err(AuditedModelError::ContextBindingRebindRequired);
        }
        let prepared = match PromptCompiler::new(&self.registry).prepare(
            &request.agent_key,
            &request.agent_version,
            &request.node_key,
            PromptPrepareInput {
                variables: request.variables.clone(),
                tool_profile: request.tool_profile.clone(),
                tool_schema: request.tool_schema.clone(),
                model_max_output_tokens: resolved.max_output_tokens,
            },
        ) {
            Ok(prepared) => prepared,
            Err(_) => {
                self.persist_precompile_failure(
                    &request,
                    &resolved,
                    CompileFailureStage::Schema,
                    "context_schema_invalid",
                )
                .await?;
                return Err(AuditedModelError::ContextCompile(
                    "context_schema_invalid".into(),
                ));
            }
        };
        let context_request = ContextCompileRequest {
            schema_version: "2".into(),
            owner: owner_kind(request.owner),
            owner_id: owner_id(request.owner).to_string(),
            node_key: request.node_key.clone(),
            compiled_at: request.compiled_at.clone(),
            model_context_window: resolved.capabilities.context_window,
            policy,
            tokenizer_profile: tokenizer_profile.clone(),
            prepared_prompt: prepared.envelope.clone(),
            candidates: request.context_candidates.clone(),
            atomic_groups: request.context_atomic_groups.clone(),
        };
        let compiled = match ContextCompiler::compile(context_request.clone()) {
            Ok(compiled) => compiled,
            Err(error) => {
                self.persist_compile_attempt(&request, &resolved, error.attempt(&context_request))
                    .await?;
                return Err(AuditedModelError::ContextCompile(error.code.into()));
            }
        };
        let context_snapshot_id = Uuid::new_v4();
        let finalized = PromptCompiler::new(&self.registry)
            .finalize(
                prepared,
                context_snapshot_id.to_string(),
                &compiled,
                &tokenizer_profile,
            )
            .map_err(|error| AuditedModelError::Compile(error.to_string()))?;
        let snapshot = finalized.prompt_snapshot;
        let context_snapshot = finalized.context_snapshot;
        let prompt = prompt_from_snapshot(&snapshot)?;
        let call_id = self
            .audit
            .prepare_with_context(PrepareAuditedCallWithContext {
                model_call: PrepareAuditedCall {
                    owner: request.owner,
                    root_call_id: request.root_call_id,
                    parent_call_id: request.parent_call_id,
                    attempt: request.attempt,
                    snapshot,
                    model_id: resolved.model_id,
                    behavior_fingerprint: resolved.behavior_fingerprint.clone(),
                    model_snapshot: resolved.model_snapshot.clone(),
                    context_sources: request.context_sources,
                    memory_sources: request.memory_sources,
                    parameters: request.parameters,
                    asset_references: request.asset_references,
                    known_secrets: resolved.known_secrets.clone(),
                },
                context: PersistContextSnapshot {
                    owner: request.owner,
                    snapshot: context_snapshot,
                    known_secrets: resolved.known_secrets.clone(),
                },
            })
            .await
            .map_err(AuditedModelError::PrepareAudit)?;
        if let Some(step_id) = request.step_id {
            self.audit
                .associate_step(call_id, step_id)
                .await
                .map_err(AuditedModelError::PrepareAudit)?;
        }

        Ok(PreparedAuditedModelCall {
            owner: request.owner,
            model_call_id: call_id,
            context_snapshot_id,
            prompt,
            client: resolved.client,
            known_secrets: resolved.known_secrets,
            context_audit: self.context_audit.clone(),
        })
    }

    async fn execute_provider(
        &self,
        request: AuditedModelRequest,
    ) -> Result<ProviderOutput, AuditedModelError> {
        let prepared = self.prepare(request).await?;
        let call_id = prepared.model_call_id;
        match prepared.client.generate_script(prepared.prompt).await {
            Ok(output) => Ok(ProviderOutput {
                model_call_id: call_id,
                output,
                known_secrets: prepared.known_secrets,
            }),
            Err(error) => {
                let profile_incompatible = is_provider_context_overflow(&error);
                self.audit
                    .finish(FinishAuditedCall {
                        id: call_id,
                        status: AuditedTerminalStatus::Failed,
                        output_snapshot: None,
                        usage_snapshot: None,
                        error_snapshot: Some(json!({
                            "kind": if profile_incompatible {
                                "tokenizer_profile_incompatible"
                            } else {
                                llm_error_kind(&error)
                            },
                            "message": error.to_string(),
                        })),
                        structured_parse_status: None,
                        known_secrets: prepared.known_secrets.clone(),
                    })
                    .await
                    .map_err(AuditedModelError::FinishAudit)?;
                if profile_incompatible {
                    self.context_audit
                        .block_tokenizer_profile_binding(prepared.owner)
                        .await
                        .map_err(AuditedModelError::BindingAudit)?;
                    Err(AuditedModelError::TokenizerProfileIncompatible {
                        model_call_id: call_id,
                        source: error,
                    })
                } else {
                    Err(AuditedModelError::Provider {
                        model_call_id: call_id,
                        source: error,
                    })
                }
            }
        }
    }

    async fn persist_precompile_failure(
        &self,
        request: &AuditedModelRequest,
        resolved: &ResolvedBoundModel,
        stage: CompileFailureStage,
        code: &'static str,
    ) -> Result<(), AuditedModelError> {
        self.persist_compile_attempt(
            request,
            resolved,
            ContextCompileAttempt::failure(
                owner_kind(request.owner),
                owner_id(request.owner).to_string(),
                request.node_key.clone(),
                request.compiled_at.clone(),
                stage,
                code,
            ),
        )
        .await
    }

    async fn persist_compile_attempt(
        &self,
        request: &AuditedModelRequest,
        resolved: &ResolvedBoundModel,
        attempt: ContextCompileAttempt,
    ) -> Result<(), AuditedModelError> {
        let attempt_id = self
            .context_audit
            .persist_attempt(crate::PersistContextCompileAttempt {
                owner: request.owner,
                attempt,
                known_secrets: resolved.known_secrets.clone(),
            })
            .await
            .map_err(AuditedModelError::PrepareAudit)?;
        if !matches!(request.owner, AuditedCallOwner::EvalRun(_)) {
            self.context_audit
                .link_failure(request.owner, attempt_id, request.step_id)
                .await
                .map_err(AuditedModelError::PrepareAudit)?;
        }
        Ok(())
    }
}

fn owner_kind(owner: AuditedCallOwner) -> ExecutorOwner {
    match owner {
        AuditedCallOwner::Conversation(_)
        | AuditedCallOwner::AgentRun(_)
        | AuditedCallOwner::EvalRun(_) => ExecutorOwner::Rust,
    }
}

fn owner_id(owner: AuditedCallOwner) -> Uuid {
    match owner {
        AuditedCallOwner::Conversation(id)
        | AuditedCallOwner::AgentRun(id)
        | AuditedCallOwner::EvalRun(id) => id,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextContextCandidateInput {
    pub candidate_id: String,
    pub source_kind: String,
    pub source_id: String,
    pub source_version: String,
    pub trust: TrustLevel,
    pub priority: ContextPriority,
    pub required: bool,
    pub render_order: u32,
    pub observed_at: String,
    pub text: String,
}

/// Builds a text candidate with explicit source and priority evidence for an audited node.
pub fn text_context_candidate(input: TextContextCandidateInput) -> ContextCandidate {
    let payload = ContextPayload::Text { text: input.text };
    ContextCandidate {
        candidate_id: input.candidate_id,
        source_kind: input.source_kind,
        source_id: input.source_id,
        source_version: input.source_version,
        fact_key: None,
        trust: input.trust,
        priority: input.priority,
        required: input.required,
        render_order: input.render_order,
        observed_at: input.observed_at,
        valid_until: None,
        supersedes: Vec::new(),
        content_hash: sha256_hex(
            canonical_json(&serde_json::to_value(&payload).expect("Context payload serialization"))
                .as_bytes(),
        ),
        atomic_group_id: None,
        payload,
    }
}

fn prompt_from_snapshot(snapshot: &PromptSnapshot) -> Result<LLMPrompt, AuditedModelError> {
    let output_schema = snapshot
        .output_schema
        .as_ref()
        .map(|value| {
            let name = value.get("name").and_then(Value::as_str).ok_or_else(|| {
                AuditedModelError::Compile("output schema name is missing".into())
            })?;
            let strict = value
                .get("strict")
                .and_then(Value::as_bool)
                .ok_or_else(|| {
                    AuditedModelError::Compile("output schema strict is missing".into())
                })?;
            let schema = value.get("schema").cloned().ok_or_else(|| {
                AuditedModelError::Compile("output schema body is missing".into())
            })?;
            Ok(LLMJsonSchema {
                name: name.into(),
                strict,
                schema,
            })
        })
        .transpose()?;
    Ok(LLMPrompt {
        system: snapshot.system.clone(),
        user: snapshot.user.clone(),
        max_output_tokens: snapshot.max_output_tokens,
        output_schema,
    })
}

fn llm_error_kind(error: &LLMError) -> &'static str {
    match error {
        LLMError::Config(_) => "config",
        LLMError::Timeout => "timeout",
        LLMError::Provider(_) => "provider",
        LLMError::Transport(_) => "transport",
    }
}

fn is_provider_context_overflow(error: &LLMError) -> bool {
    let LLMError::Provider(message) = error else {
        return false;
    };
    let normalized = message.to_ascii_lowercase();
    [
        "context_length_exceeded",
        "maximum context length",
        "context window exceeded",
        "too many tokens",
        "prompt is too long",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

#[derive(Debug)]
pub enum AuditedModelError {
    Compile(String),
    ModelResolution(BoxError),
    ModelRebindRequired,
    ContextBindingRebindRequired,
    BindingAudit(BoxError),
    ModelCapabilityMismatch,
    TokenizerProfileUnavailable,
    ContextCompile(String),
    PrepareAudit(BoxError),
    Provider {
        model_call_id: Uuid,
        source: LLMError,
    },
    TokenizerProfileIncompatible {
        model_call_id: Uuid,
        source: LLMError,
    },
    StructuredParse {
        model_call_id: Uuid,
        message: String,
    },
    FinishAudit(BoxError),
}

impl fmt::Display for AuditedModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compile(message) => write!(formatter, "prompt compile failed: {message}"),
            Self::ModelResolution(error) => write!(formatter, "model resolution failed: {error}"),
            Self::ModelRebindRequired => formatter.write_str("model_rebind_required"),
            Self::ContextBindingRebindRequired => {
                formatter.write_str("context_binding_rebind_required")
            }
            Self::BindingAudit(error) => write!(formatter, "context binding audit failed: {error}"),
            Self::ModelCapabilityMismatch => formatter.write_str("model_capability_mismatch"),
            Self::TokenizerProfileUnavailable => {
                formatter.write_str("tokenizer_profile_unavailable")
            }
            Self::ContextCompile(code) => formatter.write_str(code),
            Self::PrepareAudit(error) => write!(formatter, "audit_persistence_failed: {error}"),
            Self::Provider { source, .. } => write!(formatter, "{source}"),
            Self::TokenizerProfileIncompatible { .. } => {
                formatter.write_str("tokenizer_profile_incompatible")
            }
            Self::StructuredParse { message, .. } => {
                write!(formatter, "structured output parse failed: {message}")
            }
            Self::FinishAudit(error) => write!(formatter, "audit finalization failed: {error}"),
        }
    }
}

impl std::error::Error for AuditedModelError {}

impl AuditedModelError {
    pub fn model_call_id(&self) -> Option<Uuid> {
        match self {
            Self::Provider { model_call_id, .. }
            | Self::TokenizerProfileIncompatible { model_call_id, .. }
            | Self::StructuredParse { model_call_id, .. } => Some(*model_call_id),
            _ => None,
        }
    }

    pub fn provider_error(&self) -> Option<&LLMError> {
        match self {
            Self::Provider { source, .. } | Self::TokenizerProfileIncompatible { source, .. } => {
                Some(source)
            }
            _ => None,
        }
    }
}
