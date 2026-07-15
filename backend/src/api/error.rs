//! 将 Application、Repository 和模型错误稳定映射为现有 HTTP 协议。

use crate::agents::ScriptAgentError;
use crate::application;
use crate::application::agents::runtime::AgentRuntimeError;
use crate::application::asset_generation::AssetGenerationApplicationError;
use crate::application::conversations::ConversationApplicationError;
use crate::application::materials::MaterialApplicationError;
use crate::application::projects::ProjectApplicationError;
use crate::application::scripts::ScriptApplicationError;
use crate::application::topics::TopicApplicationError;
use crate::bootstrap::AppStateError;
use crate::model_routing::ModelResolveError;
use crate::repositories::{
    AssetGenerationRepositoryError, ConversationRepositoryError, MaterialRepositoryError,
    ProjectRepositoryError, ScriptRepositoryError, TopicRepositoryError,
    WorkspaceMenuRepositoryError,
};
use axum::{
    extract::{rejection::JsonRejection, FromRequest},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use novex_model::LLMError;
use serde_json::json;

pub(crate) struct ValidJson<T>(pub(crate) T);

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
pub(crate) enum ScriptApiError {
    State(String),
    Agent(ScriptAgentError),
    AgentRuntime(AgentRuntimeError),
    ModelResolve(ModelResolveError),
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

impl From<ModelResolveError> for ScriptApiError {
    fn from(error: ModelResolveError) -> Self {
        Self::ModelResolve(error)
    }
}

impl From<WorkspaceMenuRepositoryError> for ScriptApiError {
    fn from(error: WorkspaceMenuRepositoryError) -> Self {
        Self::WorkspaceMenuRepository(error)
    }
}

impl From<application::workspace::WorkspaceApplicationError> for ScriptApiError {
    fn from(error: application::workspace::WorkspaceApplicationError) -> Self {
        match error {
            application::workspace::WorkspaceApplicationError::Repository(error) => {
                Self::WorkspaceMenuRepository(error)
            }
        }
    }
}

impl From<AppStateError> for ScriptApiError {
    fn from(error: AppStateError) -> Self {
        Self::State(error.to_string())
    }
}

impl From<ProjectApplicationError> for ScriptApiError {
    fn from(error: ProjectApplicationError) -> Self {
        match error {
            ProjectApplicationError::ProjectRepository(error) => Self::ProjectRepository(error),
            ProjectApplicationError::ConversationRepository(error) => {
                Self::ConversationRepository(error)
            }
            ProjectApplicationError::ModelResolve(error) => Self::ModelResolve(error),
            ProjectApplicationError::Llm(error) => Self::StrategyDraftLlm(error),
            ProjectApplicationError::InvalidOutput(message) => Self::StrategyDraftOutput(message),
            ProjectApplicationError::Serialization(message) => Self::State(message),
        }
    }
}

impl From<MaterialApplicationError> for ScriptApiError {
    fn from(error: MaterialApplicationError) -> Self {
        match error {
            MaterialApplicationError::ProjectRepository(error) => Self::ProjectRepository(error),
            MaterialApplicationError::MaterialRepository(error) => Self::MaterialRepository(error),
            MaterialApplicationError::ProjectNotFound(project_id) => {
                Self::Agent(ScriptAgentError::ProjectNotFound(project_id))
            }
            MaterialApplicationError::UploadValidation(error) => {
                Self::MaterialValidation(error.to_string())
            }
            MaterialApplicationError::UploadStorage(message) => Self::State(message),
            MaterialApplicationError::Validation(message) => Self::MaterialValidation(message),
        }
    }
}

impl From<ScriptApplicationError> for ScriptApiError {
    fn from(error: ScriptApplicationError) -> Self {
        match error {
            ScriptApplicationError::Agent(error) => Self::Agent(error),
            ScriptApplicationError::ConversationRepository(error) => {
                Self::ConversationRepository(error)
            }
            ScriptApplicationError::ModelResolve(error) => Self::ModelResolve(error),
            ScriptApplicationError::Serialization(message) => Self::State(message),
        }
    }
}

impl From<ConversationApplicationError> for ScriptApiError {
    fn from(error: ConversationApplicationError) -> Self {
        match error {
            ConversationApplicationError::ConversationRepository(error) => {
                Self::ConversationRepository(error)
            }
            ConversationApplicationError::ProjectRepository(error) => {
                Self::ProjectRepository(error)
            }
            ConversationApplicationError::Agent(error) => Self::Agent(error),
            ConversationApplicationError::Runtime(error) => Self::AgentRuntime(error),
            ConversationApplicationError::ModelResolve(error) => Self::ModelResolve(error),
            ConversationApplicationError::Validation(message) => {
                Self::ConversationValidation(message)
            }
        }
    }
}

impl From<TopicApplicationError> for ScriptApiError {
    fn from(error: TopicApplicationError) -> Self {
        match error {
            TopicApplicationError::ProjectRepository(error) => Self::ProjectRepository(error),
            TopicApplicationError::TopicRepository(error) => Self::TopicRepository(error),
            TopicApplicationError::Agent(error) => Self::Agent(error),
            TopicApplicationError::Runtime(error) => Self::AgentRuntime(error),
            TopicApplicationError::ModelResolve(error) => Self::ModelResolve(error),
            TopicApplicationError::Validation(message) => Self::TopicValidation(message),
        }
    }
}

impl From<AssetGenerationApplicationError> for ScriptApiError {
    fn from(error: AssetGenerationApplicationError) -> Self {
        match error {
            AssetGenerationApplicationError::Agent(error) => Self::Agent(error),
            AssetGenerationApplicationError::AssetRepository(error) => {
                Self::AssetGenerationRepository(error)
            }
            AssetGenerationApplicationError::MaterialRepository(error) => {
                Self::MaterialRepository(error)
            }
            AssetGenerationApplicationError::ModelResolve(error) => Self::ModelResolve(error),
            AssetGenerationApplicationError::Validation(message) => Self::AssetValidation(message),
        }
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
            Self::ModelResolve(error) => model_resolve_error_response(error).into_response(),
        }
    }
}

fn model_resolve_error_response(error: ModelResolveError) -> (StatusCode, Json<serde_json::Value>) {
    let (status, code, message) = match error {
        ModelResolveError::NotFound(_) => (StatusCode::NOT_FOUND, "model_not_found", "模型不存在"),
        ModelResolveError::Disabled(_) => {
            (StatusCode::CONFLICT, "model_disabled", "模型已停用或删除")
        }
        ModelResolveError::TypeMismatch { .. } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "model_type_mismatch",
            "模型类型不匹配",
        ),
        ModelResolveError::InvalidConfig(_) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_model_config",
            "模型配置无效",
        ),
        ModelResolveError::Storage => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "model_storage_error",
            "模型配置服务暂时不可用",
        ),
    };
    (
        status,
        Json(json!({ "error": { "code": code, "message": message } })),
    )
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
        MaterialRepositoryError::InvalidMetadata(message) => {
            (StatusCode::BAD_REQUEST, Json(json!({ "error": message })))
        }
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
