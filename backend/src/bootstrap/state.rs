//! 保存进程级依赖并按业务用例组装 Application Service。

use super::AppConfig;
use crate::agents::LLMClient;
use crate::application::ai_models::AiModelService;
use crate::application::asset_generation::AssetGenerationService;
use crate::application::conversations::ConversationService;
use crate::application::health::HealthService;
use crate::application::materials::MaterialService;
use crate::application::projects::ProjectService;
use crate::application::publication::PublicationService;
use crate::application::scripts::ScriptService;
use crate::application::sound_subtitle::SoundSubtitleService;
use crate::application::topics::TopicService;
use crate::application::voice_catalog::VoiceCatalogService;
use crate::application::work_generation::WorkGenerationService;
use crate::application::work_library::WorkLibraryService;
use crate::application::workspace::WorkspaceService;
use crate::model_routing::{
    ModelClientResolver, PostgresModelClientResolver, StaticModelClientResolver,
};
use crate::repositories::{
    PostgresAiModelRepository, PostgresAssetGenerationRepository, PostgresContextAuditRepository,
    PostgresConversationRepository, PostgresMaterialRepository, PostgresModelCallRepository,
    PostgresProjectRepository, PostgresPublicationRepository, PostgresScriptRepository,
    PostgresSoundSubtitleRepository, PostgresTopicRepository, PostgresTosStagingToolRepository,
    PostgresVoiceCatalogRepository, PostgresWorkGenerationRepository,
    PostgresWorkLibraryRepository, PostgresWorkspaceMenuRepository,
};
use sqlx::PgPool;
use std::{fmt, sync::Arc};

/// Axum 的进程级依赖容器；该类型不保存任何请求级或业务流程状态。
#[derive(Clone)]
pub struct AppState {
    pub(crate) config: AppConfig,
    pub(crate) pg_pool: Option<PgPool>,
    pub(crate) redis_client: Option<redis::Client>,
    model_client_resolver: Option<Arc<dyn ModelClientResolver>>,
    agent_registry: Option<Arc<novex_agent::AgentRegistry>>,
    definition_registry: Option<Arc<novex_ai_core::DefinitionRegistry>>,
}

impl AppState {
    pub fn test() -> Self {
        Self {
            config: AppConfig::from_env(),
            pg_pool: None,
            redis_client: None,
            model_client_resolver: None,
            agent_registry: None,
            definition_registry: None,
        }
    }

    pub fn new(
        config: AppConfig,
        pg_pool: PgPool,
        redis_client: Option<redis::Client>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let definition_registry = Arc::new(novex_ai_core::DefinitionRegistry::load(
            config.agent_definitions_dir(),
        )?);
        let agent_registry = crate::application::agents::kernel::build_registry(&pg_pool)?;
        Ok(Self {
            config,
            pg_pool: Some(pg_pool),
            redis_client,
            model_client_resolver: None,
            agent_registry: Some(Arc::new(agent_registry)),
            definition_registry: Some(definition_registry),
        })
    }

    pub(crate) fn definition_registry(
        &self,
    ) -> Result<Arc<novex_ai_core::DefinitionRegistry>, AppStateError> {
        self.definition_registry
            .clone()
            .ok_or(AppStateError::MissingDependency("definition registry"))
    }

    pub(crate) fn model_call_repository(
        &self,
    ) -> Result<PostgresModelCallRepository, AppStateError> {
        Ok(PostgresModelCallRepository::new(self.database_pool()?))
    }

    pub(crate) fn context_audit_repository(
        &self,
    ) -> Result<PostgresContextAuditRepository, AppStateError> {
        Ok(PostgresContextAuditRepository::new(self.database_pool()?))
    }

