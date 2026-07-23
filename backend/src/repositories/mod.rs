pub mod ai_model_repository;
pub mod asset_generation_repository;
pub mod conversation_repository;
pub mod material_repository;
pub mod project_repository;
pub mod publication_repository;
pub mod script_repository;
pub mod sound_subtitle_repository;
pub mod topic_repository;
pub mod tos_staging_tool_repository;
pub mod voice_catalog_repository;
pub mod work_generation_repository;
pub mod work_library_repository;
pub mod workspace_menu_repository;

pub use ai_model_repository::{
    AiModel, AiModelListFilter, AiModelRepository, AiModelRepositoryError, AiModelStatus,
    ChangeAiModelStatusInput, CreateAiModelInput, DeleteAiModelInput, DeleteAiModelOutcome,
    PostgresAiModelRepository, UpdateAiModelInput,
};
pub use asset_generation_repository::{
    AssetCandidateSource, AssetCandidateStatus, AssetCandidateType, AssetGenerationParseError,
    AssetGenerationProvider, AssetGenerationRepository, AssetGenerationRepositoryError,
    AssetGenerationTask, AssetGenerationTaskStatus, AssetGenerationTaskType,
    CreateAssetCandidateInput, CreateAssetGenerationTaskInput, PostgresAssetGenerationRepository,
    SceneAssetCandidate,
};
pub use conversation_repository::{
    ConversationRepository, ConversationRepositoryError, PostgresConversationRepository,
};
pub use material_repository::{
    redact_sensitive_material_metadata, validate_material_metadata, AudioUsage,
    CreateMaterialInput, Material, MaterialListFilter, MaterialParseError, MaterialRepository,
    MaterialRepositoryError, MaterialSourceFilter, MaterialStatus, MaterialStatusFilter,
    MaterialType, PostgresMaterialRepository, UpdateMaterialInput,
};
pub use project_repository::{
    AccountStrategyProfile, CreateProjectInput, PostgresProjectRepository, Project,
    ProjectRepository, ProjectRepositoryError, UpdateProjectStrategyProfileInput,
};
pub use publication_repository::{
    PostgresPublicationRepository, PublicationPackageContext, PublicationPackageRecord,
    PublicationPlanRecord, PublicationRepositoryError, PublicationTargetRecord,
    SavePublicationTarget,
};
pub use script_repository::{PostgresScriptRepository, ScriptRepository, ScriptRepositoryError};
pub use sound_subtitle_repository::{
    AudioMaterialInspection, CreateSoundSubtitleTaskInput, PostgresSoundSubtitleRepository,
    SoundSubtitleRepositoryError, SoundSubtitleTask, MAX_IN_FLIGHT_SOUND_TASKS_PER_PROJECT,
};
pub use topic_repository::{
    CreateContentTopicInput, CreateTopicGenerationBatchInput, CreateTopicQualityEvaluationInput,
    CreateTopicReviewSnapshotInput, PostgresTopicRepository, TopicRepository, TopicRepositoryError,
    UpdateContentTopicInput, UpdateTopicGenerationBatchInput,
};
pub use tos_staging_tool_repository::{
    PostgresTosStagingToolRepository, SaveTosStagingToolConfigInput, TosStagingToolConfig,
    TosStagingToolRepositoryError,
};
pub use voice_catalog_repository::{
    PostgresVoiceCatalogRepository, VoiceCatalogEntry, VoiceCatalogRepositoryError,
    VoiceCatalogSnapshot, VoiceCatalogSync,
};
pub use work_generation_repository::{
    PostgresWorkGenerationRepository, WorkGenerationAttemptRecord, WorkGenerationRepository,
    WorkGenerationRunRecord, WorkGenerationStepRecord, WorkGenerationTaskCounts,
    WorkGenerationTaskDetails, WorkGenerationTaskFilter, WorkGenerationTaskRecord, WorkPlanRecord,
    WorkRecord, WorkRepositoryError,
};
pub use work_library_repository::{
    PostgresWorkLibraryRepository, WorkArtifactRecord, WorkDiffConfirmation,
    WorkLibraryRepositoryError, WorkPublicationHandoff, WorkVersionDiffPlanRecord,
    WorkVersionRecord,
};
pub use workspace_menu_repository::{
    PostgresWorkspaceMenuRepository, WorkspaceMenu, WorkspaceMenuRepositoryError,
    WorkspaceMenuTreeNode,
};
