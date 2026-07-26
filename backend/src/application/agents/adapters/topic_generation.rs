//! 编排普通与补充选题生成，并在入库前串联质量评估和最多一次重写。

use super::topic_quality::{
    quality_items_by_key, topic_candidate_key, topic_quality_item_passes,
    topic_quality_pass_rate_is_low, TopicQualityLlmOutput,
};
use super::{
    prompt::account_strategy_context_fields, record_step, AgentRuntimeError, TopicAgentAdapter,
};
use crate::domain::conversation::{AgentMessage, AgentMessageRole, CreateAgentStepInput};
use crate::domain::topic::{
    ContentTopic, ContentTopicSource, ContentTopicStatus, TopicGenerationBatch,
    TopicGenerationBatchStatus, TopicQualityEvaluationStatus, TopicQualityGateResult,
};
use crate::repositories::{
    CreateContentTopicInput, CreateTopicGenerationBatchInput, CreateTopicQualityEvaluationInput,
    Project, TopicRepository, UpdateTopicGenerationBatchInput,
};
use chrono::Utc;
use novex_agent::{
    text_context_candidate, AgentOutcome, AgentSession, AuditedCallOwner, AuditedExecutionBinding,
    AuditedModelError, AuditedModelRequest, ModelExecutionRef, StepRecorder, StoredMessage,
    TextContextCandidateInput,
};
use novex_ai_core::{ContextCandidate, ContextPriority, TrustLevel};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use uuid::Uuid;

