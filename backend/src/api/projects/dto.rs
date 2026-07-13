use crate::application::projects::normalize_account_strategy_profile;
use crate::repositories::{AccountStrategyProfile, Project};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, Deserialize, PartialEq, Validate)]
pub struct CreateProjectRequest {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    #[serde(default)]
    #[validate(length(max = 500))]
    pub positioning: String,
    #[serde(default)]
    #[validate(length(max = 2000))]
    pub description: String,
    #[serde(default)]
    pub strategy_profile: Option<AccountStrategyProfileRequest>,
}

impl CreateProjectRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("项目名称不能为空".to_string());
        }

        self.validate()
            .map_err(|error| format!("项目参数无效: {error}"))?;
        if let Some(strategy_profile) = &self.strategy_profile {
            strategy_profile.normalize()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UpdateProjectStrategyProfileRequest {
    pub name: String,
    #[serde(default)]
    pub positioning: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub strategy_profile: AccountStrategyProfileRequest,
}

impl UpdateProjectStrategyProfileRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("账号名称不能为空".to_string());
        }
        if self.name.trim().chars().count() > 120 {
            return Err("账号名称不能超过 120 个字符".to_string());
        }
        if self.positioning.trim().chars().count() > 500 {
            return Err("定位摘要不能超过 500 个字符".to_string());
        }
        if self.description.trim().chars().count() > 2_000 {
            return Err("账号描述不能超过 2000 个字符".to_string());
        }
        self.strategy_profile.normalize()?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct StrategyProfileDraftRequest {
    pub model_id: Uuid,
    #[serde(default)]
    pub direction_notes: String,
}

impl StrategyProfileDraftRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        if self.model_id.is_nil() {
            return Err("必须选择文本模型".to_string());
        }
        if self.direction_notes.trim().chars().count() > ACCOUNT_STRATEGY_TEXT_LIMIT {
            return Err("补充方向不能超过 1000 个字符".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StrategyProfileDraftResponse {
    pub draft: AccountStrategyProfile,
    pub draft_summary: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct AccountStrategyProfileRequest {
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

impl AccountStrategyProfileRequest {
    pub fn normalize(&self) -> Result<AccountStrategyProfile, String> {
        normalize_account_strategy_profile(&AccountStrategyProfile {
            target_audience: self.target_audience.clone(),
            content_pillars: self.content_pillars.clone(),
            tone_style: self.tone_style.clone(),
            forbidden_topics: self.forbidden_topics.clone(),
            reference_accounts: self.reference_accounts.clone(),
            topic_preferences: self.topic_preferences.clone(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProjectResponse {
    pub project_id: Uuid,
    pub name: String,
    pub positioning: String,
    pub description: String,
    pub strategy_profile: AccountStrategyProfile,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Project> for ProjectResponse {
    fn from(project: Project) -> Self {
        Self {
            project_id: project.id,
            name: project.name,
            positioning: project.positioning,
            description: project.description,
            strategy_profile: project.strategy_profile,
            status: project.status,
            created_at: project.created_at,
            updated_at: project.updated_at,
        }
    }
}

const ACCOUNT_STRATEGY_TEXT_LIMIT: usize = 1_000;
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProjectListResponse {
    pub projects: Vec<ProjectResponse>,
}
