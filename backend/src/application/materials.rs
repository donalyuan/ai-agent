//! 素材 CRUD 用例，统一处理项目边界与素材归属。

use crate::repositories::{
    CreateMaterialInput, Material, MaterialListFilter, MaterialRepository, MaterialRepositoryError,
    MaterialStatus, MaterialType, PostgresMaterialRepository, PostgresProjectRepository,
    ProjectRepository, ProjectRepositoryError, UpdateMaterialInput,
};
use serde_json::Value;
use std::fmt;
use uuid::Uuid;

#[derive(Clone)]
/// 在项目边界内执行素材创建、查询、更新和状态变更。
pub struct MaterialService {
    project_repository: PostgresProjectRepository,
    material_repository: PostgresMaterialRepository,
}

impl MaterialService {
    pub fn new(
        project_repository: PostgresProjectRepository,
        material_repository: PostgresMaterialRepository,
    ) -> Self {
        Self {
            project_repository,
            material_repository,
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
                    material_type: command.material_type,
                    file_url: command.file_url,
                    file_name: command.file_name,
                    thumbnail_url: command.thumbnail_url,
                    tags: command.tags,
                    metadata: command.metadata,
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
    pub material_type: MaterialType,
    pub file_url: String,
    pub thumbnail_url: Option<String>,
    pub file_name: String,
    pub tags: Vec<String>,
    pub metadata: Value,
}

#[derive(Debug)]
pub enum MaterialApplicationError {
    ProjectRepository(ProjectRepositoryError),
    MaterialRepository(MaterialRepositoryError),
    ProjectNotFound(Uuid),
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

impl fmt::Display for MaterialApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectRepository(error) => write!(formatter, "{error}"),
            Self::MaterialRepository(error) => write!(formatter, "{error}"),
            Self::ProjectNotFound(project_id) => {
                write!(formatter, "project not found: {project_id}")
            }
        }
    }
}

impl std::error::Error for MaterialApplicationError {}
