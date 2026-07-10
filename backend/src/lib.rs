use agents::{LLMClient, ScriptAgentError, ScriptAgentService, ScriptGenerationMode};
use axum::{
    extract::{rejection::JsonRejection, FromRequest, Path, Query, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use novex_model::{LLMError, LLMJsonSchema, LLMPrompt, OpenAIClient, OpenAIConfig};
use repositories::{
    AssetCandidateSource, AssetCandidateType, AssetGenerationRepository,
    AssetGenerationRepositoryError, AssetGenerationTaskStatus, AssetGenerationTaskType,
    ConversationRepository, ConversationRepositoryError, CreateAssetCandidateInput,
    CreateAssetGenerationTaskInput, CreateContentTopicInput, CreateProjectInput,
    MaterialListFilter, MaterialRepository, MaterialRepositoryError, MaterialStatusFilter,
    MaterialType, PostgresAssetGenerationRepository, PostgresConversationRepository,
    PostgresMaterialRepository, PostgresProjectRepository, PostgresScriptRepository,
    PostgresTopicRepository, PostgresWorkspaceMenuRepository, ProjectRepository,
    ProjectRepositoryError, ScriptRepositoryError, TopicRepository, TopicRepositoryError,
    UpdateContentTopicInput, UpdateProjectStrategyProfileInput, WorkspaceMenuRepositoryError,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use uuid::Uuid;

use crate::agents::conversation::CreateAgentConversationInput;
use crate::agents::conversational_runtime::{AgentRuntime, AgentRuntimeError, AgentTurnRequest};
use crate::agents::models::{
    AccountStrategyProfileRequest, AgentConversationResponse, AgentMessageListResponse,
    AgentMessageResponse, AgentRunResponse, AgentTurnResponseBody, AssetGenerationPlanRequest,
    AssetGenerationPlanResponse, AssetGenerationTaskListResponse, AssetGenerationTaskRequest,
    AssetGenerationTaskResponse, ContentTopicFilter, ContentTopicListResponse,
    ContentTopicResponse, ContentTopicStatsResponse, ContentTopicStatus,
    CreateAgentConversationRequest, CreateContentTopicRequest, CreateProjectRequest,
    GenerateScriptRequest, MaterialListQuery, MaterialListResponse, MaterialPayloadRequest,
    MaterialResponse, MaterialStatusRequest, PrepareScriptFromTopicRequest,
    PrepareScriptFromTopicResponse, ProjectListResponse, ProjectResponse,
    SceneAssetCandidateListResponse, SceneAssetCandidateResponse, ScriptListFilter,
    ScriptListResponse, ScriptResponse, SendAgentMessageRequest, StrategyProfileDraftRequest,
    StrategyProfileDraftResponse, TopicGenerationBatchListResponse,
    TopicGenerationBatchSummaryResponse, TopicGroupListQuery, TopicGroupListResponse,
    TopicGroupSummaryResponse, TopicQualityEvaluationResponse, TopicReviewSnapshotResponse,
    TopicScriptRequestPreview, UpdateContentTopicRequest, UpdateContentTopicStatusRequest,
    UpdateProjectStrategyProfileRequest, UpdateScriptStatusRequest, UpdateScriptStatusResponse,
    WorkspaceMenuListResponse, WorkspaceMenuNodeResponse,
};

pub mod agents;
pub mod repositories;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub environment: String,
    pub database_url: String,
    pub redis_url: String,
    pub openai_api_key: String,
    pub openai_base_url: String,
    pub openai_model: String,
    pub openai_timeout_seconds: u64,
    pub openai_reasoning_effort: Option<String>,
    pub openai_max_output_tokens: u32,
    pub asset_storage_root: String,
    pub asset_generation_providers: Vec<String>,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            environment: std::env::var("NOVEX_ENV")
                .or_else(|_| std::env::var("AI_AGENT_ENV"))
                .unwrap_or_else(|_| "development".to_string()),
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://postgres:postgres@biga-postgres:5432/video_agent".to_string()
            }),
            redis_url: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://bs-redis:6379/2".to_string()),
            openai_api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            openai_base_url: std::env::var("OPENAI_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
            openai_model: std::env::var("OPENAI_MODEL")
                .unwrap_or_else(|_| "gpt-4-turbo".to_string()),
            openai_timeout_seconds: std::env::var("OPENAI_TIMEOUT_SECONDS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(30),
            openai_reasoning_effort: std::env::var("OPENAI_REASONING_EFFORT")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("none"))
                .or_else(|| Some("low".to_string())),
            openai_max_output_tokens: std::env::var("OPENAI_MAX_OUTPUT_TOKENS")
                .ok()
                .and_then(|value| value.parse().ok())
                .filter(|value| *value > 0)
                .unwrap_or(3000),
            asset_storage_root: std::env::var("ASSET_STORAGE_ROOT")
                .unwrap_or_else(|_| "/app/storage/assets".to_string()),
            asset_generation_providers: parse_asset_generation_providers(
                std::env::var("ASSET_GENERATION_PROVIDERS")
                    .unwrap_or_else(|_| "gpt-image-2,jimeng".to_string())
                    .as_str(),
            ),
        }
    }
}

fn parse_asset_generation_providers(value: &str) -> Vec<String> {
    let providers: Vec<String> = value
        .split(',')
        .map(str::trim)
        .filter(|provider| !provider.is_empty())
        .filter(|provider| repositories::AssetGenerationProvider::try_from(*provider).is_ok())
        .map(ToString::to_string)
        .collect();

    if providers.is_empty() {
        vec!["gpt-image-2".to_string(), "jimeng".to_string()]
    } else {
        providers
    }
}

#[derive(Clone)]
pub struct AppState {
    config: AppConfig,
    pg_pool: Option<PgPool>,
    redis_client: Option<redis::Client>,
    llm_client: Option<Arc<dyn LLMClient>>,
}

impl AppState {
    pub fn test() -> Self {
        Self {
            config: AppConfig::from_env(),
            pg_pool: None,
            redis_client: None,
            llm_client: None,
        }
    }

