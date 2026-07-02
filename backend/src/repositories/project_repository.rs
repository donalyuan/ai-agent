use async_trait::async_trait;
use sqlx::PgPool;
use std::fmt;
use uuid::Uuid;

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
