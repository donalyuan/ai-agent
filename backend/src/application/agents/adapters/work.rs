//! 作品 Agent adapter：把自然语言意图转换为受约束快照补丁，并原子生成待确认差异。

use super::{record_step, AgentRuntimeError, WorkAgentAdapter};
use crate::domain::conversation::CreateAgentStepInput;
use chrono::Utc;
use novex_agent::{
    text_context_candidate, AgentOutcome, AgentSession, AuditedCallOwner, AuditedModelError,
    AuditedModelRequest, ModelExecutionRef, StepRecorder, StoredMessage, TextContextCandidateInput,
};
use novex_ai_core::{ContextPriority, TrustLevel};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkAgentOutput {
    assistant_message: String,
    #[serde(default)]
    input_snapshot_patch: Option<Value>,
    #[serde(default)]
    model_snapshot_patch: Option<Value>,
    #[serde(default)]
    parameter_snapshot_patch: Option<Value>,
    #[serde(default)]
    prompt_snapshot_patch: Option<Value>,
    #[serde(default)]
    timeline_snapshot_patch: Option<Value>,
}

impl WorkAgentOutput {
    fn validate(&self) -> Result<(), AgentRuntimeError> {
        if self.assistant_message.trim().is_empty() {
            return Err(AgentRuntimeError::InvalidLlmOutput(
                "assistant_message 不能为空".to_string(),
            ));
        }
        let patches = self.patches();
        if patches.iter().all(|patch| patch.is_none()) {
            return Err(AgentRuntimeError::InvalidLlmOutput(
                "至少需要一个非空 snapshot patch".to_string(),
            ));
        }
        if patches.into_iter().flatten().any(|patch| {
            !patch.is_object() || patch.as_object().is_some_and(|value| value.is_empty())
        }) {
            return Err(AgentRuntimeError::InvalidLlmOutput(
                "snapshot patch 必须是非空 object".to_string(),
            ));
        }
        Ok(())
    }

    fn patches(&self) -> [&Option<Value>; 5] {
        [
            &self.input_snapshot_patch,
            &self.model_snapshot_patch,
            &self.parameter_snapshot_patch,
            &self.prompt_snapshot_patch,
            &self.timeline_snapshot_patch,
        ]
    }
}