impl TopicAgentAdapter {
    pub(super) async fn handle_topic_turn(
        &self,
        conversation: &AgentSession,
        user_message: &StoredMessage,
        run_id: Uuid,
        supplement_target_batch_id: Option<Uuid>,
        model: ModelExecutionRef,
        steps: Arc<dyn StepRecorder>,
    ) -> Result<AgentOutcome, AgentRuntimeError> {
        let project_id = conversation
            .project_id
            .ok_or_else(|| AgentRuntimeError::Validation("选题会话缺少 project_id".to_string()))?;
        let topic_repository = &self.topic_repository;
        let project = self.project_repository.get_project(project_id).await?;
        let audited = model.audited.as_ref().ok_or_else(|| {
            AgentRuntimeError::Kernel("audited model execution is required".into())
        })?;

        record_step(
            steps.as_ref(),
            CreateAgentStepInput {
                agent_run_id: run_id,
                step_order: 1,
                step_type: "read_project_context".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "project_id": project_id }),
                output: Some(json!({
                    "name": project.name,
                    "positioning": project.positioning,
                    "description": project.description
                })),
                error_message: None,
            },
        )
        .await?;

        let requested_count = requested_topic_count(&user_message.content);
        let supplement_context = match supplement_target_batch_id {
            Some(target_batch_id) => {
                let root_batch = topic_repository
                    .resolve_supplement_root_batch(project_id, target_batch_id)
                    .await?;
                let existing_topics = topic_repository
                    .list_topics_for_batch_group(project_id, root_batch.id)
                    .await?;
                let history_messages = previous_conversation_messages(
                    self.conversation_repository
                        .list_messages(conversation.id)
                        .await?,
                    user_message.id,
                );
                Some(TopicSupplementPromptContext {
                    root_batch,
                    existing_topics,
                    history_messages,
                })
            }
            None => None,
        };
        let supplement_of_batch_id = supplement_context
            .as_ref()
            .map(|context| context.root_batch.id);
        let batch = topic_repository
            .create_generation_batch(CreateTopicGenerationBatchInput {
                project_id,
                source_run_id: Some(run_id),
                supplement_of_batch_id,
                prompt: user_message.content.clone(),
                requested_count,
                status: TopicGenerationBatchStatus::Running,
                error_message: None,
                metadata: json!({
                    "conversation_id": conversation.id,
                    "supplement_of_batch_id": supplement_of_batch_id
                }),
            })
            .await?;

        let compiled_at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
        let generation_context = build_audited_topic_generation_context(
            &project,
            &batch,
            requested_count,
            user_message,
            supplement_context.as_ref(),
            &compiled_at,
        );
        let node_key = if supplement_context.is_some() {
            "topic.supplement"
        } else {
            "topic.generate"
        };
        let generated = audited
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
                    node_key: node_key.into(),
                    variables: BTreeMap::new(),
                    context_candidates: generation_context.candidates,
                    context_atomic_groups: Vec::new(),
                    compiled_at,
                    tool_profile: "chat".into(),
                    tool_schema: None,
                    binding: audited.binding.clone(),
                    context_sources: Value::Array(generation_context.sources),
                    memory_sources: json!([]),
                    parameters: json!({ "max_output_tokens": 2000 }),
                    asset_references: json!([]),
                },
                |raw| TopicLlmOutput::parse_and_validate(raw).map_err(|error| error.to_string()),
            )
            .await
            .map(|response| (response.output, response.model_call_id))
            .map_err(audited_topic_error);

        let (candidates, generation_model_call_id) = match generated {
            Ok(generated) => generated,
            Err(error) => {
                self.mark_topic_batch_failed(
                    topic_repository.as_ref(),
                    batch.id,
                    error.to_string(),
                )
                .await;
                self.record_failed_topic_step(
                    steps.as_ref(),
                    run_id,
                    2,
                    "generate_topics",
                    error.to_string(),
                )
                .await;
                return Err(error);
            }
        };

        let generation_step_id = record_step(
            steps.as_ref(),
            CreateAgentStepInput {
                agent_run_id: run_id,
                step_order: 2,
                step_type: "generate_topics".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "message_id": user_message.id, "requested_count": requested_count }),
                output: Some(json!({ "topic_count": candidates.len() })),
                error_message: None,
            },
        )
        .await?;
        audited
            .executor
            .associate_step(generation_model_call_id, generation_step_id)
            .await
            .map_err(|error| AgentRuntimeError::Kernel(error.to_string()))?;

        let quality_compiled_at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
        let quality_context = build_audited_topic_quality_context(
            &project,
            &batch,
            user_message,
            &candidates,
            supplement_context.as_ref(),
            &quality_compiled_at,
        );
        let quality = audited
            .executor
            .execute_parsed(
                topic_model_request(
                    audited,
                    run_id,
                    "topic.quality_review",
                    quality_context,
                    quality_compiled_at,
                ),
                |raw| {
                    TopicQualityLlmOutput::parse_and_validate(raw, &candidates)
                        .map(|output| output.result())
                        .map_err(|error| error.to_string())
                },
            )
            .await
            .map(|response| (response.output, response.model_call_id))
            .map_err(audited_topic_error);
        let (quality_result, quality_model_call_id) = match quality {
            Ok(quality) => quality,
            Err(error) => {
                let message = error.to_string();
                self.mark_topic_batch_failed(topic_repository.as_ref(), batch.id, message.clone())
                    .await;
                self.record_failed_topic_step(
                    steps.as_ref(),
                    run_id,
                    3,
                    "evaluate_topic_quality",
                    message.clone(),
                )
                .await;
                self.save_failed_topic_quality_evaluation(
                    topic_repository.as_ref(),
                    project_id,
                    batch.id,
                    run_id,
                    message,
                )
                .await;
                return Err(error);
            }
        };
        let initial_quality_items_by_key = quality_items_by_key(&quality_result);
        let pass_count = candidates
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                let key = topic_candidate_key(*index);
                initial_quality_items_by_key
                    .get(key.as_str())
                    .is_some_and(|item| topic_quality_item_passes(item))
            })
            .count() as i32;
        let reject_count = candidates.len() as i32 - pass_count;
        let quality_evaluation = topic_repository
            .create_topic_quality_evaluation(CreateTopicQualityEvaluationInput {
                project_id,
                batch_id: batch.id,
                source_run_id: Some(run_id),
                status: TopicQualityEvaluationStatus::Succeeded,
                pass_count,
                reject_count,
                rewrite_triggered: false,
                result: quality_result.clone(),
                error_message: None,
            })
            .await?;

        let quality_step_id = record_step(
            steps.as_ref(),
            CreateAgentStepInput {
                agent_run_id: run_id,
                step_order: 3,
                step_type: "evaluate_topic_quality".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "batch_id": batch.id, "candidate_count": candidates.len() }),
                output: Some(json!({
                    "topic_quality_evaluation_id": quality_evaluation.id,
                    "pass_count": pass_count,
                    "reject_count": reject_count,
                    "rewrite_triggered": false
                })),
                error_message: None,
            },
        )
        .await?;
        audited
            .executor
            .associate_step(quality_model_call_id, quality_step_id)
            .await
            .map_err(|error| AgentRuntimeError::Kernel(error.to_string()))?;

        let mut final_candidates = candidates;
        let mut final_quality_result = quality_result;
        let mut final_quality_evaluation = quality_evaluation;
        let mut final_pass_count = pass_count;
        let mut final_reject_count = reject_count;
        let mut rewrite_triggered = false;
        let mut persist_step_order = 4;

        // 低通过率只触发一次同模型重写，防止质量循环失控并限制调用成本。
        if topic_quality_pass_rate_is_low(final_pass_count, final_candidates.len()) {
            let rewrite_compiled_at =
                Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
            let rewrite_context = build_audited_topic_rewrite_context(
                &project,
                &batch,
                requested_count,
                user_message,
                &final_quality_result,
                supplement_context.as_ref(),
                &rewrite_compiled_at,
            );
            let rewrite = audited
                .executor
                .execute_parsed(
                    topic_model_request(
                        audited,
                        run_id,
                        "topic.rewrite",
                        rewrite_context,
                        rewrite_compiled_at,
                    ),
                    |raw| {
                        TopicLlmOutput::parse_and_validate(raw).map_err(|error| error.to_string())
                    },
                )
                .await
                .map(|response| (response.output, response.model_call_id))
                .map_err(audited_topic_error);
            let (rewritten_candidates, rewrite_model_call_id) = match rewrite {
                Ok(rewrite) => rewrite,
                Err(error) => {
                    self.mark_topic_batch_failed(
                        topic_repository.as_ref(),
                        batch.id,
                        error.to_string(),
                    )
                    .await;
                    self.record_failed_topic_step(
                        steps.as_ref(),
                        run_id,
                        4,
                        "rewrite_topics",
                        error.to_string(),
                    )
                    .await;
                    return Err(error);
                }
            };
            let rewrite_step_id = record_step(
                steps.as_ref(),
                CreateAgentStepInput {
                    agent_run_id: run_id,
                    step_order: 4,
                    step_type: "rewrite_topics".to_string(),
                    status: "succeeded".to_string(),
                    input: json!({
                        "batch_id": batch.id,
                        "requested_count": requested_count,
                        "previous_pass_count": final_pass_count,
                        "previous_reject_count": final_reject_count
                    }),
                    output: Some(json!({ "topic_count": rewritten_candidates.len() })),
                    error_message: None,
                },
            )
            .await?;
            audited
                .executor
                .associate_step(rewrite_model_call_id, rewrite_step_id)
                .await
                .map_err(|error| AgentRuntimeError::Kernel(error.to_string()))?;

            let rewrite_quality_compiled_at =
                Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
            let rewrite_quality_context = build_audited_topic_quality_context(
                &project,
                &batch,
                user_message,
                &rewritten_candidates,
                supplement_context.as_ref(),
                &rewrite_quality_compiled_at,
            );
            let rewrite_quality = audited
                .executor
                .execute_parsed(
                    topic_model_request(
                        audited,
                        run_id,
                        "topic.quality_review",
                        rewrite_quality_context,
                        rewrite_quality_compiled_at,
                    ),
                    |raw| {
                        TopicQualityLlmOutput::parse_and_validate(raw, &rewritten_candidates)
                            .map(|output| output.result())
                            .map_err(|error| error.to_string())
                    },
                )
                .await
                .map(|response| (response.output, response.model_call_id))
                .map_err(audited_topic_error);
            let (rewritten_quality_result, rewrite_quality_model_call_id) = match rewrite_quality {
                Ok(quality) => quality,
                Err(error) => {
                    let message = error.to_string();
                    self.mark_topic_batch_failed(
                        topic_repository.as_ref(),
                        batch.id,
                        message.clone(),
                    )
                    .await;
                    self.record_failed_topic_step(
                        steps.as_ref(),
                        run_id,
                        5,
                        "evaluate_topic_quality",
                        message.clone(),
                    )
                    .await;
                    self.save_failed_topic_quality_evaluation(
                        topic_repository.as_ref(),
                        project_id,
                        batch.id,
                        run_id,
                        message,
                    )
                    .await;
                    return Err(error);
                }
            };
            let rewritten_items_by_key = quality_items_by_key(&rewritten_quality_result);
            let rewritten_pass_count = rewritten_candidates
                .iter()
                .enumerate()
                .filter(|(index, _)| {
                    let key = topic_candidate_key(*index);
                    rewritten_items_by_key
                        .get(key.as_str())
                        .is_some_and(|item| topic_quality_item_passes(item))
                })
                .count() as i32;
            let rewritten_reject_count = rewritten_candidates.len() as i32 - rewritten_pass_count;
            let rewritten_quality_evaluation = topic_repository
                .create_topic_quality_evaluation(CreateTopicQualityEvaluationInput {
                    project_id,
                    batch_id: batch.id,
                    source_run_id: Some(run_id),
                    status: TopicQualityEvaluationStatus::Succeeded,
                    pass_count: rewritten_pass_count,
                    reject_count: rewritten_reject_count,
                    rewrite_triggered: true,
                    result: rewritten_quality_result.clone(),
                    error_message: None,
                })
                .await?;
            let rewrite_quality_step_id = record_step(
                steps.as_ref(),
                CreateAgentStepInput {
                    agent_run_id: run_id,
                    step_order: 5,
                    step_type: "evaluate_topic_quality".to_string(),
                    status: "succeeded".to_string(),
                    input: json!({
                        "batch_id": batch.id,
                        "candidate_count": rewritten_candidates.len(),
                        "rewrite_triggered": true
                    }),
                    output: Some(json!({
                        "topic_quality_evaluation_id": rewritten_quality_evaluation.id,
                        "pass_count": rewritten_pass_count,
                        "reject_count": rewritten_reject_count,
                        "rewrite_triggered": true
                    })),
                    error_message: None,
                },
            )
            .await?;
            audited
                .executor
                .associate_step(rewrite_quality_model_call_id, rewrite_quality_step_id)
                .await
                .map_err(|error| AgentRuntimeError::Kernel(error.to_string()))?;

            final_candidates = rewritten_candidates;
            final_quality_result = rewritten_quality_result;
            final_quality_evaluation = rewritten_quality_evaluation;
            final_pass_count = rewritten_pass_count;
            final_reject_count = rewritten_reject_count;
            rewrite_triggered = true;
            persist_step_order = 6;
        }

        if final_pass_count == 0 {
            let message = "质量闸门未产生可用选题".to_string();
            self.mark_topic_batch_failed(topic_repository.as_ref(), batch.id, message.clone())
                .await;
            return Err(AgentRuntimeError::Validation(message));
        }

        let final_quality_items_by_key = quality_items_by_key(&final_quality_result);
        let mut created_topic_ids = Vec::with_capacity(final_pass_count as usize);
        // 淘汰项只保留在质量报告中，只有通过质量闸门的候选才能写入选题池。
        for (index, candidate) in final_candidates.into_iter().enumerate() {
            let candidate_key = topic_candidate_key(index);
            let Some(quality_item) = final_quality_items_by_key.get(candidate_key.as_str()) else {
                continue;
            };
            if !topic_quality_item_passes(quality_item) {
                continue;
            }
            let topic = topic_repository
                .create_topic(CreateContentTopicInput {
                    project_id,
                    batch_id: Some(batch.id),
                    title: candidate.title,
                    angle: candidate.angle,
                    target_audience: candidate.target_audience,
                    hook_points: candidate.hook_points,
                    content_type: candidate.content_type,
                    score: Some(candidate.score.unwrap_or(0.0)),
                    score_reason: candidate.score_reason,
                    tags: candidate.tags,
                    source: ContentTopicSource::Agent,
                    metadata: json!({
                        "source_run_id": run_id,
                        "quality_gate": {
                            "evaluation_id": final_quality_evaluation.id,
                            "candidate_key": candidate_key,
                            "quality_score": quality_item.quality_score,
                            "flags": quality_item.flags,
                            "reason": quality_item.reason
                        }
                    }),
                })
                .await?;
            created_topic_ids.push(topic.id);
        }

        topic_repository
            .update_generation_batch(
                batch.id,
                UpdateTopicGenerationBatchInput {
                    status: TopicGenerationBatchStatus::Succeeded,
                    error_message: None,
                    metadata: json!({
                        "conversation_id": conversation.id,
                        "supplement_of_batch_id": supplement_of_batch_id,
                        "created_topic_ids": created_topic_ids,
                        "topic_count": created_topic_ids.len(),
                        "quality_evaluation_id": final_quality_evaluation.id,
                        "quality_pass_count": final_pass_count,
                        "quality_reject_count": final_reject_count,
                        "quality_rewrite_triggered": rewrite_triggered
                    }),
                },
            )
            .await?;

        record_step(
            steps.as_ref(),
            CreateAgentStepInput {
                agent_run_id: run_id,
                step_order: persist_step_order,
                step_type: "persist_topics".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "batch_id": batch.id }),
                output: Some(json!({
                    "batch_id": batch.id,
                    "supplement_of_batch_id": supplement_of_batch_id,
                    "created_topic_ids": created_topic_ids,
                    "topic_count": created_topic_ids.len(),
                    "quality_evaluation_id": final_quality_evaluation.id
                })),
                error_message: None,
            },
        )
        .await?;

        let topic_count = created_topic_ids.len();
        Ok(AgentOutcome::new(
            format!(
                "已生成 {topic_count} 个候选选题，通过 {final_pass_count} 条，淘汰 {final_reject_count} 条。"
            ),
            json!({
                "intent": "generate_topics",
                "batch_id": batch.id,
                "supplement_of_batch_id": supplement_of_batch_id,
                "created_topic_ids": created_topic_ids,
                "topic_count": topic_count,
                "quality_evaluation_id": final_quality_evaluation.id,
                "quality_pass_count": final_pass_count,
                "quality_reject_count": final_reject_count,
                "quality_rewrite_triggered": rewrite_triggered,
                "status": ContentTopicStatus::Idea
            }),
        ))
    }

    async fn mark_topic_batch_failed(
        &self,
        topic_repository: &dyn TopicRepository,
        batch_id: Uuid,
        error_message: String,
    ) {
        let _ = topic_repository
            .update_generation_batch(
                batch_id,
                UpdateTopicGenerationBatchInput {
                    status: TopicGenerationBatchStatus::Failed,
                    error_message: Some(error_message),
                    metadata: json!({}),
                },
            )
            .await;
    }

    pub(super) async fn record_failed_topic_step(
        &self,
        steps: &dyn StepRecorder,
        run_id: Uuid,
        step_order: i32,
        step_type: &str,
        error_message: String,
    ) {
        let _ = record_step(
            steps,
            CreateAgentStepInput {
                agent_run_id: run_id,
                step_order,
                step_type: step_type.to_string(),
                status: "failed".to_string(),
                input: json!({}),
                output: None,
                error_message: Some(error_message),
            },
        )
        .await;
    }
}

