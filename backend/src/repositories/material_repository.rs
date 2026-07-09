use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{postgres::PgRow, PgPool, Row};
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterialType {
    Video,
    Image,
    Audio,
    Subtitle,
}

impl MaterialType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Image => "image",
            Self::Audio => "audio",
            Self::Subtitle => "subtitle",
        }
    }
}

impl TryFrom<&str> for MaterialType {
    type Error = MaterialParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "video" => Ok(Self::Video),
            "image" => Ok(Self::Image),
            "audio" => Ok(Self::Audio),
            "subtitle" => Ok(Self::Subtitle),
            other => Err(MaterialParseError::MaterialType(other.to_string())),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterialStatus {
    Active,
    Archived,
}

impl MaterialStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }
}

impl TryFrom<&str> for MaterialStatus {
    type Error = MaterialParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "active" => Ok(Self::Active),
            "archived" => Ok(Self::Archived),
            other => Err(MaterialParseError::Status(other.to_string())),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MaterialStatusFilter {
    #[default]
    Active,
    Archived,
    All,
}

impl MaterialStatusFilter {
    fn as_optional_str(self) -> Option<&'static str> {
        match self {
            Self::Active => Some("active"),
            Self::Archived => Some("archived"),
            Self::All => None,
        }
    }
}

impl TryFrom<&str> for MaterialStatusFilter {
    type Error = MaterialParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "active" => Ok(Self::Active),
            "archived" => Ok(Self::Archived),
            "all" => Ok(Self::All),
            other => Err(MaterialParseError::StatusFilter(other.to_string())),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub id: Uuid,
    pub project_id: Uuid,
    pub material_type: MaterialType,
    pub file_url: String,
    pub file_name: String,
    pub thumbnail_url: Option<String>,
    pub tags: Vec<String>,
    pub metadata: Value,
    pub usage_count: i32,
    pub status: MaterialStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaterialListFilter {
    pub material_type: Option<MaterialType>,
    pub status: MaterialStatusFilter,
    pub q: Option<String>,
    pub tag: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateMaterialInput {
    pub project_id: Uuid,
    pub material_type: MaterialType,
    pub file_url: String,
    pub file_name: String,
    pub thumbnail_url: Option<String>,
    pub tags: Vec<String>,
    pub metadata: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateMaterialInput {
    pub project_id: Uuid,
    pub material_type: MaterialType,
    pub file_url: String,
    pub file_name: String,
    pub thumbnail_url: Option<String>,
    pub tags: Vec<String>,
    pub metadata: Value,
}

#[derive(Clone)]
pub struct PostgresMaterialRepository {
    pool: PgPool,
}

impl PostgresMaterialRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn ensure_project_exists(&self, project_id: Uuid) -> Result<(), MaterialRepositoryError> {
        let exists =
            sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1)")
                .bind(project_id)
                .fetch_one(&self.pool)
                .await
                .map_err(MaterialRepositoryError::from)?;
        if exists {
            Ok(())
        } else {
            Err(MaterialRepositoryError::ProjectNotFound(project_id))
        }
    }
}

#[async_trait]
pub trait MaterialRepository: Send + Sync {
    async fn create_material(
        &self,
        input: CreateMaterialInput,
    ) -> Result<Material, MaterialRepositoryError>;

    async fn get_material(&self, material_id: Uuid) -> Result<Material, MaterialRepositoryError>;

    async fn list_materials(
        &self,
        project_id: Uuid,
        filter: MaterialListFilter,
    ) -> Result<Vec<Material>, MaterialRepositoryError>;

    async fn update_material(
        &self,
        material_id: Uuid,
        input: UpdateMaterialInput,
    ) -> Result<Material, MaterialRepositoryError>;

    async fn update_material_status(
        &self,
        material_id: Uuid,
        status: MaterialStatus,
    ) -> Result<Material, MaterialRepositoryError>;
}

#[async_trait]
impl MaterialRepository for PostgresMaterialRepository {
    async fn create_material(
        &self,
        input: CreateMaterialInput,
    ) -> Result<Material, MaterialRepositoryError> {
        self.ensure_project_exists(input.project_id).await?;
        let metadata = material_metadata(input.metadata, input.thumbnail_url)?;
        let row = sqlx::query(
            r#"
            INSERT INTO materials (
                project_id, material_type, file_url, file_name, tags, metadata, status
            )
            VALUES ($1, $2, $3, $4, $5, $6, 'active')
            RETURNING id, project_id, material_type, file_url, file_name, tags,
                      metadata, usage_count, status, created_at, updated_at
            "#,
        )
        .bind(input.project_id)
        .bind(input.material_type.as_str())
        .bind(input.file_url)
        .bind(input.file_name)
        .bind(input.tags)
        .bind(metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(MaterialRepositoryError::from)?;

        material_from_row(row)
    }

    async fn get_material(&self, material_id: Uuid) -> Result<Material, MaterialRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT id, project_id, material_type, file_url, file_name, tags,
                   metadata, usage_count, status, created_at, updated_at
            FROM materials
            WHERE id = $1
            "#,
        )
        .bind(material_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(MaterialRepositoryError::from)?
        .ok_or(MaterialRepositoryError::MaterialNotFound(material_id))?;

        material_from_row(row)
    }

