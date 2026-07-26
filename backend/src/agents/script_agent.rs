use crate::agents::llm::{
    ScriptContextFragment, ScriptLLMOutput, ScriptLLMScene, ScriptMetadataLLMOutput,
    ScriptNodeInputBuilder, ScriptSceneLLMOutput,
};
use crate::domain::script::{
    Scene, Script, ScriptGenerationInput, ScriptListFilter, ScriptStatus, ScriptSummary,
};
use crate::domain::topic::{ContentTopic, ContentTopicStatus};
use crate::repositories::{
    ProjectRepository, ProjectRepositoryError, ScriptRepository, ScriptRepositoryError,
    TopicRepository, TopicRepositoryError,
};
use chrono::Utc;
use novex_agent::{
    text_context_candidate, AuditedCallOwner, AuditedModelError, AuditedModelExecutor,
    AuditedModelRequest, FixedModelBinding, TextContextCandidateInput,
};
use novex_model::LLMError;
use serde_json::json;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use validator::Validate;

const MAX_LLM_PARSE_ATTEMPTS: usize = 3;
const MAX_LLM_PROVIDER_ATTEMPTS: usize = 3;

#[derive(Clone)]
pub struct ScriptAgentService {
    model_executor: Arc<dyn ScriptModelExecutor>,
    script_repository: Arc<dyn ScriptRepository>,
    project_repository: Arc<dyn ProjectRepository>,
    topic_repository: Option<Arc<dyn TopicRepository>>,
    generation_mode: ScriptGenerationMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptGenerationMode {
    Complete,
    StepwiseSingleScene,
}

impl ScriptAgentService {
    pub fn new(
        model_executor: Arc<dyn ScriptModelExecutor>,
        script_repository: Arc<dyn ScriptRepository>,
        project_repository: Arc<dyn ProjectRepository>,
    ) -> Self {
        Self {
            model_executor,
            script_repository,
            project_repository,
            topic_repository: None,
            generation_mode: ScriptGenerationMode::Complete,
        }
    }

    pub fn with_generation_mode(mut self, generation_mode: ScriptGenerationMode) -> Self {
        self.generation_mode = generation_mode;
        self
    }

    pub fn with_topic_repository(mut self, topic_repository: Arc<dyn TopicRepository>) -> Self {
        self.topic_repository = Some(topic_repository);
        self
    }

    pub async fn generate(
        &self,
        request: ScriptGenerationInput,
    ) -> Result<Script, ScriptAgentError> {
        if self.generation_mode == ScriptGenerationMode::StepwiseSingleScene {
            return self.generate_script_stepwise(request).await;
        }

        let (request, topic_context) = self.prepare_generate_request(request).await?;

        let node_input = ScriptNodeInputBuilder::build(&request);
        let scene_count = request.scene_count_or_default();
        let mut last_parse_error: Option<String> = None;
        let mut retry_count = 0;
        let mut call_sequence = ScriptCallSequence::new();

        for _ in 0..MAX_LLM_PARSE_ATTEMPTS {
            let generated = self
                .generate_raw_with_retries(
                    "script.complete",
                    node_input.context.clone(),
                    ScriptOutputContract::Complete { scene_count },
                    BTreeMap::from([("scene_count".into(), json!(scene_count))]),
                    &mut call_sequence,
                )
                .await;
            let (raw, provider_retry_count) = match generated {
                Ok(value) => value,
                Err(ScriptAgentError::ParseError(message)) => {
                    last_parse_error = Some(message);
                    retry_count += 1;
                    continue;
                }
                Err(error) => return Err(error),
            };
            retry_count += provider_retry_count;

            match ScriptLLMOutput::parse_and_validate(&raw, scene_count) {
                Ok(output) => {
                    let script = self.build_script(
                        request,
                        output,
                        retry_count,
                        "complete",
                        topic_context.as_ref(),
                    );
                    return self.save_script_and_update_topic(script).await;
                }
                Err(error) => {
                    last_parse_error = Some(error.to_string());
                    retry_count += 1;
                }
            }
        }

        Err(ScriptAgentError::ParseError(
            last_parse_error
                .map(|error| error.to_string())
                .unwrap_or_else(|| "LLM output parse failed".to_string()),
        ))
    }