impl WorkAgentAdapter {
    pub(super) async fn handle_work_turn(
        &self,
        conversation: &AgentSession,
        user_message: &StoredMessage,
        run_id: Uuid,
        model: ModelExecutionRef,
        steps: Arc<dyn StepRecorder>,
    ) -> Result<AgentOutcome, AgentRuntimeError> {
        let project_id = conversation
            .project_id
            .ok_or_else(|| AgentRuntimeError::Validation("作品会话必须绑定项目".to_string()))?;
        let Some(work_id) = conversation.subject_id else {
            if !self.project_repository.project_exists(project_id).await? {
                return Err(AgentRuntimeError::Validation(
                    "作品会话绑定项目不存在".to_string(),
                ));
            }
            let plan_step_id = record_step(
                steps.as_ref(),
                CreateAgentStepInput {
                    agent_run_id: run_id,
                    step_order: 1,
                    step_type: "recommend_work_plan".into(),
                    status: "succeeded".into(),
                    input: json!({"message_id": user_message.id}),
                    output: Some(
                        json!({"requires_confirmation": true, "downstream_called": false}),
                    ),
                    error_message: None,
                },
            )
            .await?;
            let audited = model.audited.as_ref().ok_or_else(|| {
                AgentRuntimeError::Kernel("audited model execution is required".into())
            })?;
            let reply = audited
                .executor
                .execute(AuditedModelRequest {
                        owner: AuditedCallOwner::AgentRun(run_id),
                        step_id: Some(plan_step_id),
                        root_call_id: None,
                        parent_call_id: None,
                        attempt: 1,
                        agent_key: audited.agent_key.clone(),
                        agent_version: audited.agent_version.clone(),
                        node_key: "work.plan".into(),
                        variables: BTreeMap::new(),
                        context_candidates: vec![text_context_candidate(
                            TextContextCandidateInput {
                                candidate_id: format!("message:{}", user_message.id),
                                source_kind: "conversation_entry".into(),
                                source_id: user_message.id.to_string(),
                                source_version: user_message.created_at.to_rfc3339_opts(
                                    chrono::SecondsFormat::Nanos,
                                    true,
                                ),
                                trust: TrustLevel::UserInstruction,
                                priority: ContextPriority::P0,
                                required: true,
                                render_order: 0,
                                observed_at: Utc::now()
                                    .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
                                text: user_message.content.clone(),
                            },
                        )],
                        context_atomic_groups: Vec::new(),
                        compiled_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
                        tool_profile: "chat".into(),
                        tool_schema: None,
                        binding: audited.binding.clone(),
                        context_sources: json!([{"id": format!("message:{}", user_message.id), "trust": "user_instruction", "source": "conversation_user_message"}]),
                        memory_sources: json!([]),
                        parameters: json!({"max_output_tokens": 1200}),
                        asset_references: json!([]),
                })
                .await
                .map(|response| response.output)
                .map_err(work_audited_error)?;
            return Ok(AgentOutcome::new(
                reply.trim(),
                json!({"intent": "recommend_work_plan", "requires_confirmation": true, "downstream_called": false}),
            ));
        };
        let repository = self.work_library_repository.as_ref();
        if conversation.subject_type.as_deref() != Some("work") {
            return Err(AgentRuntimeError::Validation(
                "作品会话 subject_type 必须为 work".to_string(),
            ));
        }
        let context = repository
            .work_agent_context(work_id, project_id)
            .await
            .map_err(|error| AgentRuntimeError::Validation(error.to_string()))?;
        record_step(steps.as_ref(), CreateAgentStepInput {
                agent_run_id: run_id,
                step_order: 1,
                step_type: "read_work_manifest".into(),
                status: "succeeded".into(),
                input: json!({"subject_id": conversation.subject_id}),
                output: Some(json!({"work_id": work_id, "version_no": context["current_version"]["version_no"]})),
                error_message: None,
            })
            .await?;
        let observed_at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
        let work_version = context["current_version"]["version_no"]
            .as_i64()
            .map_or_else(|| "unknown".to_string(), |value| value.to_string());
        let work_context = format!(
            "当前作品和草稿：\n{}\n",
            serde_json::to_string_pretty(&context).unwrap_or_else(|_| "{}".to_string())
        );
        let user_instruction = format!("用户修改要求：\n{}", user_message.content);
        let audited = model.audited.as_ref().ok_or_else(|| {
            AgentRuntimeError::Kernel("audited model execution is required".into())
        })?;
        let work_candidate_id = format!("work:{work_id}:manifest:{work_version}");
        let instruction_candidate_id = format!("message:{}", user_message.id);
        let response = audited
            .executor
            .execute_parsed(
                        AuditedModelRequest {
                            owner: AuditedCallOwner::AgentRun(run_id),
                            step_id: None,
                            root_call_id: None,
                            parent_call_id: None,
                            attempt: 1,
                            agent_key: audited.agent_key.clone(),
                            agent_version: audited.agent_version.clone(),
                            node_key: "work.patch".into(),
                            variables: BTreeMap::new(),
                            context_candidates: vec![
                                text_context_candidate(TextContextCandidateInput {
                                    candidate_id: work_candidate_id.clone(),
                                    source_kind: "current_work".into(),
                                    source_id: work_id.to_string(),
                                    source_version: work_version,
                                    trust: TrustLevel::ConfirmedFact,
                                    priority: ContextPriority::P1,
                                    required: true,
                                    render_order: 0,
                                    observed_at: observed_at.clone(),
                                    text: work_context,
                                }),
                                text_context_candidate(TextContextCandidateInput {
                                    candidate_id: instruction_candidate_id.clone(),
                                    source_kind: "conversation_entry".into(),
                                    source_id: user_message.id.to_string(),
                                    source_version: user_message.created_at.to_rfc3339_opts(
                                        chrono::SecondsFormat::Nanos,
                                        true,
                                    ),
                                    trust: TrustLevel::UserInstruction,
                                    priority: ContextPriority::P0,
                                    required: true,
                                    render_order: 1,
                                    observed_at,
                                    text: user_instruction,
                                }),
                            ],
                            context_atomic_groups: Vec::new(),
                            compiled_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
                            tool_profile: "chat".into(),
                            tool_schema: None,
                            binding: audited.binding.clone(),
                            context_sources: json!([
                                {"id": format!("work:{work_id}:manifest"), "trust": "confirmed_fact", "source": "work_manifest"},
                                {"id": format!("message:{}", user_message.id), "trust": "user_instruction", "source": "conversation_user_message"},
                                {"id": work_candidate_id, "trust": "confirmed_fact", "source": "work_manifest"},
                                {"id": instruction_candidate_id, "trust": "user_instruction", "source": "conversation_user_message"}
                            ]),
                            memory_sources: json!([]),
                            parameters: json!({"max_output_tokens": 1800}),
                            asset_references: json!([]),
                        },
                        parse_work_agent_output,
            )
            .await
            .map_err(work_audited_error)?;
        let (output, model_call_id) = (response.output, response.model_call_id);
        let (draft, diff) = repository
            .apply_agent_edit(work_id, project_id, output.patches())
            .await?;
        let patch_step_id = record_step(steps.as_ref(), CreateAgentStepInput {
                agent_run_id: run_id,
                step_order: 2,
                step_type: "apply_work_patch".into(),
                status: "succeeded".into(),
                input: json!({"message_id": user_message.id}),
                output: Some(json!({"draft_version_id": draft.id, "diff_plan_id": diff.id, "requires_confirmation": true, "downstream_called": false})),
                error_message: None,
            })
            .await?;
        audited
            .executor
            .associate_step(model_call_id, patch_step_id)
            .await
            .map_err(|error| AgentRuntimeError::Kernel(error.to_string()))?;
        Ok(AgentOutcome::new(
            output.assistant_message.trim(),
            json!({
                "intent": "edit_work_draft",
                "draft_version_id": draft.id,
                "version_no": draft.version_no,
                "diff": diff,
                "requires_confirmation": true,
                "downstream_called": false,
            }),
        ))
    }
}

fn parse_work_agent_output(raw: &str) -> Result<WorkAgentOutput, String> {
    let output: WorkAgentOutput =
        serde_json::from_str(raw.trim()).map_err(|error| format!("作品补丁 JSON 无效: {error}"))?;
    output.validate().map_err(|error| error.to_string())?;
    Ok(output)
}

fn work_audited_error(error: AuditedModelError) -> AgentRuntimeError {
    match error {
        AuditedModelError::Provider { source, .. } => AgentRuntimeError::Llm(source),
        AuditedModelError::StructuredParse { message, .. } => {
            AgentRuntimeError::InvalidLlmOutput(message)
        }
        error => AgentRuntimeError::Kernel(error.to_string()),
    }
}