    async fn list_materials(
        &self,
        project_id: Uuid,
        filter: MaterialListFilter,
    ) -> Result<Vec<Material>, MaterialRepositoryError> {
        self.ensure_project_exists(project_id).await?;
        let material_type = filter.material_type.map(MaterialType::as_str);
        let status = filter.status.as_optional_str();
        let q = filter
            .q
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let tag = filter
            .tag
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let rows = sqlx::query(
            r#"
            SELECT id, project_id, material_type, file_url, file_name, tags,
                   metadata, usage_count, status, created_at, updated_at
            FROM materials
            WHERE project_id = $1
              AND ($2::text IS NULL OR material_type = $2)
              AND ($3::text IS NULL OR status = $3)
              AND ($4::text IS NULL OR file_name ILIKE '%' || $4 || '%' OR file_url ILIKE '%' || $4 || '%')
              AND ($5::text IS NULL OR tags @> ARRAY[$5]::text[])
            ORDER BY updated_at DESC, id DESC
            "#,
        )
        .bind(project_id)
        .bind(material_type)
        .bind(status)
        .bind(q)
        .bind(tag)
        .fetch_all(&self.pool)
        .await
        .map_err(MaterialRepositoryError::from)?;

        rows.into_iter().map(material_from_row).collect()
    }

    async fn update_material(
        &self,
        material_id: Uuid,
        input: UpdateMaterialInput,
    ) -> Result<Material, MaterialRepositoryError> {
        let metadata = material_metadata(input.metadata, input.thumbnail_url)?;
        let row = sqlx::query(
            r#"
            UPDATE materials
            SET material_type = $3,
                file_url = $4,
                file_name = $5,
                tags = $6,
                metadata = $7,
                updated_at = NOW()
            WHERE id = $1
              AND project_id = $2
            RETURNING id, project_id, material_type, file_url, file_name, tags,
                      metadata, usage_count, status, created_at, updated_at
            "#,
        )
        .bind(material_id)
        .bind(input.project_id)
        .bind(input.material_type.as_str())
        .bind(input.file_url)
        .bind(input.file_name)
        .bind(input.tags)
        .bind(metadata)
        .fetch_optional(&self.pool)
        .await
        .map_err(MaterialRepositoryError::from)?
        .ok_or(MaterialRepositoryError::MaterialNotFound(material_id))?;

        material_from_row(row)
    }

    async fn update_material_status(
        &self,
        material_id: Uuid,
        status: MaterialStatus,
    ) -> Result<Material, MaterialRepositoryError> {
        let row = sqlx::query(
            r#"
            UPDATE materials
            SET status = $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, project_id, material_type, file_url, file_name, tags,
                      metadata, usage_count, status, created_at, updated_at
            "#,
        )
        .bind(material_id)
        .bind(status.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(MaterialRepositoryError::from)?
        .ok_or(MaterialRepositoryError::MaterialNotFound(material_id))?;

        material_from_row(row)
    }
}

fn material_metadata(
    metadata: Value,
    thumbnail_url: Option<String>,
) -> Result<Value, MaterialRepositoryError> {
    let mut object = match metadata {
        Value::Object(object) => object,
        _ => {
            return Err(MaterialRepositoryError::Storage(
                "material metadata must be a JSON object".to_string(),
            ))
        }
    };
    object.remove("thumbnail_url");
    if let Some(thumbnail_url) = thumbnail_url.filter(|value| !value.trim().is_empty()) {
        object.insert(
            "thumbnail_url".to_string(),
            Value::String(thumbnail_url.trim().to_string()),
        );
    }
    Ok(Value::Object(object))
}

fn material_from_row(row: PgRow) -> Result<Material, MaterialRepositoryError> {
    let material_type_value: String = row.get("material_type");
    let material_type = MaterialType::try_from(material_type_value.as_str())
        .map_err(|error| MaterialRepositoryError::Storage(error.to_string()))?;
    let status_value: String = row.get("status");
    let status = MaterialStatus::try_from(status_value.as_str())
        .map_err(|error| MaterialRepositoryError::Storage(error.to_string()))?;
    let metadata: Value = row.get("metadata");
    let thumbnail_url = metadata_thumbnail_url(&metadata);

    Ok(Material {
        id: row.get("id"),
        project_id: row.get("project_id"),
        material_type,
        file_url: row.get("file_url"),
        file_name: row.get("file_name"),
        thumbnail_url,
        tags: row.get("tags"),
        metadata,
        usage_count: row.get("usage_count"),
        status,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn metadata_thumbnail_url(metadata: &Value) -> Option<String> {
    let Value::Object(object) = metadata else {
        return None;
    };
    object
        .get("thumbnail_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

#[derive(Debug, Eq, PartialEq)]
pub enum MaterialParseError {
    MaterialType(String),
    Status(String),
    StatusFilter(String),
}

impl fmt::Display for MaterialParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MaterialType(value) => write!(formatter, "unknown material type: {value}"),
            Self::Status(value) => write!(formatter, "unknown material status: {value}"),
            Self::StatusFilter(value) => {
                write!(formatter, "unknown material status filter: {value}")
            }
        }
    }
}

impl std::error::Error for MaterialParseError {}

#[derive(Debug)]
pub enum MaterialRepositoryError {
    MaterialNotFound(Uuid),
    ProjectNotFound(Uuid),
    Storage(String),
}

impl From<sqlx::Error> for MaterialRepositoryError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

impl fmt::Display for MaterialRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MaterialNotFound(material_id) => {
                write!(formatter, "material not found: {material_id}")
            }
            Self::ProjectNotFound(project_id) => {
                write!(formatter, "project not found: {project_id}")
            }
            Self::Storage(message) => write!(formatter, "material storage error: {message}"),
        }
    }
}

impl std::error::Error for MaterialRepositoryError {}
