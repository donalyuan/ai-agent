//! RoleExecutor：执行单个角色的完整生命周期

use crate::error::{ProductionError, ProductionResult};
use crate::roles::definition::RoleDefinition;
use crate::state::artifacts::ArtifactType;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// 单次角色执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleExecutionResult {
    /// 执行的角色 key
    pub role: String,
    /// 执行状态
    pub status: RoleExecutionStatus,
    /// 实际耗时（毫秒）
    pub execution_time_ms: u64,
    /// 本次执行产出的产物列表（type + id）
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

/// 可执行角色的上下文（注入依赖而非硬编码）
pub struct RoleExecutor;

impl RoleExecutor {
    /// 检查角色所需的输入产物是否全部就绪
    ///
    /// # 参数
    /// - `def`: 角色定义，包含 input_artifacts 列表
    /// - `available_artifacts`: 当前已有的产物类型集合
    ///
    /// # 错误
    /// 若有必需输入产物缺失，返回 `MissingInputArtifact` 错误
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

    /// 验证角色输出是否符合预期产物 schema
    ///
    /// 实际 schema 验证由各产物的 `validate` 函数完成；
    /// 此处仅检查必需字段的顶层结构。
    pub fn validate_output(
        def: &RoleDefinition,
        output: &Value,
    ) -> ProductionResult<()> {
        for artifact_type in &def.output_artifacts {
            let key = match artifact_type {
                ArtifactType::CreativeBrief => "creative_brief",
                ArtifactType::StoryBible => "story_bible",
                ArtifactType::CharacterBible => "character_bibles",
                ArtifactType::ScriptDraft => "script_draft",
                ArtifactType::DirectorialTreatment => "directorial_treatment",
                ArtifactType::ShotContract => "shot_contracts",
                ArtifactType::PerformanceBrief => "performance_briefs",
                ArtifactType::SoundPlan => "sound_plan",
                ArtifactType::ContinuityLedger => "continuity_ledgers",
                ArtifactType::TakeReview => "take_reviews",
            };
            if output.get(key).is_none() {
                return Err(ProductionError::InvalidArtifactSchema {
                    details: format!("角色输出缺少必需字段: {}", key),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roles::definition::{Lifecycle, PromptRef, RoleDefinition};
    use serde_json::json;

    fn make_def(role_key: &str, inputs: Vec<ArtifactType>, outputs: Vec<ArtifactType>) -> RoleDefinition {
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
        let output = json!({ "creative_brief": { "target_audience": "test", "key_messages": [] } });
        assert!(RoleExecutor::validate_output(&def, &output).is_ok());
    }

    #[test]
    fn test_validate_output_missing_field() {
        let def = make_def("producer", vec![], vec![ArtifactType::CreativeBrief]);
        let output = json!({ "other_field": {} });
        assert!(RoleExecutor::validate_output(&def, &output).is_err());
    }
}