    pub fn new(
        config: AppConfig,
        pg_pool: PgPool,
        redis_client: Option<redis::Client>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self {
            config,
            pg_pool: Some(pg_pool),
            redis_client,
            llm_client: None,
        })
    }

    pub fn with_llm_client(mut self, llm_client: Arc<dyn LLMClient>) -> Self {
        self.llm_client = Some(llm_client);
        self
    }

    fn script_agent_service(&self) -> Result<ScriptAgentService, ScriptApiError> {
        let pool = self
            .pg_pool
            .clone()
            .ok_or_else(|| ScriptApiError::State("database pool is not configured".to_string()))?;
        let llm_client = self.openai_client()?;
        let script_repository = Arc::new(PostgresScriptRepository::new(pool.clone()));
        let project_repository = Arc::new(PostgresProjectRepository::new(pool.clone()));
        let topic_repository = Arc::new(PostgresTopicRepository::new(pool));

        let service = ScriptAgentService::new(llm_client, script_repository, project_repository)
            .with_generation_mode(self.script_generation_mode())
            .with_topic_repository(topic_repository);

        Ok(service)
    }

    fn project_repository(&self) -> Result<PostgresProjectRepository, ScriptApiError> {
        let pool = self
            .pg_pool
            .clone()
            .ok_or_else(|| ScriptApiError::State("database pool is not configured".to_string()))?;

        Ok(PostgresProjectRepository::new(pool))
    }

    fn conversation_repository(&self) -> Result<PostgresConversationRepository, ScriptApiError> {
        let pool = self
            .pg_pool
            .clone()
            .ok_or_else(|| ScriptApiError::State("database pool is not configured".to_string()))?;

        Ok(PostgresConversationRepository::new(pool))
    }

    fn topic_repository(&self) -> Result<PostgresTopicRepository, ScriptApiError> {
        let pool = self
            .pg_pool
            .clone()
            .ok_or_else(|| ScriptApiError::State("database pool is not configured".to_string()))?;

        Ok(PostgresTopicRepository::new(pool))
    }

    fn material_repository(&self) -> Result<PostgresMaterialRepository, ScriptApiError> {
        let pool = self
            .pg_pool
            .clone()
            .ok_or_else(|| ScriptApiError::State("database pool is not configured".to_string()))?;

        Ok(PostgresMaterialRepository::new(pool))
    }

    fn asset_generation_repository(
        &self,
    ) -> Result<PostgresAssetGenerationRepository, ScriptApiError> {
        let pool = self
            .pg_pool
            .clone()
            .ok_or_else(|| ScriptApiError::State("database pool is not configured".to_string()))?;

        Ok(PostgresAssetGenerationRepository::new(pool))
    }

    fn agent_runtime(&self) -> Result<AgentRuntime, ScriptApiError> {
        let pool = self
            .pg_pool
            .clone()
            .ok_or_else(|| ScriptApiError::State("database pool is not configured".to_string()))?;
        let conversation_repository = Arc::new(PostgresConversationRepository::new(pool.clone()));
        let script_repository = Arc::new(PostgresScriptRepository::new(pool.clone()));
        let project_repository = Arc::new(PostgresProjectRepository::new(pool.clone()));
        let topic_repository = Arc::new(PostgresTopicRepository::new(pool));
        let llm_client = self.openai_client()?;

        Ok(AgentRuntime::new(
            conversation_repository,
            script_repository,
            project_repository,
            llm_client,
        )
        .with_topic_repository(topic_repository))
    }

    fn workspace_menu_repository(&self) -> Result<PostgresWorkspaceMenuRepository, ScriptApiError> {
        let pool = self
            .pg_pool
            .clone()
            .ok_or_else(|| ScriptApiError::State("database pool is not configured".to_string()))?;

        Ok(PostgresWorkspaceMenuRepository::new(pool))
    }

    fn script_agent_service_without_llm(&self) -> Result<ScriptAgentService, ScriptApiError> {
        let pool = self
            .pg_pool
            .clone()
            .ok_or_else(|| ScriptApiError::State("database pool is not configured".to_string()))?;
        let script_repository = Arc::new(PostgresScriptRepository::new(pool.clone()));
        let project_repository = Arc::new(PostgresProjectRepository::new(pool));

        Ok(ScriptAgentService::new(
            Arc::new(UnconfiguredLLMClient),
            script_repository,
            project_repository,
        ))
    }

    fn openai_client(&self) -> Result<Arc<dyn LLMClient>, ScriptApiError> {
        if let Some(llm_client) = &self.llm_client {
            return Ok(llm_client.clone());
        }
        Ok(Arc::new(LazyOpenAIClient {
            config: OpenAIConfig {
                api_key: self.config.openai_api_key.clone(),
                base_url: self.config.openai_base_url.clone(),
                model: self.config.openai_model.clone(),
                timeout_seconds: self.config.openai_timeout_seconds,
                responses_reasoning_effort: self.config.openai_reasoning_effort.clone(),
                responses_max_output_tokens: self.config.openai_max_output_tokens,
            },
        }))
    }

    fn script_generation_mode(&self) -> ScriptGenerationMode {
        match self.config.openai_reasoning_effort.as_deref() {
            Some(effort) if effort.eq_ignore_ascii_case("xhigh") => {
                ScriptGenerationMode::StepwiseSingleScene
            }
            _ => ScriptGenerationMode::Complete,
        }
    }
}

struct LazyOpenAIClient {
    config: OpenAIConfig,
}

#[async_trait::async_trait]
impl LLMClient for LazyOpenAIClient {
    async fn generate_script(&self, prompt: LLMPrompt) -> Result<String, LLMError> {
        let client = OpenAIClient::new(self.config.clone())?;
        client.generate_script(prompt).await
    }
}

struct UnconfiguredLLMClient;

#[async_trait::async_trait]
impl LLMClient for UnconfiguredLLMClient {
    async fn generate_script(&self, _prompt: LLMPrompt) -> Result<String, LLMError> {
        Err(LLMError::Config(
            "LLM client is not configured for this route".to_string(),
        ))
    }
}

#[derive(Serialize)]
struct HealthResponse {
    service: &'static str,
    status: &'static str,
    environment: String,
}

#[derive(Serialize)]
struct ReadyResponse {
    service: &'static str,
    status: &'static str,
    postgres: &'static str,
    redis: &'static str,
}

#[derive(Serialize)]
struct DeletedContentTopicResponse {
    topic_id: Uuid,
    deleted_at: DateTime<Utc>,
}

#[derive(Debug, Default, Deserialize)]
struct TopicGroupProjectQuery {
    #[serde(default)]
    project_id: Option<Uuid>,
}

pub fn build_app() -> Router {
    build_app_with_state(AppState::test())
}

pub fn build_app_with_state(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin("*".parse::<HeaderValue>().unwrap())
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::ACCEPT,
            header::CONTENT_TYPE,
            HeaderName::from_static("idempotency-key"),
        ]);

    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/api/video-workspace/menus", get(list_workspace_menus))
        .route("/api/projects", get(list_projects).post(create_project))
        .route(
            "/api/projects/:project_id/strategy-profile",
            put(update_project_strategy_profile),
        )
        .route(
            "/api/projects/:project_id/strategy-profile/draft",
            post(generate_project_strategy_profile_draft),
        )
        .route(
            "/api/projects/:project_id/topics",
            get(list_topics).post(create_topic),
        )
        .route(
            "/api/projects/:project_id/materials",
            get(list_materials).post(create_material),
        )
        .route(
            "/api/materials/:material_id",
            get(get_material).put(update_material),
        )
        .route(
            "/api/materials/:material_id/status",
            put(update_material_status),
        )
        .route(
            "/api/projects/:project_id/topic-generation-batches",
            get(list_topic_generation_batches),
        )
        .route(
            "/api/projects/:project_id/topic-groups",
            get(list_topic_groups),
        )
        .route(
            "/api/topic-groups/:root_batch_id/reviews",
            post(create_topic_group_review),
        )
        .route(
            "/api/topic-groups/:root_batch_id/reviews/latest",
            get(get_latest_topic_group_review),
        )
        .route(
            "/api/topic-generation-batches/:batch_id/quality-evaluation",
            get(get_latest_topic_quality_evaluation),
        )
        .route(
            "/api/topics/:topic_id",
            put(update_topic).delete(delete_topic),
        )
        .route("/api/topics/:topic_id/status", put(update_topic_status))
        .route(
            "/api/topics/:topic_id/prepare-script",
            post(prepare_script_from_topic),
        )
        .route("/api/agent/conversations", post(create_agent_conversation))
        .route(
            "/api/agent/conversations/:conversation_id/messages",
            get(list_agent_messages).post(send_agent_message),
        )
        .route("/api/scripts/generate", post(generate_script))
        .route("/api/scripts/:script_id", get(get_script))
        .route("/api/projects/:project_id/scripts", get(list_scripts))
        .route("/api/scripts/:script_id/status", put(update_script_status))
        .route(
            "/api/scripts/:script_id/asset-generation-plan",
            post(create_asset_generation_plan),
        )
        .route(
            "/api/scripts/:script_id/asset-generation-tasks",
            get(list_asset_generation_tasks).post(create_asset_generation_tasks),
        )
        .route(
            "/api/scripts/:script_id/asset-candidates",
            get(list_asset_candidates),
        )
        .route(
            "/api/scenes/:scene_id/asset-candidates/:candidate_id/select",
            put(select_asset_candidate),
        )
        .route(
            "/api/scenes/:scene_id/asset-candidates/:candidate_id/reject",
            put(reject_asset_candidate),
        )
        .route(
            "/api/scenes/:scene_id/asset-generation-tasks",
            post(create_scene_asset_generation_task),
        )
        .route(
            "/api/asset-generation-tasks/:task_id/confirm",
            post(confirm_asset_generation_task),
        )
        .route(
            "/api/asset-generation-tasks/:task_id/dismiss",
            post(dismiss_asset_generation_task),
        )
        .nest_service(
            "/assets",
            ServeDir::new(state.config.asset_storage_root.clone()),
        )
        .layer(cors)
        .with_state(state)
}

pub async fn build_runtime_state() -> Result<AppState, Box<dyn std::error::Error + Send + Sync>> {
    let config = AppConfig::from_env();
    let pg_pool = connect_runtime_pg_pool(&config.database_url, 5).await?;
    let redis_client = redis::Client::open(config.redis_url.clone())?;

    AppState::new(config, pg_pool, Some(redis_client))
}