fn audited_topic_error(error: AuditedModelError) -> AgentRuntimeError {
    match error {
        AuditedModelError::Provider { source, .. } => AgentRuntimeError::Llm(source),
        AuditedModelError::StructuredParse { message, .. } => {
            AgentRuntimeError::InvalidLlmOutput(message)
        }
        error => AgentRuntimeError::Kernel(error.to_string()),
    }
}

fn topic_model_request(
    audited: &AuditedExecutionBinding,
    run_id: Uuid,
    node_key: &str,
    context: AuditedTopicContext,
    compiled_at: String,
) -> AuditedModelRequest {
    AuditedModelRequest {
        owner: AuditedCallOwner::AgentRun(run_id),
        step_id: None,
        root_call_id: None,
        parent_call_id: None,
        attempt: 1,
        agent_key: audited.agent_key.clone(),
        agent_version: audited.agent_version.clone(),
        node_key: node_key.into(),
        variables: BTreeMap::new(),
        context_candidates: context.candidates,
        context_atomic_groups: Vec::new(),
        compiled_at,
        tool_profile: "chat".into(),
        tool_schema: None,
        binding: audited.binding.clone(),
        context_sources: Value::Array(context.sources),
        memory_sources: json!([]),
        parameters: json!({ "max_output_tokens": 2000 }),
        asset_references: json!([]),
    }
}

