//! 选题用例，维护项目边界、状态规则、主题组归一和评审编排。

use crate::agents::ScriptAgentError;
use crate::application::agents::adapters::{
    AgentRuntimeError, AuditedTopicReviewExecution, TopicAgentAdapter,
};
use crate::application::agents::kernel::{
    active_rust_definition_binding, PostgresAgentKernelStore,
};
use crate::domain::conversation::ModelBindingEvidence;
use crate::domain::script::ScriptStyle;
use crate::domain::topic::{
    ContentTopic, ContentTopicFilter, ContentTopicStatus, TopicGenerationBatchSummary,
    TopicGroupSort, TopicGroupSummary, TopicQualityEvaluation, TopicReviewSnapshot,
};
use crate::model_routing::{model_behavior_evidence, ModelClientResolver, ModelResolveError};
use crate::repositories::{
    CreateContentTopicInput, PostgresConversationRepository, PostgresProjectRepository,
    PostgresTopicRepository, ProjectRepository, ProjectRepositoryError, TopicRepository,
    TopicRepositoryError, UpdateContentTopicInput,
};
use novex_agent::{AuditedExecutionBinding, AuditedModelExecutor, FixedModelBinding};
use novex_ai_core::{validate_model_capabilities, DefinitionRegistry};
use serde_json::Value;
use std::{fmt, sync::Arc};
use uuid::Uuid;

#[derive(Clone)]
/// 管理选题、批次、主题组评审及进入脚本生成前的状态规则。
pub struct TopicService {
    project_repository: PostgresProjectRepository,
    topic_repository: PostgresTopicRepository,
    conversation_repository: PostgresConversationRepository,
    model_resolver: Arc<dyn ModelClientResolver>,
    definition_registry: Arc<DefinitionRegistry>,
    audited_model_executor: Arc<AuditedModelExecutor>,
}

impl TopicService {
    pub fn new(
        project_repository: PostgresProjectRepository,
        topic_repository: PostgresTopicRepository,
        conversation_repository: PostgresConversationRepository,
        model_resolver: Arc<dyn ModelClientResolver>,
        definition_registry: Arc<DefinitionRegistry>,
        audited_model_executor: Arc<AuditedModelExecutor>,
    ) -> Self {
        Self {
            project_repository,
            topic_repository,
            conversation_repository,
            model_resolver,
            definition_registry,
            audited_model_executor,
        }
    }

    pub async fn create(
        &self,
        input: CreateContentTopicInput,
    ) -> Result<ContentTopic, TopicApplicationError> {
        self.ensure_project_exists(input.project_id).await?;
        self.topic_repository
            .create_topic(input)
            .await
            .map_err(Into::into)
    }

    pub async fn list(
        &self,
        project_id: Uuid,
        filter: ContentTopicFilter,
    ) -> Result<TopicListResult, TopicApplicationError> {
        self.ensure_project_exists(project_id).await?;
        let topics = self
            .topic_repository
            .list_topics(project_id, filter)
            .await?;
        let stats = self
            .topic_repository
            .count_topics_by_status(project_id)
            .await?;
        Ok(TopicListResult { topics, stats })
    }