pub async fn connect_runtime_pg_pool(
    database_url: &str,
    max_connections: u32,
) -> Result<PgPool, Box<dyn std::error::Error + Send + Sync>> {
    let pg_pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pg_pool).await?;
    sync_content_strategy_menu_state(&pg_pool).await?;

    Ok(pg_pool)
}

async fn sync_content_strategy_menu_state(pool: &PgPool) -> Result<(), sqlx::Error> {
    let menu_table_exists = sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('public.video_workspace_menus') IS NOT NULL",
    )
    .fetch_one(pool)
    .await?;

    if !menu_table_exists {
        return Ok(());
    }

    sqlx::query(
        r#"
        UPDATE video_workspace_menus
        SET
            is_enabled = true,
            status = 'active',
            metadata = jsonb_set(COALESCE(metadata, '{}'::jsonb), '{phase}', '2'::jsonb, true),
            updated_at = NOW()
        WHERE menu_key IN ('content-strategy', 'account-strategy', 'topic-history', 'topic-generator')
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        service: "novex-api",
        status: "ok",
        environment: state.config.environment,
    })
}

async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    let postgres_ok = match state.pg_pool {
        Some(pool) => sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&pool)
            .await
            .is_ok(),
        None => false,
    };

    let redis_ok = match state.redis_client {
        Some(client) => match client.get_multiplexed_async_connection().await {
            Ok(mut connection) => redis::cmd("PING")
                .query_async::<String>(&mut connection)
                .await
                .map(|value| value == "PONG")
                .unwrap_or(false),
            Err(_) => false,
        },
        None => false,
    };

    let status_code = if postgres_ok && redis_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let body = ReadyResponse {
        service: "novex-api",
        status: if postgres_ok && redis_ok {
            "ready"
        } else {
            "not_ready"
        },
        postgres: if postgres_ok { "ok" } else { "error" },
        redis: if redis_ok { "ok" } else { "error" },
    };

    (status_code, Json(body))
}

async fn create_project(
    State(state): State<AppState>,
    ValidJson(request): ValidJson<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectResponse>), ScriptApiError> {
    request
        .validate_for_api()
        .map_err(ScriptApiError::ProjectValidation)?;
    let repository = state.project_repository()?;
    let project = repository
        .create_project(CreateProjectInput {
            name: request.name.trim().to_string(),
            positioning: request.positioning.trim().to_string(),
            description: request.description.trim().to_string(),
            strategy_profile: request
                .strategy_profile
                .as_ref()
                .map(|profile| profile.normalize())
                .transpose()
                .map_err(ScriptApiError::ProjectValidation)?
                .unwrap_or_default(),
        })
        .await?;

    Ok((StatusCode::CREATED, Json(ProjectResponse::from(project))))
}

async fn list_projects(
    State(state): State<AppState>,
) -> Result<Json<ProjectListResponse>, ScriptApiError> {
    let repository = state.project_repository()?;
    let projects = repository.list_projects().await?;

    Ok(Json(ProjectListResponse {
        projects: projects.into_iter().map(ProjectResponse::from).collect(),
    }))
}

async fn update_project_strategy_profile(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    ValidJson(request): ValidJson<UpdateProjectStrategyProfileRequest>,
) -> Result<Json<ProjectResponse>, ScriptApiError> {
    request
        .validate_for_api()
        .map_err(ScriptApiError::ProjectValidation)?;
    let strategy_profile = request
        .strategy_profile
        .normalize()
        .map_err(ScriptApiError::ProjectValidation)?;
    let repository = state.project_repository()?;
    let project = repository
        .update_strategy_profile(
            project_id,
            UpdateProjectStrategyProfileInput {
                name: request.name.trim().to_string(),
                positioning: request.positioning.trim().to_string(),
                description: request.description.trim().to_string(),
                strategy_profile,
            },
        )
        .await?;

    Ok(Json(ProjectResponse::from(project)))
}

async fn generate_project_strategy_profile_draft(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    ValidJson(request): ValidJson<StrategyProfileDraftRequest>,
) -> Result<Json<StrategyProfileDraftResponse>, ScriptApiError> {
    request
        .validate_for_api()
        .map_err(ScriptApiError::ProjectValidation)?;
    let repository = state.project_repository()?;
    let project = repository.get_project(project_id).await?;
    let llm_client = state.openai_client()?;
    let raw = generate_strategy_profile_draft_with_retry(
        llm_client.as_ref(),
        build_strategy_profile_draft_prompt(&project, &request.direction_notes),
    )
    .await
    .map_err(ScriptApiError::StrategyDraftLlm)?;
    let output = StrategyProfileDraftLlmOutput::parse_and_validate(&raw)
        .map_err(ScriptApiError::StrategyDraftOutput)?;

    Ok(Json(StrategyProfileDraftResponse {
        draft: output.draft,
        draft_summary: output.draft_summary,
    }))
}

