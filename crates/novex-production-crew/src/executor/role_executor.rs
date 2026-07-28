//! RoleExecutor：单个角色完整生命周期的执行器
//!
//! 正式 Full Crew 只允许 durable `prepare/execute/finalize` 协议。prepare 完成
//! step/lease、冻结 binding、精确 package、Context、资源和审计锚点校验，但不调用 provider。

use crate::durable::canonical_digest;
use crate::durable::repository::{
    DurableProductionRepository, PreparedAgentBindingInput, RoleFinalizeCommand,
    RoleFinalizeFailure, RoleInputPackage, RolePrepareSnapshot,
};
use crate::durable::resource::ResourceRequest;
use crate::error::{ProductionError, ProductionResult};
use crate::roles::definition::{Lifecycle, RoleDefinition};
use crate::roles::RoleRegistry;
use crate::state::artifacts::output_contract::{validate_role_output, ValidatedRoleOutput};
use crate::state::artifacts::ArtifactType;
use novex_agent::{
    text_context_candidate, AuditedCallOwner, AuditedModelExecutor, AuditedModelRequest,
    AuditedTerminalStatus, FinishAuditedCall, FixedDefinitionBinding, FixedModelBinding,
    PreparedAuditedModelCall, PreparedAuditedModelOutcome, ResolvedBindingEvidence,
    TextContextCandidateInput,
};
use novex_ai_core::{
    definition_digest, ContextPriority, DefinitionRegistry, DefinitionStatus, ExecutorOwner,
    ModelCapabilities, TrustLevel,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

// ─────────────────────────────────────────────
// 公共数据结构
// ─────────────────────────────────────────────

/// 单次角色执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleExecutionResult {
    /// 执行的角色 key
    pub role: String,
    /// 执行状态
    pub status: RoleExecutionStatus,
    /// 实际耗时（毫秒）
    pub execution_time_ms: u64,
    /// 本次执行产出的产物列表（type + id + version）
    pub output_artifacts: Vec<ArtifactSummary>,
    /// 对应的模型调用记录 UUID（审计用）
    pub model_call_id: Option<Uuid>,
    /// 下一个待执行角色（若有）
    pub next_role: Option<String>,
}

/// 产物摘要（用于 API 响应）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactSummary {
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    pub id: Uuid,
    pub version: i32,
    pub character_id: Option<String>,
    pub shot_id: Option<String>,
}

/// 角色执行状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleExecutionStatus {
    Completed,
    Failed,
    WaitingGate,
}

/// 普通 ProductionRun 在创建时冻结的完整角色 Definition/model binding。
#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenRoleBindingSnapshot {
    pub definition_key: String,
    pub definition_version: String,
    pub definition_digest: String,
    pub registry_digest: String,
    pub lifecycle: String,
    pub prompt_bindings: BTreeMap<String, FixedDefinitionBinding>,
    pub model_binding: FixedModelBinding,
    pub model_capabilities: ModelCapabilities,
}

/// 当前 worker 已认领的 durable role step。
pub struct RolePrepareContext {
    pub pool: PgPool,
    pub definition_registry: Arc<DefinitionRegistry>,
    pub audited_executor: Arc<AuditedModelExecutor>,
    pub step_id: Uuid,
    pub lease_owner: String,
    pub attempt: i32,
}

/// provider 调用前已经持久化完成的所有执行锚点。
pub struct PreparedRoleExecution {
    pub run_id: Uuid,
    pub production_project_id: Uuid,
    pub step_id: Uuid,
    pub attempt: i32,
    pub revision_epoch: i32,
    pub role_key: String,
    pub agent_run_id: Uuid,
    pub model_call_id: Uuid,
    pub context_snapshot_id: Uuid,
    pub input_packages: Vec<RoleInputPackage>,
    prepared_call: PreparedAuditedModelCall,
}

impl PreparedRoleExecution {
    pub fn into_prepared_call(self) -> PreparedAuditedModelCall {
        self.prepared_call
    }
}

/// execute 阶段产生的不可变结果，finalize 可用它进行幂等重放。
#[derive(Clone)]
pub struct ExecutedRoleExecution {
    pub run_id: Uuid,
    pub production_project_id: Uuid,
    pub step_id: Uuid,
    pub attempt: i32,
    pub revision_epoch: i32,
    pub role_key: String,
    pub agent_run_id: Uuid,
    pub model_call_id: Uuid,
    pub context_snapshot_id: Uuid,
    pub input_packages: Vec<RoleInputPackage>,
    pub output: Option<Value>,
    pub validated_output: Option<ValidatedRoleOutput>,
    pub output_digest: Option<String>,
    pub failure: Option<RoleFinalizeFailure>,
    pub model_call_finish: FinishAuditedCall,
    pub execution_time_ms: u64,
    pub output_tokens: u64,
}

#[derive(Clone)]
pub struct RoleFinalizeContext {
    pub pool: PgPool,
    pub lease_owner: String,
}

// ─────────────────────────────────────────────
// RoleExecutor 实现
// ─────────────────────────────────────────────

/// 无状态执行器；正式执行只能通过 durable prepare/execute/finalize 协议。
pub struct RoleExecutor;

