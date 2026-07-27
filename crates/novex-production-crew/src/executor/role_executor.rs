//! RoleExecutor：单个角色完整生命周期的执行器
//!
//! 执行流程：
//!   1. 从 RoleRegistry 加载 RoleDefinition
//!   2. 从 DB 读取输入产物，检查就绪状态
//!   3. 装配 ContextCandidate 列表
//!   4. 构建 FixedModelBinding（固定本次调用的 Context Policy 和 Tokenizer 摘要）
//!   5. 调用 AuditedModelExecutor（自动持久化 ModelCall 审计记录）
//!   6. 解析并验证 AI 输出
//!   7. 写入产物（version 自增，status=draft）
//!   8. 更新项目阶段状态，返回执行结果

use crate::error::{ProductionError, ProductionResult};
use crate::roles::definition::RoleDefinition;
use crate::roles::RoleRegistry;
use crate::state::artifacts::ArtifactType;
use crate::state::ProductionStateRepository;
use novex_agent::{
    text_context_candidate, AuditedCallOwner, AuditedModelExecutor, AuditedModelRequest,
    FixedModelBinding, TextContextCandidateInput,
};
use novex_ai_core::{ContextPriority, DefinitionRegistry, TrustLevel};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::{BTreeMap, HashMap};
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

/// 单次角色执行的依赖上下文（注入而非硬编码）
pub struct RoleExecutionContext {
    /// 数据库连接池（读取/写入产物）
    pub pool: PgPool,
    /// Agent / Prompt 定义注册表（只读）
    pub definition_registry: Arc<DefinitionRegistry>,
    /// 带审计的模型执行器
    pub audited_executor: Arc<AuditedModelExecutor>,
    /// 当前制作项目 ID
    pub project_id: Uuid,
    /// 角色标识（如 "producer"）
    pub role_key: String,
    /// 用户补充输入（可选）
    pub user_input: Option<String>,
    /// 优选模型 ID（从项目元数据或 AppConfig 获取）
    pub preferred_model_id: Uuid,
}

// ─────────────────────────────────────────────
// RoleExecutor 实现
// ─────────────────────────────────────────────

/// 无状态执行器，通过 `RoleExecutionContext` 注入所有依赖
pub struct RoleExecutor;

impl RoleExecutor {
    /// 执行单个角色的完整生命周期。
    ///
    /// `role_registry` 仅用于查找 `RoleDefinition`（避免 executor 持有 registry 所有权）。
    pub async fn execute(
        ctx: RoleExecutionContext,
        role_registry: &RoleRegistry,
    ) -> ProductionResult<RoleExecutionResult> {
        let start = std::time::Instant::now();
        let repo = ProductionStateRepository::new(ctx.pool.clone());

        // 1. 加载角色定义
        let role_def = role_registry.get(&ctx.role_key)?.clone();

        // 2. 读取输入产物（approved 优先，其次 draft）
        let input_artifacts = repo
            .get_input_artifacts(ctx.project_id, &role_def.input_artifacts)
            .await?;

        // 3. 检查输入就绪
        let available: Vec<ArtifactType> = input_artifacts.keys().cloned().collect();
        Self::check_inputs_ready(&role_def, &available)?;

        // 4. 装配 ContextCandidate 列表
        // valid_timestamp 要求时间戳以 'Z' 结尾（UTC），to_rfc3339() 输出 +00:00 不符合要求
        let compiled_at = chrono::Utc::now()
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let (context_candidates, context_sources) = build_context_candidates(
            &role_def,
            &input_artifacts,
            ctx.user_input.as_deref(),
            &compiled_at,
        );

        // 5. 计算 agent/node 键名（production.<role_key>.execute）
        let agent_key = format!("production.{}", ctx.role_key);
        let agent_version = "2.0.0";
        let node_key = format!("{}.execute", agent_key);

        // 6. 构建 FixedModelBinding（固定 Context Policy 摘要 + Tokenizer Profile 摘要）
        let binding: FixedModelBinding = ctx
            .audited_executor
            .build_binding(&agent_key, agent_version, ctx.preferred_model_id)
            .await
            .map_err(|e| ProductionError::AgentExecution(e.to_string()))?;

        // 7. 在 agent_runs 中创建执行记录，为 context_compile_attempts 的外键提供锚点。
        //    即使在错误路径下，context 编译失败记录也能正常持久化。
        let run_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO agent_runs (id, agent_type, status, input) VALUES ($1, 'production', 'running', $2)"
        )
        .bind(run_id)
        .bind(serde_json::json!({
            "project_id": ctx.project_id,
            "role_key": ctx.role_key,
        }))
        .execute(&ctx.pool)
        .await
        .map_err(ProductionError::Database)?;

