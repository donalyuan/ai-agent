//! 编排普通与补充选题生成，并在入库前串联质量评估和最多一次重写。

use super::topic_quality::{
    build_topic_quality_gate_prompt, build_topic_rewrite_user_message, quality_items_by_key,
    topic_candidate_key, topic_quality_item_passes, topic_quality_pass_rate_is_low,
    TopicQualityLlmOutput,
};
use super::{
    format_account_strategy_context, record_step, truncate_for_prompt, AgentRuntimeError,
    TopicAgentAdapter,
};
use crate::domain::conversation::{AgentMessage, AgentMessageRole, CreateAgentStepInput};
use crate::domain::topic::{
    ContentTopic, ContentTopicSource, ContentTopicStatus, TopicGenerationBatch,
    TopicGenerationBatchStatus, TopicQualityEvaluationStatus,
};
use crate::repositories::{
    CreateContentTopicInput, CreateTopicGenerationBatchInput, CreateTopicQualityEvaluationInput,
    TopicRepository, UpdateTopicGenerationBatchInput,
};
use novex_agent::{
    AgentOutcome, AgentSession, AuditedCallOwner, AuditedExecutionBinding, AuditedModelError,
    AuditedModelRequest, ModelExecutionRef, StepRecorder, StoredMessage,
};
use novex_ai_core::{DynamicFragment, PromptCompileInput, TrustLevel};
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
        let account_strategy_context = format_account_strategy_context(&project);
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

        let generation_prompt = build_topic_generation_prompt(
            &account_strategy_context,
            requested_count,
            &user_message.content,
            supplement_context.as_ref(),
        );
        let fragment_id = format!("topic-batch:{}:generation", batch.id);
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
                    compile_input: PromptCompileInput {
                        schema_version: "1".into(),
                        variables: BTreeMap::new(),
                        fragments: vec![DynamicFragment {
                            id: fragment_id.clone(),
                            trust: TrustLevel::Reference,
                            source: "topic_generation_context".into(),
                            content: Some(generation_prompt.clone()),
                            asset: None,
                        }],
                    },
                    tool_profile: "chat".into(),
                    tool_schema: None,
                    binding: audited.binding.clone(),
                    context_sources: topic_generation_context_sources(
                        &fragment_id,
                        project_id,
                        user_message.id,
                        supplement_context.as_ref(),
                    ),
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

        let quality_prompt = build_topic_quality_gate_prompt(
            &account_strategy_context,
            &user_message.content,
            &candidates,
            supplement_context.as_ref(),
        );
        let quality = audited
            .executor
            .execute_parsed(
                    topic_model_request(
                        audited,
                        run_id,
                        "topic.quality_review",
                        format!("topic-batch:{}:quality-review", batch.id),
                        "topic_quality_candidates",
                        TrustLevel::Candidate,
                        quality_prompt.clone(),
                        json!([
                            {"id": format!("project:{project_id}:account-strategy"), "trust": "confirmed_fact", "source": "project_account_strategy"},
                            {"id": format!("topic-batch:{}:candidates", batch.id), "trust": "candidate", "source": "topic_generation_candidates"}
                        ]),
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
            let rewrite_user_message =
                build_topic_rewrite_user_message(&user_message.content, &final_quality_result);
            let rewrite_prompt = build_topic_generation_prompt(
                &account_strategy_context,
                requested_count,
                &rewrite_user_message,
                supplement_context.as_ref(),
            );
            let rewrite = audited
                .executor
                .execute_parsed(
                        topic_model_request(
                            audited,
                            run_id,
                            "topic.rewrite",
                            format!("topic-batch:{}:rewrite", batch.id),
                            "topic_quality_rewrite",
                            TrustLevel::Candidate,
                            rewrite_prompt.clone(),
                            json!([
                                {"id": format!("project:{project_id}:account-strategy"), "trust": "confirmed_fact", "source": "project_account_strategy"},
                                {"id": format!("topic-batch:{}:quality-result", batch.id), "trust": "candidate", "source": "topic_quality_result"}
                            ]),
                        ),
                        |raw| TopicLlmOutput::parse_and_validate(raw).map_err(|error| error.to_string()),
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

            let rewrite_quality_prompt = build_topic_quality_gate_prompt(
                &account_strategy_context,
                &user_message.content,
                &rewritten_candidates,
                supplement_context.as_ref(),
            );
            let rewrite_quality = audited
                .executor
                .execute_parsed(
                        topic_model_request(
                            audited,
                            run_id,
                            "topic.quality_review",
                            format!("topic-batch:{}:rewrite-quality", batch.id),
                            "topic_rewrite_candidates",
                            TrustLevel::Candidate,
                            rewrite_quality_prompt.clone(),
                            json!([
                                {"id": format!("project:{project_id}:account-strategy"), "trust": "confirmed_fact", "source": "project_account_strategy"},
                                {"id": format!("topic-batch:{}:rewritten-candidates", batch.id), "trust": "candidate", "source": "topic_rewrite_candidates"}
                            ]),
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

#[allow(clippy::too_many_arguments)]
fn topic_model_request(
    audited: &AuditedExecutionBinding,
    run_id: Uuid,
    node_key: &str,
    fragment_id: String,
    fragment_source: &str,
    trust: TrustLevel,
    content: String,
    context_sources: Value,
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
        compile_input: PromptCompileInput {
            schema_version: "1".into(),
            variables: BTreeMap::new(),
            fragments: vec![DynamicFragment {
                id: fragment_id,
                trust,
                source: fragment_source.into(),
                content: Some(content),
                asset: None,
            }],
        },
        tool_profile: "chat".into(),
        tool_schema: None,
        binding: audited.binding.clone(),
        context_sources,
        memory_sources: json!([]),
        parameters: json!({ "max_output_tokens": 2000 }),
        asset_references: json!([]),
    }
}

fn topic_generation_context_sources(
    compiled_fragment_id: &str,
    project_id: Uuid,
    user_message_id: Uuid,
    supplement_context: Option<&TopicSupplementPromptContext>,
) -> Value {
    let mut sources = vec![
        json!({
            "id": compiled_fragment_id,
            "trust": "reference",
            "source": "topic_generation_context"
        }),
        json!({
            "id": format!("project:{project_id}:account-strategy"),
            "trust": "confirmed_fact",
            "source": "project_account_strategy"
        }),
        json!({
            "id": format!("message:{user_message_id}"),
            "trust": "user_instruction",
            "source": "conversation_user_message"
        }),
    ];
    if let Some(context) = supplement_context {
        sources.push(json!({
            "id": format!("topic-batch:{}:existing-topics", context.root_batch.id),
            "trust": "reference",
            "source": "topic_batch_group"
        }));
        for message in &context.history_messages {
            sources.push(json!({
                "id": format!("message:{}", message.id),
                "trust": "reference",
                "source": "conversation_history"
            }));
        }
    }
    Value::Array(sources)
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
    let mut previous_messages = messages
        .into_iter()
        .filter(|message| message.id != current_user_message_id)
        .filter(|message| !message.content.trim().is_empty())
        .collect::<Vec<_>>();
    const MAX_HISTORY_MESSAGES: usize = 6;
    if previous_messages.len() > MAX_HISTORY_MESSAGES {
        previous_messages =
            previous_messages.split_off(previous_messages.len() - MAX_HISTORY_MESSAGES);
    }
    previous_messages
}

pub(super) fn build_topic_generation_prompt(
    account_strategy_context: &str,
    requested_count: i32,
    user_message: &str,
    supplement_context: Option<&TopicSupplementPromptContext>,
) -> String {
    let supplement_context_text = supplement_context
        .map(format_topic_supplement_context)
        .unwrap_or_default();
    format!(
        r#"请基于项目定位和用户补充要求生成 {requested_count} 个候选选题。

账号策略资料：
{account_strategy_context}

用户补充要求：{user_message}
{supplement_context_text}

输出要求：
1. 必须只输出一个 JSON 对象。
2. 顶层对象必须只包含 topics 字段。
3. topics 数组每项必须包含 title、angle、target_audience、hook_points、content_type、score、score_reason、tags。
4. score 必须是 0 到 100 的数字。
5. hook_points 和 tags 必须是非空字符串数组。
6. 不允许把 topics 写成字符串数组；每个选题必须是包含完整字段的对象。

JSON Schema：
{{
  "topics": [
    {{
      "title": "选题标题",
      "angle": "选题角度",
      "target_audience": "目标受众",
      "hook_points": ["主要看点"],
      "content_type": "knowledge",
      "score": 88,
      "score_reason": "评分理由",
      "tags": ["标签"]
    }}
  ]
}}"#,
        requested_count = requested_count,
        account_strategy_context = account_strategy_context,
        user_message = user_message,
        supplement_context_text = supplement_context_text
    )
}

fn format_topic_supplement_context(context: &TopicSupplementPromptContext) -> String {
    format!(
        r#"
主题上下文：
原始生成要求：{original_prompt}
已有选题：
{existing_topics}
历史对话摘要：
{history_messages}

补充生成要求：
1. 必须基于同一主题继续扩展，不要转向无关主题。
2. 必须避免重复已有选题的标题、角度和核心看点。
3. 可以补充遗漏人群、场景、反例、复盘、工具链或执行细节，但必须延续原始生成要求。
"#,
        original_prompt = truncate_for_prompt(&context.root_batch.prompt, 500),
        existing_topics = format_existing_topic_context(context),
        history_messages = format_conversation_history(&context.history_messages)
    )
}

pub(super) fn format_existing_topic_context(context: &TopicSupplementPromptContext) -> String {
    if context.existing_topics.is_empty() {
        return "- 无".to_string();
    }

    const MAX_EXISTING_TOPICS: usize = 20;
    context
        .existing_topics
        .iter()
        .take(MAX_EXISTING_TOPICS)
        .enumerate()
        .map(|(index, topic)| {
            let source = if topic.batch_id == Some(context.root_batch.id) {
                "原始生成"
            } else {
                "补充生成"
            };
            let tags = if topic.tags.is_empty() {
                "无".to_string()
            } else {
                topic.tags.join("、")
            };
            format!(
                "{}. [{}] 标题：{}；角度：{}；标签：{}",
                index + 1,
                source,
                truncate_for_prompt(&topic.title, 120),
                truncate_for_prompt(&topic.angle, 180),
                truncate_for_prompt(&tags, 160)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_conversation_history(messages: &[AgentMessage]) -> String {
    if messages.is_empty() {
        return "- 无".to_string();
    }

    messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            format!(
                "{}. {}：{}",
                index + 1,
                agent_message_role_label(&message.role),
                truncate_for_prompt(&message.content, 240)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
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
