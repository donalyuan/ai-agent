//! 素材 CRUD 用例，统一处理项目边界与素材归属。

use super::material_upload::{
    inspect_upload, probe_media, LocalMaterialStorage, UploadValidationError,
};
use crate::repositories::{
    CreateMaterialInput, Material, MaterialListFilter, MaterialRepository, MaterialRepositoryError,
    MaterialStatus, MaterialType, PostgresMaterialRepository, PostgresProjectRepository,
    ProjectRepository, ProjectRepositoryError, UpdateMaterialInput,
};
use serde_json::{json, Map, Value};
use std::fmt;
use uuid::Uuid;

#[derive(Clone)]
/// 在项目边界内执行素材创建、查询、更新和状态变更。
pub struct MaterialService {
    project_repository: PostgresProjectRepository,
    material_repository: PostgresMaterialRepository,
    storage: LocalMaterialStorage,
}

impl MaterialService {
    pub fn new(
        project_repository: PostgresProjectRepository,
        material_repository: PostgresMaterialRepository,
        asset_storage_root: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            project_repository,
            material_repository,
            storage: LocalMaterialStorage::new(asset_storage_root),
        }
    }

    pub async fn create(
        &self,
        input: CreateMaterialInput,
    ) -> Result<Material, MaterialApplicationError> {
        self.ensure_project_exists(input.project_id).await?;
        self.material_repository
            .create_material(input)
            .await
            .map_err(Into::into)
    }

    pub async fn list(
        &self,
        project_id: Uuid,
        filter: MaterialListFilter,
    ) -> Result<Vec<Material>, MaterialApplicationError> {
        self.ensure_project_exists(project_id).await?;
        self.material_repository
            .list_materials(project_id, filter)
            .await
            .map_err(Into::into)
    }

    pub async fn upload(
        &self,
        command: MaterialUploadCommand,
    ) -> Result<Material, MaterialApplicationError> {
        self.ensure_project_exists(command.project_id).await?;
        let detected = inspect_upload(
            &command.original_file_name,
            command.content_type.as_deref(),
            &command.bytes,
        )?;
        let stored = self
            .storage
            .store(
                command.project_id,
                Uuid::new_v4(),
                &detected.extension,
                &command.bytes,
            )
            .await
            .map_err(|error| MaterialApplicationError::UploadStorage(error.to_string()))?;

        let probe = if matches!(
            detected.material_type,
            MaterialType::Video | MaterialType::Audio
        ) {
            match probe_media(&stored.absolute_path).await {
                Ok(probe) => Some(probe),
                Err(error) => {
                    let _ = self.storage.remove(&stored).await;
                    return Err(error.into());
                }
            }
        } else {
            None
        };

        let mut metadata = Map::from_iter([
            ("source".to_string(), json!("user_upload")),
            ("storage_provider".to_string(), json!("local")),
            ("mime_type".to_string(), json!(detected.mime_type)),
            ("format".to_string(), json!(detected.format)),
            ("file_size_bytes".to_string(), json!(command.bytes.len())),
        ]);
        if let Some(width) = detected
            .width
            .or_else(|| probe.as_ref().and_then(|value| value.width))
        {
            metadata.insert("width".to_string(), json!(width));
        }
        if let Some(height) = detected
            .height
            .or_else(|| probe.as_ref().and_then(|value| value.height))
        {
            metadata.insert("height".to_string(), json!(height));
        }
        if let Some(duration_sec) = probe.as_ref().and_then(|value| value.duration_sec) {
            metadata.insert("duration_sec".to_string(), json!(duration_sec));
        }
        if detected.material_type == MaterialType::Subtitle {
            metadata.insert("subtitle_format".to_string(), json!(detected.extension));
        }

        let input = CreateMaterialInput {
            project_id: command.project_id,
            material_type: detected.material_type,
            file_url: stored.public_url.clone(),
            file_name: command.file_name,
            thumbnail_url: None,
            tags: command.tags,
            metadata: Value::Object(metadata),
        };
        match self.material_repository.create_material(input).await {
            Ok(material) => Ok(material),
            Err(error) => {
                let _ = self.storage.remove(&stored).await;
                Err(error.into())
            }
        }
    }

    pub async fn get(&self, material_id: Uuid) -> Result<Material, MaterialApplicationError> {
        self.material_repository
            .get_material(material_id)
            .await
            .map_err(Into::into)
    }

    pub async fn update(
        &self,
        material_id: Uuid,
        command: MaterialUpdateCommand,
    ) -> Result<Material, MaterialApplicationError> {
        let current = self.material_repository.get_material(material_id).await?;
        self.material_repository
            .update_material(
                material_id,
                UpdateMaterialInput {
                    project_id: current.project_id,
                    material_type: current.material_type,
                    file_url: current.file_url,
                    file_name: command.file_name,
                    thumbnail_url: current.thumbnail_url,
                    tags: command.tags,
                    metadata: current.metadata,
                },
            )
            .await
            .map_err(Into::into)
    }

    pub async fn update_status(
        &self,
        material_id: Uuid,
        status: MaterialStatus,
    ) -> Result<Material, MaterialApplicationError> {
        self.material_repository
            .update_material_status(material_id, status)
            .await
            .map_err(Into::into)
    }

    async fn ensure_project_exists(
        &self,
        project_id: Uuid,
    ) -> Result<(), MaterialApplicationError> {
        if self.project_repository.project_exists(project_id).await? {
            Ok(())
        } else {
            Err(MaterialApplicationError::ProjectNotFound(project_id))
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MaterialUpdateCommand {
    pub file_name: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MaterialUploadCommand {
    pub project_id: Uuid,
    pub original_file_name: String,
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub tags: Vec<String>,
}

#[derive(Debug)]
pub enum MaterialApplicationError {
    ProjectRepository(ProjectRepositoryError),
    MaterialRepository(MaterialRepositoryError),
    ProjectNotFound(Uuid),
    UploadValidation(UploadValidationError),
    UploadStorage(String),
}

impl From<ProjectRepositoryError> for MaterialApplicationError {
    fn from(error: ProjectRepositoryError) -> Self {
        Self::ProjectRepository(error)
    }
}

impl From<MaterialRepositoryError> for MaterialApplicationError {
    fn from(error: MaterialRepositoryError) -> Self {
        Self::MaterialRepository(error)
    }
}

impl From<UploadValidationError> for MaterialApplicationError {
    fn from(error: UploadValidationError) -> Self {
        Self::UploadValidation(error)
    }
}

impl fmt::Display for MaterialApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectRepository(error) => write!(formatter, "{error}"),
            Self::MaterialRepository(error) => write!(formatter, "{error}"),
            Self::ProjectNotFound(project_id) => {
                write!(formatter, "project not found: {project_id}")
            }
            Self::UploadValidation(error) => write!(formatter, "{error}"),
            Self::UploadStorage(message) => {
                write!(formatter, "material upload storage error: {message}")
            }
        }
    }
}

impl std::error::Error for MaterialApplicationError {}
