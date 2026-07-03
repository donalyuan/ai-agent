use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub positioning: String,
    pub description: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateProjectInput {
    pub name: String,
    pub positioning: String,
    pub description: String,
}

#[derive(Clone)]
pub struct PostgresProjectRepository {
    pool: PgPool,
}

impl PostgresProjectRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
pub trait ProjectRepository: Send + Sync {
    async fn project_exists(&self, project_id: Uuid) -> Result<bool, ProjectRepositoryError>;

    async fn create_project(
        &self,
        input: CreateProjectInput,
    ) -> Result<Project, ProjectRepositoryError>;

    async fn list_projects(&self) -> Result<Vec<Project>, ProjectRepositoryError>;
}

#[async_trait]
impl ProjectRepository for PostgresProjectRepository {
    async fn project_exists(&self, project_id: Uuid) -> Result<bool, ProjectRepositoryError> {
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1)")
            .bind(project_id)
            .fetch_one(&self.pool)
            .await
            .map_err(ProjectRepositoryError::from)
    }

    async fn create_project(
        &self,
        input: CreateProjectInput,
    ) -> Result<Project, ProjectRepositoryError> {
        let row = sqlx::query(
            r#"
            INSERT INTO projects (name, positioning, description)
            VALUES ($1, $2, $3)
            RETURNING id, name, positioning, description, status, created_at, updated_at
            "#,
        )
        .bind(input.name)
        .bind(input.positioning)
        .bind(input.description)
        .fetch_one(&self.pool)
        .await
        .map_err(ProjectRepositoryError::from)?;

        Ok(project_from_row(row))
    }

    async fn list_projects(&self) -> Result<Vec<Project>, ProjectRepositoryError> {
        let projects = sqlx::query(
            r#"
            SELECT id, name, positioning, description, status, created_at, updated_at
            FROM projects
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(ProjectRepositoryError::from)?
        .into_iter()
        .map(project_from_row)
        .collect();

        Ok(projects)
    }
}

fn project_from_row(row: sqlx::postgres::PgRow) -> Project {
    Project {
        id: row.get("id"),
        name: row.get("name"),
        positioning: row.get("positioning"),
        description: row.get("description"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

#[derive(Debug)]
pub enum ProjectRepositoryError {
    Storage(String),
}

impl From<sqlx::Error> for ProjectRepositoryError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

impl fmt::Display for ProjectRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(message) => write!(formatter, "project storage error: {message}"),
        }
    }
}

impl std::error::Error for ProjectRepositoryError {}