async fn generate_strategy_profile_draft_with_retry(
    llm_client: &dyn LLMClient,
    prompt: LLMPrompt,
) -> Result<String, LLMError> {
    let first_result = llm_client.generate_script(prompt.clone()).await;
    match first_result {
        Ok(raw) => Ok(raw),
        Err(error) if is_retryable_strategy_draft_error(&error) => {
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

fn build_strategy_profile_draft_prompt(
    project: &repositories::Project,
    direction_notes: &str,
) -> LLMPrompt {
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
    draft: AccountStrategyProfileRequest,
    draft_summary: String,
}

impl StrategyProfileDraftLlmOutput {
    fn parse_and_validate(raw: &str) -> Result<StrategyProfileDraftResponse, String> {
        let json_text = extract_json_object(raw)?;
        let output: Self = serde_json::from_str(json_text).map_err(|error| error.to_string())?;
        let draft = output.draft.normalize()?;
        if account_strategy_profile_is_empty(&draft) {
            return Err("draft must not be empty".to_string());
        }
        let draft_summary = output.draft_summary.trim().to_string();
        if draft_summary.is_empty() {
            return Err("draft_summary must not be empty".to_string());
        }
        Ok(StrategyProfileDraftResponse {
            draft,
            draft_summary,
        })
    }
}

fn account_strategy_profile_is_empty(profile: &repositories::AccountStrategyProfile) -> bool {
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

async fn list_workspace_menus(
    State(state): State<AppState>,
) -> Result<Json<WorkspaceMenuListResponse>, ScriptApiError> {
    let repository = state.workspace_menu_repository()?;
    let menus = repository.list_visible_menu_tree().await?;

    Ok(Json(WorkspaceMenuListResponse {
        menus: menus
            .into_iter()
            .map(WorkspaceMenuNodeResponse::from)
            .collect(),
    }))
}

async fn create_topic(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    ValidJson(request): ValidJson<CreateContentTopicRequest>,
) -> Result<(StatusCode, Json<ContentTopicResponse>), ScriptApiError> {
    request
        .validate_for_api()
        .map_err(ScriptApiError::TopicValidation)?;
    ensure_project_exists(&state, project_id).await?;

    let repository = state.topic_repository()?;
    let topic = repository
        .create_topic(CreateContentTopicInput {
            project_id,
            batch_id: None,
            title: request.title.trim().to_string(),
            angle: request.angle.trim().to_string(),
            target_audience: request.target_audience.trim().to_string(),
            hook_points: trim_string_list(request.hook_points),
            content_type: request.content_type.trim().to_string(),
            score: request.score,
            score_reason: request.score_reason.trim().to_string(),
            tags: trim_string_list(request.tags),
            source: crate::agents::models::ContentTopicSource::Manual,
            metadata: json!({}),
        })
        .await?;

    Ok((StatusCode::CREATED, Json(ContentTopicResponse::from(topic))))
}

async fn list_topics(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(filter): Query<ContentTopicFilter>,
) -> Result<Json<ContentTopicListResponse>, ScriptApiError> {
    ensure_project_exists(&state, project_id).await?;
    let repository = state.topic_repository()?;
    let topics = repository.list_topics(project_id, filter).await?;
    let stats = repository.count_topics_by_status(project_id).await?;

    Ok(Json(ContentTopicListResponse {
        topics: topics.into_iter().map(ContentTopicResponse::from).collect(),
        stats: ContentTopicStatsResponse::from_counts(stats),
    }))
}

async fn create_material(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    ValidJson(request): ValidJson<MaterialPayloadRequest>,
) -> Result<(StatusCode, Json<MaterialResponse>), ScriptApiError> {
    ensure_project_exists(&state, project_id).await?;
    let input = request
        .into_create_input(project_id)
        .map_err(ScriptApiError::MaterialValidation)?;
    let material = state.material_repository()?.create_material(input).await?;

    Ok((StatusCode::CREATED, Json(MaterialResponse::from(material))))
}

async fn list_materials(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<MaterialListQuery>,
) -> Result<Json<MaterialListResponse>, ScriptApiError> {
    ensure_project_exists(&state, project_id).await?;
    let filter = query
        .into_filter()
        .map_err(ScriptApiError::MaterialValidation)?;
    let materials = state
        .material_repository()?
        .list_materials(project_id, filter)
        .await?;

    Ok(Json(MaterialListResponse {
        materials: materials.into_iter().map(MaterialResponse::from).collect(),
    }))
}

async fn get_material(
    State(state): State<AppState>,
    Path(material_id): Path<Uuid>,
) -> Result<Json<MaterialResponse>, ScriptApiError> {
    let material = state
        .material_repository()?
        .get_material(material_id)
        .await?;

    Ok(Json(MaterialResponse::from(material)))
}

async fn update_material(
    State(state): State<AppState>,
    Path(material_id): Path<Uuid>,
    ValidJson(request): ValidJson<MaterialPayloadRequest>,
) -> Result<Json<MaterialResponse>, ScriptApiError> {
    let repository = state.material_repository()?;
    let current = repository.get_material(material_id).await?;
    let input = request
        .into_update_input(current.project_id)
        .map_err(ScriptApiError::MaterialValidation)?;
    let material = repository.update_material(material_id, input).await?;

    Ok(Json(MaterialResponse::from(material)))
}

async fn update_material_status(
    State(state): State<AppState>,
    Path(material_id): Path<Uuid>,
    ValidJson(request): ValidJson<MaterialStatusRequest>,
) -> Result<Json<MaterialResponse>, ScriptApiError> {
    let status = request
        .parse_status()
        .map_err(ScriptApiError::MaterialValidation)?;
    let material = state
        .material_repository()?
        .update_material_status(material_id, status)
        .await?;

    Ok(Json(MaterialResponse::from(material)))
}

async fn list_topic_generation_batches(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<TopicGenerationBatchListResponse>, ScriptApiError> {
    ensure_project_exists(&state, project_id).await?;
    let repository = state.topic_repository()?;
    let batches = repository.list_generation_batches(project_id, 20).await?;

    Ok(Json(TopicGenerationBatchListResponse {
        batches: batches
            .into_iter()
            .map(TopicGenerationBatchSummaryResponse::from)
            .collect(),
    }))
}

async fn list_topic_groups(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<TopicGroupListQuery>,
) -> Result<Json<TopicGroupListResponse>, ScriptApiError> {
    ensure_project_exists(&state, project_id).await?;
    let repository = state.topic_repository()?;
    let topic_groups = repository
        .list_topic_group_summaries(project_id, query.sort, 20)
        .await?;

    Ok(Json(TopicGroupListResponse {
        topic_groups: topic_groups
            .into_iter()
            .map(TopicGroupSummaryResponse::from)
            .collect(),
    }))
}

async fn create_topic_group_review(
    State(state): State<AppState>,
    Path(root_batch_id): Path<Uuid>,
    Query(query): Query<TopicGroupProjectQuery>,
) -> Result<(StatusCode, Json<TopicReviewSnapshotResponse>), ScriptApiError> {
    let repository = state.topic_repository()?;
    let project_id =
        resolve_topic_group_project_id(&repository, root_batch_id, query.project_id).await?;
    let snapshot = state
        .agent_runtime()?
        .review_topic_group(project_id, root_batch_id)
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(TopicReviewSnapshotResponse::from(snapshot)),
    ))
}

async fn get_latest_topic_group_review(
    State(state): State<AppState>,
    Path(root_batch_id): Path<Uuid>,
    Query(query): Query<TopicGroupProjectQuery>,
) -> Result<Json<Option<TopicReviewSnapshotResponse>>, ScriptApiError> {
    let repository = state.topic_repository()?;
    let project_id =
        resolve_topic_group_project_id(&repository, root_batch_id, query.project_id).await?;
    let snapshot = repository
        .get_latest_topic_review_snapshot(project_id, root_batch_id)
        .await?;

    Ok(Json(snapshot.map(TopicReviewSnapshotResponse::from)))
}

async fn get_latest_topic_quality_evaluation(
    State(state): State<AppState>,
    Path(batch_id): Path<Uuid>,
    Query(query): Query<TopicGroupProjectQuery>,
) -> Result<Json<Option<TopicQualityEvaluationResponse>>, ScriptApiError> {
    let repository = state.topic_repository()?;
    let batch = repository.get_generation_batch(batch_id).await?;
    if query
        .project_id
        .is_some_and(|project_id| project_id != batch.project_id)
    {
        return Err(ScriptApiError::TopicRepository(
            TopicRepositoryError::BatchNotFound(batch_id),
        ));
    }
    let evaluation = repository
        .get_latest_topic_quality_evaluation(batch.project_id, batch_id)
        .await?;

    Ok(Json(evaluation.map(TopicQualityEvaluationResponse::from)))
}

async fn resolve_topic_group_project_id(
    repository: &PostgresTopicRepository,
    root_batch_id: Uuid,
    requested_project_id: Option<Uuid>,
) -> Result<Uuid, ScriptApiError> {
    let batch = repository.get_generation_batch(root_batch_id).await?;
    if batch.supplement_of_batch_id.is_some() {
        return Err(ScriptApiError::TopicRepository(
            TopicRepositoryError::BatchNotFound(root_batch_id),
        ));
    }
    if requested_project_id.is_some_and(|project_id| project_id != batch.project_id) {
        return Err(ScriptApiError::TopicRepository(
            TopicRepositoryError::BatchNotFound(root_batch_id),
        ));
    }

    Ok(batch.project_id)
}

async fn update_topic(
    State(state): State<AppState>,
    Path(topic_id): Path<Uuid>,
    ValidJson(request): ValidJson<UpdateContentTopicRequest>,
) -> Result<Json<ContentTopicResponse>, ScriptApiError> {
    request
        .validate_for_api()
        .map_err(ScriptApiError::TopicValidation)?;
    let repository = state.topic_repository()?;
    let topic = repository
        .update_topic(
            topic_id,
            UpdateContentTopicInput {
                title: request.title.trim().to_string(),
                angle: request.angle.trim().to_string(),
                target_audience: request.target_audience.trim().to_string(),
                hook_points: trim_string_list(request.hook_points),
                content_type: request.content_type.trim().to_string(),
                score: request.score,
                score_reason: request.score_reason.trim().to_string(),
                tags: trim_string_list(request.tags),
            },
        )
        .await?;

    Ok(Json(ContentTopicResponse::from(topic)))
}

async fn delete_topic(
    State(state): State<AppState>,
    Path(topic_id): Path<Uuid>,
) -> Result<Json<DeletedContentTopicResponse>, ScriptApiError> {
    let repository = state.topic_repository()?;
    let topic = repository.soft_delete_topic(topic_id).await?;
    let deleted_at = topic.deleted_at.ok_or_else(|| {
        ScriptApiError::TopicValidation("选题删除失败：缺少软删除时间".to_string())
    })?;

    Ok(Json(DeletedContentTopicResponse {
        topic_id: topic.id,
        deleted_at,
    }))
}

async fn update_topic_status(
    State(state): State<AppState>,
    Path(topic_id): Path<Uuid>,
    ValidJson(request): ValidJson<UpdateContentTopicStatusRequest>,
) -> Result<Json<ContentTopicResponse>, ScriptApiError> {
    if request.status == ContentTopicStatus::Scripted {
        return Err(ScriptApiError::TopicValidation(
            "选题只能在脚本生成成功后自动变为已成稿".to_string(),
        ));
    }

    let repository = state.topic_repository()?;
    let topic = repository
        .update_topic_status(topic_id, request.status)
        .await?;

    Ok(Json(ContentTopicResponse::from(topic)))
}

async fn prepare_script_from_topic(
    State(state): State<AppState>,
    Path(topic_id): Path<Uuid>,
    ValidJson(request): ValidJson<PrepareScriptFromTopicRequest>,
) -> Result<Json<PrepareScriptFromTopicResponse>, ScriptApiError> {
    request
        .validate_for_api()
        .map_err(ScriptApiError::TopicValidation)?;
    let repository = state.topic_repository()?;
    let topic = repository.get_topic(topic_id).await?;
    if topic.deleted_at.is_some() {
        return Err(ScriptApiError::TopicValidation(
            "已移除选题不能进入脚本生成确认流程".to_string(),
        ));
    }
    if topic.status != ContentTopicStatus::Approved {
        return Err(ScriptApiError::TopicValidation(
            "只有已确认选题可以进入脚本生成确认流程".to_string(),
        ));
    }

    let style = request.style_or_default();
    let scene_count = request.scene_count_or_default();
    let script_request = TopicScriptRequestPreview {
        project_id: topic.project_id,
        topic_id: topic.id,
        topic: topic.title.clone(),
        style,
        scene_count,
    };
    let topic_snapshot = topic.snapshot();

    Ok(Json(PrepareScriptFromTopicResponse {
        topic: ContentTopicResponse::from(topic),
        topic_snapshot,
        script_request,
    }))
}

async fn ensure_project_exists(state: &AppState, project_id: Uuid) -> Result<(), ScriptApiError> {
    if state
        .project_repository()?
        .project_exists(project_id)
        .await?
    {
        Ok(())
    } else {
        Err(ScriptApiError::Agent(ScriptAgentError::ProjectNotFound(
            project_id,
        )))
    }
}

fn trim_string_list(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

async fn create_agent_conversation(
    State(state): State<AppState>,
    ValidJson(request): ValidJson<CreateAgentConversationRequest>,
) -> Result<(StatusCode, Json<AgentConversationResponse>), ScriptApiError> {
    request
        .validate_for_api()
        .map_err(ScriptApiError::ConversationValidation)?;

    if matches!(request.agent_type.as_str(), "script" | "topic") {
        let project_id = request.project_id.ok_or_else(|| {
            ScriptApiError::ConversationValidation("Agent 会话必须绑定项目".to_string())
        })?;
        if request.agent_type == "script" {
            if let Some(script_id) = request.subject_id {
                let script = state
                    .script_agent_service_without_llm()?
                    .get_script(script_id)
                    .await?;
                if script.project_id != project_id {
                    return Err(ScriptApiError::ConversationValidation(
                        "脚本不属于当前项目".to_string(),
                    ));
                }
            } else if !state
                .project_repository()?
                .project_exists(project_id)
                .await?
            {
                return Err(ScriptApiError::Agent(ScriptAgentError::ProjectNotFound(
                    project_id,
                )));
            }
        } else if !state
            .project_repository()?
            .project_exists(project_id)
            .await?
        {
            return Err(ScriptApiError::Agent(ScriptAgentError::ProjectNotFound(
                project_id,
            )));
        }
    }

    let repository = state.conversation_repository()?;
    let conversation = repository
        .create_conversation(CreateAgentConversationInput {
            project_id: request.project_id,
            agent_type: request.agent_type.trim().to_string(),
            subject_type: request.subject_type.map(|value| value.trim().to_string()),
            subject_id: request.subject_id,
            title: request.title.trim().to_string(),
            metadata: request.metadata,
        })
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(AgentConversationResponse::from(conversation)),
    ))
}

async fn list_agent_messages(
    State(state): State<AppState>,
    Path(conversation_id): Path<Uuid>,
) -> Result<Json<AgentMessageListResponse>, ScriptApiError> {
    let repository = state.conversation_repository()?;
    repository.get_conversation(conversation_id).await?;
    let messages = repository.list_messages(conversation_id).await?;

    Ok(Json(AgentMessageListResponse {
        messages: messages
            .into_iter()
            .map(AgentMessageResponse::from)
            .collect(),
    }))
}

async fn send_agent_message(
    State(state): State<AppState>,
    Path(conversation_id): Path<Uuid>,
    ValidJson(request): ValidJson<SendAgentMessageRequest>,
) -> Result<Json<AgentTurnResponseBody>, ScriptApiError> {
    request
        .validate_for_api()
        .map_err(ScriptApiError::ConversationValidation)?;
    let response = state
        .agent_runtime()?
        .handle_turn(AgentTurnRequest {
            conversation_id,
            user_message: request.content,
            supplement_of_batch_id: request.supplement_of_batch_id,
        })
        .await?;

    Ok(Json(AgentTurnResponseBody {
        user_message: AgentMessageResponse::from(response.user_message),
        assistant_message: AgentMessageResponse::from(response.agent_message),
        run: AgentRunResponse::from(response.run),
    }))
}

async fn generate_script(
    State(state): State<AppState>,
    ValidJson(request): ValidJson<GenerateScriptRequest>,
) -> Result<Json<ScriptResponse>, ScriptApiError> {
    let service = state.script_agent_service()?;
    let script = service.generate_script(request).await?;

    Ok(Json(ScriptResponse::from(script)))
}

async fn get_script(
    State(state): State<AppState>,
    Path(script_id): Path<Uuid>,
) -> Result<Json<ScriptResponse>, ScriptApiError> {
    let service = state.script_agent_service_without_llm()?;
    let script = service.get_script(script_id).await?;

    Ok(Json(ScriptResponse::from(script)))
}

async fn list_scripts(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(filter): Query<ScriptListFilter>,
) -> Result<Json<ScriptListResponse>, ScriptApiError> {
    let service = state.script_agent_service_without_llm()?;
    let result = service.list_scripts(project_id, filter).await?;
    let response = ScriptListResponse {
        scripts: result.scripts.into_iter().map(Into::into).collect(),
        total: result.total,
        limit: result.limit,
        offset: result.offset,
    };

    Ok(Json(response))
}

async fn update_script_status(
    State(state): State<AppState>,
    Path(script_id): Path<Uuid>,
    ValidJson(request): ValidJson<UpdateScriptStatusRequest>,
) -> Result<Json<UpdateScriptStatusResponse>, ScriptApiError> {
    let service = state.script_agent_service_without_llm()?;
    let script = service.update_status(script_id, request.status).await?;

    Ok(Json(UpdateScriptStatusResponse::from(script)))
}

async fn create_asset_generation_plan(
    State(state): State<AppState>,
    Path(script_id): Path<Uuid>,
    ValidJson(request): ValidJson<AssetGenerationPlanRequest>,
) -> Result<Json<AssetGenerationPlanResponse>, ScriptApiError> {
    let provider = request
        .validate_for_api()
        .map_err(ScriptApiError::AssetValidation)?;
    ensure_asset_provider_enabled(&state, provider)?;
    let script = state
        .script_agent_service_without_llm()?
        .get_script(script_id)
        .await?;
    let reference_material_count = if request.use_reference_materials {
        active_image_material_ids(&state, script.project_id)
            .await?
            .len() as i32
    } else {
        0
    };
    let response = build_asset_generation_plan_response(
        script.id,
        script.scenes.len(),
        request.image_candidates_per_scene,
        provider,
        enabled_asset_generation_providers(&state),
        reference_material_count,
    );

    Ok(Json(response))
}

async fn create_asset_generation_tasks(
    State(state): State<AppState>,
    Path(script_id): Path<Uuid>,
    ValidJson(request): ValidJson<AssetGenerationTaskRequest>,
) -> Result<(StatusCode, Json<AssetGenerationTaskListResponse>), ScriptApiError> {
    let provider = request
        .validate_for_api()
        .map_err(ScriptApiError::AssetValidation)?;
    ensure_asset_provider_enabled(&state, provider)?;
    let script = state
        .script_agent_service_without_llm()?
        .get_script(script_id)
        .await?;
    let plan = build_asset_generation_plan_response(
        script.id,
        script.scenes.len(),
        request.image_candidates_per_scene,
        provider,
        enabled_asset_generation_providers(&state),
        0,
    );
    ensure_asset_generation_plan_can_create(&plan)?;

    let repository = state.asset_generation_repository()?;
    let existing_tasks = repository.list_tasks(script.id).await?;
    let reference_material_ids = if request.use_reference_materials {
        active_image_material_ids(&state, script.project_id).await?
    } else {
        Vec::new()
    };
    create_existing_material_candidates(&state, script.project_id, script.id, &script.scenes)
        .await?;
    let scene_ids: Vec<Uuid> = script.scenes.iter().map(|scene| scene.id).collect();
    let mut tasks = Vec::new();

    let image_task_key = script_image_task_idempotency_key(
        script.id,
        provider,
        request.image_candidates_per_scene,
        request.use_reference_materials,
        &reference_material_ids,
    );
    let had_matching_image_task = existing_tasks
        .iter()
        .any(|task| task.params.get("idempotency_key") == Some(&json!(image_task_key)));
    let image_task = repository
        .create_task(CreateAssetGenerationTaskInput {
            project_id: script.project_id,
            script_id: Some(script.id),
            scene_id: None,
            provider,
            task_type: AssetGenerationTaskType::ImageCandidates,
            status: AssetGenerationTaskStatus::Pending,
            candidate_count: plan.image_candidate_count,
            reference_material_ids: reference_material_ids.clone(),
            idempotency_key: Some(image_task_key.clone()),
            params: json!({
                "idempotency_key": image_task_key,
                "image_candidates_per_scene": request.image_candidates_per_scene,
                "scene_ids": scene_ids,
                "use_reference_materials": request.use_reference_materials
            }),
        })
        .await?;
    tasks.push(AssetGenerationTaskResponse::from(image_task));

    let mut reused_video_task_count = 0;
    for scene in &script.scenes {
        let video_task_key = scene_video_task_idempotency_key(scene.id, provider);
        if existing_tasks
            .iter()
            .any(|task| task.params.get("idempotency_key") == Some(&json!(video_task_key)))
        {
            reused_video_task_count += 1;
        }
        let video_task = repository
            .create_task(CreateAssetGenerationTaskInput {
                project_id: script.project_id,
                script_id: Some(script.id),
                scene_id: Some(scene.id),
                provider,
                task_type: AssetGenerationTaskType::VideoDraft,
                status: AssetGenerationTaskStatus::Draft,
                candidate_count: 0,
                reference_material_ids: reference_material_ids.clone(),
                idempotency_key: Some(video_task_key.clone()),
                params: json!({
                    "idempotency_key": video_task_key,
                    "scene_id": scene.id,
                    "requires_manual_confirmation": true
                }),
            })
            .await?;
        repository
            .create_candidate(CreateAssetCandidateInput {
                project_id: script.project_id,
                script_id: script.id,
                scene_id: scene.id,
                material_id: None,
                candidate_type: AssetCandidateType::Video,
                source: AssetCandidateSource::VideoTask,
                rank: 10_000,
                generation_task_id: Some(video_task.id),
                metadata: json!({ "requires_manual_confirmation": true }),
            })
            .await?;
        tasks.push(AssetGenerationTaskResponse::from(video_task));
    }

    let status_code = if had_matching_image_task && reused_video_task_count == script.scenes.len() {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };

    Ok((
        status_code,
        Json(AssetGenerationTaskListResponse {
            script_id: script.id,
            tasks,
        }),
    ))
}

async fn list_asset_generation_tasks(
    State(state): State<AppState>,
    Path(script_id): Path<Uuid>,
) -> Result<Json<AssetGenerationTaskListResponse>, ScriptApiError> {
    state
        .script_agent_service_without_llm()?
        .get_script(script_id)
        .await?;
    let tasks = state
        .asset_generation_repository()?
        .list_tasks(script_id)
        .await?
        .into_iter()
        .map(AssetGenerationTaskResponse::from)
        .collect();

    Ok(Json(AssetGenerationTaskListResponse { script_id, tasks }))
}

async fn list_asset_candidates(
    State(state): State<AppState>,
    Path(script_id): Path<Uuid>,
) -> Result<Json<SceneAssetCandidateListResponse>, ScriptApiError> {
    state
        .script_agent_service_without_llm()?
        .get_script(script_id)
        .await?;
    let candidates = state
        .asset_generation_repository()?
        .list_candidates(script_id)
        .await?;
    let responses = asset_candidate_responses(&state, candidates).await?;

    Ok(Json(SceneAssetCandidateListResponse {
        candidates: responses,
    }))
}

async fn select_asset_candidate(
    State(state): State<AppState>,
    Path((scene_id, candidate_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<SceneAssetCandidateResponse>, ScriptApiError> {
    let candidate = state
        .asset_generation_repository()?
        .select_candidate(scene_id, candidate_id)
        .await?;
    let response = asset_candidate_response(&state, candidate).await?;

    Ok(Json(response))
}

async fn reject_asset_candidate(
    State(state): State<AppState>,
    Path((scene_id, candidate_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<SceneAssetCandidateResponse>, ScriptApiError> {
    let candidate = state
        .asset_generation_repository()?
        .reject_candidate(scene_id, candidate_id)
        .await?;
    let response = asset_candidate_response(&state, candidate).await?;

    Ok(Json(response))
}

async fn create_scene_asset_generation_task(
    State(state): State<AppState>,
    Path(scene_id): Path<Uuid>,
    headers: HeaderMap,
    ValidJson(request): ValidJson<AssetGenerationTaskRequest>,
) -> Result<(StatusCode, Json<AssetGenerationTaskResponse>), ScriptApiError> {
    let request_idempotency_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            ScriptApiError::AssetValidation(
                "单镜头重生必须提供 UUID 格式 Idempotency-Key".to_string(),
            )
        })?;
    let request_idempotency_key =
        Uuid::parse_str(request_idempotency_key.trim()).map_err(|_| {
            ScriptApiError::AssetValidation(
                "单镜头重生必须提供 UUID 格式 Idempotency-Key".to_string(),
            )
        })?;
    let provider = request
        .validate_for_api()
        .map_err(ScriptApiError::AssetValidation)?;
    ensure_asset_provider_enabled(&state, provider)?;
    let (script_id, project_id) = scene_context(&state, scene_id).await?;
    let reference_material_ids = if request.use_reference_materials {
        active_image_material_ids(&state, project_id).await?
    } else {
        Vec::new()
    };
    let idempotency_key = format!("scene-image:{scene_id}:{request_idempotency_key}");
    let result = state
        .asset_generation_repository()?
        .create_or_reuse_scene_image_task(CreateAssetGenerationTaskInput {
            project_id,
            script_id: Some(script_id),
            scene_id: Some(scene_id),
            provider,
            task_type: AssetGenerationTaskType::ImageCandidates,
            status: AssetGenerationTaskStatus::Pending,
            candidate_count: request.image_candidates_per_scene,
            reference_material_ids,
            idempotency_key: Some(idempotency_key.clone()),
            params: json!({
                "idempotency_key": idempotency_key,
                "image_candidates_per_scene": request.image_candidates_per_scene,
                "scene_id": scene_id,
                "use_reference_materials": request.use_reference_materials
            }),
        })
        .await?;

    Ok((
        if result.created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(AssetGenerationTaskResponse::from(result.task)),
    ))
}

async fn confirm_asset_generation_task(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Result<Json<AssetGenerationTaskResponse>, ScriptApiError> {
    let task = state
        .asset_generation_repository()?
        .confirm_video_task(task_id)
        .await?;

    Ok(Json(AssetGenerationTaskResponse::from(task)))
}

async fn dismiss_asset_generation_task(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Result<Json<AssetGenerationTaskResponse>, ScriptApiError> {
    let task = state
        .asset_generation_repository()?
        .dismiss_task(task_id)
        .await?;

    Ok(Json(AssetGenerationTaskResponse::from(task)))
}

fn build_asset_generation_plan_response(
    script_id: Uuid,
    scene_count: usize,
    image_candidates_per_scene: i32,
    provider: repositories::AssetGenerationProvider,
    enabled_providers: Vec<String>,
    reference_material_count: i32,
) -> AssetGenerationPlanResponse {
    let image_candidate_count = scene_count as i32 * image_candidates_per_scene;
    let can_create = image_candidate_count <= 48;
    let warnings = if can_create {
        Vec::new()
    } else {
        vec!["单次最多生成 48 张图片候选，请减少分镜或候选数量".to_string()]
    };

    AssetGenerationPlanResponse {
        script_id,
        scene_count,
        image_candidate_count,
        max_image_candidate_count: 48,
        provider: provider.as_str().to_string(),
        enabled_providers,
        reference_material_count,
        video_task_count: scene_count as i32,
        can_create,
        warnings,
    }
}

fn enabled_asset_generation_providers(state: &AppState) -> Vec<String> {
    parse_asset_generation_providers(&state.config.asset_generation_providers.join(","))
}

fn ensure_asset_provider_enabled(
    state: &AppState,
    provider: repositories::AssetGenerationProvider,
) -> Result<(), ScriptApiError> {
    let enabled = enabled_asset_generation_providers(state);
    if enabled.iter().any(|item| item == provider.as_str()) {
        Ok(())
    } else {
        Err(ScriptApiError::AssetValidation(format!(
            "素材生成供应商 {} 未启用",
            provider.as_str()
        )))
    }
}

fn script_image_task_idempotency_key(
    script_id: Uuid,
    provider: repositories::AssetGenerationProvider,
    image_candidates_per_scene: i32,
    use_reference_materials: bool,
    reference_material_ids: &[Uuid],
) -> String {
    let mut references: Vec<String> = reference_material_ids.iter().map(Uuid::to_string).collect();
    references.sort();
    format!(
        "script:{script_id}:image:{}:{image_candidates_per_scene}:{use_reference_materials}:{}",
        provider.as_str(),
        references.join("|")
    )
}

fn scene_video_task_idempotency_key(
    scene_id: Uuid,
    provider: repositories::AssetGenerationProvider,
) -> String {
    format!("scene:{scene_id}:video-draft:{}", provider.as_str())
}

fn ensure_asset_generation_plan_can_create(
    plan: &AssetGenerationPlanResponse,
) -> Result<(), ScriptApiError> {
    if plan.can_create {
        Ok(())
    } else {
        Err(ScriptApiError::AssetValidation(
            "单次最多生成 48 张图片候选，请减少分镜或候选数量".to_string(),
        ))
    }
}

async fn active_image_material_ids(
    state: &AppState,
    project_id: Uuid,
) -> Result<Vec<Uuid>, ScriptApiError> {
    let materials = state
        .material_repository()?
        .list_materials(
            project_id,
            MaterialListFilter {
                material_type: Some(MaterialType::Image),
                status: MaterialStatusFilter::Active,
                ..MaterialListFilter::default()
            },
        )
        .await?;

    Ok(materials.into_iter().map(|material| material.id).collect())
}

async fn create_existing_material_candidates(
    state: &AppState,
    project_id: Uuid,
    script_id: Uuid,
    scenes: &[crate::agents::models::Scene],
) -> Result<(), ScriptApiError> {
    let materials = state
        .material_repository()?
        .list_materials(
            project_id,
            MaterialListFilter {
                material_type: Some(MaterialType::Image),
                status: MaterialStatusFilter::Active,
                ..MaterialListFilter::default()
            },
        )
        .await?;
    let repository = state.asset_generation_repository()?;

    for scene in scenes {
        for (index, material) in materials.iter().enumerate() {
            repository
                .create_candidate(CreateAssetCandidateInput {
                    project_id,
                    script_id,
                    scene_id: scene.id,
                    material_id: Some(material.id),
                    candidate_type: AssetCandidateType::Image,
                    source: AssetCandidateSource::ExistingMaterial,
                    rank: index as i32 + 1,
                    generation_task_id: None,
                    metadata: json!({ "reuse_reason": "active image material" }),
                })
                .await?;
        }
    }

    Ok(())
}

async fn asset_candidate_responses(
    state: &AppState,
    candidates: Vec<repositories::SceneAssetCandidate>,
) -> Result<Vec<SceneAssetCandidateResponse>, ScriptApiError> {
    let mut responses = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        responses.push(asset_candidate_response(state, candidate).await?);
    }
    Ok(responses)
}

async fn asset_candidate_response(
    state: &AppState,
    candidate: repositories::SceneAssetCandidate,
) -> Result<SceneAssetCandidateResponse, ScriptApiError> {
    let material = match candidate.material_id {
        Some(material_id) => Some(
            state
                .material_repository()?
                .get_material(material_id)
                .await?,
        ),
        None => None,
    };

    Ok(SceneAssetCandidateResponse::from_candidate(
        candidate, material,
    ))
}

async fn scene_context(state: &AppState, scene_id: Uuid) -> Result<(Uuid, Uuid), ScriptApiError> {
    let pool = state
        .pg_pool
        .clone()
        .ok_or_else(|| ScriptApiError::State("database pool is not configured".to_string()))?;
    sqlx::query_as::<_, (Uuid, Uuid)>(
        r#"
        SELECT s.id AS script_id, s.project_id
        FROM scenes sc
        JOIN scripts s ON s.id = sc.script_id
        WHERE sc.id = $1
        "#,
    )
    .bind(scene_id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| ScriptApiError::AssetValidation(error.to_string()))?
    .ok_or_else(|| ScriptApiError::AssetValidation("分镜不存在".to_string()))
}

struct ValidJson<T>(T);

#[async_trait::async_trait]
impl<S, T> FromRequest<S> for ValidJson<T>
where
    S: Send + Sync,
    T: serde::de::DeserializeOwned,
{
    type Rejection = ScriptApiError;

    async fn from_request(
        request: axum::extract::Request,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(request, state)
            .await
            .map_err(ScriptApiError::JsonRejection)?;

        Ok(Self(value))
    }
}

#[derive(Debug)]
enum ScriptApiError {
    State(String),
    Agent(ScriptAgentError),
    AgentRuntime(AgentRuntimeError),
    ProjectRepository(ProjectRepositoryError),
    MaterialRepository(MaterialRepositoryError),
    AssetGenerationRepository(AssetGenerationRepositoryError),
    ConversationRepository(ConversationRepositoryError),
    TopicRepository(TopicRepositoryError),
    WorkspaceMenuRepository(WorkspaceMenuRepositoryError),
    ProjectValidation(String),
    MaterialValidation(String),
    AssetValidation(String),
    ConversationValidation(String),
    TopicValidation(String),
    StrategyDraftLlm(LLMError),
    StrategyDraftOutput(String),
    JsonRejection(JsonRejection),
}

impl From<ScriptAgentError> for ScriptApiError {
    fn from(error: ScriptAgentError) -> Self {
        Self::Agent(error)
    }
}

impl From<ProjectRepositoryError> for ScriptApiError {
    fn from(error: ProjectRepositoryError) -> Self {
        Self::ProjectRepository(error)
    }
}

impl From<MaterialRepositoryError> for ScriptApiError {
    fn from(error: MaterialRepositoryError) -> Self {
        Self::MaterialRepository(error)
    }
}

impl From<AssetGenerationRepositoryError> for ScriptApiError {
    fn from(error: AssetGenerationRepositoryError) -> Self {
        Self::AssetGenerationRepository(error)
    }
}

impl From<ConversationRepositoryError> for ScriptApiError {
    fn from(error: ConversationRepositoryError) -> Self {
        Self::ConversationRepository(error)
    }
}

impl From<TopicRepositoryError> for ScriptApiError {
    fn from(error: TopicRepositoryError) -> Self {
        Self::TopicRepository(error)
    }
}

impl From<AgentRuntimeError> for ScriptApiError {
    fn from(error: AgentRuntimeError) -> Self {
        Self::AgentRuntime(error)
    }
}

impl From<WorkspaceMenuRepositoryError> for ScriptApiError {
    fn from(error: WorkspaceMenuRepositoryError) -> Self {
        Self::WorkspaceMenuRepository(error)
    }
}

impl IntoResponse for ScriptApiError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::State(message) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": message })),
            )
                .into_response(),
            Self::ProjectRepository(error) => {
                project_repository_error_response(error).into_response()
            }
            Self::MaterialRepository(error) => {
                material_repository_error_response(error).into_response()
            }
            Self::AssetGenerationRepository(error) => {
                asset_generation_repository_error_response(error).into_response()
            }
            Self::ConversationRepository(error) => {
                conversation_repository_error_response(error).into_response()
            }
            Self::TopicRepository(error) => topic_repository_error_response(error).into_response(),
            Self::WorkspaceMenuRepository(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "视频工作台菜单读取失败", "details": error.to_string() })),
            )
                .into_response(),
            Self::ProjectValidation(message) => {
                (StatusCode::BAD_REQUEST, Json(json!({ "error": message }))).into_response()
            }
            Self::MaterialValidation(message) => {
                (StatusCode::BAD_REQUEST, Json(json!({ "error": message }))).into_response()
            }
            Self::AssetValidation(message) => {
                (StatusCode::BAD_REQUEST, Json(json!({ "error": message }))).into_response()
            }
            Self::ConversationValidation(message) => {
                (StatusCode::BAD_REQUEST, Json(json!({ "error": message }))).into_response()
            }
            Self::TopicValidation(message) => {
                (StatusCode::BAD_REQUEST, Json(json!({ "error": message }))).into_response()
            }
            Self::StrategyDraftLlm(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "AI 策略草稿生成失败", "details": error.to_string() })),
            )
                .into_response(),
            Self::StrategyDraftOutput(message) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "AI 策略草稿输出无效", "details": message })),
            )
                .into_response(),
            Self::JsonRejection(error) => invalid_json_response(error).into_response(),
            Self::Agent(error) => script_agent_error_response(error).into_response(),
            Self::AgentRuntime(error) => agent_runtime_error_response(error).into_response(),
        }
    }
}