    pub async fn list_generation_batches(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<TopicGenerationBatchSummary>, TopicApplicationError> {
        self.ensure_project_exists(project_id).await?;
        self.topic_repository
            .list_generation_batches(project_id, 20)
            .await
            .map_err(Into::into)
    }

    pub async fn list_groups(
        &self,
        project_id: Uuid,
        sort: TopicGroupSort,
    ) -> Result<Vec<TopicGroupSummary>, TopicApplicationError> {
        self.ensure_project_exists(project_id).await?;
        self.topic_repository
            .list_topic_group_summaries(project_id, sort, 20)
            .await
            .map_err(Into::into)
    }

    pub async fn review_group(
        &self,
        root_batch_id: Uuid,
        requested_project_id: Option<Uuid>,
        model_id: Uuid,
    ) -> Result<TopicReviewSnapshot, TopicApplicationError> {
        let project_id = self
            .resolve_group_project_id(root_batch_id, requested_project_id)
            .await?;
        let resolved = self.model_resolver.text_client(model_id).await?;
        let evidence = model_behavior_evidence(&resolved.snapshot)?;
        let agent = self
            .definition_registry
            .active_agent("video.topic")
            .map_err(|error| TopicApplicationError::Definition(error.to_string()))?;
        validate_model_capabilities(&agent.model_requirements, &evidence.capabilities)
            .map_err(|_| TopicApplicationError::ModelCapabilityMismatch)?;
        let definition = active_rust_definition_binding(&self.definition_registry, "video.topic")
            .map_err(TopicApplicationError::Definition)?;
        let model_binding = ModelBindingEvidence {
            model_id,
            behavior_fingerprint: evidence.behavior_fingerprint.clone(),
            model_capabilities: serde_json::to_value(&evidence.capabilities)
                .map_err(|error| TopicApplicationError::Serialization(error.to_string()))?,
        };
        let store = Arc::new(PostgresAgentKernelStore::new(
            self.conversation_repository.clone(),
        ));
        self.topic_adapter()
            .review_topic_group_audited(
                project_id,
                root_batch_id,
                resolved.snapshot,
                AuditedTopicReviewExecution {
                    definition: definition.clone(),
                    model_binding,
                    audited: AuditedExecutionBinding {
                        executor: self.audited_model_executor.clone(),
                        agent_key: definition.agent_key,
                        agent_version: definition.agent_version,
                        binding: FixedModelBinding {
                            model_id,
                            behavior_fingerprint: evidence.behavior_fingerprint,
                        },
                    },
                },
                self.conversation_repository.clone(),
                store.clone(),
                store,
            )
            .await
            .map_err(Into::into)
    }

    pub async fn latest_group_review(
        &self,
        root_batch_id: Uuid,
        requested_project_id: Option<Uuid>,
    ) -> Result<Option<TopicReviewSnapshot>, TopicApplicationError> {
        let project_id = self
            .resolve_group_project_id(root_batch_id, requested_project_id)
            .await?;
        self.topic_repository
            .get_latest_topic_review_snapshot(project_id, root_batch_id)
            .await
            .map_err(Into::into)
    }

    pub async fn latest_quality_evaluation(
        &self,
        batch_id: Uuid,
        requested_project_id: Option<Uuid>,
    ) -> Result<Option<TopicQualityEvaluation>, TopicApplicationError> {
        let batch = self.topic_repository.get_generation_batch(batch_id).await?;
        if requested_project_id.is_some_and(|project_id| project_id != batch.project_id) {
            return Err(TopicRepositoryError::BatchNotFound(batch_id).into());
        }
        self.topic_repository
            .get_latest_topic_quality_evaluation(batch.project_id, batch_id)
            .await
            .map_err(Into::into)
    }

    pub async fn update(
        &self,
        topic_id: Uuid,
        input: UpdateContentTopicInput,
    ) -> Result<ContentTopic, TopicApplicationError> {
        self.topic_repository
            .update_topic(topic_id, input)
            .await
            .map_err(Into::into)
    }

    pub async fn delete(&self, topic_id: Uuid) -> Result<DeletedTopic, TopicApplicationError> {
        let topic = self.topic_repository.soft_delete_topic(topic_id).await?;
        let deleted_at = topic.deleted_at.ok_or_else(|| {
            TopicApplicationError::Validation("选题删除失败：缺少软删除时间".to_string())
        })?;
        Ok(DeletedTopic {
            topic_id: topic.id,
            deleted_at,
        })
    }

    pub async fn update_status(
        &self,
        topic_id: Uuid,
        status: ContentTopicStatus,
    ) -> Result<ContentTopic, TopicApplicationError> {
        if status == ContentTopicStatus::Scripted {
            return Err(TopicApplicationError::Validation(
                "选题只能在脚本生成成功后自动变为已成稿".to_string(),
            ));
        }
        self.topic_repository
            .update_topic_status(topic_id, status)
            .await
            .map_err(Into::into)
    }

    pub async fn prepare_script(
        &self,
        topic_id: Uuid,
        style: ScriptStyle,
        scene_count: u8,
    ) -> Result<PreparedTopicScript, TopicApplicationError> {
        let topic = self.topic_repository.get_topic(topic_id).await?;
        if topic.deleted_at.is_some() {
            return Err(TopicApplicationError::Validation(
                "已移除选题不能进入脚本生成确认流程".to_string(),
            ));
        }
        if topic.status != ContentTopicStatus::Approved {
            return Err(TopicApplicationError::Validation(
                "只有已确认选题可以进入脚本生成确认流程".to_string(),
            ));
        }
        let topic_snapshot = topic.snapshot();
        Ok(PreparedTopicScript {
            topic,
            topic_snapshot,
            style,
            scene_count,
        })
    }

    async fn ensure_project_exists(&self, project_id: Uuid) -> Result<(), TopicApplicationError> {
        if self.project_repository.project_exists(project_id).await? {
            Ok(())
        } else {
            Err(ScriptAgentError::ProjectNotFound(project_id).into())
        }
    }

    /// 补充批次不能作为新的主题组根；跨项目请求统一表现为批次不存在。
    async fn resolve_group_project_id(
        &self,
        root_batch_id: Uuid,
        requested_project_id: Option<Uuid>,
    ) -> Result<Uuid, TopicApplicationError> {
        let batch = self
            .topic_repository
            .get_generation_batch(root_batch_id)
            .await?;
        if batch.supplement_of_batch_id.is_some()
            || requested_project_id.is_some_and(|project_id| project_id != batch.project_id)
        {
            return Err(TopicRepositoryError::BatchNotFound(root_batch_id).into());
        }
        Ok(batch.project_id)
    }

    fn topic_adapter(&self) -> TopicAgentAdapter {
        TopicAgentAdapter::new(
            Arc::new(self.conversation_repository.clone()),
            Arc::new(self.project_repository.clone()),
            Arc::new(self.topic_repository.clone()),
        )
    }
}

pub struct TopicListResult {
    pub topics: Vec<ContentTopic>,
    pub stats: Vec<(ContentTopicStatus, i64)>,
}

pub struct DeletedTopic {
    pub topic_id: Uuid,
    pub deleted_at: chrono::DateTime<chrono::Utc>,
}

pub struct PreparedTopicScript {
    pub topic: ContentTopic,
    pub topic_snapshot: Value,
    pub style: ScriptStyle,
    pub scene_count: u8,
}

#[derive(Debug)]
pub enum TopicApplicationError {
    ProjectRepository(ProjectRepositoryError),
    TopicRepository(TopicRepositoryError),
    Agent(ScriptAgentError),
    Runtime(AgentRuntimeError),
    ModelResolve(ModelResolveError),
    Definition(String),
    ModelCapabilityMismatch,
    Serialization(String),
    Validation(String),
}

impl From<ProjectRepositoryError> for TopicApplicationError {
    fn from(error: ProjectRepositoryError) -> Self {
        Self::ProjectRepository(error)
    }
}

impl From<TopicRepositoryError> for TopicApplicationError {
    fn from(error: TopicRepositoryError) -> Self {
        Self::TopicRepository(error)
    }
}

impl From<ScriptAgentError> for TopicApplicationError {
    fn from(error: ScriptAgentError) -> Self {
        Self::Agent(error)
    }
}

impl From<AgentRuntimeError> for TopicApplicationError {
    fn from(error: AgentRuntimeError) -> Self {
        Self::Runtime(error)
    }
}

impl From<ModelResolveError> for TopicApplicationError {
    fn from(error: ModelResolveError) -> Self {
        Self::ModelResolve(error)
    }
}

impl fmt::Display for TopicApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectRepository(error) => write!(formatter, "{error}"),
            Self::TopicRepository(error) => write!(formatter, "{error}"),
            Self::Agent(error) => write!(formatter, "{error}"),
            Self::Runtime(error) => write!(formatter, "{error}"),
            Self::ModelResolve(error) => write!(formatter, "{error}"),
            Self::Definition(message) => write!(formatter, "{message}"),
            Self::ModelCapabilityMismatch => formatter.write_str("model capability mismatch"),
            Self::Serialization(message) => write!(formatter, "serialization error: {message}"),
            Self::Validation(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for TopicApplicationError {}