impl RoleExecutor {
    /// 在创建 ProductionRun 前解析 active Definition 和固定模型能力证据。
    pub async fn freeze_active_binding(
        role_key: &str,
        agent_version: &str,
        definition_registry: &DefinitionRegistry,
        audited_executor: &AuditedModelExecutor,
        model_id: Uuid,
    ) -> ProductionResult<FrozenRoleBindingSnapshot> {
        let agent_key = format!("production.{role_key}");
        let agent = definition_registry
            .agent(&agent_key, agent_version)
            .map_err(|error| ProductionError::CapabilityMismatch {
                reason: error.to_string(),
            })?;
        if agent.status != DefinitionStatus::Active || agent.executor_owner != ExecutorOwner::Rust {
            return Err(ProductionError::CapabilityMismatch {
                reason: format!("role {role_key} is not an active Rust Definition"),
            });
        }
        let evidence = audited_executor
            .build_binding_evidence(&agent_key, agent_version, model_id)
            .await
            .map_err(|error| ProductionError::CapabilityMismatch {
                reason: error.to_string(),
            })?;
        let prompt_bindings = prompt_bindings(definition_registry, agent)?;
        Ok(FrozenRoleBindingSnapshot {
            definition_key: agent_key,
            definition_version: agent_version.into(),
            definition_digest: definition_digest(agent).map_err(|error| {
                ProductionError::CapabilityMismatch {
                    reason: error.to_string(),
                }
            })?,
            registry_digest: definition_registry.digest().into(),
            lifecycle: "active".into(),
            prompt_bindings,
            model_binding: evidence.binding,
            model_capabilities: evidence.capabilities,
        })
    }

    /// 完成 durable role 的 provider 前准备，不执行模型请求。
    pub async fn prepare(
        ctx: RolePrepareContext,
        role_registry: &RoleRegistry,
    ) -> ProductionResult<PreparedRoleExecution> {
        let repo = DurableProductionRepository::new(ctx.pool.clone());
        let snapshot = match repo
            .load_role_prepare_snapshot(ctx.step_id, &ctx.lease_owner, ctx.attempt)
            .await
        {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let _ = repo
                    .fail_role_prepare(
                        ctx.step_id,
                        &ctx.lease_owner,
                        ctx.attempt,
                        None,
                        error.code(),
                        &error.to_string(),
                    )
                    .await;
                return Err(error);
            }
        };
        let role_def = match role_registry.get(&snapshot.role_key) {
            Ok(role) if role.lifecycle == Lifecycle::Active => role,
            Ok(_) => {
                let error = ProductionError::CapabilityMismatch {
                    reason: format!("role {} is not active", snapshot.role_key),
                };
                let _ = repo
                    .fail_role_prepare(
                        snapshot.step_id,
                        &ctx.lease_owner,
                        snapshot.attempt,
                        None,
                        error.code(),
                        &error.to_string(),
                    )
                    .await;
                return Err(error);
            }
            Err(error) => return Err(error),
        };
        let frozen: FrozenRoleBindingSnapshot =
            match serde_json::from_value(snapshot.role_binding.clone()) {
                Ok(binding) => binding,
                Err(error) => {
                    let error = ProductionError::CapabilityMismatch {
                        reason: format!("frozen role binding is incomplete: {error}"),
                    };
                    let _ = repo
                        .fail_role_prepare(
                            snapshot.step_id,
                            &ctx.lease_owner,
                            snapshot.attempt,
                            None,
                            error.code(),
                            &error.to_string(),
                        )
                        .await;
                    return Err(error);
                }
            };
        let evidence = match validate_frozen_binding(
            &snapshot,
            &frozen,
            &ctx.definition_registry,
            &ctx.audited_executor,
        )
        .await
        {
            Ok(evidence) => evidence,
            Err(error) => {
                let _ = repo
                    .fail_role_prepare(
                        snapshot.step_id,
                        &ctx.lease_owner,
                        snapshot.attempt,
                        None,
                        error.code(),
                        &error.to_string(),
                    )
                    .await;
                return Err(error);
            }
        };
        if let Err(error) = check_durable_inputs_ready(role_def, role_registry, &snapshot) {
            let _ = repo
                .fail_role_prepare(
                    snapshot.step_id,
                    &ctx.lease_owner,
                    snapshot.attempt,
                    None,
                    error.code(),
                    &error.to_string(),
                )
                .await;
            return Err(error);
        }

        let compiled_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let (context_candidates, context_sources) =
            build_durable_context_candidates(&snapshot, &compiled_at)?;
        let input_snapshot = json!({
            "production_run_id": snapshot.run_id,
            "production_step_id": snapshot.step_id,
            "attempt": snapshot.attempt,
            "revision_epoch": snapshot.revision_epoch,
            "role_key": snapshot.role_key,
            "source_snapshot": snapshot.source_snapshot,
            "input_packages": snapshot.input_packages,
            "dependency_anchors": snapshot.dependency_anchors,
            "revision_instruction": snapshot.revision_instruction,
            "media_review": snapshot.media_review,
        });
        let binding_input = prepared_agent_binding(&frozen);
        let agent_run_id = repo
            .create_role_agent_run(&snapshot, &ctx.lease_owner, &binding_input, &input_snapshot)
            .await?;
        let estimated_input_tokens = estimate_input_tokens(&context_candidates)?;
        let resource_request =
            ResourceRequest::role_call(estimated_input_tokens, evidence.max_output_tokens);
        let reservation_digest = canonical_digest(&json!({
            "step_id": snapshot.step_id,
            "attempt": snapshot.attempt,
            "binding": frozen,
            "input": input_snapshot,
            "resources": resource_request,
        }))?;
        if let Err(error) = repo
            .reserve_resources(
                snapshot.step_id,
                &ctx.lease_owner,
                snapshot.attempt,
                resource_request,
                &reservation_digest,
            )
            .await
        {
            let _ = repo
                .fail_role_prepare(
                    snapshot.step_id,
                    &ctx.lease_owner,
                    snapshot.attempt,
                    Some(agent_run_id),
                    error.code(),
                    &error.to_string(),
                )
                .await;
            return Err(error);
        }

