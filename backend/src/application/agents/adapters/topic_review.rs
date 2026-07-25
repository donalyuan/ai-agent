//! 评审完整主题组并保存快照；评审结果只辅助决策，不改变选题生命周期状态。

use super::{
    format_account_strategy_context, record_step, truncate_for_prompt, AgentRuntimeError,
    TopicAgentAdapter,
};
use crate::application::agents::kernel::run_lifecycle_error;
use crate::domain::conversation::{
    AgentConversationDefinitionBindingInput, CreateAgentStepInput, ModelBindingEvidence,
};
use crate::domain::topic::{
    ContentTopic, TopicGenerationBatch, TopicReviewItem, TopicReviewResult, TopicReviewSnapshot,
    TopicReviewSnapshotStatus,
};
use crate::repositories::{
    CreateTopicReviewSnapshotInput, PostgresConversationRepository, TopicRepository,
    TopicRepositoryError,
};
use novex_agent::{
    AuditedCallOwner, AuditedExecutionBinding, AuditedModelError, AuditedModelRequest,
    RunLifecycleCoordinator, RunRecorder, StartRun, StepRecorder,
};
use novex_ai_core::{AgentKey, DynamicFragment, PromptCompileInput, TrustLevel};
use novex_model::ModelExecutionSnapshot;
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use uuid::Uuid;

pub struct AuditedTopicReviewExecution {
    pub definition: AgentConversationDefinitionBindingInput,
    pub model_binding: ModelBindingEvidence,
    pub audited: AuditedExecutionBinding,
}

impl TopicAgentAdapter {
    #[allow(clippy::too_many_arguments)]
    pub async fn review_topic_group_audited(
        &self,
        project_id: Uuid,
        root_batch_id: Uuid,
        model_execution: ModelExecutionSnapshot,
        execution: AuditedTopicReviewExecution,
        run_repository: PostgresConversationRepository,
        runs: Arc<dyn RunRecorder>,
        steps: Arc<dyn StepRecorder>,
    ) -> Result<TopicReviewSnapshot, AgentRuntimeError> {
        let topic_repository = &self.topic_repository;
        let project = self.project_repository.get_project(project_id).await?;
        let root_batch = topic_repository.get_generation_batch(root_batch_id).await?;
        if root_batch.project_id != project_id || root_batch.supplement_of_batch_id.is_some() {
            return Err(AgentRuntimeError::TopicRepository(
                TopicRepositoryError::BatchNotFound(root_batch_id),
            ));
        }
        let topics = topic_repository
            .list_topics_for_batch_group(project_id, root_batch_id)
            .await?;
        if topics.is_empty() {
            return Err(AgentRuntimeError::Validation(
                "主题组没有可评审选题".to_string(),
            ));
        }
        let account_strategy_context = format_account_strategy_context(&project);
        let model_id = model_execution.model_id;
        let model_snapshot = serde_json::to_value(model_execution)
            .map_err(|error| AgentRuntimeError::Kernel(error.to_string()))?;

        RunLifecycleCoordinator::new(runs)
            .execute(
                StartRun {
                    session_id: root_batch_id,
                    project_id: Some(project_id),
                    agent_key: AgentKey::new("topic").expect("topic is a valid static AgentKey"),
                    input: json!({
                        "intent": "review_topic_group",
                        "root_batch_id": root_batch_id
                    }),
                    model_id: Some(model_id),
                    model_snapshot: Some(model_snapshot),
                },
                |run_id| async move {
                    run_repository
                        .create_run_binding(
                            run_id,
                            execution.definition,
                            execution.model_binding,
                            false,
                        )
                        .await
                        .map_err(|error| AgentRuntimeError::Kernel(error.to_string()))?;
                    self.review_topic_group_with_run(
                        topic_repository.as_ref(),
                        &account_strategy_context,
                        &root_batch,
                        &topics,
                        run_id,
                        execution.audited,
                        steps,
                    )
                    .await
                },
                |snapshot| Some(json!({ "topic_review_snapshot_id": snapshot.id })),
                |error| error.to_string(),
            )
            .await
            .map_err(run_lifecycle_error)
    }

