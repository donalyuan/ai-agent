use async_trait::async_trait;
use chrono::Utc;
use novex_api::repositories::{
    AccountStrategyProfile, CreateProjectInput, Project, ProjectRepository, ProjectRepositoryError,
    UpdateProjectStrategyProfileInput,
};
use std::cmp::Reverse;
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

    async fn get_project(&self, project_id: Uuid) -> Result<Project, ProjectRepositoryError> {
        self.projects
            .lock()
            .unwrap()
            .iter()
            .find(|project| project.id == project_id)
            .cloned()
            .ok_or(ProjectRepositoryError::NotFound(project_id))
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
            strategy_profile: input.strategy_profile,
            status: "active".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        self.projects.lock().unwrap().push(project.clone());
        Ok(project)
    }

    async fn update_strategy_profile(
        &self,
        project_id: Uuid,
        input: UpdateProjectStrategyProfileInput,
    ) -> Result<Project, ProjectRepositoryError> {
        let mut projects = self.projects.lock().unwrap();
        let project = projects
            .iter_mut()
            .find(|project| project.id == project_id)
            .ok_or(ProjectRepositoryError::NotFound(project_id))?;
        project.name = input.name;
        project.positioning = input.positioning;
        project.description = input.description;
        project.strategy_profile = input.strategy_profile;
        project.updated_at = Utc::now();
        Ok(project.clone())
    }

    async fn list_projects(&self) -> Result<Vec<Project>, ProjectRepositoryError> {
        let mut projects = self.projects.lock().unwrap().clone();
        projects.sort_by_key(|project| Reverse(project.created_at));
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
            strategy_profile: AccountStrategyProfile::default(),
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
            strategy_profile: AccountStrategyProfile::default(),
        })
        .await
        .unwrap();
    let second = repository
        .create_project(CreateProjectInput {
            name: "效率工具号".to_string(),
            positioning: "办公效率账号".to_string(),
            description: "讲解自动化工具".to_string(),
            strategy_profile: AccountStrategyProfile {
                target_audience: "内容运营负责人".to_string(),
                content_pillars: vec!["AI 工具".to_string()],
                tone_style: "直接清晰".to_string(),
                forbidden_topics: vec!["夸大收益".to_string()],
                reference_accounts: vec!["参考账号A".to_string()],
                topic_preferences: "优先教程".to_string(),
            },
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

#[tokio::test]
async fn project_repository_updates_strategy_profile_without_touching_other_projects() {
    let repository = MemoryProjectRepository {
        projects: Mutex::new(Vec::new()),
    };
    let target = repository
        .create_project(CreateProjectInput {
            name: "科技博主".to_string(),
            positioning: "科技知识账号".to_string(),
            description: "面向程序员的知识短视频".to_string(),
            strategy_profile: AccountStrategyProfile::default(),
        })
        .await
        .unwrap();
    let other = repository
        .create_project(CreateProjectInput {
            name: "财经号".to_string(),
            positioning: "财经知识账号".to_string(),
            description: "面向新手投资者".to_string(),
            strategy_profile: AccountStrategyProfile::default(),
        })
        .await
        .unwrap();

    let profile = AccountStrategyProfile {
        target_audience: "内容运营负责人".to_string(),
        content_pillars: vec!["AI 工具".to_string(), "内容生产".to_string()],
        tone_style: "直接清晰".to_string(),
        forbidden_topics: vec!["夸大收益".to_string()],
        reference_accounts: vec!["参考账号A".to_string()],
        topic_preferences: "优先教程和案例".to_string(),
    };
    let updated = repository
        .update_strategy_profile(
            target.id,
            UpdateProjectStrategyProfileInput {
                name: "AI 工具账号".to_string(),
                positioning: "AI 工具教程账号".to_string(),
                description: "面向内容运营负责人的短视频".to_string(),
                strategy_profile: profile.clone(),
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.name, "AI 工具账号");
    assert_eq!(updated.strategy_profile, profile);
    assert_eq!(
        repository.get_project(other.id).await.unwrap().name,
        "财经号"
    );
}