        let request = AuditedModelRequest {
            owner: AuditedCallOwner::AgentRun(agent_run_id),
            // production_steps 不是 agent_steps，审计关联由 durable repository 完成。
            step_id: None,
            root_call_id: None,
            parent_call_id: None,
            attempt: snapshot.attempt,
            agent_key: frozen.definition_key.clone(),
            agent_version: frozen.definition_version.clone(),
            node_key: format!("{}.execute", frozen.definition_key),
            variables: BTreeMap::new(),
            context_candidates,
            context_atomic_groups: Vec::new(),
            compiled_at,
            tool_profile: "chat".into(),
            tool_schema: None,
            binding: frozen.model_binding.clone(),
            context_sources: Value::Array(context_sources),
            memory_sources: json!([]),
            parameters: json!({"resource_reservation_digest": reservation_digest}),
            asset_references: json!([]),
        };
        let prepared_call = match ctx.audited_executor.prepare(request).await {
            Ok(prepared) => prepared,
            Err(error) => {
                let production_error = ProductionError::AgentExecution(error.to_string());
                let _ = repo
                    .fail_role_prepare(
                        snapshot.step_id,
                        &ctx.lease_owner,
                        snapshot.attempt,
                        Some(agent_run_id),
                        production_error.code(),
                        &production_error.to_string(),
                    )
                    .await;
                return Err(production_error);
            }
        };
        let model_call_id = prepared_call.model_call_id();
        let context_snapshot_id = prepared_call.context_snapshot_id();
        if let Err(error) = repo
            .attach_role_prepare_audit(
                snapshot.step_id,
                &ctx.lease_owner,
                snapshot.attempt,
                agent_run_id,
                model_call_id,
                context_snapshot_id,
            )
            .await
        {
            let _ = repo
                .fail_role_prepare(
                    snapshot.step_id,
                    &ctx.lease_owner,
                    snapshot.attempt,
                    Some(agent_run_id),
                    error.code(),
                    &error.to_string(),
                )
                .await;
            return Err(error);
        }
        Ok(PreparedRoleExecution {
            run_id: snapshot.run_id,
            production_project_id: snapshot.production_project_id,
            step_id: snapshot.step_id,
            attempt: snapshot.attempt,
            revision_epoch: snapshot.revision_epoch,
            role_key: snapshot.role_key,
            agent_run_id,
            model_call_id,
            context_snapshot_id,
            input_packages: snapshot.input_packages,
            prepared_call,
        })
    }

    /// 执行已经 prepare 的 provider 请求并完成解析/schema 校验，但不写业务数据库。
    pub async fn execute_prepared(prepared: PreparedRoleExecution) -> ExecutedRoleExecution {
        let started = std::time::Instant::now();
        let PreparedRoleExecution {
            run_id,
            production_project_id,
            step_id,
            attempt,
            revision_epoch,
            role_key,
            agent_run_id,
            model_call_id,
            context_snapshot_id,
            input_packages,
            prepared_call,
        } = prepared;

        let (output, validated_output, output_digest, failure, model_call_finish, output_tokens) =
            match prepared_call.execute().await {
                PreparedAuditedModelOutcome::Failed(provider_failure) => {
                    let failure = RoleFinalizeFailure {
                        code: if provider_failure.result_uncertain {
                            "attention_required".into()
                        } else {
                            "agent_execution_failed".into()
                        },
                        message: provider_failure.message.clone(),
                        result_uncertain: provider_failure.result_uncertain,
                    };
                    (
                        None,
                        None,
                        None,
                        Some(failure),
                        provider_failure.into_finish(),
                        0,
                    )
                }
                PreparedAuditedModelOutcome::Succeeded(provider_success) => {
                    let raw = provider_success.output.clone();
                    let output_tokens = estimate_text_tokens(&raw);
                    let usage = Some(json!({
                        "measurement": "local_utf8_estimate",
                        "output_tokens": output_tokens,
                    }));
                    if provider_success.contains_known_secret() {
                        let message =
                            "provider output contained a configured credential".to_string();
                        let finish = provider_success.finish(
                            AuditedTerminalStatus::Failed,
                            usage,
                            Some(json!({
                                "kind": "unsafe_provider_output",
                                "message": message,
                            })),
                            Some("schema_failed".into()),
                        );
                        (
                            None,
                            None,
                            None,
                            Some(RoleFinalizeFailure {
                                code: "agent_execution_failed".into(),
                                message,
                                result_uncertain: false,
                            }),
                            finish,
                            output_tokens,
                        )
                    } else {
                        match serde_json::from_str::<Value>(&raw) {
                            Err(error) => {
                                let message = format!("JSON parse failed: {error}");
                                let finish = provider_success.finish(
                                    AuditedTerminalStatus::Failed,
                                    usage,
                                    Some(json!({
                                        "kind": "structured_parse",
                                        "message": message,
                                    })),
                                    Some("failed".into()),
                                );
                                (
                                    None,
                                    None,
                                    None,
                                    Some(RoleFinalizeFailure {
                                        code: "invalid_artifact_schema".into(),
                                        message,
                                        result_uncertain: false,
                                    }),
                                    finish,
                                    output_tokens,
                                )
                            }
                            Ok(output) => match validate_role_output(&role_key, &output) {
                                Err(error) => {
                                    let message = error.to_string();
                                    let finish = provider_success.finish(
                                        AuditedTerminalStatus::Failed,
                                        usage,
                                        Some(json!({
                                            "kind": "output_schema",
                                            "message": message,
                                        })),
                                        Some("schema_failed".into()),
                                    );
                                    (
                                        Some(output),
                                        None,
                                        None,
                                        Some(RoleFinalizeFailure {
                                            code: error.code().into(),
                                            message,
                                            result_uncertain: false,
                                        }),
                                        finish,
                                        output_tokens,
                                    )
                                }
                                Ok(validated) => match canonical_digest(&output) {
                                    Ok(digest) => {
                                        let finish = provider_success.finish(
                                            AuditedTerminalStatus::Succeeded,
                                            usage,
                                            None,
                                            Some("succeeded".into()),
                                        );
                                        (
                                            Some(output),
                                            Some(validated),
                                            Some(digest),
                                            None,
                                            finish,
                                            output_tokens,
                                        )
                                    }
                                    Err(error) => {
                                        let message = error.to_string();
                                        let finish = provider_success.finish(
                                            AuditedTerminalStatus::Failed,
                                            usage,
                                            Some(json!({
                                                "kind": "output_digest",
                                                "message": message,
                                            })),
                                            Some("schema_failed".into()),
                                        );
                                        (
                                            Some(output),
                                            None,
                                            None,
                                            Some(RoleFinalizeFailure {
                                                code: "serialization_error".into(),
                                                message,
                                                result_uncertain: false,
                                            }),
                                            finish,
                                            output_tokens,
                                        )
                                    }
                                },
                            },
                        }
                    }
                }
            };

        ExecutedRoleExecution {
            run_id,
            production_project_id,
            step_id,
            attempt,
            revision_epoch,
            role_key,
            agent_run_id,
            model_call_id,
            context_snapshot_id,
            input_packages,
            output,
            validated_output,
            output_digest,
            failure,
            model_call_finish,
            execution_time_ms: started.elapsed().as_millis() as u64,
            output_tokens,
        }
    }

    /// 原子提交本次 role attempt；终态重放返回第一次提交的结果。
    pub async fn finalize(
        ctx: RoleFinalizeContext,
        execution: &ExecutedRoleExecution,
    ) -> ProductionResult<RoleExecutionResult> {
        let repo = DurableProductionRepository::new(ctx.pool);
        let command = RoleFinalizeCommand {
            run_id: execution.run_id,
            production_project_id: execution.production_project_id,
            step_id: execution.step_id,
            attempt: execution.attempt,
            revision_epoch: execution.revision_epoch,
            role_key: execution.role_key.clone(),
            agent_run_id: execution.agent_run_id,
            model_call_id: execution.model_call_id,
            context_snapshot_id: execution.context_snapshot_id,
            input_packages: execution.input_packages.clone(),
            output: execution.output.clone(),
            validated_output: execution.validated_output.clone(),
            output_digest: execution.output_digest.clone(),
            failure: execution.failure.clone(),
            model_call_finish: execution.model_call_finish.clone(),
            execution_time_ms: execution.execution_time_ms,
            output_tokens: execution.output_tokens,
        };
        let record = match repo
            .finalize_role_execution(&ctx.lease_owner, command)
            .await
        {
            Ok(record) => record,
            Err(error @ ProductionError::Database(_)) => {
                let _ = repo
                    .fail_role_finalize_database(
                        execution.step_id,
                        &ctx.lease_owner,
                        execution.attempt,
                        execution.agent_run_id,
                        execution.model_call_id,
                        &error.to_string(),
                    )
                    .await;
                return Err(error);
            }
            Err(error) => return Err(error),
        };
        if let Some(failure) = record.error {
            return Err(role_finalize_error(&failure));
        }
        let output_artifacts =
            record
                .output_artifacts
                .into_iter()
                .map(|artifact| {
                    let artifact_type = serde_json::from_value(json!(artifact.artifact_type))
                        .map_err(|_| ProductionError::TransitionConflict {
                            reason: "stored finalize result contains an unknown artifact type"
                                .into(),
                        })?;
                    Ok(ArtifactSummary {
                        artifact_type,
                        id: artifact.id,
                        version: artifact.version,
                        character_id: artifact.character_id,
                        shot_id: artifact.shot_id,
                    })
                })
                .collect::<ProductionResult<Vec<_>>>()?;
        Ok(RoleExecutionResult {
            role: record.role,
            status: RoleExecutionStatus::Completed,
            execution_time_ms: record.execution_time_ms,
            output_artifacts,
            model_call_id: Some(record.model_call_id),
            next_role: None,
        })
    }

    /// 检查角色所需的输入产物是否全部就绪。
    ///
    /// 若有必需输入产物缺失，返回 `MissingInputArtifact` 错误，
    /// 错误中包含所有缺失产物类型列表。
    pub fn check_inputs_ready(
        def: &RoleDefinition,
        available_artifacts: &[ArtifactType],
    ) -> ProductionResult<()> {
        for required in &def.input_artifacts {
            if !available_artifacts.contains(required) {
                return Err(ProductionError::MissingInputArtifact {
                    artifact_type: format!("{:?}", required),
                });
            }
        }
        Ok(())
    }

    /// 解析并验证角色的完整强类型输出，任何失败都发生在产物持久化之前。
    pub fn validate_output(def: &RoleDefinition, output: &Value) -> ProductionResult<()> {
        validate_role_output(&def.role_key, output)?;
        Ok(())
    }
}

