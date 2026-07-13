//! 校验质量模型输出并计算候选通过率，为选题入库和有限重写提供确定性规则。

use super::topic_generation::{
    format_existing_topic_context, TopicLlmOutput, TopicSupplementPromptContext,
};
use super::{truncate_for_prompt, AgentRuntime};
use crate::domain::topic::{
    TopicQualityDecision, TopicQualityEvaluationStatus, TopicQualityFlag, TopicQualityGateItem,
    TopicQualityGateResult,
};
use crate::repositories::{CreateTopicQualityEvaluationInput, TopicRepository};
use novex_model::{LLMJsonSchema, LLMPrompt};
use serde::Deserialize;
use serde_json::json;
use std::fmt;
use uuid::Uuid;

impl AgentRuntime {
    pub(super) async fn save_failed_topic_quality_evaluation(
        &self,
        topic_repository: &dyn TopicRepository,
        project_id: Uuid,
        batch_id: Uuid,
        run_id: Uuid,
        error_message: String,
    ) {
        let _ = topic_repository
            .create_topic_quality_evaluation(CreateTopicQualityEvaluationInput {
                project_id,
                batch_id,
                source_run_id: Some(run_id),
                status: TopicQualityEvaluationStatus::Failed,
                pass_count: 0,
                reject_count: 0,
                rewrite_triggered: false,
                result: TopicQualityGateResult::default(),
                error_message: Some(error_message),
            })
            .await;
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct TopicQualityLlmOutput {
    pub(super) summary: String,
    pub(super) items: Vec<TopicQualityGateItem>,
}

impl TopicQualityLlmOutput {
    pub(super) fn parse_and_validate(
        raw: &str,
        candidates: &[TopicLlmOutput],
    ) -> Result<Self, TopicQualityError> {
        let json_text = extract_topic_quality_json_object(raw)?;
        let mut output: Self =
            serde_json::from_str(json_text).map_err(|error| TopicQualityError::InvalidJson {
                message: error.to_string(),
            })?;
        output.normalize();
        output.validate(candidates)?;
        Ok(output)
    }

    fn normalize(&mut self) {
        self.summary = self.summary.trim().to_string();
        for item in &mut self.items {
            item.candidate_key = item.candidate_key.trim().to_string();
            item.title = item.title.trim().to_string();
            item.reason = item.reason.trim().to_string();
        }
    }

    fn validate(&self, candidates: &[TopicLlmOutput]) -> Result<(), TopicQualityError> {
        if self.summary.is_empty() {
            return Err(TopicQualityError::Validation(
                "summary is required".to_string(),
            ));
        }
        if self.items.len() != candidates.len() {
            return Err(TopicQualityError::Validation(
                "every candidate must have one quality item".to_string(),
            ));
        }

        let candidate_keys = (0..candidates.len())
            .map(topic_candidate_key)
            .collect::<std::collections::HashSet<_>>();
        let mut reviewed_keys = std::collections::HashSet::new();
        for item in &self.items {
            if !candidate_keys.contains(&item.candidate_key) {
                return Err(TopicQualityError::Validation(
                    "candidate_key must belong to current candidates".to_string(),
                ));
            }
            if !reviewed_keys.insert(item.candidate_key.clone()) {
                return Err(TopicQualityError::Validation(
                    "candidate_key must not be duplicated".to_string(),
                ));
            }
            if !(0..=100).contains(&item.quality_score) {
                return Err(TopicQualityError::Validation(
                    "quality_score must be between 0 and 100".to_string(),
                ));
            }
            if item.reason.trim().is_empty() {
                return Err(TopicQualityError::Validation(
                    "reason is required".to_string(),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn result(self) -> TopicQualityGateResult {
        TopicQualityGateResult {
            summary: self.summary,
            items: self.items,
        }
    }
}

#[derive(Debug)]
pub(super) enum TopicQualityError {
    InvalidJson { message: String },
    Validation(String),
}

impl fmt::Display for TopicQualityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson { message } => {
                write!(formatter, "invalid topic quality JSON: {message}")
            }
            Self::Validation(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for TopicQualityError {}

pub(super) fn topic_candidate_key(index: usize) -> String {
    format!("candidate-{}", index + 1)
}

pub(super) fn quality_items_by_key(
    result: &TopicQualityGateResult,
) -> std::collections::HashMap<&str, &TopicQualityGateItem> {
    result
        .items
        .iter()
        .map(|item| (item.candidate_key.as_str(), item))
        .collect()
}

pub(super) fn topic_quality_item_passes(item: &TopicQualityGateItem) -> bool {
    item.decision == TopicQualityDecision::Pass
        && item.quality_score >= 70
        && !item.flags.iter().any(|flag| {
            matches!(
                flag,
                TopicQualityFlag::OffPositioning
                    | TopicQualityFlag::ComplianceRisk
                    | TopicQualityFlag::Duplicate
            )
        })
}

pub(super) fn topic_quality_pass_rate_is_low(pass_count: i32, candidate_count: usize) -> bool {
    candidate_count > 0 && pass_count * 100 < candidate_count as i32 * 60
}

pub(super) fn build_topic_rewrite_user_message(
    original_user_message: &str,
    quality_result: &TopicQualityGateResult,
) -> String {
    format!(
        r#"{original_user_message}

基于质量闸门淘汰原因重写候选选题。请保留原始用户要求和账号定位，但避开以下问题：
{quality_context}

重写要求：
1. 不要复用被淘汰候选的泛化标题。
2. 强化具体场景、目标受众、脚本化路径和差异化角度。
3. 仍然只输出 topic_generation_batch JSON Schema。"#,
        original_user_message = original_user_message,
        quality_context = format_topic_quality_rewrite_context(quality_result)
    )
}

fn format_topic_quality_rewrite_context(quality_result: &TopicQualityGateResult) -> String {
    if quality_result.items.is_empty() {
        return "- 无".to_string();
    }

    quality_result
        .items
        .iter()
        .filter(|item| !topic_quality_item_passes(item))
        .map(|item| {
            let flags = if item.flags.is_empty() {
                "无".to_string()
            } else {
                item.flags
                    .iter()
                    .map(TopicQualityFlag::as_str)
                    .collect::<Vec<_>>()
                    .join("、")
            };
            format!(
                "- {}：{}；质量分={}；flags={}；原因={}",
                item.candidate_key, item.title, item.quality_score, flags, item.reason
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn build_topic_quality_gate_prompt(
    account_strategy_context: &str,
    user_message: &str,
    candidates: &[TopicLlmOutput],
    supplement_context: Option<&TopicSupplementPromptContext>,
) -> LLMPrompt {
    let existing_topic_context = supplement_context
        .map(format_existing_topic_context)
        .unwrap_or_else(|| "- 无".to_string());
    LLMPrompt {
        system: "你是短视频内容策略质量闸门。你必须只输出符合 JSON Schema 的合法 JSON 对象，不要输出 Markdown 或解释。"
            .to_string(),
        user: format!(
            r#"请评估候选选题是否允许进入选题池。质量闸门只做入库前筛选，不允许自动确认、归档、删除选题或生成脚本。

账号策略资料：
{account_strategy_context}

用户生成要求：{user_message}

同主题组已有选题：
{existing_topic_context}

待评估候选：
{candidate_context}

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
{{
  "summary": "本批次 2 条中 1 条通过，1 条因泛化被淘汰。",
  "items": [
    {{
      "candidate_key": "candidate-1",
      "title": "候选标题",
      "decision": "pass",
      "quality_score": 86,
      "flags": [],
      "reason": "贴合账号定位，脚本化路径清晰。"
    }}
  ]
}}"#,
            account_strategy_context = account_strategy_context,
            user_message = user_message,
            existing_topic_context = existing_topic_context,
            candidate_context = format_topic_quality_candidate_context(candidates)
        ),
        max_output_tokens: Some(2_000),
        output_schema: Some(topic_quality_gate_output_schema()),
    }
}

fn format_topic_quality_candidate_context(candidates: &[TopicLlmOutput]) -> String {
    candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let tags = if candidate.tags.is_empty() {
                "无".to_string()
            } else {
                candidate.tags.join("、")
            };
            let hook_points = if candidate.hook_points.is_empty() {
                "无".to_string()
            } else {
                candidate.hook_points.join("、")
            };
            format!(
                "{}. candidate_key={}；标题：{}；角度：{}；目标受众：{}；看点：{}；内容类型：{}；原始评分：{}；评分理由：{}；标签：{}",
                index + 1,
                topic_candidate_key(index),
                truncate_for_prompt(&candidate.title, 120),
                truncate_for_prompt(&candidate.angle, 180),
                truncate_for_prompt(&candidate.target_audience, 120),
                truncate_for_prompt(&hook_points, 180),
                truncate_for_prompt(&candidate.content_type, 80),
                candidate.score.map(|score| score.to_string()).unwrap_or_else(|| "无".to_string()),
                truncate_for_prompt(&candidate.score_reason, 180),
                truncate_for_prompt(&tags, 160)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn topic_quality_gate_output_schema() -> LLMJsonSchema {
    LLMJsonSchema {
        name: "topic_quality_gate".to_string(),
        strict: true,
        schema: json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["summary", "items"],
            "properties": {
                "summary": { "type": "string", "minLength": 1 },
                "items": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": [
                            "candidate_key", "title", "decision", "quality_score", "flags", "reason"
                        ],
                        "properties": {
                            "candidate_key": { "type": "string", "minLength": 1 },
                            "title": { "type": "string", "minLength": 1 },
                            "decision": { "type": "string", "enum": ["pass", "reject"] },
                            "quality_score": { "type": "integer", "minimum": 0, "maximum": 100 },
                            "flags": {
                                "type": "array",
                                "items": {
                                    "type": "string",
                                    "enum": [
                                        "too_generic", "duplicate", "off_positioning",
                                        "hard_to_script", "compliance_risk", "score_untrusted"
                                    ]
                                }
                            },
                            "reason": { "type": "string", "minLength": 1 }
                        }
                    }
                }
            }
        }),
    }
}

fn extract_topic_quality_json_object(raw: &str) -> Result<&str, TopicQualityError> {
    let start = raw
        .find('{')
        .ok_or_else(|| TopicQualityError::Validation("missing JSON object start".to_string()))?;
    let end = raw
        .rfind('}')
        .ok_or_else(|| TopicQualityError::Validation("missing JSON object end".to_string()))?;
    if start > end {
        return Err(TopicQualityError::Validation(
            "invalid JSON object bounds".to_string(),
        ));
    }
    Ok(&raw[start..=end])
}