    fn audited_model_executor(
        &self,
        pool: PgPool,
    ) -> Result<Arc<novex_agent::AuditedModelExecutor>, AppStateError> {
        let resolver = self.text_model_resolver(pool.clone());
        let bound_resolver: Arc<dyn novex_agent::BoundModelResolver> = resolver;
        Ok(Arc::new(novex_agent::AuditedModelExecutor::new(
            self.definition_registry()?,
            bound_resolver,
            Arc::new(PostgresModelCallRepository::new(pool)),
            Arc::new(PostgresContextAuditRepository::new(self.database_pool()?)),
        )))
    }

    pub fn with_llm_client(mut self, llm_client: Arc<dyn LLMClient>) -> Self {
        self.model_client_resolver = Some(Arc::new(StaticModelClientResolver::new(llm_client)));
        self
    }

    pub fn with_model_client_resolver(mut self, resolver: Arc<dyn ModelClientResolver>) -> Self {
        self.model_client_resolver = Some(resolver);
        self
    }

    pub(crate) fn script_service(&self) -> Result<ScriptService, AppStateError> {
        let pool = self.database_pool()?;
        Ok(ScriptService::new(
            PostgresScriptRepository::new(pool.clone()),
            PostgresProjectRepository::new(pool.clone()),
            PostgresTopicRepository::new(pool.clone()),
            PostgresConversationRepository::new(pool.clone()),
            self.text_model_resolver(pool.clone()),
            self.definition_registry()?,
            self.audited_model_executor(pool)?,
        ))
    }

    pub(crate) fn project_service(&self) -> Result<ProjectService, AppStateError> {
        let pool = self.database_pool()?;
        Ok(ProjectService::new(
            PostgresProjectRepository::new(pool.clone()),
            PostgresConversationRepository::new(pool.clone()),
            self.text_model_resolver(pool.clone()),
            self.definition_registry()?,
            self.audited_model_executor(pool)?,
        ))
    }

    pub(crate) fn conversation_service(&self) -> Result<ConversationService, AppStateError> {
        let pool = self.database_pool()?;
        Ok(ConversationService::new(
            PostgresConversationRepository::new(pool.clone()),
            PostgresScriptRepository::new(pool.clone()),
            PostgresProjectRepository::new(pool.clone()),
            PostgresAiModelRepository::new(pool.clone()),
            PostgresVoiceCatalogRepository::new(pool.clone()),
            PostgresWorkLibraryRepository::new(pool.clone()),
            self.text_model_resolver(pool.clone()),
            self.agent_registry
                .clone()
                .ok_or(AppStateError::MissingDependency("agent registry"))?,
            self.definition_registry()?,
            self.audited_model_executor(pool)?,
        ))
    }

    pub(crate) fn topic_service(&self) -> Result<TopicService, AppStateError> {
        let pool = self.database_pool()?;
        Ok(TopicService::new(
            PostgresProjectRepository::new(pool.clone()),
            PostgresTopicRepository::new(pool.clone()),
            PostgresConversationRepository::new(pool.clone()),
            self.text_model_resolver(pool.clone()),
            self.definition_registry()?,
            self.audited_model_executor(pool)?,
        ))
    }

    pub(crate) fn material_service(&self) -> Result<MaterialService, AppStateError> {
        let pool = self.database_pool()?;
        Ok(MaterialService::new(
            PostgresProjectRepository::new(pool.clone()),
            PostgresMaterialRepository::new(pool),
            self.config.asset_storage_root.clone(),
        ))
    }

    pub(crate) fn asset_generation_service(&self) -> Result<AssetGenerationService, AppStateError> {
        let pool = self.database_pool()?;
        Ok(AssetGenerationService::new(
            pool.clone(),
            PostgresAiModelRepository::new(pool.clone()),
            PostgresAssetGenerationRepository::new(pool.clone()),
            PostgresMaterialRepository::new(pool.clone()),
            PostgresScriptRepository::new(pool.clone()),
        ))
    }