// ─────────────────────────────────────────────
// 辅助函数
// ─────────────────────────────────────────────

fn prompt_bindings(
    registry: &DefinitionRegistry,
    agent: &novex_ai_core::AgentDefinition,
) -> ProductionResult<BTreeMap<String, FixedDefinitionBinding>> {
    agent
        .nodes
        .iter()
        .map(|(node_key, reference)| {
            let prompt = registry
                .prompts()
                .iter()
                .find(|prompt| {
                    prompt.prompt_key == reference.key && prompt.version == reference.version
                })
                .ok_or_else(|| ProductionError::CapabilityMismatch {
                    reason: format!(
                        "prompt {}@{} is absent from DefinitionRegistry",
                        reference.key, reference.version
                    ),
                })?;
            if matches!(
                prompt.status,
                DefinitionStatus::Candidate | DefinitionStatus::Revoked
            ) || prompt.executor_owner != ExecutorOwner::Rust
            {
                return Err(ProductionError::CapabilityMismatch {
                    reason: format!(
                        "prompt {}@{} is not production executable",
                        reference.key, reference.version
                    ),
                });
            }
            Ok((
                node_key.clone(),
                FixedDefinitionBinding {
                    key: reference.key.clone(),
                    version: reference.version.clone(),
                    digest: definition_digest(prompt).map_err(|error| {
                        ProductionError::CapabilityMismatch {
                            reason: error.to_string(),
                        }
                    })?,
                },
            ))
        })
        .collect()
}

