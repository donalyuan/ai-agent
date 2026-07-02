use crate::agents::llm::{LLMOutputError, ScriptLLMOutput, ScriptPromptBuilder};
use crate::agents::models::{
    GenerateScriptRequest, Scene, Script, ScriptListFilter, ScriptStatus, ScriptSummary,
};
use crate::repositories::{
    ProjectRepository, ProjectRepositoryError, ScriptRepository, ScriptRepositoryError,
};
use chrono::Utc;
use novex_model::{LLMClient, LLMError};
use serde_json::json;
use std::fmt;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

const MAX_LLM_PARSE_ATTEMPTS: usize = 3;

#[derive(Clone)]
pub struct ScriptAgentService {
    llm_client: Arc<dyn LLMClient>,
    script_repository: Arc<dyn ScriptRepository>,
    project_repository: Arc<dyn ProjectRepository>,
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
        }
    }

    pub async fn generate_script(
        &self,
        request: GenerateScriptRequest,
    ) -> Result<Script, ScriptAgentError> {
        request.validate().map_err(|error| {
            ScriptAgentError::Validation(format!("invalid generate script request: {error}"))
        })?;
        self.ensure_project_exists(request.project_id).await?;
        if let Some(parent_id) = request.parent_id {
            self.ensure_parent_script_matches_project(parent_id, request.project_id)
                .await?;
        }

        let prompt = ScriptPromptBuilder::build(&request);
        let scene_count = request.scene_count_or_default();
        let mut last_parse_error: Option<LLMOutputError> = None;

        for attempt_index in 0..MAX_LLM_PARSE_ATTEMPTS {
            let raw = self
                .llm_client
                .generate_script(prompt.clone().into())
                .await
                .map_err(ScriptAgentError::from)?;

            match ScriptLLMOutput::parse_and_validate(&raw, scene_count) {
                Ok(output) => {
                    let retry_count = attempt_index;
                    let script = self.build_script(request, output, retry_count);
                    return self
                        .script_repository
                        .save_script(script)
                        .await
                        .map_err(ScriptAgentError::from);
                }
                Err(error) => {
                    last_parse_error = Some(error);
                }
            }
        }

        Err(ScriptAgentError::ParseError(
            last_parse_error
                .map(|error| error.to_string())
                .unwrap_or_else(|| "LLM output parse failed".to_string()),
        ))
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

    fn build_script(
        &self,
        request: GenerateScriptRequest,
        output: ScriptLLMOutput,
        retry_count: usize,
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
        let content = json!({
            "topic": request.topic,
            "style": style.as_str(),
            "total_duration_sec": total_duration_sec,
            "metadata": {
                "retry_count": retry_count
            }
        });

        Script::new(
            Uuid::new_v4(),
            request.project_id,
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

impl From<ScriptRepositoryError> for ScriptAgentError {
    fn from(error: ScriptRepositoryError) -> Self {
        match error {
            ScriptRepositoryError::NotFound(script_id) => Self::ScriptNotFound(script_id),
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
