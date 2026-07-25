//! 项目与账号策略用例，集中处理仓储协作、模型调用和运行记录。

use crate::application::agents::adapters::AgentRuntimeError;
use crate::application::agents::kernel::{
    active_rust_definition_binding, run_lifecycle, run_lifecycle_error,
};
use crate::domain::conversation::ModelBindingEvidence;
use crate::model_routing::{model_behavior_evidence, ModelClientResolver, ModelResolveError};
use crate::repositories::{
    AccountStrategyProfile, AgentBindingError, ConversationRepositoryError, CreateProjectInput,
    PostgresConversationRepository, PostgresProjectRepository, Project, ProjectRepository,
    ProjectRepositoryError, UpdateProjectStrategyProfileInput,
};
use novex_agent::{
    AuditedCallOwner, AuditedModelError, AuditedModelExecutor, AuditedModelRequest,
    FixedModelBinding, StartRun,
};
use novex_ai_core::{
    AgentKey, DefinitionRegistry, DynamicFragment, PromptCompileInput, TrustLevel,
};
use novex_model::LLMError;
use serde::Deserialize;
use serde_json::json;
use std::{fmt, sync::Arc};
use uuid::Uuid;

const ACCOUNT_STRATEGY_TEXT_LIMIT: usize = 1_000;
const ACCOUNT_STRATEGY_LIST_LIMIT: usize = 20;
const ACCOUNT_STRATEGY_LIST_ITEM_LIMIT: usize = 120;

#[derive(Clone)]
/// 管理项目资料和 AI 策略草稿生成，不会自动保存模型生成的草稿。
pub struct ProjectService {
    project_repository: PostgresProjectRepository,
    conversation_repository: PostgresConversationRepository,
    model_resolver: Arc<dyn ModelClientResolver>,
    definition_registry: Arc<DefinitionRegistry>,
    audited_model_executor: Arc<AuditedModelExecutor>,
}

impl ProjectService {
    pub fn new(
        project_repository: PostgresProjectRepository,
        conversation_repository: PostgresConversationRepository,
        model_resolver: Arc<dyn ModelClientResolver>,
        definition_registry: Arc<DefinitionRegistry>,
        audited_model_executor: Arc<AuditedModelExecutor>,
    ) -> Self {
        Self {
            project_repository,
            conversation_repository,
            model_resolver,
            definition_registry,
            audited_model_executor,
        }
    }

    pub async fn create(
        &self,
        input: CreateProjectInput,
    ) -> Result<Project, ProjectApplicationError> {
        self.project_repository
            .create_project(input)
            .await
            .map_err(Into::into)
    }

    pub async fn list(&self) -> Result<Vec<Project>, ProjectApplicationError> {
        self.project_repository
            .list_projects()
            .await
            .map_err(Into::into)
    }

    pub async fn update_strategy_profile(
        &self,
        project_id: Uuid,
        input: UpdateProjectStrategyProfileInput,
    ) -> Result<Project, ProjectApplicationError> {
        self.project_repository
            .update_strategy_profile(project_id, input)
            .await
            .map_err(Into::into)
    }