fn topic_repository_error_response(
    error: TopicRepositoryError,
) -> (StatusCode, Json<serde_json::Value>) {
    match error {
        TopicRepositoryError::TopicNotFound(topic_id) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "选题不存在", "topic_id": topic_id })),
        ),
        TopicRepositoryError::BatchNotFound(batch_id) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "选题生成批次不存在", "batch_id": batch_id })),
        ),
        TopicRepositoryError::BatchCannotBeSupplemented(batch_id) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "该历史生成批次不可补充", "batch_id": batch_id })),
        ),
        TopicRepositoryError::TopicCannotBeDeleted(topic_id) => (
            StatusCode::BAD_REQUEST,
            Json(
                json!({ "error": "已生成脚本或已被脚本引用的选题不可删除", "topic_id": topic_id }),
            ),
        ),
        TopicRepositoryError::InvalidStatusTransition { topic_id, from, to } => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "选题状态流转非法",
                "topic_id": topic_id,
                "from": from,
                "to": to
            })),
        ),
        TopicRepositoryError::Storage(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "选题存储失败", "details": message })),
        ),
    }
}

fn project_repository_error_response(
    error: ProjectRepositoryError,
) -> (StatusCode, Json<serde_json::Value>) {
    match error {
        ProjectRepositoryError::NotFound(project_id) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "项目不存在", "project_id": project_id })),
        ),
        ProjectRepositoryError::Storage(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "项目存储失败", "details": message })),
        ),
    }
}