        let request = AuditedModelRequest {
            owner: AuditedCallOwner::AgentRun(run_id),
            step_id: None,
            root_call_id: None,
            parent_call_id: None,
            attempt: 1,
            agent_key: agent_key.clone(),
            agent_version: agent_version.to_string(),
            node_key,
            // 制作角色 Prompt 通过 fragments（ContextCandidate）注入上下文，variables 留空
            variables: BTreeMap::new(),
            context_candidates,
            context_atomic_groups: Vec::new(),
            compiled_at,
            tool_profile: "chat".into(),
            tool_schema: None,
            binding,
            context_sources: Value::Array(context_sources),
            memory_sources: json!([]),
            parameters: json!({}),
            asset_references: json!([]),
        };

        // 8. 调用模型（execute_parsed 自动持久化 ModelCall 审计记录并处理 finish/fail）
        let response = ctx
            .audited_executor
            .execute_parsed(request, |raw| {
                serde_json::from_str::<Value>(raw)
                    .map_err(|e| format!("JSON parse failed: {e}"))
            })
            .await
            .map_err(|e| ProductionError::AgentExecution(e.to_string()))?;

        let model_call_id = response.model_call_id;
        let output = response.output;

        // 9. 验证输出结构（检查必需产物键存在）
        Self::validate_output(&role_def, &output)?;

        // 10. 写入各输出产物（version 自增，status=draft）
        let mut artifact_summaries = Vec::new();
        for &artifact_type in &role_def.output_artifacts {
            let saved = repo
                .save_artifact(ctx.project_id, artifact_type, &output, &ctx.role_key)
                .await?;
            artifact_summaries.extend(saved);
        }

        // 11. 更新项目阶段状态（Producer 完成 → briefing→scripting，以此类推）
        if let Some(next_status) = project_status_after_role(&ctx.role_key) {
            repo.update_project_status(ctx.project_id, next_status.to_string())
                .await?;
        }