    /// 生成草稿只返回预填数据；持久化必须由后续人工确认的更新用例完成。
    pub async fn generate_strategy_profile_draft(
        &self,
        project_id: Uuid,
        model_id: Uuid,
        direction_notes: &str,
    ) -> Result<StrategyProfileDraft, ProjectApplicationError> {
        let project = self.project_repository.get_project(project_id).await?;
        let resolved = self.model_resolver.text_client(model_id).await?;
        let evidence = model_behavior_evidence(&resolved.snapshot)?;
        let model_snapshot = serde_json::to_value(&resolved.snapshot)
            .map_err(|error| ProjectApplicationError::Serialization(error.to_string()))?;
        let prompt = build_strategy_profile_draft_prompt(&project, direction_notes);
        let fragment = DynamicFragment {
            id: format!("project:{project_id}:strategy-profile-draft"),
            trust: TrustLevel::Reference,
            source: "project_strategy_context".into(),
            content: Some(prompt),
            asset: None,
        };
        let context_sources = json!([{
            "id": fragment.id,
            "trust": fragment.trust,
            "source": fragment.source,
        }]);
        let compile_input = PromptCompileInput {
            schema_version: "1".into(),
            variables: Default::default(),
            fragments: vec![fragment],
        };
        let definition =
            active_rust_definition_binding(&self.definition_registry, "video.project-strategy")
                .map_err(ProjectApplicationError::Serialization)?;
        let model_binding = ModelBindingEvidence {
            model_id,
            behavior_fingerprint: evidence.behavior_fingerprint.clone(),
            model_capabilities: serde_json::to_value(&evidence.capabilities)
                .map_err(|error| ProjectApplicationError::Serialization(error.to_string()))?,
        };
        let fixed_binding = FixedModelBinding {
            model_id,
            behavior_fingerprint: evidence.behavior_fingerprint,
        };
        let repository = self.conversation_repository.clone();
        let executor = self.audited_model_executor.clone();
        run_lifecycle(self.conversation_repository.clone())
            .execute(
                StartRun {
                    session_id: project_id,
                    project_id: Some(project_id),
                    agent_key: AgentKey::new("topic").expect("topic is a valid static AgentKey"),
                    input: json!({ "intent": "strategy_profile_draft" }),
                    model_id: Some(model_id),
                    model_snapshot: Some(model_snapshot),
                },
                |run_id| async move {
                    repository
                        .create_run_binding(run_id, definition.clone(), model_binding, false)
                        .await?;
                    let request = AuditedModelRequest {
                        owner: AuditedCallOwner::AgentRun(run_id),
                        step_id: None,
                        root_call_id: None,
                        parent_call_id: None,
                        attempt: 1,
                        agent_key: definition.agent_key,
                        agent_version: definition.agent_version,
                        node_key: "project.strategy_draft".into(),
                        compile_input,
                        tool_profile: "chat".into(),
                        tool_schema: None,
                        binding: fixed_binding,
                        context_sources,
                        memory_sources: json!([]),
                        parameters: json!({ "max_output_tokens": 1200 }),
                        asset_references: json!([]),
                    };
                    generate_strategy_profile_draft_with_retry(executor.as_ref(), request).await
                },
                |_| Some(json!({ "draft_generated": true })),
                |error| format!("{error:?}"),
            )
            .await
            .map_err(run_lifecycle_error)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrategyProfileDraft {
    pub draft: AccountStrategyProfile,
    pub draft_summary: String,
}

#[derive(Debug)]
pub enum ProjectApplicationError {
    ProjectRepository(ProjectRepositoryError),
    ConversationRepository(ConversationRepositoryError),
    Runtime(AgentRuntimeError),
    ModelResolve(ModelResolveError),
    AgentBinding(AgentBindingError),
    Llm(LLMError),
    InvalidOutput(String),
    Serialization(String),
}

impl From<ProjectRepositoryError> for ProjectApplicationError {
    fn from(error: ProjectRepositoryError) -> Self {
        Self::ProjectRepository(error)
    }
}

impl From<ConversationRepositoryError> for ProjectApplicationError {
    fn from(error: ConversationRepositoryError) -> Self {
        Self::ConversationRepository(error)
    }
}

impl From<AgentRuntimeError> for ProjectApplicationError {
    fn from(error: AgentRuntimeError) -> Self {
        Self::Runtime(error)
    }
}

impl From<ModelResolveError> for ProjectApplicationError {
    fn from(error: ModelResolveError) -> Self {
        Self::ModelResolve(error)
    }
}

impl From<AgentBindingError> for ProjectApplicationError {
    fn from(error: AgentBindingError) -> Self {
        Self::AgentBinding(error)
    }
}

impl fmt::Display for ProjectApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectRepository(error) => write!(formatter, "{error}"),
            Self::ConversationRepository(error) => write!(formatter, "{error}"),
            Self::Runtime(error) => write!(formatter, "{error}"),
            Self::ModelResolve(error) => write!(formatter, "{error}"),
            Self::AgentBinding(error) => write!(formatter, "{error}"),
            Self::Llm(error) => write!(formatter, "{error}"),
            Self::InvalidOutput(message) | Self::Serialization(message) => {
                formatter.write_str(message)
            }
        }
    }
}

