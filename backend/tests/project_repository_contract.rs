use async_trait::async_trait;
use novex_api::repositories::{ProjectRepository, ProjectRepositoryError};
use std::collections::HashSet;
use uuid::Uuid;

struct MemoryProjectRepository {
    existing_project_ids: HashSet<Uuid>,
}

#[async_trait]
impl ProjectRepository for MemoryProjectRepository {
    async fn project_exists(&self, project_id: Uuid) -> Result<bool, ProjectRepositoryError> {
        Ok(self.existing_project_ids.contains(&project_id))
    }
}

#[tokio::test]
async fn project_repository_reports_project_existence() {
    let project_id = Uuid::new_v4();
    let repository = MemoryProjectRepository {
        existing_project_ids: HashSet::from([project_id]),
    };

    assert!(repository.project_exists(project_id).await.unwrap());
    assert!(!repository.project_exists(Uuid::new_v4()).await.unwrap());
}
