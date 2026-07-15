use super::dto::*;
use crate::api::error::{ScriptApiError, ValidJson};
use crate::application::materials::{GeneratedMaterialCommand, MaterialUploadCommand};
use crate::bootstrap::AppState;
use crate::repositories::AudioUsage;
use axum::{
    extract::{multipart::MultipartRejection, Multipart, Path, Query, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

struct MultipartMaterialFile {
    original_file_name: String,
    content_type: Option<String>,
    bytes: Vec<u8>,
}

pub(super) async fn upload_material(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    multipart: Result<Multipart, MultipartRejection>,
) -> Result<(StatusCode, Json<MaterialResponse>), ScriptApiError> {
    let mut multipart = multipart.map_err(|_| invalid_multipart_request())?;
    let mut file = None;
    let mut file_name = None;
    let mut tags = Vec::new();
    let mut audio_usage = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| invalid_multipart_request())?
    {
        match field.name() {
            Some("file") => {
                let original_file_name = field
                    .file_name()
                    .map(str::to_string)
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        ScriptApiError::MaterialValidation("上传文件缺少文件名".to_string())
                    })?;
                let content_type = field.content_type().map(str::to_string);
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| invalid_multipart_request())?;
                file = Some(MultipartMaterialFile {
                    original_file_name,
                    content_type,
                    bytes: bytes.to_vec(),
                });
            }
            Some("file_name") => {
                file_name = Some(
                    field
                        .text()
                        .await
                        .map_err(|_| invalid_multipart_request())?,
                );
            }
            Some("tags") => {
                let value = field
                    .text()
                    .await
                    .map_err(|_| invalid_multipart_request())?;
                tags = serde_json::from_str::<Vec<String>>(&value).map_err(|_| {
                    ScriptApiError::MaterialValidation("素材标签格式无效".to_string())
                })?;
            }
            Some("audio_usage") => {
                let value = field
                    .text()
                    .await
                    .map_err(|_| invalid_multipart_request())?;
                audio_usage =
                    Some(AudioUsage::try_from(value.trim()).map_err(|_| {
                        ScriptApiError::MaterialValidation("音频用途无效".to_string())
                    })?);
            }
            _ => {}
        }
    }

    let file = file
        .ok_or_else(|| ScriptApiError::MaterialValidation("请选择要上传的素材文件".to_string()))?;
    let fallback_name = std::path::Path::new(&file.original_file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(&file.original_file_name);
    let file_name = normalize_material_name(file_name.as_deref().unwrap_or(fallback_name))
        .map_err(ScriptApiError::MaterialValidation)?;
    let tags = normalize_material_tags(&tags).map_err(ScriptApiError::MaterialValidation)?;
    let material = state
        .material_service()?
        .upload(MaterialUploadCommand {
            project_id,
            original_file_name: file.original_file_name,
            content_type: file.content_type,
            bytes: file.bytes,
            file_name,
            tags,
            audio_usage,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(MaterialResponse::from(material))))
}