impl std::error::Error for ProjectApplicationError {}

pub fn normalize_account_strategy_profile(
    profile: &AccountStrategyProfile,
) -> Result<AccountStrategyProfile, String> {
    Ok(AccountStrategyProfile {
        target_audience: normalize_text_field(
            "目标受众",
            &profile.target_audience,
            ACCOUNT_STRATEGY_TEXT_LIMIT,
        )?,
        content_pillars: normalize_string_list(
            "内容支柱",
            &profile.content_pillars,
            ACCOUNT_STRATEGY_LIST_LIMIT,
            ACCOUNT_STRATEGY_LIST_ITEM_LIMIT,
        )?,
        tone_style: normalize_text_field(
            "表达风格",
            &profile.tone_style,
            ACCOUNT_STRATEGY_TEXT_LIMIT,
        )?,
        forbidden_topics: normalize_string_list(
            "禁区方向",
            &profile.forbidden_topics,
            ACCOUNT_STRATEGY_LIST_LIMIT,
            ACCOUNT_STRATEGY_LIST_ITEM_LIMIT,
        )?,
        reference_accounts: normalize_string_list(
            "参考账号",
            &profile.reference_accounts,
            ACCOUNT_STRATEGY_LIST_LIMIT,
            ACCOUNT_STRATEGY_LIST_ITEM_LIMIT,
        )?,
        topic_preferences: normalize_text_field(
            "选题偏好",
            &profile.topic_preferences,
            ACCOUNT_STRATEGY_TEXT_LIMIT,
        )?,
    })
}

fn normalize_text_field(label: &str, value: &str, max_chars: usize) -> Result<String, String> {
    let normalized = value.trim().to_string();
    if normalized.chars().count() > max_chars {
        return Err(format!("{label}不能超过 {max_chars} 个字符"));
    }
    Ok(normalized)
}

fn normalize_string_list(
    label: &str,
    values: &[String],
    max_items: usize,
    max_item_chars: usize,
) -> Result<Vec<String>, String> {
    if values.len() > max_items {
        return Err(format!("{label}最多填写 {max_items} 项"));
    }

    let mut normalized = Vec::new();
    for value in values {
        let item = value.trim();
        if item.is_empty() {
            continue;
        }
        if item.chars().count() > max_item_chars {
            return Err(format!("{label}单项不能超过 {max_item_chars} 个字符"));
        }
        if !normalized.iter().any(|existing| existing == item) {
            normalized.push(item.to_string());
        }
    }
    Ok(normalized)
}

async fn generate_strategy_profile_draft_with_retry(
    executor: &AuditedModelExecutor,
    request: AuditedModelRequest,
) -> Result<StrategyProfileDraft, ProjectApplicationError> {
    let first_result = executor
        .execute_parsed(request.clone(), |raw| {
            parse_strategy_profile_draft(raw).map_err(|error| error.to_string())
        })
        .await;
    match first_result {
        Ok(response) => Ok(response.output),
        Err(AuditedModelError::Provider {
            model_call_id,
            source,
        }) if is_retryable_strategy_draft_error(&source) => {
            // 策略草稿只允许同模型再试一次，避免瞬时上游故障扩大成本。
            let mut retry = request;
            retry.root_call_id = Some(model_call_id);
            retry.attempt = 2;
            executor
                .execute_parsed(retry, |raw| {
                    parse_strategy_profile_draft(raw).map_err(|error| error.to_string())
                })
                .await
                .map(|response| response.output)
                .map_err(project_audited_model_error)
        }
        Err(error) => Err(project_audited_model_error(error)),
    }
}

fn project_audited_model_error(error: AuditedModelError) -> ProjectApplicationError {
    match error {
        AuditedModelError::Provider { source, .. } => ProjectApplicationError::Llm(source),
        AuditedModelError::StructuredParse { message, .. } => {
            ProjectApplicationError::InvalidOutput(message)
        }
        error => ProjectApplicationError::Runtime(AgentRuntimeError::Kernel(error.to_string())),
    }
}

