//! 音色目录同步与缓存读取用例，供应商调用由 Worker 执行。

use crate::repositories::{
    PostgresVoiceCatalogRepository, VoiceCatalogRepositoryError, VoiceCatalogSnapshot,
    VoiceCatalogSync,
};
use uuid::Uuid;

#[derive(Clone)]
pub struct VoiceCatalogService {
    repository: PostgresVoiceCatalogRepository,
}

impl VoiceCatalogService {
    pub fn new(repository: PostgresVoiceCatalogRepository) -> Self {
        Self { repository }
    }

    pub async fn request_sync(
        &self,
        model_id: Uuid,
        trigger_source: &str,
    ) -> Result<(VoiceCatalogSync, bool), VoiceCatalogRepositoryError> {
        self.repository.request_sync(model_id, trigger_source).await
    }

    pub async fn catalog(
        &self,
        model_id: Uuid,
        include_unavailable: bool,
    ) -> Result<VoiceCatalogSnapshot, VoiceCatalogRepositoryError> {
        self.repository.catalog(model_id, include_unavailable).await
    }
}
