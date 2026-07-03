use agents::{LLMClient, ScriptAgentError, ScriptAgentService, ScriptGenerationMode};
use axum::{
    extract::{rejection::JsonRejection, FromRequest, Path, Query, State},
    http::{header, HeaderValue, Method, StatusCode},
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use novex_model::{LLMError, LLMPrompt, OpenAIClient, OpenAIConfig};
use repositories::{
    CreateProjectInput, PostgresProjectRepository, PostgresScriptRepository,
    PostgresWorkspaceMenuRepository, ProjectRepository, ProjectRepositoryError,
    WorkspaceMenuRepositoryError,
};
use serde::Serialize;
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use crate::agents::models::{
    CreateProjectRequest, GenerateScriptRequest, ProjectListResponse, ProjectResponse,
    ScriptListFilter, ScriptListResponse, ScriptResponse, UpdateScriptStatusRequest,
    UpdateScriptStatusResponse, WorkspaceMenuListResponse, WorkspaceMenuNodeResponse,
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
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    config: AppConfig,
    pg_pool: Option<PgPool>,
    redis_client: Option<redis::Client>,
}

impl AppState {
    pub fn test() -> Self {
        Self {
            config: AppConfig::from_env(),
            pg_pool: None,
            redis_client: None,
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
        })
    }

    fn script_agent_service(&self) -> Result<ScriptAgentService, ScriptApiError> {
        let pool = self
            .pg_pool
            .clone()
            .ok_or_else(|| ScriptApiError::State("database pool is not configured".to_string()))?;
        let llm_client = self.openai_client()?;
        let script_repository = Arc::new(PostgresScriptRepository::new(pool.clone()));
        let project_repository = Arc::new(PostgresProjectRepository::new(pool));

        let service = ScriptAgentService::new(llm_client, script_repository, project_repository)
            .with_generation_mode(self.script_generation_mode());

        Ok(service)
    }

    fn project_repository(&self) -> Result<PostgresProjectRepository, ScriptApiError> {
        let pool = self
            .pg_pool
            .clone()
            .ok_or_else(|| ScriptApiError::State("database pool is not configured".to_string()))?;

        Ok(PostgresProjectRepository::new(pool))
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

pub fn build_app() -> Router {
    build_app_with_state(AppState::test())
}

pub fn build_app_with_state(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin("*".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::OPTIONS])
        .allow_headers([header::ACCEPT, header::CONTENT_TYPE]);

    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/api/video-workspace/menus", get(list_workspace_menus))
        .route("/api/projects", get(list_projects).post(create_project))
        .route("/api/scripts/generate", post(generate_script))
        .route("/api/scripts/:script_id", get(get_script))
        .route("/api/projects/:project_id/scripts", get(list_scripts))
        .route("/api/scripts/:script_id/status", put(update_script_status))
        .layer(cors)
        .with_state(state)
}

pub async fn build_runtime_state() -> Result<AppState, Box<dyn std::error::Error + Send + Sync>> {
    let config = AppConfig::from_env();
    let pg_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect_lazy(&config.database_url)?;
    let redis_client = redis::Client::open(config.redis_url.clone())?;

    AppState::new(config, pg_pool, Some(redis_client))
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
    ProjectRepository(ProjectRepositoryError),
    WorkspaceMenuRepository(WorkspaceMenuRepositoryError),
    ProjectValidation(String),
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
            Self::ProjectRepository(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "项目存储失败", "details": error.to_string() })),
            )
                .into_response(),
            Self::WorkspaceMenuRepository(error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "视频工作台菜单读取失败", "details": error.to_string() })),
            )
                .into_response(),
            Self::ProjectValidation(message) => {
                (StatusCode::BAD_REQUEST, Json(json!({ "error": message }))).into_response()
            }
            Self::JsonRejection(error) => invalid_json_response(error).into_response(),
            Self::Agent(error) => script_agent_error_response(error).into_response(),
        }
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
        ScriptAgentError::LLMError(message)
        | ScriptAgentError::ParseError(message)
        | ScriptAgentError::DatabaseError(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "内部错误", "details": message })),
        ),
    }
}