async fn validate_frozen_binding(
    snapshot: &RolePrepareSnapshot,
    frozen: &FrozenRoleBindingSnapshot,
    registry: &DefinitionRegistry,
    executor: &AuditedModelExecutor,
) -> ProductionResult<ResolvedBindingEvidence> {
    let expected_agent_key = format!("production.{}", snapshot.role_key);
    if frozen.lifecycle != "active"
        || frozen.definition_key != expected_agent_key
        || frozen.definition_version.trim().is_empty()
    {
        return Err(ProductionError::CapabilityMismatch {
            reason: "ordinary ProductionRun requires an active frozen role Definition".into(),
        });
    }
    let agent = registry
        .agent(&frozen.definition_key, &frozen.definition_version)
        .map_err(|error| ProductionError::CapabilityMismatch {
            reason: error.to_string(),
        })?;
    if agent.status != DefinitionStatus::Active || agent.executor_owner != ExecutorOwner::Rust {
        return Err(ProductionError::CapabilityMismatch {
            reason: "candidate, revoked, or non-Rust Definition cannot execute in ProductionRun"
                .into(),
        });
    }
    let current_definition_digest =
        definition_digest(agent).map_err(|error| ProductionError::CapabilityMismatch {
            reason: error.to_string(),
        })?;
    let current_prompts = prompt_bindings(registry, agent)?;
    if frozen.definition_digest != current_definition_digest
        || frozen.registry_digest != registry.digest()
        || frozen.prompt_bindings != current_prompts
    {
        return Err(ProductionError::CapabilityMismatch {
            reason: "frozen role Definition or Prompt binding no longer matches its audit evidence"
                .into(),
        });
    }
    let evidence = executor
        .build_binding_evidence(
            &frozen.definition_key,
            &frozen.definition_version,
            frozen.model_binding.model_id,
        )
        .await
        .map_err(|error| ProductionError::CapabilityMismatch {
            reason: error.to_string(),
        })?;
    if evidence.binding != frozen.model_binding
        || evidence.capabilities != frozen.model_capabilities
    {
        return Err(ProductionError::CapabilityMismatch {
            reason: "frozen model behavior, capability, Context Policy, or Tokenizer drifted"
                .into(),
        });
    }
    Ok(evidence)
}

fn check_durable_inputs_ready(
    role_def: &RoleDefinition,
    role_registry: &RoleRegistry,
    snapshot: &RolePrepareSnapshot,
) -> ProductionResult<()> {
    let mut available = HashSet::new();
    for package in &snapshot.input_packages {
        for item in &package.items {
            if let Ok(artifact_type) =
                serde_json::from_value::<ArtifactType>(json!(item.artifact_type))
            {
                available.insert(artifact_type);
            }
        }
    }
    for dependency in &snapshot.dependency_anchors {
        let Some(role_key) = dependency.role_key.as_deref() else {
            continue;
        };
        let dependency_role = role_registry.get(role_key)?;
        available.extend(dependency_role.output_artifacts.iter().copied());
    }
    SelfContainedInputCheck::check(role_def, &available)
}

struct SelfContainedInputCheck;

impl SelfContainedInputCheck {
    fn check(role_def: &RoleDefinition, available: &HashSet<ArtifactType>) -> ProductionResult<()> {
        for required in &role_def.input_artifacts {
            if !available.contains(required) {
                return Err(ProductionError::MissingInputArtifact {
                    artifact_type: format!("{required:?}"),
                });
            }
        }
        Ok(())
    }
}