#[derive(Default)]
struct AuditedTopicContext {
    candidates: Vec<ContextCandidate>,
    sources: Vec<Value>,
}

#[allow(clippy::too_many_arguments)]
fn push_topic_context(
    context: &mut AuditedTopicContext,
    candidate_id: String,
    source_kind: &str,
    source_id: String,
    source_version: String,
    trust: TrustLevel,
    priority: ContextPriority,
    required: bool,
    compiled_at: &str,
    content: String,
) {
    context.sources.push(json!({
        "id": candidate_id,
        "trust": trust,
        "source": source_kind,
    }));
    context
        .candidates
        .push(text_context_candidate(TextContextCandidateInput {
            candidate_id,
            source_kind: source_kind.into(),
            source_id,
            source_version,
            trust,
            priority,
            required,
            render_order: context.candidates.len() as u32,
            observed_at: compiled_at.into(),
            text: content,
        }));
}

fn build_audited_topic_generation_context(
    project: &Project,
    batch: &TopicGenerationBatch,
    requested_count: i32,
    user_message: &StoredMessage,
    supplement_context: Option<&TopicSupplementPromptContext>,
    compiled_at: &str,
) -> AuditedTopicContext {
    let mut context = AuditedTopicContext::default();
    let batch_version = batch
        .updated_at
        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    let message_version = user_message
        .created_at
        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    push_topic_context(
        &mut context,
        format!("topic-batch:{}:generation-header", batch.id),
        "topic_batch",
        batch.id.to_string(),
        batch_version.clone(),
        TrustLevel::Reference,
        ContextPriority::P1,
        true,
        compiled_at,
        format!(
            "请基于项目定位和用户补充要求生成 {requested_count} 个候选选题。\n\n账号策略资料："
        ),
    );

    let project_version = project
        .updated_at
        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    for field in account_strategy_context_fields(project) {
        push_topic_context(
            &mut context,
            format!("project:{}:account-strategy:{}", project.id, field.key),
            "account_strategy",
            format!("{}:{}", project.id, field.key),
            project_version.clone(),
            TrustLevel::ConfirmedFact,
            ContextPriority::P1,
            true,
            compiled_at,
            field.rendered,
        );
    }

    push_topic_context(
        &mut context,
        format!("message:{}:current-request", user_message.id),
        "user_instruction",
        user_message.id.to_string(),
        message_version.clone(),
        TrustLevel::UserInstruction,
        ContextPriority::P0,
        true,
        compiled_at,
        format!("\n用户补充要求：{}", user_message.content),
    );

    if let Some(supplement) = supplement_context {
        append_topic_supplement_context(&mut context, supplement, compiled_at);
    }

    push_topic_context(
        &mut context,
        format!("message:{}:output-requirements", user_message.id),
        "user_instruction",
        user_message.id.to_string(),
        message_version.clone(),
        TrustLevel::UserInstruction,
        ContextPriority::P0,
        true,
        compiled_at,
        r#"

输出要求：
1. 必须只输出一个 JSON 对象。
2. 顶层对象必须只包含 topics 字段。
3. topics 数组每项必须包含 title、angle、target_audience、hook_points、content_type、score、score_reason、tags。
4. score 必须是 0 到 100 的数字。
5. hook_points 和 tags 必须是非空字符串数组。
6. 不允许把 topics 写成字符串数组；每个选题必须是包含完整字段的对象。"#
            .to_string(),
    );
    push_topic_context(
        &mut context,
        format!("message:{}:output-example", user_message.id),
        "user_instruction",
        user_message.id.to_string(),
        message_version,
        TrustLevel::UserInstruction,
        ContextPriority::P0,
        true,
        compiled_at,
        r#"
JSON Schema：
{
  "topics": [
    {
      "title": "选题标题",
      "angle": "选题角度",
      "target_audience": "目标受众",
      "hook_points": ["主要看点"],
      "content_type": "knowledge",
      "score": 88,
      "score_reason": "评分理由",
      "tags": ["标签"]
    }
  ]
}"#
        .to_string(),
    );
    context
}