    #[allow(clippy::too_many_arguments)]
    async fn review_topic_group_with_run(
        &self,
        topic_repository: &dyn TopicRepository,
        account_strategy_context: &str,
        root_batch: &TopicGenerationBatch,
        topics: &[ContentTopic],
        run_id: Uuid,
        audited: AuditedExecutionBinding,
        steps: Arc<dyn StepRecorder>,
    ) -> Result<TopicReviewSnapshot, AgentRuntimeError> {
        record_step(
            steps.as_ref(),
            CreateAgentStepInput {
                agent_run_id: run_id,
                step_order: 1,
                step_type: "read_topic_group".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "root_batch_id": root_batch.id }),
                output: Some(json!({ "topic_count": topics.len() })),
                error_message: None,
            },
        )
        .await?;

        let prompt = build_topic_group_review_prompt(account_strategy_context, root_batch, topics);
        let fragment_id = format!("topic-group:{}:review", root_batch.id);
        let reviewed = audited
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
                            node_key: "topic.group_review".into(),
                            compile_input: PromptCompileInput {
                                schema_version: "1".into(),
                                variables: BTreeMap::new(),
                                fragments: vec![DynamicFragment {
                                    id: fragment_id.clone(),
                                    trust: TrustLevel::Candidate,
                                    source: "topic_group_review_context".into(),
                                    content: Some(prompt),
                                    asset: None,
                                }],
                            },
                            tool_profile: "chat".into(),
                            tool_schema: None,
                            binding: audited.binding.clone(),
                            context_sources: json!([
                                {"id": format!("project:{}:account-strategy", root_batch.project_id), "trust": "confirmed_fact", "source": "project_account_strategy"},
                                {"id": format!("topic-batch:{}:group", root_batch.id), "trust": "candidate", "source": "topic_batch_group"},
                                {"id": fragment_id, "trust": "candidate", "source": "topic_group_review_context"}
                            ]),
                            memory_sources: json!([]),
                            parameters: json!({"max_output_tokens": 2000}),
                            asset_references: json!([]),
                        },
                        |raw| {
                            TopicReviewLlmOutput::parse_and_validate(raw, topics)
                                .map_err(|error| error.to_string())
                        },
            )
            .await
            .map(|response| (response.output, response.model_call_id))
            .map_err(topic_review_audited_error);
        let (review_output, model_call_id) = match reviewed {
            Ok(reviewed) => reviewed,
            Err(error) => {
                let message = error.to_string();
                self.record_failed_topic_step(
                    steps.as_ref(),
                    run_id,
                    2,
                    "review_topic_group",
                    message.clone(),
                )
                .await;
                self.save_failed_topic_review_snapshot(
                    topic_repository,
                    root_batch.project_id,
                    root_batch.id,
                    run_id,
                    message.clone(),
                )
                .await;
                return Err(error);
            }
        };

        let review_step_id = record_step(
            steps.as_ref(),
            CreateAgentStepInput {
                agent_run_id: run_id,
                step_order: 2,
                step_type: "review_topic_group".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "root_batch_id": root_batch.id }),
                output: Some(json!({
                    "topic_review_count": review_output.topic_reviews.len()
                })),
                error_message: None,
            },
        )
        .await?;
        audited
            .executor
            .associate_step(model_call_id, review_step_id)
            .await
            .map_err(|error| AgentRuntimeError::Kernel(error.to_string()))?;

        let review_summary = review_output.review_summary.clone();
        let review_result = review_output.result();
        let snapshot = topic_repository
            .create_topic_review_snapshot(CreateTopicReviewSnapshotInput {
                project_id: root_batch.project_id,
                root_batch_id: root_batch.id,
                source_run_id: Some(run_id),
                status: TopicReviewSnapshotStatus::Succeeded,
                review_summary,
                result: review_result,
                error_message: None,
                metadata: json!({}),
            })
            .await?;

        record_step(
            steps.as_ref(),
            CreateAgentStepInput {
                agent_run_id: run_id,
                step_order: 3,
                step_type: "persist_topic_review_snapshot".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "root_batch_id": root_batch.id }),
                output: Some(json!({ "topic_review_snapshot_id": snapshot.id })),
                error_message: None,
            },
        )
        .await?;
        Ok(snapshot)
    }

    async fn save_failed_topic_review_snapshot(
        &self,
        topic_repository: &dyn TopicRepository,
        project_id: Uuid,
        root_batch_id: Uuid,
        run_id: Uuid,
        error_message: String,
    ) {
        let _ = topic_repository
            .create_topic_review_snapshot(CreateTopicReviewSnapshotInput {
                project_id,
                root_batch_id,
                source_run_id: Some(run_id),
                status: TopicReviewSnapshotStatus::Failed,
                review_summary: String::new(),
                result: TopicReviewResult::default(),
                error_message: Some(error_message),
                metadata: json!({}),
            })
            .await;
    }
}

fn topic_review_audited_error(error: AuditedModelError) -> AgentRuntimeError {
    match error {
        AuditedModelError::Provider { source, .. } => AgentRuntimeError::Llm(source),
        AuditedModelError::StructuredParse { message, .. } => {
            AgentRuntimeError::InvalidLlmOutput(message)
        }
        error => AgentRuntimeError::Kernel(error.to_string()),
    }
}

#[derive(Debug, Deserialize)]
struct TopicReviewLlmOutput {
    review_summary: String,
    topic_reviews: Vec<TopicReviewItem>,
}

impl TopicReviewLlmOutput {
    fn parse_and_validate(raw: &str, topics: &[ContentTopic]) -> Result<Self, TopicReviewError> {
        let json_text = extract_topic_review_json_object(raw)?;
        let mut output: Self =
            serde_json::from_str(json_text).map_err(|error| TopicReviewError::InvalidJson {
                message: error.to_string(),
            })?;
        output.normalize();
        output.validate(topics)?;
        Ok(output)
    }