fn build_durable_context_candidates(
    snapshot: &RolePrepareSnapshot,
    compiled_at: &str,
) -> ProductionResult<(Vec<novex_ai_core::ContextCandidate>, Vec<Value>)> {
    let initial_input = snapshot
        .source_snapshot
        .get("initial_input")
        .filter(|value| !value.is_null())
        .ok_or_else(|| ProductionError::TransitionConflict {
            reason: "ProductionRun source snapshot has no persisted user instruction".into(),
        })?;
    let source_digest = canonical_digest(&snapshot.source_snapshot)?;
    let mut candidates = vec![text_context_candidate(TextContextCandidateInput {
        candidate_id: "production_intent_instruction".into(),
        source_kind: "user_instruction".into(),
        source_id: format!("production_run:{}", snapshot.run_id),
        source_version: source_digest.clone(),
        trust: TrustLevel::UserInstruction,
        priority: ContextPriority::P0,
        required: true,
        render_order: 0,
        observed_at: compiled_at.into(),
        text: serde_json::to_string_pretty(initial_input)?,
    })];
    let mut sources = vec![json!({
        "id": "production_intent_instruction",
        "source": "user_instruction",
        "trust": "user_instruction",
        "run_id": snapshot.run_id,
        "digest": source_digest,
        "revision_epoch": snapshot.revision_epoch,
    })];
    let mut render_order = 1u32;
    if let Some(instruction) = &snapshot.revision_instruction {
        if instruction.revision_epoch != snapshot.revision_epoch
            || instruction.owner_role != snapshot.role_key
            || instruction.actor_type != "local_operator"
            || instruction.actor_id.trim().is_empty()
            || instruction.source != "script_revision_command"
            || instruction.trust != "user_instruction"
            || instruction.instruction.trim().is_empty()
            || canonical_digest(&instruction.instruction)? != instruction.instruction_digest
        {
            return Err(ProductionError::TransitionConflict {
                reason: "revision instruction is outside the current governed role input".into(),
            });
        }
        let candidate_id = format!("revision_instruction:{}", instruction.id);
        candidates.push(text_context_candidate(TextContextCandidateInput {
            candidate_id: candidate_id.clone(),
            source_kind: instruction.source.clone(),
            source_id: format!("production_revision_instruction:{}", instruction.id),
            source_version: instruction.instruction_digest.clone(),
            trust: TrustLevel::UserInstruction,
            priority: ContextPriority::P0,
            required: true,
            render_order,
            observed_at: compiled_at.into(),
            text: instruction.instruction.clone(),
        }));
        sources.push(json!({
            "id": candidate_id,
            "source": instruction.source,
            "trust": instruction.trust,
            "instruction_id": instruction.id,
            "actor_type": instruction.actor_type,
            "actor_id": instruction.actor_id,
            "digest": instruction.instruction_digest,
            "revision_epoch": instruction.revision_epoch,
            "owner_role": instruction.owner_role,
        }));
        render_order += 1;
    }
    for package in &snapshot.input_packages {
        let candidate_id = format!("package:{}", package.id);
        candidates.push(text_context_candidate(TextContextCandidateInput {
            candidate_id: candidate_id.clone(),
            source_kind: "project".into(),
            source_id: format!("production_package:{}", package.id),
            source_version: package.digest.clone(),
            trust: TrustLevel::ConfirmedFact,
            priority: ContextPriority::P1,
            required: true,
            render_order,
            observed_at: compiled_at.into(),
            text: serde_json::to_string_pretty(package)?,
        }));
        sources.push(json!({
            "id": candidate_id,
            "source": "project",
            "trust": "confirmed_fact",
            "package_id": package.id,
            "package_type": package.package_type,
            "digest": package.digest,
            "revision_epoch": package.revision_epoch,
        }));
        render_order += 1;
    }
    for dependency in &snapshot.dependency_anchors {
        let candidate_id = format!("dependency:{}", dependency.step_id);
        candidates.push(text_context_candidate(TextContextCandidateInput {
            candidate_id: candidate_id.clone(),
            source_kind: "project".into(),
            source_id: format!("production_step:{}", dependency.step_id),
            source_version: dependency.output_digest.clone(),
            trust: TrustLevel::ConfirmedFact,
            priority: ContextPriority::P1,
            required: true,
            render_order,
            observed_at: compiled_at.into(),
            text: serde_json::to_string_pretty(dependency)?,
        }));
        sources.push(json!({
            "id": candidate_id,
            "source": "project",
            "trust": "confirmed_fact",
            "step_id": dependency.step_id,
            "step_key": dependency.step_key,
            "digest": dependency.output_digest,
        }));
        render_order += 1;
    }
    if let Some(media_review) = &snapshot.media_review {
        let candidate_id = format!("media_evidence:{}", media_review.evidence.evidence_id);
        candidates.push(text_context_candidate(TextContextCandidateInput {
            candidate_id: candidate_id.clone(),
            source_kind: "project".into(),
            source_id: format!(
                "media_evidence_snapshot:{}",
                media_review.evidence.evidence_id
            ),
            source_version: media_review.evidence.evidence_digest.clone(),
            trust: TrustLevel::ConfirmedFact,
            priority: ContextPriority::P1,
            required: true,
            render_order,
            observed_at: compiled_at.into(),
            text: serde_json::to_string_pretty(media_review)?,
        }));
        sources.push(json!({
            "id": candidate_id,
            "source": "media_evidence",
            "trust": "confirmed_fact",
            "inventory_id": media_review.inventory.inventory_id,
            "inventory_digest": media_review.inventory.inventory_digest,
            "evidence_snapshot_id": media_review.evidence.evidence_id,
            "evidence_digest": media_review.evidence.evidence_digest,
            "work_version_id": media_review.inventory.work_version_id,
        }));
    }
    Ok((candidates, sources))
}

fn estimate_input_tokens(candidates: &[novex_ai_core::ContextCandidate]) -> ProductionResult<u64> {
    let bytes = serde_json::to_vec(candidates)?.len() as u64;
    Ok(bytes.saturating_add(3) / 4)
}

fn estimate_text_tokens(text: &str) -> u64 {
    (text.len() as u64).saturating_add(3) / 4
}

fn role_finalize_error(failure: &RoleFinalizeFailure) -> ProductionError {
    match failure.code.as_str() {
        "attention_required" => ProductionError::AttentionRequired,
        "invalid_artifact_schema" => ProductionError::InvalidArtifactSchema {
            details: failure.message.clone(),
        },
        "capability_mismatch" => ProductionError::CapabilityMismatch {
            reason: failure.message.clone(),
        },
        "transition_conflict" => ProductionError::TransitionConflict {
            reason: failure.message.clone(),
        },
        _ => ProductionError::AgentExecution(failure.message.clone()),
    }
}

