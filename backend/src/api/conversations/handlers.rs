use super::dto::*;
use crate::api::error::{ScriptApiError, ValidJson};
use crate::bootstrap::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

pub(super) async fn create_agent_conversation(
    State(state): State<AppState>,
    ValidJson(request): ValidJson<CreateAgentConversationRequest>,
) -> Result<(StatusCode, Json<AgentConversationResponse>), ScriptApiError> {
    request
        .validate_for_api()
        .map_err(ScriptApiError::ConversationValidation)?;
    let conversation = state
        .conversation_service()?
        .create(request.into_command())
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(AgentConversationResponse::from(conversation)),
    ))
}

pub(super) async fn list_agent_messages(
    State(state): State<AppState>,
    Path(conversation_id): Path<Uuid>,
) -> Result<Json<AgentMessageListResponse>, ScriptApiError> {
    let messages = state
        .conversation_service()?
        .list_messages(conversation_id)
        .await?;
    Ok(Json(AgentMessageListResponse {
        messages: messages
            .into_iter()
            .map(AgentMessageResponse::from)
            .collect(),
    }))
}

pub(super) async fn send_agent_message(
    State(state): State<AppState>,
    Path(conversation_id): Path<Uuid>,
    ValidJson(request): ValidJson<SendAgentMessageRequest>,
) -> Result<Json<AgentTurnResponseBody>, ScriptApiError> {
    request
        .validate_for_api()
        .map_err(ScriptApiError::ConversationValidation)?;
    let response = state
        .conversation_service()?
        .send_message(
            conversation_id,
            request.model_id,
            request.content,
            request.supplement_of_batch_id,
            request.sound_context.map(|context| context.normalized()),
        )
        .await?;

    Ok(Json(AgentTurnResponseBody {
        user_message: AgentMessageResponse::from(response.user_message),
        assistant_message: AgentMessageResponse::from(response.agent_message),
        run: AgentRunResponse::from(response.run),
    }))
}