    pub async fn generate_script_stepwise(
        &self,
        request: ScriptGenerationInput,
    ) -> Result<Script, ScriptAgentError> {
        let (request, topic_context) = self.prepare_generate_request(request).await?;

        let (metadata, metadata_retries) = self.generate_metadata(&request).await?;
        let scene_count = request.scene_count_or_default();
        let mut scenes = Vec::with_capacity(usize::from(scene_count));
        let mut retry_count = metadata_retries;

        for sequence in 1..=scene_count {
            let (scene, scene_retries) = self.generate_single_scene(&request, sequence).await?;
            retry_count += scene_retries;
            scenes.push(scene);
        }

        let output = ScriptLLMOutput {
            title: metadata.title,
            hook: metadata.hook,
            scenes,
        };
        let script = self.build_script(
            request,
            output,
            retry_count,
            "stepwise_single_scene",
            topic_context.as_ref(),
        );

        self.save_script_and_update_topic(script).await
    }

    pub async fn get_script(&self, script_id: Uuid) -> Result<Script, ScriptAgentError> {
        self.script_repository
            .get_script(script_id)
            .await
            .map_err(ScriptAgentError::from)
    }

    pub async fn list_scripts(
        &self,
        project_id: Uuid,
        filter: ScriptListFilter,
    ) -> Result<ScriptListResult, ScriptAgentError> {
        filter.validate().map_err(|error| {
            ScriptAgentError::Validation(format!("invalid script list filter: {error}"))
        })?;
        self.ensure_project_exists(project_id).await?;

        let total = self
            .script_repository
            .count_scripts(project_id, filter.status.clone())
            .await?;
        let limit = filter.limit_or_default();
        let offset = filter.offset_or_default();
        let scripts = self
            .script_repository
            .list_script_summaries(project_id, filter)
            .await?;

        Ok(ScriptListResult {
            scripts,
            total,
            limit,
            offset,
        })
    }

    pub async fn update_status(
        &self,
        script_id: Uuid,
        status: ScriptStatus,
    ) -> Result<Script, ScriptAgentError> {
        self.script_repository
            .update_script_status(script_id, status)
            .await
            .map_err(ScriptAgentError::from)
    }

    async fn ensure_project_exists(&self, project_id: Uuid) -> Result<(), ScriptAgentError> {
        if self.project_repository.project_exists(project_id).await? {
            Ok(())
        } else {
            Err(ScriptAgentError::ProjectNotFound(project_id))
        }
    }

    async fn prepare_generate_request(
        &self,
        mut request: ScriptGenerationInput,
    ) -> Result<(ScriptGenerationInput, Option<ContentTopic>), ScriptAgentError> {
        request.validate().map_err(|error| {
            ScriptAgentError::Validation(format!("invalid generate script request: {error}"))
        })?;
        self.ensure_project_exists(request.project_id).await?;
        if let Some(parent_id) = request.parent_id {
            self.ensure_parent_script_matches_project(parent_id, request.project_id)
                .await?;
        }

        let topic_context = if let Some(topic_id) = request.topic_id {
            let topic_repository = self.topic_repository.as_ref().ok_or_else(|| {
                ScriptAgentError::Validation("topic repository is not configured".to_string())
            })?;
            let topic = topic_repository.get_topic(topic_id).await?;
            if topic.project_id != request.project_id {
                return Err(ScriptAgentError::Validation(
                    "topic_id must belong to the same project".to_string(),
                ));
            }
            if topic.status != ContentTopicStatus::Approved {
                return Err(ScriptAgentError::Validation(
                    "only approved topics can generate scripts".to_string(),
                ));
            }
            if request.topic.trim().is_empty() {
                request.topic = topic.title.clone();
            }
            Some(topic)
        } else {
            let topic = request.topic.trim().to_string();
            if topic.chars().count() < 10 {
                return Err(ScriptAgentError::Validation(
                    "topic must be at least 10 characters when topic_id is not provided"
                        .to_string(),
                ));
            }
            request.topic = topic;
            None
        };

        Ok((request, topic_context))
    }