fn append_account_strategy_candidates(
    context: &mut AuditedTopicContext,
    project: &Project,
    compiled_at: &str,
) {
    let project_version = project
        .updated_at
        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    for field in account_strategy_context_fields(project) {
        push_topic_context(
            context,
            format!("project:{}:account-strategy:{}", project.id, field.key),
            "account_strategy",
            format!("{}:{}", project.id, field.key),
            project_version.clone(),
            TrustLevel::ConfirmedFact,
            ContextPriority::P1,
            true,
            compiled_at,
            field.rendered,
        );
    }
}

fn build_audited_topic_quality_context(
    project: &Project,
    batch: &TopicGenerationBatch,
    user_message: &StoredMessage,
    candidates: &[TopicLlmOutput],
    supplement_context: Option<&TopicSupplementPromptContext>,
    compiled_at: &str,
) -> AuditedTopicContext {
    let mut context = AuditedTopicContext::default();
    let message_version = user_message
        .created_at
        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    let batch_version = batch
        .updated_at
        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    push_topic_context(
        &mut context,
        format!("message:{}:quality-header", user_message.id),
        "user_instruction",
        user_message.id.to_string(),
        message_version.clone(),
        TrustLevel::UserInstruction,
        ContextPriority::P0,
        true,
        compiled_at,
        "请评估候选选题是否允许进入选题池。质量闸门只做入库前筛选，不允许自动确认、归档、删除选题或生成脚本。\n\n账号策略资料："
            .to_string(),
    );
    append_account_strategy_candidates(&mut context, project, compiled_at);
    push_topic_context(
        &mut context,
        format!("message:{}:quality-request", user_message.id),
        "user_instruction",
        user_message.id.to_string(),
        message_version.clone(),
        TrustLevel::UserInstruction,
        ContextPriority::P0,
        true,
        compiled_at,
        format!("\n用户生成要求：{}", user_message.content),
    );
    push_topic_context(
        &mut context,
        format!("topic-batch:{}:existing-header", batch.id),
        "topic_batch",
        batch.id.to_string(),
        batch_version.clone(),
        TrustLevel::Reference,
        ContextPriority::P1,
        true,
        compiled_at,
        "\n同主题组已有选题：".to_string(),
    );
    if let Some(supplement) = supplement_context {
        for (index, topic) in supplement.existing_topics.iter().enumerate() {
            let source = if topic.batch_id == Some(supplement.root_batch.id) {
                "原始生成"
            } else {
                "补充生成"
            };
            push_topic_context(
                &mut context,
                format!("topic:{}:quality-existing", topic.id),
                "existing_topic",
                topic.id.to_string(),
                topic
                    .updated_at
                    .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
                TrustLevel::ConfirmedFact,
                ContextPriority::P1,
                true,
                compiled_at,
                format!(
                    "{}. [{}] 标题：{}；角度：{}；标签：{}",
                    index + 1,
                    source,
                    topic.title.trim(),
                    topic.angle.trim(),
                    if topic.tags.is_empty() {
                        "无".to_string()
                    } else {
                        topic.tags.join("、")
                    }
                ),
            );
        }
        if supplement.existing_topics.is_empty() {
            push_empty_topic_context(&mut context, batch, "quality-no-existing", compiled_at);
        }
    } else {
        push_empty_topic_context(&mut context, batch, "quality-no-existing", compiled_at);
    }
    push_topic_context(
        &mut context,
        format!("topic-batch:{}:candidate-header", batch.id),
        "topic_batch",
        batch.id.to_string(),
        batch_version,
        TrustLevel::Reference,
        ContextPriority::P1,
        true,
        compiled_at,
        "\n待评估候选：".to_string(),
    );
    for (index, candidate) in candidates.iter().enumerate() {
        push_topic_context(
            &mut context,
            format!("topic-batch:{}:candidate:{}", batch.id, index + 1),
            "topic_candidate",
            format!("{}:candidate-{}", batch.id, index + 1),
            compiled_at.to_string(),
            TrustLevel::Candidate,
            ContextPriority::P4,
            true,
            compiled_at,
            format_topic_candidate_for_quality(index, candidate),
        );
    }
    push_topic_context(
        &mut context,
        format!("message:{}:quality-rules", user_message.id),
        "user_instruction",
        user_message.id.to_string(),
        message_version,
        TrustLevel::UserInstruction,
        ContextPriority::P0,
        true,
        compiled_at,
        r#"
评估维度：
1. 账号匹配度：是否贴合账号策略资料。
2. 具体度：是否避免百科式、泛化标题。
3. 差异化：是否避免同批或同主题组已有选题重复。
4. 脚本化可行性：是否适合短视频结构化表达。
5. 风险与禁区：是否存在合规风险或明显偏题。
6. 评分可信度：候选原始评分与理由是否一致。

输出要求：
1. 必须只输出一个 JSON 对象。
2. items 必须逐一覆盖待评估候选，candidate_key 必须原样使用。
3. decision 只能是 pass 或 reject。
4. quality_score 必须是 0 到 100 的整数。
5. flags 只能使用 too_generic、duplicate、off_positioning、hard_to_script、compliance_risk、score_untrusted。
6. 出现 off_positioning、compliance_risk 或无法差异化的 duplicate 时必须 reject。
7. quality_score 低于 70 时必须 reject。

JSON Schema：
{
  "summary": "本批次 2 条中 1 条通过，1 条因泛化被淘汰。",
  "items": [
    {
      "candidate_key": "candidate-1",
      "title": "候选标题",
      "decision": "pass",
      "quality_score": 86,
      "flags": [],
      "reason": "贴合账号定位，脚本化路径清晰。"
    }
  ]
}"#
        .to_string(),
    );
    context
}

