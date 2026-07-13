//! 评审完整主题组并保存快照；评审结果只辅助决策，不改变选题生命周期状态。

use super::{
    format_account_strategy_context, truncate_for_prompt, AgentRuntime, AgentRuntimeError,
};
use crate::domain::conversation::{CreateAgentRunInput, CreateAgentStepInput, FinishAgentRunInput};
use crate::domain::topic::{
    ContentTopic, TopicGenerationBatch, TopicReviewItem, TopicReviewResult, TopicReviewSnapshot,
    TopicReviewSnapshotStatus,
};
use crate::repositories::{CreateTopicReviewSnapshotInput, TopicRepository, TopicRepositoryError};
use novex_model::{LLMJsonSchema, LLMPrompt};
use serde::Deserialize;
use serde_json::json;
use std::fmt;
use uuid::Uuid;

impl AgentRuntime {
    pub async fn review_topic_group(
        &self,
        project_id: Uuid,
        root_batch_id: Uuid,
    ) -> Result<TopicReviewSnapshot, AgentRuntimeError> {
        let topic_repository = self.topic_repository.as_ref().ok_or_else(|| {
            AgentRuntimeError::Validation("选题 Agent 未配置 topic repository".to_string())
        })?;
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

        let run = self
            .conversation_repository
            .create_run(CreateAgentRunInput {
                conversation_id: root_batch_id,
                project_id: Some(project_id),
                agent_type: "topic".to_string(),
                input: json!({
                    "intent": "review_topic_group",
                    "root_batch_id": root_batch_id
                }),
                model_id: self
                    .model_execution
                    .as_ref()
                    .map(|snapshot| snapshot.model_id),
                model_snapshot: self
                    .model_execution
                    .as_ref()
                    .and_then(|snapshot| serde_json::to_value(snapshot).ok()),
            })
            .await?;

        let result = self
            .review_topic_group_with_run(
                topic_repository.as_ref(),
                &format_account_strategy_context(&project),
                &root_batch,
                &topics,
                run.id,
            )
            .await;

        match result {
            Ok(snapshot) => {
                self.conversation_repository
                    .finish_run(FinishAgentRunInput {
                        agent_run_id: run.id,
                        status: "succeeded".to_string(),
                        output: Some(json!({ "topic_review_snapshot_id": snapshot.id })),
                        error_message: None,
                    })
                    .await?;
                Ok(snapshot)
            }
            Err(error) => {
                let _ = self
                    .conversation_repository
                    .finish_run(FinishAgentRunInput {
                        agent_run_id: run.id,
                        status: "failed".to_string(),
                        output: None,
                        error_message: Some(error.to_string()),
                    })
                    .await;
                Err(error)
            }
        }
    }

    async fn review_topic_group_with_run(
        &self,
        topic_repository: &dyn TopicRepository,
        account_strategy_context: &str,
        root_batch: &TopicGenerationBatch,
        topics: &[ContentTopic],
        run_id: Uuid,
    ) -> Result<TopicReviewSnapshot, AgentRuntimeError> {
        self.conversation_repository
            .add_step(CreateAgentStepInput {
                agent_run_id: run_id,
                step_order: 1,
                step_type: "read_topic_group".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "root_batch_id": root_batch.id }),
                output: Some(json!({ "topic_count": topics.len() })),
                error_message: None,
            })
            .await?;

        let prompt = build_topic_group_review_prompt(account_strategy_context, root_batch, topics);
        let raw = self.llm_client.generate_script(prompt).await;
        let review_output = match raw {
            Ok(raw) => match TopicReviewLlmOutput::parse_and_validate(&raw, topics) {
                Ok(output) => output,
                Err(error) => {
                    let message = error.to_string();
                    self.add_failed_topic_step(run_id, 2, "review_topic_group", message.clone())
                        .await;
                    self.save_failed_topic_review_snapshot(
                        topic_repository,
                        root_batch.project_id,
                        root_batch.id,
                        run_id,
                        message.clone(),
                    )
                    .await;
                    return Err(AgentRuntimeError::InvalidLlmOutput(message));
                }
            },
            Err(error) => {
                let message = error.to_string();
                self.add_failed_topic_step(run_id, 2, "review_topic_group", message.clone())
                    .await;
                self.save_failed_topic_review_snapshot(
                    topic_repository,
                    root_batch.project_id,
                    root_batch.id,
                    run_id,
                    message,
                )
                .await;
                return Err(AgentRuntimeError::Llm(error));
            }
        };

        self.conversation_repository
            .add_step(CreateAgentStepInput {
                agent_run_id: run_id,
                step_order: 2,
                step_type: "review_topic_group".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "root_batch_id": root_batch.id }),
                output: Some(json!({
                    "topic_review_count": review_output.topic_reviews.len()
                })),
                error_message: None,
            })
            .await?;

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

        self.conversation_repository
            .add_step(CreateAgentStepInput {
                agent_run_id: run_id,
                step_order: 3,
                step_type: "persist_topic_review_snapshot".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "root_batch_id": root_batch.id }),
                output: Some(json!({ "topic_review_snapshot_id": snapshot.id })),
                error_message: None,
            })
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
) -> LLMPrompt {
    LLMPrompt {
        system: "你是短视频内容策略评审 Agent。你必须只输出符合 JSON Schema 的合法 JSON 对象，不要输出 Markdown 或解释。"
            .to_string(),
        user: format!(
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
        ),
        max_output_tokens: Some(2_000),
        output_schema: Some(topic_review_output_schema()),
    }
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

fn topic_review_output_schema() -> LLMJsonSchema {
    LLMJsonSchema {
        name: "topic_group_review".to_string(),
        strict: true,
        schema: json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["review_summary", "topic_reviews"],
            "properties": {
                "review_summary": { "type": "string", "minLength": 1 },
                "topic_reviews": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": [
                            "topic_id",
                            "priority",
                            "reason",
                            "risk_flags",
                            "similar_topic_ids"
                        ],
                        "properties": {
                            "topic_id": { "type": "string", "format": "uuid" },
                            "priority": {
                                "type": "string",
                                "enum": ["priority", "backup", "reject"]
                            },
                            "reason": { "type": "string", "minLength": 1 },
                            "risk_flags": {
                                "type": "array",
                                "items": {
                                    "type": "string",
                                    "enum": [
                                        "too_generic",
                                        "duplicate",
                                        "hard_to_script",
                                        "off_positioning",
                                        "compliance_risk"
                                    ]
                                }
                            },
                            "similar_topic_ids": {
                                "type": "array",
                                "items": { "type": "string", "format": "uuid" }
                            }
                        }
                    }
                }
            }
        }),
    }
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
