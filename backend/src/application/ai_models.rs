//! AI 模型管理用例，集中维护默认模型、版本和删除替换规则。

use crate::repositories::{
    AiModel, AiModelListFilter, AiModelRepository, AiModelRepositoryError,
    ChangeAiModelStatusInput, CreateAiModelInput, DeleteAiModelInput, DeleteAiModelOutcome,
    PostgresAiModelRepository, UpdateAiModelInput,
};
use novex_ai_core::{DefinitionRegistry, DefinitionStatus};
use novex_model::{ApiProtocol, ModelType};
use std::fmt;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
/// 管理 AI 模型配置，并保证默认模型切换与版本检查由同一用例边界执行。
pub struct AiModelService {
    repository: PostgresAiModelRepository,
    definitions: Arc<DefinitionRegistry>,
}

impl AiModelService {
    pub fn new(
        repository: PostgresAiModelRepository,
        definitions: Arc<DefinitionRegistry>,
    ) -> Self {
        Self {
            repository,
            definitions,
        }
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
        validate_profile_reference(
            &self.definitions,
            input.model_type,
            input.api_protocol,
            input.context_window,
            input.tokenizer_profile_key.as_deref(),
            input.tokenizer_profile_version.as_deref(),
            input.status == crate::repositories::AiModelStatus::Enabled,
        )?;
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
        validate_profile_reference(
            &self.definitions,
            input.model_type,
            input.api_protocol,
            input.context_window,
            input.tokenizer_profile_key.as_deref(),
            input.tokenizer_profile_version.as_deref(),
            current.status == crate::repositories::AiModelStatus::Enabled,
        )?;
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
        if input.status == crate::repositories::AiModelStatus::Enabled {
            let current = self.repository.get(input.id).await?;
            validate_profile_reference(
                &self.definitions,
                current.model_type,
                current.api_protocol,
                current.context_window,
                current.tokenizer_profile_key.as_deref(),
                current.tokenizer_profile_version.as_deref(),
                true,
            )?;
        }
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

fn validate_profile_reference(
    definitions: &DefinitionRegistry,
    model_type: ModelType,
    protocol: ApiProtocol,
    context_window: Option<i64>,
    profile_key: Option<&str>,
    profile_version: Option<&str>,
    executable: bool,
) -> Result<(), AiModelApplicationError> {
    let complete = context_window.is_some() && profile_key.is_some() && profile_version.is_some();
    if model_type != ModelType::Text {
        return if context_window.is_none() && profile_key.is_none() && profile_version.is_none() {
            Ok(())
        } else {
            Err(AiModelApplicationError::InvalidConfig(
                "非文本模型不得配置 Context window 或 Tokenizer Profile".to_string(),
            ))
        };
    }
    if !complete {
        return if executable {
            Err(AiModelApplicationError::InvalidConfig(
                "enabled 文本模型必须显式配置 context_window 与 Tokenizer Profile key/version"
                    .to_string(),
            ))
        } else {
            Ok(())
        };
    }
    let key = profile_key.expect("complete reference").trim();
    let version = profile_version.expect("complete reference").trim();
    let profile = definitions.tokenizer_profile(key, version).map_err(|_| {
        AiModelApplicationError::InvalidConfig(format!("Tokenizer Profile 不存在：{key}@{version}"))
    })?;
    if matches!(
        profile.status,
        DefinitionStatus::Candidate | DefinitionStatus::Revoked
    ) {
        return Err(AiModelApplicationError::InvalidConfig(format!(
            "Tokenizer Profile 当前不可执行：{key}@{version}"
        )));
    }
    if !profile
        .applicable_protocols
        .iter()
        .any(|item| item == protocol.as_str())
    {
        return Err(AiModelApplicationError::InvalidConfig(format!(
            "Tokenizer Profile 与协议 {} 不兼容",
            protocol.as_str()
        )));
    }
    Ok(())
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