pub(super) async fn register_generated_material(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    multipart: Result<Multipart, MultipartRejection>,
) -> Result<(StatusCode, Json<MaterialResponse>), ScriptApiError> {
    let mut multipart = multipart.map_err(|_| invalid_multipart_request())?;
    let mut file = None;
    let mut file_name = None;
    let mut tags = Vec::new();
    let mut generation = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| invalid_multipart_request())?
    {
        match field.name() {
            Some("file") => {
                let original_file_name = field
                    .file_name()
                    .map(str::to_string)
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        ScriptApiError::MaterialValidation("生成素材文件缺少文件名".to_string())
                    })?;
                let content_type = field.content_type().map(str::to_string);
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| invalid_multipart_request())?;
                file = Some(MultipartMaterialFile {
                    original_file_name,
                    content_type,
                    bytes: bytes.to_vec(),
                });
            }
            Some("file_name") => {
                file_name = Some(
                    field
                        .text()
                        .await
                        .map_err(|_| invalid_multipart_request())?,
                );
            }
            Some("tags") => {
                let value = field
                    .text()
                    .await
                    .map_err(|_| invalid_multipart_request())?;
                tags = serde_json::from_str::<Vec<String>>(&value).map_err(|_| {
                    ScriptApiError::MaterialValidation("素材标签格式无效".to_string())
                })?;
            }
            Some("generation") => {
                let value = field
                    .text()
                    .await
                    .map_err(|_| invalid_multipart_request())?;
                generation = Some(
                    serde_json::from_str::<WorkGenerationSnapshotRequest>(&value).map_err(
                        |_| ScriptApiError::MaterialValidation("生成来源快照格式无效".to_string()),
                    )?,
                );
            }
            _ => {}
        }
    }

    let file = file.ok_or_else(|| {
        ScriptApiError::MaterialValidation("请选择要登记的生成素材文件".to_string())
    })?;
    let generation = generation
        .ok_or_else(|| ScriptApiError::MaterialValidation("缺少生成来源快照".to_string()))?
        .into_snapshot()
        .map_err(ScriptApiError::MaterialValidation)?;
    let fallback_name = std::path::Path::new(&file.original_file_name)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(&file.original_file_name);
    let file_name = normalize_material_name(file_name.as_deref().unwrap_or(fallback_name))
        .map_err(ScriptApiError::MaterialValidation)?;
    let tags = normalize_material_tags(&tags).map_err(ScriptApiError::MaterialValidation)?;
    let material = state
        .material_service()?
        .register_generated(GeneratedMaterialCommand {
            project_id,
            original_file_name: file.original_file_name,
            content_type: file.content_type,
            bytes: file.bytes,
            file_name,
            tags,
            generation,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(MaterialResponse::from(material))))
}

fn invalid_multipart_request() -> ScriptApiError {
    ScriptApiError::MaterialValidation("上传请求格式无效".to_string())
}

pub(super) async fn create_material(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    ValidJson(request): ValidJson<MaterialPayloadRequest>,
) -> Result<(StatusCode, Json<MaterialResponse>), ScriptApiError> {
    let input = request
        .into_create_input(project_id)
        .map_err(ScriptApiError::MaterialValidation)?;
    let material = state.material_service()?.create(input).await?;
    Ok((StatusCode::CREATED, Json(MaterialResponse::from(material))))
}

pub(super) async fn list_materials(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<MaterialListQuery>,
) -> Result<Json<MaterialListResponse>, ScriptApiError> {
    let filter = query
        .into_filter()
        .map_err(ScriptApiError::MaterialValidation)?;
    let materials = state.material_service()?.list(project_id, filter).await?;
    Ok(Json(MaterialListResponse {
        materials: materials.into_iter().map(MaterialResponse::from).collect(),
    }))
}

pub(super) async fn get_material(
    State(state): State<AppState>,
    Path(material_id): Path<Uuid>,
) -> Result<Json<MaterialResponse>, ScriptApiError> {
    let material = state.material_service()?.get(material_id).await?;
    Ok(Json(MaterialResponse::from(material)))
}

pub(super) async fn update_material(
    State(state): State<AppState>,
    Path(material_id): Path<Uuid>,
    ValidJson(request): ValidJson<MaterialPayloadRequest>,
) -> Result<Json<MaterialResponse>, ScriptApiError> {
    let command = request
        .into_update_command()
        .map_err(ScriptApiError::MaterialValidation)?;
    let material = state
        .material_service()?
        .update(material_id, command)
        .await?;
    Ok(Json(MaterialResponse::from(material)))
}

pub(super) async fn update_material_status(
    State(state): State<AppState>,
    Path(material_id): Path<Uuid>,
    ValidJson(request): ValidJson<MaterialStatusRequest>,
) -> Result<Json<MaterialResponse>, ScriptApiError> {
    let status = request
        .parse_status()
        .map_err(ScriptApiError::MaterialValidation)?;
    let material = state
        .material_service()?
        .update_status(material_id, status)
        .await?;
    Ok(Json(MaterialResponse::from(material)))
}