fn push_empty_topic_context(
    context: &mut AuditedTopicContext,
    batch: &TopicGenerationBatch,
    suffix: &str,
    compiled_at: &str,
) {
    push_topic_context(
        context,
        format!("topic-batch:{}:{suffix}", batch.id),
        "topic_batch",
        batch.id.to_string(),
        batch
            .updated_at
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        TrustLevel::ConfirmedFact,
        ContextPriority::P1,
        true,
        compiled_at,
        "- 无".to_string(),
    );
}

fn format_topic_candidate_for_quality(index: usize, candidate: &TopicLlmOutput) -> String {
    format!(
        "{}. candidate_key={}；标题：{}；角度：{}；目标受众：{}；看点：{}；内容类型：{}；原始评分：{}；评分理由：{}；标签：{}",
        index + 1,
        topic_candidate_key(index),
        candidate.title.trim(),
        candidate.angle.trim(),
        candidate.target_audience.trim(),
        if candidate.hook_points.is_empty() { "无".to_string() } else { candidate.hook_points.join("、") },
        candidate.content_type.trim(),
        candidate.score.map(|score| score.to_string()).unwrap_or_else(|| "无".to_string()),
        candidate.score_reason.trim(),
        if candidate.tags.is_empty() { "无".to_string() } else { candidate.tags.join("、") },
    )
}

fn build_audited_topic_rewrite_context(
    project: &Project,
    batch: &TopicGenerationBatch,
    requested_count: i32,
    user_message: &StoredMessage,
    quality_result: &TopicQualityGateResult,
    supplement_context: Option<&TopicSupplementPromptContext>,
    compiled_at: &str,
) -> AuditedTopicContext {
    let mut context = AuditedTopicContext::default();
    let message_version = user_message
        .created_at
        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    push_topic_context(
        &mut context,
        format!("topic-batch:{}:rewrite-header", batch.id),
        "topic_batch",
        batch.id.to_string(),
        batch
            .updated_at
            .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        TrustLevel::Reference,
        ContextPriority::P1,
        true,
        compiled_at,
        format!(
            "请基于项目定位和用户补充要求生成 {requested_count} 个候选选题。\n\n账号策略资料："
        ),
    );
    append_account_strategy_candidates(&mut context, project, compiled_at);
    push_topic_context(
        &mut context,
        format!("message:{}:rewrite-request", user_message.id),
        "user_instruction",
        user_message.id.to_string(),
        message_version.clone(),
        TrustLevel::UserInstruction,
        ContextPriority::P0,
        true,
        compiled_at,
        format!("\n用户补充要求：{}", user_message.content),
    );
    push_topic_context(
        &mut context,
        format!("topic-batch:{}:rewrite-quality-header", batch.id),
        "topic_batch",
        batch.id.to_string(),
        compiled_at.to_string(),
        TrustLevel::Candidate,
        ContextPriority::P4,
        true,
        compiled_at,
        "\n基于质量闸门淘汰原因重写候选选题。请保留原始用户要求和账号定位，但避开以下问题："
            .to_string(),
    );
    for item in quality_result
        .items
        .iter()
        .filter(|item| !topic_quality_item_passes(item))
    {
        push_topic_context(
            &mut context,
            format!("topic-batch:{}:quality:{}", batch.id, item.candidate_key),
            "topic_candidate",
            format!("{}:{}", batch.id, item.candidate_key),
            compiled_at.to_string(),
            TrustLevel::Candidate,
            ContextPriority::P4,
            true,
            compiled_at,
            format!(
                "- {}：{}；质量分={}；flags={}；原因={}",
                item.candidate_key,
                item.title,
                item.quality_score,
                if item.flags.is_empty() {
                    "无".to_string()
                } else {
                    item.flags
                        .iter()
                        .map(|flag| flag.as_str())
                        .collect::<Vec<_>>()
                        .join("、")
                },
                item.reason
            ),
        );
    }
    push_topic_context(
        &mut context,
        format!("message:{}:rewrite-rules", user_message.id),
        "user_instruction",
        user_message.id.to_string(),
        message_version.clone(),
        TrustLevel::UserInstruction,
        ContextPriority::P0,
        true,
        compiled_at,
        "\n重写要求：\n1. 不要复用被淘汰候选的泛化标题。\n2. 强化具体场景、目标受众、脚本化路径和差异化角度。\n3. 仍然只输出 topic_generation_batch JSON Schema。".to_string(),
    );
    if let Some(supplement) = supplement_context {
        append_topic_supplement_context(&mut context, supplement, compiled_at);
    }
    append_topic_generation_output_context(
        &mut context,
        user_message,
        &message_version,
        compiled_at,
    );
    context
}