fn is_retryable_strategy_draft_error(error: &LLMError) -> bool {
    match error {
        LLMError::Provider(message) => {
            let normalized = message.to_ascii_lowercase();
            normalized.contains("502")
                || normalized.contains("503")
                || normalized.contains("504")
                || normalized.contains("429")
                || normalized.contains("upstream_error")
                || normalized.contains("temporarily unavailable")
                || normalized.contains("decoding response body")
                || normalized.contains("rate limit")
        }
        LLMError::Transport(_) => true,
        LLMError::Config(_) | LLMError::Timeout => false,
    }
}

fn build_strategy_profile_draft_prompt(project: &Project, direction_notes: &str) -> String {
    format!(
        r#"请基于当前内容账号资料和补充方向，生成结构化账号策略草稿。

当前账号名称：{name}
定位摘要：{positioning}
账号描述：{description}
当前目标受众：{target_audience}
当前内容支柱：{content_pillars}
当前表达风格：{tone_style}
当前禁区方向：{forbidden_topics}
当前参考账号：{reference_accounts}
当前选题偏好：{topic_preferences}

补充方向：{direction_notes}

输出要求：
1. 只生成草稿，不要表达已保存或已生效。
2. draft 必须包含 target_audience、content_pillars、tone_style、forbidden_topics、reference_accounts、topic_preferences。
3. content_pillars、forbidden_topics、reference_accounts 每组最多 20 项。
4. 不得生成夸大收益、灰产引流或虚假承诺方向。
5. draft_summary 用一句中文总结草稿策略取向。"#,
        name = project.name,
        positioning = project.positioning,
        description = project.description,
        target_audience = project.strategy_profile.target_audience,
        content_pillars = format_prompt_list(&project.strategy_profile.content_pillars),
        tone_style = project.strategy_profile.tone_style,
        forbidden_topics = format_prompt_list(&project.strategy_profile.forbidden_topics),
        reference_accounts = format_prompt_list(&project.strategy_profile.reference_accounts),
        topic_preferences = project.strategy_profile.topic_preferences,
        direction_notes = direction_notes.trim()
    )
}

fn format_prompt_list(values: &[String]) -> String {
    if values.is_empty() {
        return "无".to_string();
    }
    values.join("、")
}

#[derive(Debug, Deserialize)]
struct StrategyProfileDraftLlmOutput {
    draft: AccountStrategyProfile,
    draft_summary: String,
}

fn parse_strategy_profile_draft(
    raw: &str,
) -> Result<StrategyProfileDraft, ProjectApplicationError> {
    let json_text = extract_json_object(raw).map_err(ProjectApplicationError::InvalidOutput)?;
    let output: StrategyProfileDraftLlmOutput = serde_json::from_str(json_text)
        .map_err(|error| ProjectApplicationError::InvalidOutput(error.to_string()))?;
    let draft = normalize_account_strategy_profile(&output.draft)
        .map_err(ProjectApplicationError::InvalidOutput)?;
    if account_strategy_profile_is_empty(&draft) {
        return Err(ProjectApplicationError::InvalidOutput(
            "draft must not be empty".to_string(),
        ));
    }
    let draft_summary = output.draft_summary.trim().to_string();
    if draft_summary.is_empty() {
        return Err(ProjectApplicationError::InvalidOutput(
            "draft_summary must not be empty".to_string(),
        ));
    }
    Ok(StrategyProfileDraft {
        draft,
        draft_summary,
    })
}

fn account_strategy_profile_is_empty(profile: &AccountStrategyProfile) -> bool {
    profile.target_audience.is_empty()
        && profile.content_pillars.is_empty()
        && profile.tone_style.is_empty()
        && profile.forbidden_topics.is_empty()
        && profile.reference_accounts.is_empty()
        && profile.topic_preferences.is_empty()
}

fn extract_json_object(raw: &str) -> Result<&str, String> {
    let start = raw
        .find('{')
        .ok_or_else(|| "missing JSON object start".to_string())?;
    let end = raw
        .rfind('}')
        .ok_or_else(|| "missing JSON object end".to_string())?;
    if start > end {
        return Err("invalid JSON object bounds".to_string());
    }
    Ok(&raw[start..=end])
}