fn material_repository_error_response(
    error: MaterialRepositoryError,
) -> (StatusCode, Json<serde_json::Value>) {
    match error {
        MaterialRepositoryError::MaterialNotFound(material_id) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "素材不存在", "material_id": material_id })),
        ),
        MaterialRepositoryError::ProjectNotFound(project_id) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "项目不存在", "project_id": project_id })),
        ),
        MaterialRepositoryError::MaterialInUseAsSelectedCandidate(material_id) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "已选为分镜主素材的素材不可归档", "material_id": material_id })),
        ),
        MaterialRepositoryError::Storage(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "素材存储失败", "details": message })),
        ),
    }
}

fn asset_generation_repository_error_response(
    error: AssetGenerationRepositoryError,
) -> (StatusCode, Json<serde_json::Value>) {
    match error {
        AssetGenerationRepositoryError::TaskNotFound(task_id) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "素材生成任务不存在", "task_id": task_id })),
        ),
        AssetGenerationRepositoryError::TaskNotConfirmable(task_id) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "只有待确认的 AI 视频任务可以确认", "task_id": task_id })),
        ),
        AssetGenerationRepositoryError::TaskNotDismissible(task_id) => (
            StatusCode::CONFLICT,
            Json(json!({ "error": "只有失败的素材生成任务可以清理", "task_id": task_id })),
        ),
        AssetGenerationRepositoryError::CandidateNotFound(candidate_id) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "素材候选不存在", "candidate_id": candidate_id })),
        ),
        AssetGenerationRepositoryError::CandidateNotSelectable(candidate_id) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "素材候选不可选择", "candidate_id": candidate_id })),
        ),
        AssetGenerationRepositoryError::FailedCandidateNotSelectable(candidate_id) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "失败候选不可绑定分镜", "candidate_id": candidate_id })),
        ),
        AssetGenerationRepositoryError::InvalidCandidateRelation(message) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "素材候选关系非法", "details": message })),
        ),
        AssetGenerationRepositoryError::Storage(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "素材生成存储失败", "details": message })),
        ),
    }
}

