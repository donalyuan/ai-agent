use async_trait::async_trait;
use chrono::Utc;
use novex_api::repositories::{
    CreateProjectInput, Project, ProjectRepository, ProjectRepositoryError,
};
use std::sync::Mutex;
use uuid::Uuid;

struct MemoryProjectRepository {
    projects: Mutex<Vec<Project>>,
}

#[async_trait]
impl ProjectRepository for MemoryProjectRepository {
    async fn project_exists(&self, project_id: Uuid) -> Result<bool, ProjectRepositoryError> {
        Ok(self
            .projects
            .lock()
            .unwrap()
            .iter()
            .any(|project| project.id == project_id))
    }

    async fn create_project(
        &self,
        input: CreateProjectInput,
    ) -> Result<Project, ProjectRepositoryError> {
        let project = Project {
            id: Uuid::new_v4(),
            name: input.name,
            positioning: input.positioning,
            description: input.description,
            status: "active".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        self.projects.lock().unwrap().push(project.clone());
        Ok(project)
    }

    async fn list_projects(&self) -> Result<Vec<Project>, ProjectRepositoryError> {
        let mut projects = self.projects.lock().unwrap().clone();
        projects.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(projects)
    }
}

#[tokio::test]
async fn project_repository_reports_project_existence() {
    let project_id = Uuid::new_v4();
    let repository = MemoryProjectRepository {
        projects: Mutex::new(vec![Project {
            id: project_id,
            name: "科技博主".to_string(),
            positioning: "科技知识账号".to_string(),
            description: "用于验证项目存在性".to_string(),
            status: "active".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }]),
    };

    assert!(repository.project_exists(project_id).await.unwrap());
    assert!(!repository.project_exists(Uuid::new_v4()).await.unwrap());
}

#[tokio::test]
async fn project_repository_creates_and_lists_projects_newest_first() {
    let repository = MemoryProjectRepository {
        projects: Mutex::new(Vec::new()),
    };

    let first = repository
        .create_project(CreateProjectInput {
            name: "科技博主".to_string(),
            positioning: "科技知识账号".to_string(),
            description: "面向程序员的知识短视频".to_string(),
        })
        .await
        .unwrap();
    let second = repository
        .create_project(CreateProjectInput {
            name: "效率工具号".to_string(),
            positioning: "办公效率账号".to_string(),
            description: "讲解自动化工具".to_string(),
        })
        .await
        .unwrap();

    assert_ne!(first.id, second.id);
    assert_eq!(first.status, "active");
    assert_eq!(second.positioning, "办公效率账号");

    let listed = repository.list_projects().await.unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].id, second.id);
    assert_eq!(listed[1].id, first.id);
}
