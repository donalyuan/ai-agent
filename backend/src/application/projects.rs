//! 项目与账号策略用例，集中处理仓储协作、模型调用和运行记录。

use crate::domain::conversation::{CreateAgentRunInput, FinishAgentRunInput};
use crate::model_routing::{ModelClientResolver, ModelResolveError};
use crate::repositories::{
    AccountStrategyProfile, ConversationRepository, ConversationRepositoryError,
    CreateProjectInput, PostgresConversationRepository, PostgresProjectRepository, Project,
    ProjectRepository, ProjectRepositoryError, UpdateProjectStrategyProfileInput,
};
use novex_model::{LLMClient, LLMError, LLMJsonSchema, LLMPrompt};
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
}

impl ProjectService {
    pub fn new(
        project_repository: PostgresProjectRepository,
        conversation_repository: PostgresConversationRepository,
        model_resolver: Arc<dyn ModelClientResolver>,
    ) -> Self {
        Self {
            project_repository,
            conversation_repository,
            model_resolver,
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
        let run = self
            .create_run(
                project_id,
                model_id,
                serde_json::to_value(&resolved.snapshot)
                    .map_err(|error| ProjectApplicationError::Serialization(error.to_string()))?,
            )
            .await?;

        let result = generate_strategy_profile_draft_with_retry(
            resolved.client.as_ref(),
            build_strategy_profile_draft_prompt(&project, direction_notes),
        )
        .await
        .map_err(ProjectApplicationError::Llm)
        .and_then(|raw| parse_strategy_profile_draft(&raw));

        match result {
            Ok(output) => {
                self.finish_run(
                    run.id,
                    "succeeded",
                    Some(json!({ "draft_generated": true })),
                    None,
                )
                .await?;
                Ok(output)
            }
            Err(error) => {
                self.finish_run(run.id, "failed", None, Some(format!("{error:?}")))
                    .await?;
                Err(error)
            }
        }
    }

    async fn create_run(
        &self,
        project_id: Uuid,
        model_id: Uuid,
        model_snapshot: serde_json::Value,
    ) -> Result<crate::domain::conversation::AgentRunRecord, ProjectApplicationError> {
        self.conversation_repository
            .create_run(CreateAgentRunInput {
                conversation_id: project_id,
                project_id: Some(project_id),
                agent_type: "topic".to_string(),
                input: json!({ "intent": "strategy_profile_draft" }),
                model_id: Some(model_id),
                model_snapshot: Some(model_snapshot),
            })
            .await
            .map_err(Into::into)
    }

    async fn finish_run(
        &self,
        run_id: Uuid,
        status: &str,
        output: Option<serde_json::Value>,
        error_message: Option<String>,
    ) -> Result<(), ProjectApplicationError> {
        self.conversation_repository
            .finish_run(FinishAgentRunInput {
                agent_run_id: run_id,
                status: status.to_string(),
                output,
                error_message,
            })
            .await
            .map(|_| ())
            .map_err(Into::into)
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
    ModelResolve(ModelResolveError),
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

impl From<ModelResolveError> for ProjectApplicationError {
    fn from(error: ModelResolveError) -> Self {
        Self::ModelResolve(error)
    }
}

impl fmt::Display for ProjectApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectRepository(error) => write!(formatter, "{error}"),
            Self::ConversationRepository(error) => write!(formatter, "{error}"),
            Self::ModelResolve(error) => write!(formatter, "{error}"),
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
    llm_client: &dyn LLMClient,
    prompt: LLMPrompt,
) -> Result<String, LLMError> {
    let first_result = llm_client.generate_script(prompt.clone()).await;
    match first_result {
        Ok(raw) => Ok(raw),
        Err(error) if is_retryable_strategy_draft_error(&error) => {
            // 策略草稿只允许同模型再试一次，避免瞬时上游故障扩大成本。
            llm_client.generate_script(prompt).await
        }
        Err(error) => Err(error),
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

fn build_strategy_profile_draft_prompt(project: &Project, direction_notes: &str) -> LLMPrompt {
    LLMPrompt {
        system: "你是短视频内容账号策略顾问。你必须只输出符合 JSON Schema 的合法 JSON 对象，不要输出 Markdown 或解释。"
            .to_string(),
        user: format!(
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
        ),
        max_output_tokens: Some(1_200),
        output_schema: Some(strategy_profile_draft_output_schema()),
    }
}

fn format_prompt_list(values: &[String]) -> String {
    if values.is_empty() {
        return "无".to_string();
    }
    values.join("、")
}

fn strategy_profile_draft_output_schema() -> LLMJsonSchema {
    LLMJsonSchema {
        name: "account_strategy_profile_draft".to_string(),
        strict: true,
        schema: json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["draft", "draft_summary"],
            "properties": {
                "draft": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                        "target_audience",
                        "content_pillars",
                        "tone_style",
                        "forbidden_topics",
                        "reference_accounts",
                        "topic_preferences"
                    ],
                    "properties": {
                        "target_audience": { "type": "string" },
                        "content_pillars": {
                            "type": "array",
                            "maxItems": 20,
                            "items": { "type": "string" }
                        },
                        "tone_style": { "type": "string" },
                        "forbidden_topics": {
                            "type": "array",
                            "maxItems": 20,
                            "items": { "type": "string" }
                        },
                        "reference_accounts": {
                            "type": "array",
                            "maxItems": 20,
                            "items": { "type": "string" }
                        },
                        "topic_preferences": { "type": "string" }
                    }
                },
                "draft_summary": { "type": "string" }
            }
        }),
    }
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
