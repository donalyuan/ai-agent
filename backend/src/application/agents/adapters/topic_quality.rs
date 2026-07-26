//! 校验质量模型输出并计算候选通过率，为选题入库和有限重写提供确定性规则。

use super::topic_generation::TopicLlmOutput;
use super::TopicAgentAdapter;
use crate::domain::topic::{
    TopicQualityDecision, TopicQualityEvaluationStatus, TopicQualityFlag, TopicQualityGateItem,
    TopicQualityGateResult,
};
use crate::repositories::{CreateTopicQualityEvaluationInput, TopicRepository};
use serde::Deserialize;
use std::fmt;
use uuid::Uuid;

impl TopicAgentAdapter {
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