fn prepared_agent_binding(frozen: &FrozenRoleBindingSnapshot) -> PreparedAgentBindingInput {
    PreparedAgentBindingInput {
        agent_key: frozen.definition_key.clone(),
        agent_version: frozen.definition_version.clone(),
        agent_digest: frozen.definition_digest.clone(),
        prompt_bindings: serde_json::to_value(&frozen.prompt_bindings)
            .unwrap_or_else(|_| json!({})),
        context_policy_bindings: serde_json::to_value(
            &frozen.model_binding.context_policy_bindings,
        )
        .unwrap_or_else(|_| json!({})),
        registry_digest: frozen.registry_digest.clone(),
        model_id: frozen.model_binding.model_id,
        behavior_fingerprint: frozen.model_binding.behavior_fingerprint.clone(),
        model_capabilities: serde_json::to_value(&frozen.model_capabilities)
            .unwrap_or_else(|_| json!({})),
        tokenizer_profile_key: frozen.model_binding.tokenizer_profile.key.clone(),
        tokenizer_profile_version: frozen.model_binding.tokenizer_profile.version.clone(),
        tokenizer_profile_digest: frozen.model_binding.tokenizer_profile.digest.clone(),
    }
}

// ─────────────────────────────────────────────
// 单元测试
// ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::durable::repository::RoleRevisionInstruction;
    use crate::roles::definition::{Lifecycle, PromptRef, RoleDefinition};
    use serde_json::json;

    fn make_def(
        role_key: &str,
        inputs: Vec<ArtifactType>,
        outputs: Vec<ArtifactType>,
    ) -> RoleDefinition {
        RoleDefinition {
            role_key: role_key.to_string(),
            role_name: role_key.to_string(),
            responsibilities: vec![],
            input_artifacts: inputs,
            output_artifacts: outputs,
            allowed_tools: vec![],
            prompt_definition_ref: PromptRef {
                key: format!("{}.general", role_key),
                version: "@1".to_string(),
            },
            lifecycle: Lifecycle::Active,
        }
    }

    #[test]
    fn test_check_inputs_ready_pass() {
        let def = make_def("director", vec![ArtifactType::ScriptDraft], vec![]);
        let available = vec![ArtifactType::ScriptDraft];
        assert!(RoleExecutor::check_inputs_ready(&def, &available).is_ok());
    }

    #[test]
    fn test_check_inputs_missing() {
        let def = make_def("director", vec![ArtifactType::ScriptDraft], vec![]);
        let available = vec![];
        assert!(RoleExecutor::check_inputs_ready(&def, &available).is_err());
    }

    #[test]
    fn test_validate_output_pass() {
        let def = make_def("producer", vec![], vec![ArtifactType::CreativeBrief]);
        let output = json!({
            "creative_brief": {
                "target_audience": "test",
                "tone": ["clear"],
                "key_messages": ["message"],
                "constraints": {},
                "success_criteria": ["complete"]
            }
        });
        assert!(RoleExecutor::validate_output(&def, &output).is_ok());
    }

    #[test]
    fn test_validate_output_missing_field() {
        let def = make_def("producer", vec![], vec![ArtifactType::CreativeBrief]);
        let output = json!({ "other_field": {} });
        assert!(RoleExecutor::validate_output(&def, &output).is_err());
    }

    #[test]
    fn test_validate_output_multiple_artifacts() {
        let def = make_def(
            "screenwriter",
            vec![],
            vec![
                ArtifactType::StoryBible,
                ArtifactType::CharacterBible,
                ArtifactType::ScriptDraft,
            ],
        );
        let output = json!({
            "story_bible": {
                "premise": "test",
                "theme": "theme",
                "narrative_structure": "linear",
                "world": "current"
            },
            "character_bibles": [{
                "character_id": "lead",
                "name": "Lead",
                "role": "protagonist",
                "personality": "careful",
                "motivation": "finish",
                "arc": "blocked to complete"
            }],
            "script_draft": {
                "title": "test",
                "hook": "hook",
                "scenes": [
                    {"sequence": 1, "narration": "narration", "visual_description": "visual", "emotion": "focused", "duration_sec": 5, "character_ids": ["lead"]},
                    {"sequence": 2, "narration": "narration", "visual_description": "visual", "emotion": "focused", "duration_sec": 5, "character_ids": ["lead"]},
                    {"sequence": 3, "narration": "narration", "visual_description": "visual", "emotion": "focused", "duration_sec": 5, "character_ids": ["lead"]}
                ]
            }
        });
        assert!(RoleExecutor::validate_output(&def, &output).is_ok());
    }

    #[test]
    fn durable_revision_instruction_is_compiled_as_audited_user_context() {
        let run_id = Uuid::new_v4();
        let step_id = Uuid::new_v4();
        let instruction_id = Uuid::new_v4();
        let instruction_text = "保持正式脚本不变，只调整镜头运动";
        let instruction_digest = canonical_digest(&instruction_text).unwrap();
        let snapshot = RolePrepareSnapshot {
            run_id,
            production_project_id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            step_id,
            attempt: 1,
            revision_epoch: 2,
            role_key: "director".into(),
            source_snapshot: json!({"initial_input": {"brief": "固定制作意图"}}),
            role_binding: json!({}),
            input_packages: vec![],
            dependency_anchors: vec![],
            revision_instruction: Some(RoleRevisionInstruction {
                id: instruction_id,
                revision_epoch: 2,
                owner_role: "director".into(),
                actor_type: "local_operator".into(),
                actor_id: "local_operator".into(),
                source: "script_revision_command".into(),
                trust: "user_instruction".into(),
                instruction: instruction_text.into(),
                instruction_digest: instruction_digest.clone(),
            }),
            media_review: None,
        };

        let (candidates, sources) =
            build_durable_context_candidates(&snapshot, "2026-07-27T00:00:00.000Z").unwrap();
        let candidate_value = serde_json::to_value(&candidates).unwrap();
        let rendered = candidate_value.to_string();
        assert!(rendered.contains(instruction_text));
        assert!(sources.iter().any(|source| {
            source.get("id") == Some(&json!(format!("revision_instruction:{instruction_id}")))
                && source.get("source") == Some(&json!("script_revision_command"))
                && source.get("trust") == Some(&json!("user_instruction"))
                && source.get("actor_type") == Some(&json!("local_operator"))
                && source.get("actor_id") == Some(&json!("local_operator"))
                && source.get("digest") == Some(&json!(instruction_digest))
                && source.get("revision_epoch") == Some(&json!(2))
        }));
    }

    #[test]
    fn durable_media_evidence_is_compiled_as_audited_confirmed_fact() {
        use crate::durable::media::{
            media_review_readiness, ComposeInput, FinalMediaAsset, MediaEvidenceSnapshot,
            RequiredTakeInventorySnapshot,
        };

        let run_id = Uuid::new_v4();
        let source_step_id = Uuid::new_v4();
        let work_id = Uuid::new_v4();
        let work_version_id = Uuid::new_v4();
        let work_generation_run_id = Uuid::new_v4();
        let final_asset = FinalMediaAsset {
            artifact_id: Uuid::new_v4(),
            sha256: "a".repeat(64),
            mime_type: "video/mp4".into(),
            duration_ms: 8_000,
        };
        let inventory = RequiredTakeInventorySnapshot::build(
            Uuid::new_v4(),
            run_id,
            source_step_id,
            1,
            0,
            work_id,
            work_version_id,
            work_generation_run_id,
            final_asset.clone(),
            "b".repeat(64),
            vec![ComposeInput {
                generation_step_id: Uuid::new_v4(),
                generation_attempt_id: Uuid::new_v4(),
                output_artifact_id: Uuid::new_v4(),
                segment_key: "segment-1".into(),
                scene_ids: vec![Uuid::new_v4()],
                shot_contracts: vec![],
                consumed_by_final_compose: true,
                generation_succeeded: true,
            }],
        );
        assert!(inventory.is_err(), "缺少 Scene/Shot 映射必须先 fail-closed");

        let scene_id = Uuid::new_v4();
        let inventory = RequiredTakeInventorySnapshot::build(
            Uuid::new_v4(),
            run_id,
            source_step_id,
            1,
            0,
            work_id,
            work_version_id,
            work_generation_run_id,
            final_asset.clone(),
            "b".repeat(64),
            vec![ComposeInput {
                generation_step_id: Uuid::new_v4(),
                generation_attempt_id: Uuid::new_v4(),
                output_artifact_id: Uuid::new_v4(),
                segment_key: "segment-1".into(),
                scene_ids: vec![scene_id],
                shot_contracts: vec![(scene_id, vec![Uuid::new_v4()])],
                consumed_by_final_compose: true,
                generation_succeeded: true,
            }],
        )
        .unwrap();
        let evidence = MediaEvidenceSnapshot::build(
            Uuid::new_v4(),
            run_id,
            source_step_id,
            1,
            0,
            work_version_id,
            inventory.inventory_id,
            inventory.inventory_digest.clone(),
            final_asset,
            "vision-fixture-v1".into(),
            "asr-fixture-v1".into(),
            json!({
                "final_media": {"motion": "stable", "audio": "clear"},
                "takes": [{"take_id": inventory.takes[0].take_id, "summary": "complete"}]
            }),
        )
        .unwrap();
        let media_review =
            media_review_readiness(Some(inventory.clone()), Some(evidence.clone())).unwrap();
        let snapshot = RolePrepareSnapshot {
            run_id,
            production_project_id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            step_id: Uuid::new_v4(),
            attempt: 1,
            revision_epoch: 0,
            role_key: "editor".into(),
            source_snapshot: json!({"initial_input": {"brief": "固定制作意图"}}),
            role_binding: json!({}),
            input_packages: vec![],
            dependency_anchors: vec![],
            revision_instruction: None,
            media_review: Some(media_review),
        };

        let (candidates, sources) =
            build_durable_context_candidates(&snapshot, "2026-07-27T00:00:00.000Z").unwrap();
        let rendered = serde_json::to_string(&candidates).unwrap();
        assert!(rendered.contains(&evidence.evidence_id.to_string()));
        assert!(rendered.contains("vision-fixture-v1"));
        assert!(rendered.contains("asr-fixture-v1"));
        assert!(!rendered.to_ascii_lowercase().contains("authorization"));
        assert!(sources.iter().any(|source| {
            source.get("source") == Some(&json!("media_evidence"))
                && source.get("trust") == Some(&json!("confirmed_fact"))
                && source.get("inventory_id") == Some(&json!(inventory.inventory_id))
                && source.get("inventory_digest") == Some(&json!(inventory.inventory_digest))
                && source.get("evidence_snapshot_id") == Some(&json!(evidence.evidence_id))
                && source.get("evidence_digest") == Some(&json!(evidence.evidence_digest))
        }));
    }
}