fn conversation_repository_error_response(
    error: ConversationRepositoryError,
) -> (StatusCode, Json<serde_json::Value>) {
    match error {
        ConversationRepositoryError::ConversationNotFound(conversation_id) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "会话不存在", "conversation_id": conversation_id })),
        ),
        ConversationRepositoryError::RunNotFound(run_id) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Agent 运行记录不存在", "run_id": run_id })),
        ),
        ConversationRepositoryError::Storage(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "会话存储失败", "details": message })),
        ),
    }
}

fn invalid_json_response(error: JsonRejection) -> (StatusCode, Json<serde_json::Value>) {
    let body = match error {
        JsonRejection::JsonDataError(_) => json!({
            "error": "无效的状态值",
            "allowed": ["draft", "approved", "archived"]
        }),
        other => json!({ "error": other.body_text() }),
    };

    (StatusCode::BAD_REQUEST, Json(body))
}

fn script_agent_error_response(error: ScriptAgentError) -> (StatusCode, Json<serde_json::Value>) {
    match error {
        ScriptAgentError::Validation(message) => {
            (StatusCode::BAD_REQUEST, Json(json!({ "error": message })))
        }
        ScriptAgentError::ProjectNotFound(project_id) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "项目不存在", "project_id": project_id })),
        ),
        ScriptAgentError::ScriptNotFound(script_id) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "脚本不存在", "script_id": script_id })),
        ),
        ScriptAgentError::Timeout => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "脚本生成超时，请稍后重试" })),
        ),
        ScriptAgentError::LLMError(message) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "脚本生成服务异常", "details": message })),
        ),
        ScriptAgentError::ParseError(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                json!({ "error": "脚本生成失败", "details": format!("script parse error: {message}") }),
            ),
        ),
        ScriptAgentError::DatabaseError(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "脚本存储失败", "details": message })),
        ),
    }
}