fn append_topic_generation_output_context(
    context: &mut AuditedTopicContext,
    user_message: &StoredMessage,
    message_version: &str,
    compiled_at: &str,
) {
    push_topic_context(
        context,
        format!("message:{}:rewrite-output-requirements", user_message.id),
        "user_instruction",
        user_message.id.to_string(),
        message_version.to_string(),
        TrustLevel::UserInstruction,
        ContextPriority::P0,
        true,
        compiled_at,
        "\n\n输出要求：\n1. 必须只输出一个 JSON 对象。\n2. 顶层对象必须只包含 topics 字段。\n3. topics 数组每项必须包含 title、angle、target_audience、hook_points、content_type、score、score_reason、tags。\n4. score 必须是 0 到 100 的数字。\n5. hook_points 和 tags 必须是非空字符串数组。\n6. 不允许把 topics 写成字符串数组；每个选题必须是包含完整字段的对象。".to_string(),
    );
    push_topic_context(
        context,
        format!("message:{}:rewrite-output-example", user_message.id),
        "user_instruction",
        user_message.id.to_string(),
        message_version.to_string(),
        TrustLevel::UserInstruction,
        ContextPriority::P0,
        true,
        compiled_at,
        "\nJSON Schema：\n{\n  \"topics\": [\n    {\n      \"title\": \"选题标题\",\n      \"angle\": \"选题角度\",\n      \"target_audience\": \"目标受众\",\n      \"hook_points\": [\"主要看点\"],\n      \"content_type\": \"knowledge\",\n      \"score\": 88,\n      \"score_reason\": \"评分理由\",\n      \"tags\": [\"标签\"]\n    }\n  ]\n}".to_string(),
    );
}

fn append_topic_supplement_context(
    context: &mut AuditedTopicContext,
    supplement: &TopicSupplementPromptContext,
    compiled_at: &str,
) {
    let root_version = supplement
        .root_batch
        .updated_at
        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    push_topic_context(
        context,
        format!("topic-batch:{}:original-request", supplement.root_batch.id),
        "topic_batch",
        supplement.root_batch.id.to_string(),
        root_version.clone(),
        TrustLevel::Reference,
        ContextPriority::P1,
        true,
        compiled_at,
        format!(
            "\n主题上下文：\n原始生成要求：{}\n已有选题：",
            supplement.root_batch.prompt.trim()
        ),
    );

    if supplement.existing_topics.is_empty() {
        push_topic_context(
            context,
            format!(
                "topic-batch:{}:no-existing-topics",
                supplement.root_batch.id
            ),
            "topic_batch",
            supplement.root_batch.id.to_string(),
            root_version.clone(),
            TrustLevel::ConfirmedFact,
            ContextPriority::P1,
            true,
            compiled_at,
            "- 无".to_string(),
        );
    } else {
        for (index, topic) in supplement.existing_topics.iter().enumerate() {
            let source = if topic.batch_id == Some(supplement.root_batch.id) {
                "原始生成"
            } else {
                "补充生成"
            };
            let tags = if topic.tags.is_empty() {
                "无".to_string()
            } else {
                topic.tags.join("、")
            };
            push_topic_context(
                context,
                format!("topic:{}:existing", topic.id),
                "existing_topic",
                topic.id.to_string(),
                topic
                    .updated_at
                    .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
                TrustLevel::ConfirmedFact,
                ContextPriority::P1,
                true,
                compiled_at,
                format!(
                    "{}. [{}] 标题：{}；角度：{}；标签：{}",
                    index + 1,
                    source,
                    topic.title.trim(),
                    topic.angle.trim(),
                    tags.trim()
                ),
            );
        }
    }

    push_topic_context(
        context,
        format!("topic-batch:{}:history-header", supplement.root_batch.id),
        "topic_batch",
        supplement.root_batch.id.to_string(),
        root_version.clone(),
        TrustLevel::Reference,
        ContextPriority::P2,
        false,
        compiled_at,
        "历史对话摘要：".to_string(),
    );
    if supplement.history_messages.is_empty() {
        push_topic_context(
            context,
            format!("topic-batch:{}:no-history", supplement.root_batch.id),
            "topic_batch",
            supplement.root_batch.id.to_string(),
            root_version.clone(),
            TrustLevel::Reference,
            ContextPriority::P2,
            false,
            compiled_at,
            "- 无".to_string(),
        );
    } else {
        for (index, message) in supplement.history_messages.iter().enumerate() {
            let trust = if message.role == AgentMessageRole::User {
                TrustLevel::UserInstruction
            } else {
                TrustLevel::Reference
            };
            push_topic_context(
                context,
                format!(
                    "conversation:{}:history:{}",
                    message.conversation_id, message.id
                ),
                "conversation_entry",
                message.id.to_string(),
                message
                    .created_at
                    .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
                trust,
                ContextPriority::P2,
                false,
                compiled_at,
                format!(
                    "{}. {}：{}",
                    index + 1,
                    agent_message_role_label(&message.role),
                    message.content.trim()
                ),
            );
        }
    }
    push_topic_context(
        context,
        format!("topic-batch:{}:supplement-rules", supplement.root_batch.id),
        "topic_batch",
        supplement.root_batch.id.to_string(),
        root_version,
        TrustLevel::Reference,
        ContextPriority::P1,
        true,
        compiled_at,
        r#"
补充生成要求：
1. 必须基于同一主题继续扩展，不要转向无关主题。
2. 必须避免重复已有选题的标题、角度和核心看点。
3. 可以补充遗漏人群、场景、反例、复盘、工具链或执行细节，但必须延续原始生成要求。"#
            .to_string(),
    );
}