    async fn generate_metadata(
        &self,
        request: &ScriptGenerationInput,
    ) -> Result<(ScriptMetadataLLMOutput, usize), ScriptAgentError> {
        let node_input = ScriptNodeInputBuilder::build_metadata(request);
        let mut last_parse_error: Option<String> = None;
        let mut retry_count = 0;
        let mut call_sequence = ScriptCallSequence::new();

        for _ in 0..MAX_LLM_PARSE_ATTEMPTS {
            let generated = self
                .generate_raw_with_retries(
                    "script.metadata",
                    node_input.context.clone(),
                    ScriptOutputContract::Metadata,
                    BTreeMap::new(),
                    &mut call_sequence,
                )
                .await;
            let (raw, provider_retry_count) = match generated {
                Ok(value) => value,
                Err(ScriptAgentError::ParseError(message)) => {
                    last_parse_error = Some(message);
                    retry_count += 1;
                    continue;
                }
                Err(error) => return Err(error),
            };
            retry_count += provider_retry_count;

            match ScriptMetadataLLMOutput::parse_and_validate(&raw) {
                Ok(output) => return Ok((output, retry_count)),
                Err(error) => {
                    last_parse_error = Some(error.to_string());
                    retry_count += 1;
                }
            }
        }

        Err(ScriptAgentError::ParseError(
            last_parse_error
                .map(|error| error.to_string())
                .unwrap_or_else(|| "LLM metadata parse failed".to_string()),
        ))
    }

    async fn generate_single_scene(
        &self,
        request: &ScriptGenerationInput,
        sequence: u8,
    ) -> Result<(ScriptLLMScene, usize), ScriptAgentError> {
        let node_input = ScriptNodeInputBuilder::build_single_scene(request, sequence);
        let mut last_parse_error: Option<String> = None;
        let mut retry_count = 0;
        let mut call_sequence = ScriptCallSequence::new();

        for _ in 0..MAX_LLM_PARSE_ATTEMPTS {
            let generated = self
                .generate_raw_with_retries(
                    "script.single_scene",
                    node_input.context.clone(),
                    ScriptOutputContract::SingleScene { sequence },
                    BTreeMap::new(),
                    &mut call_sequence,
                )
                .await;
            let (raw, provider_retry_count) = match generated {
                Ok(value) => value,
                Err(ScriptAgentError::ParseError(message)) => {
                    last_parse_error = Some(message);
                    retry_count += 1;
                    continue;
                }
                Err(error) => return Err(error),
            };
            retry_count += provider_retry_count;

            match ScriptSceneLLMOutput::parse_and_validate(&raw, sequence) {
                Ok(output) => return Ok((output.scene, retry_count)),
                Err(error) => {
                    last_parse_error = Some(error.to_string());
                    retry_count += 1;
                }
            }
        }

        Err(ScriptAgentError::ParseError(
            last_parse_error
                .map(|error| error.to_string())
                .unwrap_or_else(|| format!("LLM scene {sequence} parse failed")),
        ))
    }

    async fn ensure_parent_script_matches_project(
        &self,
        parent_id: Uuid,
        project_id: Uuid,
    ) -> Result<(), ScriptAgentError> {
        let parent = self.script_repository.get_script(parent_id).await?;
        if parent.project_id == project_id {
            Ok(())
        } else {
            Err(ScriptAgentError::Validation(
                "parent_id must belong to the same project".to_string(),
            ))
        }
    }