fn agent_runtime_error_response(error: AgentRuntimeError) -> (StatusCode, Json<serde_json::Value>) {
    match error {
        AgentRuntimeError::Validation(message) => {
            (StatusCode::BAD_REQUEST, Json(json!({ "error": message })))
        }
        AgentRuntimeError::UnsupportedAgent(agent_type) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "暂不支持该 Agent 类型", "agent_type": agent_type })),
        ),
        AgentRuntimeError::SceneNotFound {
            script_id,
            sequence,
        } => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "分镜不存在", "script_id": script_id, "sequence": sequence })),
        ),
        AgentRuntimeError::ConversationRepository(error) => {
            conversation_repository_error_response(error)
        }
        AgentRuntimeError::ScriptRepository(error) => match error {
            ScriptRepositoryError::NotFound(script_id) => (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "脚本不存在", "script_id": script_id })),
            ),
            ScriptRepositoryError::SceneNotFound {
                script_id,
                sequence,
            } => (
                StatusCode::NOT_FOUND,
                Json(
                    json!({ "error": "分镜不存在", "script_id": script_id, "sequence": sequence }),
                ),
            ),
            ScriptRepositoryError::Storage(message) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "脚本存储失败", "details": message })),
            ),
        },
        AgentRuntimeError::ProjectRepository(error) => match error {
            ProjectRepositoryError::NotFound(project_id) => (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "项目不存在", "project_id": project_id })),
            ),
            ProjectRepositoryError::Storage(message) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "项目存储失败", "details": message })),
            ),
        },
        AgentRuntimeError::TopicRepository(error) => topic_repository_error_response(error),
        AgentRuntimeError::ScriptAgent(error) => script_agent_error_response(error),
        AgentRuntimeError::InvalidLlmOutput(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Agent 输出无效", "details": message })),
        ),
        AgentRuntimeError::Llm(error) => match error {
            LLMError::Timeout => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "Agent 调用模型超时，请稍后重试" })),
            ),
            other => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Agent 调用模型失败", "details": other.to_string() })),
            ),
        },
    }
}
