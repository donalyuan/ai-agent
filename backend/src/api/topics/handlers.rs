use super::dto::*;
use crate::api::error::{ScriptApiError, ValidJson};
use crate::bootstrap::AppState;
use crate::domain::topic::{ContentTopicFilter, ContentTopicSource};
use crate::repositories::{CreateContentTopicInput, UpdateContentTopicInput};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde_json::json;
use uuid::Uuid;

pub(super) async fn create_topic(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    ValidJson(request): ValidJson<CreateContentTopicRequest>,
) -> Result<(StatusCode, Json<ContentTopicResponse>), ScriptApiError> {
    request
        .validate_for_api()
        .map_err(ScriptApiError::TopicValidation)?;
    let topic = state
        .topic_service()?
        .create(CreateContentTopicInput {
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
            source: ContentTopicSource::Manual,
            metadata: json!({}),
        })
        .await?;
    Ok((StatusCode::CREATED, Json(ContentTopicResponse::from(topic))))
}

pub(super) async fn list_topics(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(filter): Query<ContentTopicFilter>,
) -> Result<Json<ContentTopicListResponse>, ScriptApiError> {
    let result = state.topic_service()?.list(project_id, filter).await?;
    Ok(Json(ContentTopicListResponse {
        topics: result
            .topics
            .into_iter()
            .map(ContentTopicResponse::from)
            .collect(),
        stats: ContentTopicStatsResponse::from_counts(result.stats),
    }))
}

pub(super) async fn list_topic_generation_batches(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> Result<Json<TopicGenerationBatchListResponse>, ScriptApiError> {
    let batches = state
        .topic_service()?
        .list_generation_batches(project_id)
        .await?;
    Ok(Json(TopicGenerationBatchListResponse {
        batches: batches
            .into_iter()
            .map(TopicGenerationBatchSummaryResponse::from)
            .collect(),
    }))
}

pub(super) async fn list_topic_groups(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<TopicGroupListQuery>,
) -> Result<Json<TopicGroupListResponse>, ScriptApiError> {
    let topic_groups = state
        .topic_service()?
        .list_groups(project_id, query.sort)
        .await?;
    Ok(Json(TopicGroupListResponse {
        topic_groups: topic_groups
            .into_iter()
            .map(TopicGroupSummaryResponse::from)
            .collect(),
    }))
}

pub(super) async fn create_topic_group_review(
    State(state): State<AppState>,
    Path(root_batch_id): Path<Uuid>,
    Query(query): Query<TopicGroupProjectQuery>,
    ValidJson(request): ValidJson<TopicReviewRequest>,
) -> Result<(StatusCode, Json<TopicReviewSnapshotResponse>), ScriptApiError> {
    let snapshot = state
        .topic_service()?
        .review_group(root_batch_id, query.project_id, request.model_id)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(TopicReviewSnapshotResponse::from(snapshot)),
    ))
}

pub(super) async fn get_latest_topic_group_review(
    State(state): State<AppState>,
    Path(root_batch_id): Path<Uuid>,
    Query(query): Query<TopicGroupProjectQuery>,
) -> Result<Json<Option<TopicReviewSnapshotResponse>>, ScriptApiError> {
    let snapshot = state
        .topic_service()?
        .latest_group_review(root_batch_id, query.project_id)
        .await?;
    Ok(Json(snapshot.map(TopicReviewSnapshotResponse::from)))
}

pub(super) async fn get_latest_topic_quality_evaluation(
    State(state): State<AppState>,
    Path(batch_id): Path<Uuid>,
    Query(query): Query<TopicGroupProjectQuery>,
) -> Result<Json<Option<TopicQualityEvaluationResponse>>, ScriptApiError> {
    let evaluation = state
        .topic_service()?
        .latest_quality_evaluation(batch_id, query.project_id)
        .await?;
    Ok(Json(evaluation.map(TopicQualityEvaluationResponse::from)))
}

pub(super) async fn update_topic(
    State(state): State<AppState>,
    Path(topic_id): Path<Uuid>,
    ValidJson(request): ValidJson<UpdateContentTopicRequest>,
) -> Result<Json<ContentTopicResponse>, ScriptApiError> {
    request
        .validate_for_api()
        .map_err(ScriptApiError::TopicValidation)?;
    let topic = state
        .topic_service()?
        .update(
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

pub(super) async fn delete_topic(
    State(state): State<AppState>,
    Path(topic_id): Path<Uuid>,
) -> Result<Json<DeletedContentTopicResponse>, ScriptApiError> {
    let deleted = state.topic_service()?.delete(topic_id).await?;
    Ok(Json(DeletedContentTopicResponse {
        topic_id: deleted.topic_id,
        deleted_at: deleted.deleted_at,
    }))
}

pub(super) async fn update_topic_status(
    State(state): State<AppState>,
    Path(topic_id): Path<Uuid>,
    ValidJson(request): ValidJson<UpdateContentTopicStatusRequest>,
) -> Result<Json<ContentTopicResponse>, ScriptApiError> {
    let topic = state
        .topic_service()?
        .update_status(topic_id, request.status)
        .await?;
    Ok(Json(ContentTopicResponse::from(topic)))
}

pub(super) async fn prepare_script_from_topic(
    State(state): State<AppState>,
    Path(topic_id): Path<Uuid>,
    ValidJson(request): ValidJson<PrepareScriptFromTopicRequest>,
) -> Result<Json<PrepareScriptFromTopicResponse>, ScriptApiError> {
    request
        .validate_for_api()
        .map_err(ScriptApiError::TopicValidation)?;
    let prepared = state
        .topic_service()?
        .prepare_script(
            topic_id,
            request.style_or_default(),
            request.scene_count_or_default(),
        )
        .await?;
    let script_request = TopicScriptRequestPreview {
        project_id: prepared.topic.project_id,
        topic_id: prepared.topic.id,
        topic: prepared.topic.title.clone(),
        style: prepared.style,
        scene_count: prepared.scene_count,
    };
    Ok(Json(PrepareScriptFromTopicResponse {
        topic: ContentTopicResponse::from(prepared.topic),
        topic_snapshot: prepared.topic_snapshot,
        script_request,
    }))
}

fn trim_string_list(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}