    async fn generate_raw_with_retries(
        &self,
        node_key: &str,
        context: Vec<ScriptContextFragment>,
        contract: ScriptOutputContract,
        variables: BTreeMap<String, serde_json::Value>,
        call_sequence: &mut ScriptCallSequence,
    ) -> Result<(String, usize), ScriptAgentError> {
        let mut retry_count = 0;

        for attempt_index in 0..MAX_LLM_PROVIDER_ATTEMPTS {
            let attempt = call_sequence.next_attempt;
            call_sequence.next_attempt += 1;
            let result = self
                .model_executor
                .execute(ScriptModelCall {
                    node_key: node_key.into(),
                    context: context.clone(),
                    contract,
                    variables: variables.clone(),
                    root_call_id: call_sequence.root_call_id,
                    attempt,
                })
                .await;
            match result {
                Ok(response) => {
                    call_sequence.remember_root(response.model_call_id);
                    return Ok((response.output, retry_count));
                }
                Err(ScriptModelExecutionError::Provider {
                    model_call_id,
                    source,
                }) if attempt_index + 1 < MAX_LLM_PROVIDER_ATTEMPTS
                    && is_retryable_llm_error(&source) =>
                {
                    if let Some(model_call_id) = model_call_id {
                        call_sequence.remember_root(model_call_id);
                    }
                    retry_count += 1;
                }
                Err(ScriptModelExecutionError::Provider { source, .. }) => {
                    return Err(ScriptAgentError::from(source));
                }
                Err(ScriptModelExecutionError::StructuredParse {
                    model_call_id,
                    message,
                }) => {
                    call_sequence.remember_root(model_call_id);
                    return Err(ScriptAgentError::ParseError(message));
                }
                Err(ScriptModelExecutionError::Execution(message)) => {
                    return Err(ScriptAgentError::LLMError(message));
                }
            }
        }

        unreachable!("provider retry loop must return on success or final error")
    }

    fn build_script(
        &self,
        request: ScriptGenerationInput,
        output: ScriptLLMOutput,
        retry_count: usize,
        generation_mode: &str,
        topic_context: Option<&ContentTopic>,
    ) -> Script {
        let now = Utc::now();
        let scenes: Vec<Scene> = output
            .scenes
            .into_iter()
            .map(|scene| Scene {
                id: Uuid::new_v4(),
                sequence: scene.sequence,
                narration: scene.narration,
                visual_description: scene.visual_description,
                emotion: scene.emotion,
                duration_sec: scene.duration_sec,
            })
            .collect();
        let total_duration_sec: i32 = scenes.iter().map(|scene| scene.duration_sec).sum();
        let style = request.style_or_default();
        let mut content = json!({
            "topic": request.topic,
            "style": style.as_str(),
            "total_duration_sec": total_duration_sec,
            "metadata": {
                "retry_count": retry_count,
                "generation_mode": generation_mode
            }
        });
        if let Some(topic) = topic_context {
            content["topic_id"] = json!(topic.id);
            content["topic_snapshot"] = topic.snapshot();
        }

        Script::new(
            Uuid::new_v4(),
            request.project_id,
            topic_context.map(|topic| topic.id),
            output.title,
            output.hook,
            content,
            ScriptStatus::Draft,
            request.parent_id,
            scenes,
            now,
            now,
        )
    }

