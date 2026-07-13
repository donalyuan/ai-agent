use crate::agents::llm::{
    LLMOutputError, ScriptLLMOutput, ScriptLLMScene, ScriptMetadataLLMOutput, ScriptPromptBuilder,
    ScriptSceneLLMOutput,
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
use novex_model::{LLMClient, LLMError, LLMPrompt};
use serde_json::json;
use std::fmt;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

const MAX_LLM_PARSE_ATTEMPTS: usize = 3;
const MAX_LLM_PROVIDER_ATTEMPTS: usize = 3;

#[derive(Clone)]
pub struct ScriptAgentService {
    llm_client: Arc<dyn LLMClient>,
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
        llm_client: Arc<dyn LLMClient>,
        script_repository: Arc<dyn ScriptRepository>,
        project_repository: Arc<dyn ProjectRepository>,
    ) -> Self {
        Self {
            llm_client,
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

    pub async fn generate_script(
        &self,
        request: ScriptGenerationInput,
    ) -> Result<Script, ScriptAgentError> {
        if self.generation_mode == ScriptGenerationMode::StepwiseSingleScene {
            return self.generate_script_stepwise(request).await;
        }

        let (request, topic_context) = self.prepare_generate_request(request).await?;

        let prompt: LLMPrompt = ScriptPromptBuilder::build(&request).into();
        let scene_count = request.scene_count_or_default();
        let mut last_parse_error: Option<LLMOutputError> = None;
        let mut retry_count = 0;

        for _ in 0..MAX_LLM_PARSE_ATTEMPTS {
            let (raw, provider_retry_count) =
                self.generate_raw_with_retries(prompt.clone()).await?;
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
                    last_parse_error = Some(error);
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
        let prompt: LLMPrompt = ScriptPromptBuilder::build_metadata(request).into();
        let mut last_parse_error: Option<LLMOutputError> = None;
        let mut retry_count = 0;

        for _ in 0..MAX_LLM_PARSE_ATTEMPTS {
            let (raw, provider_retry_count) =
                self.generate_raw_with_retries(prompt.clone()).await?;
            retry_count += provider_retry_count;

            match ScriptMetadataLLMOutput::parse_and_validate(&raw) {
                Ok(output) => return Ok((output, retry_count)),
                Err(error) => {
                    last_parse_error = Some(error);
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
        let prompt: LLMPrompt = ScriptPromptBuilder::build_single_scene(request, sequence).into();
        let mut last_parse_error: Option<LLMOutputError> = None;
        let mut retry_count = 0;

        for _ in 0..MAX_LLM_PARSE_ATTEMPTS {
            let (raw, provider_retry_count) =
                self.generate_raw_with_retries(prompt.clone()).await?;
            retry_count += provider_retry_count;

            match ScriptSceneLLMOutput::parse_and_validate(&raw, sequence) {
                Ok(output) => return Ok((output.scene, retry_count)),
                Err(error) => {
                    last_parse_error = Some(error);
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
        prompt: LLMPrompt,
    ) -> Result<(String, usize), ScriptAgentError> {
        let mut retry_count = 0;

        for attempt_index in 0..MAX_LLM_PROVIDER_ATTEMPTS {
            match self.llm_client.generate_script(prompt.clone()).await {
                Ok(raw) => return Ok((raw, retry_count)),
                Err(error)
                    if attempt_index + 1 < MAX_LLM_PROVIDER_ATTEMPTS
                        && is_retryable_llm_error(&error) =>
                {
                    retry_count += 1;
                }
                Err(error) => return Err(ScriptAgentError::from(error)),
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