    pub(crate) fn workspace_service(&self) -> Result<WorkspaceService, AppStateError> {
        Ok(WorkspaceService::new(PostgresWorkspaceMenuRepository::new(
            self.database_pool()?,
        )))
    }

    pub(crate) fn health_service(&self) -> HealthService {
        HealthService::new(
            self.config.environment.clone(),
            self.pg_pool.clone(),
            self.redis_client.clone(),
        )
    }

    fn ai_model_repository(&self) -> Result<PostgresAiModelRepository, AppStateError> {
        Ok(PostgresAiModelRepository::new(self.database_pool()?))
    }

    pub(crate) fn ai_model_service(&self) -> Result<AiModelService, AppStateError> {
        Ok(AiModelService::new(
            self.ai_model_repository()?,
            self.definition_registry()?,
        ))
    }

    pub(crate) fn voice_catalog_service(&self) -> Result<VoiceCatalogService, AppStateError> {
        Ok(VoiceCatalogService::new(
            PostgresVoiceCatalogRepository::new(self.database_pool()?),
        ))
    }

    pub(crate) fn tos_staging_tool_repository(
        &self,
    ) -> Result<PostgresTosStagingToolRepository, AppStateError> {
        Ok(PostgresTosStagingToolRepository::new(self.database_pool()?))
    }

    pub(crate) fn sound_subtitle_service(&self) -> Result<SoundSubtitleService, AppStateError> {
        let pool = self.database_pool()?;
        Ok(SoundSubtitleService::new(
            PostgresAiModelRepository::new(pool.clone()),
            PostgresMaterialRepository::new(pool.clone()),
            PostgresVoiceCatalogRepository::new(pool.clone()),
            PostgresTosStagingToolRepository::new(pool.clone()),
            PostgresScriptRepository::new(pool.clone()),
            PostgresSoundSubtitleRepository::new(pool),
        ))
    }

    pub(crate) fn work_generation_service(&self) -> Result<WorkGenerationService, AppStateError> {
        let pool = self.database_pool()?;
        let asset_service = self.asset_generation_service()?;
        Ok(WorkGenerationService::new(
            PostgresWorkGenerationRepository::new(pool.clone()),
            PostgresAiModelRepository::new(pool.clone()),
            PostgresVoiceCatalogRepository::new(pool),
            asset_service,
        ))
    }

    pub(crate) fn work_library_service(&self) -> Result<WorkLibraryService, AppStateError> {
        Ok(WorkLibraryService::new(
            PostgresWorkLibraryRepository::new(self.database_pool()?),
            self.config.asset_storage_root.clone().into(),
        ))
    }

    pub(crate) fn publication_service(&self) -> Result<PublicationService, AppStateError> {
        let pool = self.database_pool()?;
        Ok(PublicationService::new(
            PostgresPublicationRepository::new(pool.clone()),
            WorkLibraryService::new(
                PostgresWorkLibraryRepository::new(pool),
                self.config.asset_storage_root.clone().into(),
            ),
            self.config.asset_storage_root.clone().into(),
        ))
    }

    fn text_model_resolver(&self, pool: PgPool) -> Arc<dyn ModelClientResolver> {
        self.model_client_resolver.clone().unwrap_or_else(|| {
            Arc::new(PostgresModelClientResolver::new(
                PostgresAiModelRepository::new(pool),
                self.definition_registry
                    .clone()
                    .expect("production state has definition registry"),
            ))
        })
    }

    fn database_pool(&self) -> Result<PgPool, AppStateError> {
        self.pg_pool
            .clone()
            .ok_or(AppStateError::DatabasePoolUnavailable)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppStateError {
    DatabasePoolUnavailable,
    MissingDependency(&'static str),
}

impl fmt::Display for AppStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DatabasePoolUnavailable => formatter.write_str("database pool is not configured"),
            Self::MissingDependency(name) => {
                write!(formatter, "missing required dependency: {name}")
            }
        }
    }
}

impl std::error::Error for AppStateError {}