    async fn save_script_and_update_topic(
        &self,
        script: Script,
    ) -> Result<Script, ScriptAgentError> {
        let topic_id = script.topic_id;
        let saved = self.script_repository.save_script(script).await?;
        if let Some(topic_id) = topic_id {
            let topic_repository = self.topic_repository.as_ref().ok_or_else(|| {
                ScriptAgentError::Validation("topic repository is not configured".to_string())
            })?;
            topic_repository
                .update_topic_status(topic_id, ContentTopicStatus::Scripted)
                .await?;
        }
        Ok(saved)
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ScriptOutputContract {
    Complete { scene_count: u8 },
    Metadata,
    SingleScene { sequence: u8 },
}

pub struct ScriptModelCall {
    node_key: String,
    context: Vec<ScriptContextFragment>,
    contract: ScriptOutputContract,
    variables: BTreeMap<String, serde_json::Value>,
    root_call_id: Option<Uuid>,
    attempt: i32,
}

pub struct ScriptModelResponse {
    model_call_id: Uuid,
    output: String,
}

pub enum ScriptModelExecutionError {
    Provider {
        model_call_id: Option<Uuid>,
        source: LLMError,
    },
    StructuredParse {
        model_call_id: Uuid,
        message: String,
    },
    Execution(String),
}

#[async_trait::async_trait]
pub trait ScriptModelExecutor: Send + Sync {
    async fn execute(
        &self,
        call: ScriptModelCall,
    ) -> Result<ScriptModelResponse, ScriptModelExecutionError>;
}

impl ScriptModelCall {
    pub fn context_fragments(&self) -> &[ScriptContextFragment] {
        &self.context
    }
}

impl ScriptModelResponse {
    pub fn new(output: impl Into<String>) -> Self {
        Self {
            model_call_id: Uuid::new_v4(),
            output: output.into(),
        }
    }
}

impl ScriptModelExecutionError {
    pub fn provider(source: LLMError) -> Self {
        Self::Provider {
            model_call_id: None,
            source,
        }
    }
}

pub struct AuditedScriptModelExecutor {
    executor: Arc<AuditedModelExecutor>,
    owner: AuditedCallOwner,
    agent_key: String,
    agent_version: String,
    binding: FixedModelBinding,
    call_ids: Option<Arc<Mutex<Vec<Uuid>>>>,
}

impl AuditedScriptModelExecutor {
    pub fn new(
        executor: Arc<AuditedModelExecutor>,
        owner: AuditedCallOwner,
        agent_key: String,
        agent_version: String,
        binding: FixedModelBinding,
    ) -> Self {
        Self {
            executor,
            owner,
            agent_key,
            agent_version,
            binding,
            call_ids: None,
        }
    }

    /// Exposes call IDs to the conversation adapter so it can link terminal calls to the existing Step.
    pub fn with_call_ids(mut self, call_ids: Arc<Mutex<Vec<Uuid>>>) -> Self {
        self.call_ids = Some(call_ids);
        self
    }
}

#[async_trait::async_trait]
impl ScriptModelExecutor for AuditedScriptModelExecutor {
    async fn execute(
        &self,
        call: ScriptModelCall,
    ) -> Result<ScriptModelResponse, ScriptModelExecutionError> {
        let compiled_at = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
        let owner_id = match self.owner {
            AuditedCallOwner::Conversation(id)
            | AuditedCallOwner::AgentRun(id)
            | AuditedCallOwner::EvalRun(id) => id,
        };
        let mut context_candidates = Vec::with_capacity(call.context.len());
        let mut context_sources = Vec::with_capacity(call.context.len());
        for fragment in call.context {
            let candidate_id = format!(
                "{}:{}:attempt:{}",
                call.node_key, fragment.key, call.attempt
            );
            context_sources.push(json!({
                "id": candidate_id,
                "trust": fragment.trust,
                "source": fragment.source_kind,
            }));
            context_candidates.push(text_context_candidate(TextContextCandidateInput {
                candidate_id,
                source_kind: fragment.source_kind.into(),
                source_id: format!("{owner_id}:{}", fragment.key),
                source_version: "1".into(),
                trust: fragment.trust,
                priority: fragment.priority,
                required: fragment.required,
                render_order: fragment.render_order,
                observed_at: compiled_at.clone(),
                text: fragment.content,
            }));
        }
        let contract = call.contract;
        let result = self
            .executor
            .execute_parsed(
                AuditedModelRequest {
                    owner: self.owner,
                    step_id: None,
                    root_call_id: call.root_call_id,
                    parent_call_id: None,
                    attempt: call.attempt,
                    agent_key: self.agent_key.clone(),
                    agent_version: self.agent_version.clone(),
                    node_key: call.node_key,
                    variables: call.variables,
                    context_candidates,
                    context_atomic_groups: Vec::new(),
                    compiled_at,
                    tool_profile: "chat".into(),
                    tool_schema: None,
                    binding: self.binding.clone(),
                    context_sources: serde_json::Value::Array(context_sources),
                    memory_sources: json!([]),
                    parameters: json!({}),
                    asset_references: json!([]),
                },
                move |raw| validate_script_output(raw, contract).map(|()| raw.to_string()),
            )
            .await;
        match result {
            Ok(response) => {
                if let Some(call_ids) = &self.call_ids {
                    call_ids
                        .lock()
                        .map_err(|_| {
                            ScriptModelExecutionError::Execution(
                                "audited call IDs lock poisoned".into(),
                            )
                        })?
                        .push(response.model_call_id);
                }
                Ok(ScriptModelResponse {
                    model_call_id: response.model_call_id,
                    output: response.output,
                })
            }
            Err(AuditedModelError::Provider {
                model_call_id,
                source,
            }) => Err(ScriptModelExecutionError::Provider {
                model_call_id: Some(model_call_id),
                source,
            }),
            Err(AuditedModelError::StructuredParse {
                model_call_id,
                message,
            }) => Err(ScriptModelExecutionError::StructuredParse {
                model_call_id,
                message,
            }),
            Err(error) => Err(ScriptModelExecutionError::Execution(error.to_string())),
        }
    }
}

fn validate_script_output(raw: &str, contract: ScriptOutputContract) -> Result<(), String> {
    match contract {
        ScriptOutputContract::Complete { scene_count } => {
            ScriptLLMOutput::parse_and_validate(raw, scene_count).map(|_| ())
        }
        ScriptOutputContract::Metadata => {
            ScriptMetadataLLMOutput::parse_and_validate(raw).map(|_| ())
        }
        ScriptOutputContract::SingleScene { sequence } => {
            ScriptSceneLLMOutput::parse_and_validate(raw, sequence).map(|_| ())
        }
    }
    .map_err(|error| error.to_string())
}

struct ScriptCallSequence {
    root_call_id: Option<Uuid>,
    next_attempt: i32,
}

impl ScriptCallSequence {
    fn new() -> Self {
        Self {
            root_call_id: None,
            next_attempt: 1,
        }
    }

    fn remember_root(&mut self, model_call_id: Uuid) {
        if self.root_call_id.is_none() {
            self.root_call_id = Some(model_call_id);
        }
    }
}

fn is_retryable_llm_error(error: &LLMError) -> bool {
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

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptListResult {
    pub scripts: Vec<ScriptSummary>,
    pub total: i64,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug)]
pub enum ScriptAgentError {
    Validation(String),
    ProjectNotFound(Uuid),
    ScriptNotFound(Uuid),
    Timeout,
    LLMError(String),
    ParseError(String),
    DatabaseError(String),
}

impl From<LLMError> for ScriptAgentError {
    fn from(error: LLMError) -> Self {
        match error {
            LLMError::Timeout => Self::Timeout,
            other => Self::LLMError(other.to_string()),
        }
    }
}

impl From<ProjectRepositoryError> for ScriptAgentError {
    fn from(error: ProjectRepositoryError) -> Self {
        Self::DatabaseError(error.to_string())
    }
}

impl From<TopicRepositoryError> for ScriptAgentError {
    fn from(error: TopicRepositoryError) -> Self {
        match error {
            TopicRepositoryError::TopicNotFound(topic_id) => {
                Self::Validation(format!("content topic not found: {topic_id}"))
            }
            TopicRepositoryError::InvalidStatusTransition { .. } => {
                Self::Validation(error.to_string())
            }
            TopicRepositoryError::BatchNotFound(_)
            | TopicRepositoryError::BatchCannotBeSupplemented(_)
            | TopicRepositoryError::TopicCannotBeDeleted(_)
            | TopicRepositoryError::Storage(_) => Self::DatabaseError(error.to_string()),
        }
    }
}

impl From<ScriptRepositoryError> for ScriptAgentError {
    fn from(error: ScriptRepositoryError) -> Self {
        match error {
            ScriptRepositoryError::NotFound(script_id) => Self::ScriptNotFound(script_id),
            ScriptRepositoryError::SceneNotFound {
                script_id,
                sequence,
            } => Self::DatabaseError(format!(
                "scene not found: script_id={script_id}, sequence={sequence}"
            )),
            ScriptRepositoryError::Storage(message) => Self::DatabaseError(message),
        }
    }
}

impl fmt::Display for ScriptAgentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(message) => write!(formatter, "{message}"),
            Self::ProjectNotFound(project_id) => {
                write!(formatter, "project not found: {project_id}")
            }
            Self::ScriptNotFound(script_id) => write!(formatter, "script not found: {script_id}"),
            Self::Timeout => write!(formatter, "script generation timeout"),
            Self::LLMError(message) => write!(formatter, "llm error: {message}"),
            Self::ParseError(message) => write!(formatter, "script parse error: {message}"),
            Self::DatabaseError(message) => write!(formatter, "database error: {message}"),
        }
    }
}

impl std::error::Error for ScriptAgentError {}
