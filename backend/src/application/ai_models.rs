//! AI 模型管理用例，集中维护默认模型、版本和删除替换规则。

use crate::repositories::{
    AiModel, AiModelListFilter, AiModelRepository, AiModelRepositoryError,
    ChangeAiModelStatusInput, CreateAiModelInput, DeleteAiModelInput, DeleteAiModelOutcome,
    PostgresAiModelRepository, UpdateAiModelInput,
};
use novex_model::ModelType;
use std::fmt;
use uuid::Uuid;

#[derive(Clone)]
/// 管理 AI 模型配置，并保证默认模型切换与版本检查由同一用例边界执行。
pub struct AiModelService {
    repository: PostgresAiModelRepository,
}

impl AiModelService {
    pub fn new(repository: PostgresAiModelRepository) -> Self {
        Self { repository }
    }

    pub async fn list(
        &self,
        filter: AiModelListFilter,
    ) -> Result<Vec<AiModel>, AiModelApplicationError> {
        self.repository.list(filter).await.map_err(Into::into)
    }

    pub async fn get(&self, model_id: Uuid) -> Result<AiModel, AiModelApplicationError> {
        self.repository.get(model_id).await.map_err(Into::into)
    }

    pub async fn create(
        &self,
        input: CreateAiModelInput,
        requested_default: bool,
    ) -> Result<AiModel, AiModelApplicationError> {
        let mut model = self.repository.create(input).await?;
        if requested_default && !model.is_default {
            model = self.repository.set_default(model.id, model.version).await?;
        }
        Ok(model)
    }

    pub async fn update(
        &self,
        model_id: Uuid,
        input: UpdateAiModelInput,
        requested_default: bool,
    ) -> Result<AiModel, AiModelApplicationError> {
        let current = self.repository.get(model_id).await?;
        let same_default_scope = current.model_type == input.model_type
            && (current.model_type != ModelType::Speech
                || current.api_protocol == input.api_protocol);
        if same_default_scope && current.is_default && !requested_default {
            return Err(AiModelApplicationError::InvalidConfig(
                "默认模型只能通过选择替代模型、停用或删除流程取消".to_string(),
            ));
        }

        let mut model = self.repository.update(model_id, input).await?;
        if requested_default && !model.is_default {
            model = self.repository.set_default(model.id, model.version).await?;
        }
        Ok(model)
    }

    pub async fn set_default(
        &self,
        model_id: Uuid,
        version: i64,
    ) -> Result<AiModel, AiModelApplicationError> {
        self.repository
            .set_default(model_id, version)
            .await
            .map_err(Into::into)
    }

    pub async fn change_status(
        &self,
        input: ChangeAiModelStatusInput,
    ) -> Result<AiModel, AiModelApplicationError> {
        self.repository
            .change_status(input)
            .await
            .map_err(Into::into)
    }

    pub async fn delete(
        &self,
        input: DeleteAiModelInput,
    ) -> Result<DeleteAiModelOutcome, AiModelApplicationError> {
        self.repository.delete(input).await.map_err(Into::into)
    }

    pub async fn list_enabled_options(
        &self,
        model_type: ModelType,
    ) -> Result<Vec<AiModel>, AiModelApplicationError> {
        self.repository
            .list_enabled_options(model_type)
            .await
            .map_err(Into::into)
    }
}

#[derive(Debug)]
pub enum AiModelApplicationError {
    Repository(AiModelRepositoryError),
    InvalidConfig(String),
}

impl From<AiModelRepositoryError> for AiModelApplicationError {
    fn from(error: AiModelRepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl fmt::Display for AiModelApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Repository(error) => write!(formatter, "{error}"),
            Self::InvalidConfig(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for AiModelApplicationError {}
