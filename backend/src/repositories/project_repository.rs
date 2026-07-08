use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Row};
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AccountStrategyProfile {
    #[serde(default)]
    pub target_audience: String,
    #[serde(default)]
    pub content_pillars: Vec<String>,
    #[serde(default)]
    pub tone_style: String,
    #[serde(default)]
    pub forbidden_topics: Vec<String>,
    #[serde(default)]
    pub reference_accounts: Vec<String>,
    #[serde(default)]
    pub topic_preferences: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub positioning: String,
    pub description: String,
    pub strategy_profile: AccountStrategyProfile,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateProjectInput {
    pub name: String,
    pub positioning: String,
    pub description: String,
    pub strategy_profile: AccountStrategyProfile,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateProjectStrategyProfileInput {
    pub name: String,
    pub positioning: String,
    pub description: String,
    pub strategy_profile: AccountStrategyProfile,
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

    async fn get_project(&self, project_id: Uuid) -> Result<Project, ProjectRepositoryError>;

    async fn create_project(
        &self,
        input: CreateProjectInput,
    ) -> Result<Project, ProjectRepositoryError>;

    async fn update_strategy_profile(
        &self,
        project_id: Uuid,
        input: UpdateProjectStrategyProfileInput,
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

    async fn get_project(&self, project_id: Uuid) -> Result<Project, ProjectRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT id, name, positioning, description, strategy_profile, status, created_at, updated_at
            FROM projects
            WHERE id = $1
            "#,
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(ProjectRepositoryError::from)?
        .ok_or(ProjectRepositoryError::NotFound(project_id))?;

        project_from_row(row)
    }

    async fn create_project(
        &self,
        input: CreateProjectInput,
    ) -> Result<Project, ProjectRepositoryError> {
        let row = sqlx::query(
            r#"
            INSERT INTO projects (name, positioning, description, strategy_profile)
            VALUES ($1, $2, $3, $4)
            RETURNING id, name, positioning, description, strategy_profile, status, created_at, updated_at
            "#,
        )
        .bind(input.name)
        .bind(input.positioning)
        .bind(input.description)
        .bind(strategy_profile_value(input.strategy_profile)?)
        .fetch_one(&self.pool)
        .await
        .map_err(ProjectRepositoryError::from)?;

        project_from_row(row)
    }

    async fn update_strategy_profile(
        &self,
        project_id: Uuid,
        input: UpdateProjectStrategyProfileInput,
    ) -> Result<Project, ProjectRepositoryError> {
        let row = sqlx::query(
            r#"
            UPDATE projects
            SET
                name = $2,
                positioning = $3,
                description = $4,
                strategy_profile = $5,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, name, positioning, description, strategy_profile, status, created_at, updated_at
            "#,
        )
        .bind(project_id)
        .bind(input.name)
        .bind(input.positioning)
        .bind(input.description)
        .bind(strategy_profile_value(input.strategy_profile)?)
        .fetch_optional(&self.pool)
        .await
        .map_err(ProjectRepositoryError::from)?
        .ok_or(ProjectRepositoryError::NotFound(project_id))?;

        project_from_row(row)
    }

    async fn list_projects(&self) -> Result<Vec<Project>, ProjectRepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, positioning, description, strategy_profile, status, created_at, updated_at
            FROM projects
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(ProjectRepositoryError::from)?;

        let projects = rows
            .into_iter()
            .map(project_from_row)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(projects)
    }
}

fn project_from_row(row: sqlx::postgres::PgRow) -> Result<Project, ProjectRepositoryError> {
    let strategy_profile: Value = row.get("strategy_profile");
    Ok(Project {
        id: row.get("id"),
        name: row.get("name"),
        positioning: row.get("positioning"),
        description: row.get("description"),
        strategy_profile: serde_json::from_value(strategy_profile)
            .map_err(|error| ProjectRepositoryError::Storage(error.to_string()))?,
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn strategy_profile_value(
    strategy_profile: AccountStrategyProfile,
) -> Result<Value, ProjectRepositoryError> {
    serde_json::to_value(strategy_profile)
        .map_err(|error| ProjectRepositoryError::Storage(error.to_string()))
}

#[derive(Debug)]
pub enum ProjectRepositoryError {
    NotFound(Uuid),
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
            Self::NotFound(project_id) => write!(formatter, "project not found: {project_id}"),
            Self::Storage(message) => write!(formatter, "project storage error: {message}"),
        }
    }
}

impl std::error::Error for ProjectRepositoryError {}