    fn normalize(&mut self) {
        self.review_summary = self.review_summary.trim().to_string();
        for item in &mut self.topic_reviews {
            item.reason = item.reason.trim().to_string();
        }
    }

    fn validate(&self, topics: &[ContentTopic]) -> Result<(), TopicReviewError> {
        if self.review_summary.is_empty() {
            return Err(TopicReviewError::Validation(
                "review_summary is required".to_string(),
            ));
        }
        if self.topic_reviews.is_empty() {
            return Err(TopicReviewError::Validation(
                "topic_reviews must not be empty".to_string(),
            ));
        }

        let topic_ids = topics.iter().map(|topic| topic.id).collect::<Vec<_>>();
        let topic_id_set = topic_ids
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        let mut reviewed_ids = std::collections::HashSet::new();
        for item in &self.topic_reviews {
            if !topic_id_set.contains(&item.topic_id) {
                return Err(TopicReviewError::Validation(
                    "topic_id must belong to current topic group".to_string(),
                ));
            }
            if !reviewed_ids.insert(item.topic_id) {
                return Err(TopicReviewError::Validation(
                    "topic_id must not be duplicated".to_string(),
                ));
            }
            if item.reason.trim().is_empty() {
                return Err(TopicReviewError::Validation(
                    "reason is required".to_string(),
                ));
            }
            for similar_topic_id in &item.similar_topic_ids {
                if !topic_id_set.contains(similar_topic_id) {
                    return Err(TopicReviewError::Validation(
                        "similar_topic_id must belong to current topic group".to_string(),
                    ));
                }
            }
        }
        for topic_id in topic_ids {
            if !reviewed_ids.contains(&topic_id) {
                return Err(TopicReviewError::Validation(
                    "every visible topic must be reviewed".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn result(self) -> TopicReviewResult {
        TopicReviewResult {
            topic_reviews: self.topic_reviews,
        }
    }
}

#[derive(Debug)]
enum TopicReviewError {
    InvalidJson { message: String },
    Validation(String),
}

impl fmt::Display for TopicReviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson { message } => {
                write!(formatter, "invalid topic review JSON: {message}")
            }
            Self::Validation(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for TopicReviewError {}

fn build_topic_group_review_prompt(
    account_strategy_context: &str,
    root_batch: &TopicGenerationBatch,
    topics: &[ContentTopic],
) -> String {
    format!(
        r#"请评审当前主题组内的候选选题，帮助运营人员快速筛选优先推荐、可备选、建议淘汰和疑似重复项。

评审只作为决策辅助，不允许自动确认、归档、删除选题或生成脚本。

账号策略资料：
{account_strategy_context}

原始生成要求：{original_prompt}

当前主题组选题：
{topic_context}

输出要求：
1. 必须只输出一个 JSON 对象。
2. review_summary 用一句中文总结本主题组的筛选建议。
3. topic_reviews 必须覆盖当前主题组全部选题。
4. priority 只能是 priority、backup、reject。
5. risk_flags 只能使用 too_generic、duplicate、hard_to_script、off_positioning、compliance_risk。
6. similar_topic_ids 只能引用当前主题组内的 topic_id。

JSON Schema：
{{
  "review_summary": "本主题组更适合优先制作工具落地和真实案例方向。",
  "topic_reviews": [
    {{
      "topic_id": "uuid",
      "priority": "priority",
      "reason": "账号匹配度高，脚本化路径清晰。",
      "risk_flags": ["duplicate"],
      "similar_topic_ids": ["uuid"]
    }}
  ]
}}"#,
        account_strategy_context = account_strategy_context,
        original_prompt = truncate_for_prompt(&root_batch.prompt, 500),
        topic_context = format_topic_group_review_topic_context(root_batch, topics)
    )
}

fn format_topic_group_review_topic_context(
    root_batch: &TopicGenerationBatch,
    topics: &[ContentTopic],
) -> String {
    topics
        .iter()
        .enumerate()
        .map(|(index, topic)| {
            let source = if topic.batch_id == Some(root_batch.id) {
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
                "{}. [{}] topic_id={}；标题：{}；角度：{}；评分：{}；状态：{}；标签：{}",
                index + 1,
                source,
                topic.id,
                truncate_for_prompt(&topic.title, 120),
                truncate_for_prompt(&topic.angle, 180),
                topic
                    .score
                    .map(|score| score.to_string())
                    .unwrap_or_else(|| "无".to_string()),
                topic.status.as_str(),
                truncate_for_prompt(&tags, 160)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_topic_review_json_object(raw: &str) -> Result<&str, TopicReviewError> {
    let start = raw
        .find('{')
        .ok_or_else(|| TopicReviewError::Validation("missing JSON object start".to_string()))?;
    let end = raw
        .rfind('}')
        .ok_or_else(|| TopicReviewError::Validation("missing JSON object end".to_string()))?;
    if start > end {
        return Err(TopicReviewError::Validation(
            "invalid JSON object bounds".to_string(),
        ));
    }
    Ok(&raw[start..=end])
}