        Ok(RoleExecutionResult {
            role: ctx.role_key,
            status: RoleExecutionStatus::Completed,
            execution_time_ms: start.elapsed().as_millis() as u64,
            output_artifacts: artifact_summaries,
            model_call_id: Some(model_call_id),
            next_role: next_role_after_role(&role_def.role_key).map(str::to_string),
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

    /// 验证角色输出是否包含必需产物的顶层键。
    ///
    /// 每个输出产物类型对应一个固定的 JSON 顶层键（如 `creative_brief`、`character_bibles`）。
    /// 此处只做顶层键存在性检查，详细 schema 由 DB 约束和产物 `validate()` 保障。
    pub fn validate_output(def: &RoleDefinition, output: &Value) -> ProductionResult<()> {
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

// ─────────────────────────────────────────────
// 辅助函数
// ─────────────────────────────────────────────

/// 将输入产物和用户输入装配为 ContextCandidate 列表。
///
/// 返回 (context_candidates, context_sources_json_array)。
/// - 用户输入：trust=UserInstruction, priority=P0（最高优先级，最先渲染）
/// - 已就绪产物：trust=ConfirmedFact, priority=P1（已确认事实）
fn build_context_candidates(
    role_def: &RoleDefinition,
    input_artifacts: &HashMap<ArtifactType, Value>,
    user_input: Option<&str>,
    compiled_at: &str,
) -> (Vec<novex_ai_core::ContextCandidate>, Vec<Value>) {
    let mut candidates = Vec::new();
    let mut sources = Vec::new();
    let mut render_order: u32 = 0;

    // 用户补充输入（P0，最高优先）
    // source_kind 必须是 allowed_sources 中的值；user_instruction 在 required_sources 中，
    // 要求 required=true
    if let Some(input) = user_input {
        let candidate_id = "user_input".to_string();
        sources.push(json!({
            "id": candidate_id,
            "trust": "user_instruction",
            "source": "user_instruction",
        }));
        candidates.push(text_context_candidate(TextContextCandidateInput {
            candidate_id,
            source_kind: "user_instruction".to_string(),
            source_id: "production_user_input".to_string(),
            source_version: "1".to_string(),
            trust: TrustLevel::UserInstruction,
            priority: ContextPriority::P0,
            // required_sources 包含 "user_instruction"，对应候选必须 required=true
            required: true,
            render_order,
            observed_at: compiled_at.to_string(),
            text: input.to_string(),
        }));
        render_order += 1;
    }

    // 各输入产物（P1，已确认事实）
    // source_kind 使用 "project"（在 allowed_sources 中，语义最接近制作项目产物）
    for artifact_type in &role_def.input_artifacts {
        if let Some(artifact_row) = input_artifacts.get(artifact_type) {
            // 从 DB row 中提取 content 字段，否则整体作为文本
            let content_text = artifact_row
                .get("content")
                .map(|c| {
                    serde_json::to_string_pretty(c).unwrap_or_else(|_| c.to_string())
                })
                .unwrap_or_else(|| {
                    serde_json::to_string_pretty(artifact_row)
                        .unwrap_or_else(|_| artifact_row.to_string())
                });

            let version = artifact_row
                .get("version")
                .and_then(|v| v.as_i64())
                .unwrap_or(1);
            let status = artifact_row
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("draft");

            // 格式化带标题的文本片段，方便模型定位上下文来源
            let artifact_name = format!("{:?}", artifact_type);
            let text = format!(
                "=== {} (v{}, {}) ===\n{}",
                artifact_name, version, status, content_text
            );

            let candidate_id = format!(
                "{}_v{}",
                artifact_name.to_lowercase(),
                version
            );
            // source_id 格式：production_project:{project_id}:{artifact_type}
            let source_id = format!(
                "production_project:{}",
                artifact_name.to_lowercase()
            );

            sources.push(json!({
                "id": candidate_id,
                "trust": "confirmed_fact",
                "source": "project",
            }));
            candidates.push(text_context_candidate(TextContextCandidateInput {
                candidate_id,
                // "project" 在 allowed_sources 中，用于制作项目的产物数据
                source_kind: "project".to_string(),
                source_id,
                source_version: version.to_string(),
                trust: TrustLevel::ConfirmedFact,
                priority: ContextPriority::P1,
                required: true,
                render_order,
                observed_at: compiled_at.to_string(),
                text,
            }));
            render_order += 1;
        }
    }

    (candidates, sources)
}

/// 角色完成后项目应转入的下一个阶段状态。
///
/// 仅覆盖产出产物到数据库的核心角色；纯协作建议角色（cinematographer、character_critic）不改变状态。
fn project_status_after_role(role_key: &str) -> Option<&'static str> {
    match role_key {
        "producer" => Some("scripting"),
        "screenwriter" => Some("directing"),
        "director" => Some("generating"),
        "editor" => Some("qc"),
        "qc" => Some("approved"),
        _ => None, // cinematographer、performance_director、sound_director、character_critic 不触发状态变更
    }
}

/// 角色序列中的下一个角色（用于 API 响应提示）。
fn next_role_after_role(role_key: &str) -> Option<&'static str> {
    match role_key {
        "producer" => Some("screenwriter"),
        "screenwriter" => Some("director"),
        "director" => Some("cinematographer"),
        "cinematographer" => Some("performance_director"),
        "performance_director" => Some("sound_director"),
        "sound_director" => Some("editor"),
        "editor" => Some("qc"),
        _ => None,
    }
}

// ─────────────────────────────────────────────
// 单元测试
// ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
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
        let output = json!({ "creative_brief": { "target_audience": "test", "key_messages": [] } });
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
            "story_bible": { "premise": "test" },
            "character_bibles": [],
            "script_draft": { "title": "test", "scenes": [] }
        });
        assert!(RoleExecutor::validate_output(&def, &output).is_ok());
    }

    #[test]
    fn test_build_context_candidates_user_input_only() {
        let def = make_def("producer", vec![], vec![]);
        let artifacts = HashMap::new();
        let (candidates, sources) =
            build_context_candidates(&def, &artifacts, Some("制作一个美妆教程"), "2026-01-01T00:00:00Z");
        assert_eq!(candidates.len(), 1);
        assert_eq!(sources.len(), 1);
        // user_instruction 在 allowed_sources + required_sources 中
        assert_eq!(sources[0]["source"], "user_instruction");
    }

    #[test]
    fn test_build_context_candidates_with_artifact() {
        let def = make_def("screenwriter", vec![ArtifactType::CreativeBrief], vec![]);
        let mut artifacts = HashMap::new();
        artifacts.insert(
            ArtifactType::CreativeBrief,
            json!({
                "id": "some-uuid",
                "version": 1,
                "status": "approved",
                "content": { "target_audience": "18-25岁女性", "key_messages": ["美妆小技巧"] }
            }),
        );
        let (candidates, sources) =
            build_context_candidates(&def, &artifacts, None, "2026-01-01T00:00:00Z");
        assert_eq!(candidates.len(), 1);
        // 产物使用 "project" source_kind（在 allowed_sources 中）
        assert_eq!(sources[0]["source"], "project");
    }

    #[test]
    fn test_project_status_after_producer() {
        assert_eq!(project_status_after_role("producer"), Some("scripting"));
        assert_eq!(project_status_after_role("cinematographer"), None);
        assert_eq!(project_status_after_role("qc"), Some("approved"));
    }

    #[test]
    fn test_next_role_sequence() {
        assert_eq!(next_role_after_role("producer"), Some("screenwriter"));
        assert_eq!(next_role_after_role("qc"), None);
    }
}