#[derive(Debug, Deserialize)]
struct TopicLlmOutputEnvelope {
    topics: Vec<TopicLlmOutput>,
}

#[derive(Debug, Deserialize)]
pub(super) struct TopicLlmOutput {
    pub(super) title: String,
    pub(super) angle: String,
    pub(super) target_audience: String,
    pub(super) hook_points: Vec<String>,
    pub(super) content_type: String,
    pub(super) score: Option<f64>,
    pub(super) score_reason: String,
    pub(super) tags: Vec<String>,
}

impl TopicLlmOutput {
    pub(super) fn parse_and_validate(raw: &str) -> Result<Vec<Self>, TopicOutputError> {
        let json_text = extract_topic_json_object(raw)?;
        let mut envelope: TopicLlmOutputEnvelope =
            serde_json::from_str(json_text).map_err(|error| TopicOutputError::InvalidJson {
                message: error.to_string(),
            })?;
        let topics = &mut envelope.topics;
        if topics.is_empty() {
            return Err(TopicOutputError::Validation(
                "topic output must not be empty".to_string(),
            ));
        }
        for topic in topics {
            topic.normalize();
            topic.validate()?;
        }
        Ok(envelope.topics)
    }

    fn normalize(&mut self) {
        self.title = self.title.trim().to_string();
        self.angle = self.angle.trim().to_string();
        self.target_audience = self.target_audience.trim().to_string();
        self.content_type = self.content_type.trim().to_string();
        self.score_reason = self.score_reason.trim().to_string();
        self.hook_points = self
            .hook_points
            .drain(..)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect();
        self.tags = self
            .tags
            .drain(..)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect();
    }

    fn validate(&self) -> Result<(), TopicOutputError> {
        if self.title.is_empty() {
            return Err(TopicOutputError::Validation(
                "title is required".to_string(),
            ));
        }
        if self.angle.is_empty() {
            return Err(TopicOutputError::Validation(
                "angle is required".to_string(),
            ));
        }
        if self.target_audience.is_empty() {
            return Err(TopicOutputError::Validation(
                "target_audience is required".to_string(),
            ));
        }
        if self.hook_points.is_empty() {
            return Err(TopicOutputError::Validation(
                "hook_points is required".to_string(),
            ));
        }
        if self.content_type.is_empty() {
            return Err(TopicOutputError::Validation(
                "content_type is required".to_string(),
            ));
        }
        let Some(score) = self.score else {
            return Err(TopicOutputError::Validation(
                "score is required".to_string(),
            ));
        };
        if !(0.0..=100.0).contains(&score) {
            return Err(TopicOutputError::Validation(
                "score must be between 0 and 100".to_string(),
            ));
        }
        if self.score_reason.is_empty() {
            return Err(TopicOutputError::Validation(
                "score_reason is required".to_string(),
            ));
        }
        if self.tags.is_empty() {
            return Err(TopicOutputError::Validation("tags is required".to_string()));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(super) enum TopicOutputError {
    InvalidJson { message: String },
    Validation(String),
}

impl fmt::Display for TopicOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson { message } => write!(formatter, "invalid topic JSON: {message}"),
            Self::Validation(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for TopicOutputError {}

/// 补充生成使用同一主题组和有限会话历史，避免生成语义漂移。
#[derive(Clone, Debug)]
pub(super) struct TopicSupplementPromptContext {
    pub(super) root_batch: TopicGenerationBatch,
    pub(super) existing_topics: Vec<ContentTopic>,
    pub(super) history_messages: Vec<AgentMessage>,
}

pub(super) fn previous_conversation_messages(
    messages: Vec<AgentMessage>,
    current_user_message_id: Uuid,
) -> Vec<AgentMessage> {
    messages
        .into_iter()
        .filter(|message| message.id != current_user_message_id)
        .filter(|message| !message.content.trim().is_empty())
        .collect()
}

fn agent_message_role_label(role: &AgentMessageRole) -> &'static str {
    match role {
        AgentMessageRole::System => "system",
        AgentMessageRole::User => "user",
        AgentMessageRole::Assistant => "assistant",
        AgentMessageRole::Tool => "tool",
    }
}

pub(super) fn requested_topic_count(user_message: &str) -> i32 {
    let parsed = user_message
        .split(|character: char| !character.is_ascii_digit())
        .find_map(|segment| segment.parse::<i32>().ok())
        .unwrap_or(5);
    parsed.clamp(1, 20)
}

fn extract_topic_json_object(raw: &str) -> Result<&str, TopicOutputError> {
    let start = raw
        .find('{')
        .ok_or_else(|| TopicOutputError::Validation("missing JSON object start".to_string()))?;
    let end = raw
        .rfind('}')
        .ok_or_else(|| TopicOutputError::Validation("missing JSON object end".to_string()))?;
    if start > end {
        return Err(TopicOutputError::Validation(
            "invalid JSON object bounds".to_string(),
        ));
    }
    Ok(&raw[start..=end])
}
