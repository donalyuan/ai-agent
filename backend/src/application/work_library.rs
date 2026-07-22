use crate::repositories::{
    PostgresWorkLibraryRepository, WorkArtifactRecord, WorkDiffConfirmation,
    WorkLibraryRepositoryError, WorkPublicationHandoff, WorkVersionDiffPlanRecord,
    WorkVersionRecord,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::{Component, Path, PathBuf};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

#[derive(Clone, Debug, Default)]
pub struct VersionSnapshotPatches {
    pub input: Option<Value>,
    pub model: Option<Value>,
    pub parameter: Option<Value>,
    pub prompt: Option<Value>,
    pub timeline: Option<Value>,
}

#[derive(Clone, Debug)]
pub struct ValidatedArtifact {
    pub artifact: WorkArtifactRecord,
    pub absolute_path: PathBuf,
}

#[derive(Debug)]
pub enum WorkLibraryApplicationError {
    Repository(WorkLibraryRepositoryError),
    Validation(String),
    ArtifactIntegrity { artifact_id: Uuid, reason: String },
}

impl fmt::Display for WorkLibraryApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Repository(error) => write!(formatter, "{error}"),
            Self::Validation(message) => formatter.write_str(message),
            Self::ArtifactIntegrity {
                artifact_id,
                reason,
            } => {
                write!(formatter, "产物 {artifact_id} 完整性校验失败: {reason}")
            }
        }
    }
}

impl std::error::Error for WorkLibraryApplicationError {}

impl From<WorkLibraryRepositoryError> for WorkLibraryApplicationError {
    fn from(value: WorkLibraryRepositoryError) -> Self {
        Self::Repository(value)
    }
}

#[derive(Clone)]
pub struct WorkLibraryService {
    repository: PostgresWorkLibraryRepository,
    storage_root: PathBuf,
}

impl WorkLibraryService {
    pub fn new(repository: PostgresWorkLibraryRepository, storage_root: PathBuf) -> Self {
        Self {
            repository,
            storage_root,
        }
    }

    pub async fn list_works(
        &self,
        project_id: Uuid,
        archived: bool,
        query: Option<&str>,
    ) -> Result<Value, WorkLibraryApplicationError> {
        Ok(self
            .repository
            .list_works(project_id, archived, query)
            .await?)
    }

    pub async fn details(&self, work_id: Uuid) -> Result<Value, WorkLibraryApplicationError> {
        Ok(self.repository.work_details(work_id).await?)
    }

    pub async fn derive(
        &self,
        source_id: Uuid,
        patches: VersionSnapshotPatches,
    ) -> Result<WorkVersionRecord, WorkLibraryApplicationError> {
        let (record, _) = self
            .repository
            .derive_version(
                source_id,
                "edit",
                [
                    &patches.input,
                    &patches.model,
                    &patches.parameter,
                    &patches.prompt,
                    &patches.timeline,
                ],
            )
            .await?;
        Ok(record)
    }

    pub async fn regenerate(
        &self,
        source_id: Uuid,
    ) -> Result<WorkVersionRecord, WorkLibraryApplicationError> {
        let patches = VersionSnapshotPatches::default();
        let (record, _) = self
            .repository
            .derive_version(
                source_id,
                "full_regeneration",
                [
                    &patches.input,
                    &patches.model,
                    &patches.parameter,
                    &patches.prompt,
                    &patches.timeline,
                ],
            )
            .await?;
        Ok(record)
    }

    pub async fn analyze_diff(
        &self,
        draft_id: Uuid,
    ) -> Result<WorkVersionDiffPlanRecord, WorkLibraryApplicationError> {
        Ok(self.repository.analyze_diff(draft_id).await?)
    }

    pub async fn confirm_diff(
        &self,
        diff_id: Uuid,
        key: &str,
    ) -> Result<WorkDiffConfirmation, WorkLibraryApplicationError> {
        require_key(key, "差异确认")?;
        Ok(self.repository.confirm_diff(diff_id, key.trim()).await?)
    }

    pub async fn delete_blank(&self, work_id: Uuid) -> Result<(), WorkLibraryApplicationError> {
        Ok(self.repository.delete_blank_work(work_id).await?)
    }

    pub async fn archive(&self, work_id: Uuid) -> Result<Value, WorkLibraryApplicationError> {
        Ok(self.repository.set_archived(work_id, true).await?)
    }

    pub async fn restore(&self, work_id: Uuid) -> Result<Value, WorkLibraryApplicationError> {
        Ok(self.repository.set_archived(work_id, false).await?)
    }

    pub async fn validate_artifact(
        &self,
        artifact_id: Uuid,
    ) -> Result<ValidatedArtifact, WorkLibraryApplicationError> {
        let artifact = self.repository.artifact(artifact_id).await?;
        if artifact.version_status != "completed" {
            return Err(WorkLibraryApplicationError::Validation(
                "只有完成版本的产物可以下载".into(),
            ));
        }
        self.validate_record(artifact).await
    }

    pub async fn download_manifest(
        &self,
        version_id: Uuid,
    ) -> Result<Value, WorkLibraryApplicationError> {
        let records = self.repository.version_artifacts(version_id).await?;
        let mut artifacts = Vec::with_capacity(records.len());
        for record in records {
            let status = match self.validate_record(record.clone()).await {
                Ok(_) => "available",
                Err(WorkLibraryApplicationError::ArtifactIntegrity { reason, .. })
                    if reason.contains("不存在") =>
                {
                    "missing"
                }
                Err(WorkLibraryApplicationError::ArtifactIntegrity { .. }) => "corrupt",
                Err(error) => return Err(error),
            };
            artifacts.push(json!({"artifact": record, "integrity_status": status}));
        }
        Ok(json!({"work_version_id": version_id, "artifacts": artifacts}))
    }

    pub async fn production_package(
        &self,
        version_id: Uuid,
    ) -> Result<Value, WorkLibraryApplicationError> {
        for artifact in self.repository.version_artifacts(version_id).await? {
            self.validate_record(artifact).await?;
        }
        let package = self.repository.version_package(version_id).await?;
        Ok(redact_sensitive_json(package))
    }

    pub async fn create_handoff(
        &self,
        version_id: Uuid,
        key: &str,
    ) -> Result<WorkPublicationHandoff, WorkLibraryApplicationError> {
        require_key(key, "发布草稿交接")?;
        let artifacts = self.repository.version_artifacts(version_id).await?;
        let final_video = artifacts
            .iter()
            .find(|artifact| artifact.role == "final_video")
            .ok_or_else(|| {
                WorkLibraryApplicationError::Validation("完成版本缺少成片 artifact".into())
            })?;
        self.validate_record(final_video.clone()).await?;
        if let Some(subtitle) = artifacts
            .iter()
            .find(|artifact| artifact.role == "subtitle")
        {
            self.validate_record(subtitle.clone()).await?;
        }
        Ok(self
            .repository
            .create_handoff(version_id, key.trim())
            .await?)
    }

    async fn validate_record(
        &self,
        artifact: WorkArtifactRecord,
    ) -> Result<ValidatedArtifact, WorkLibraryApplicationError> {
        let relative = Path::new(&artifact.storage_path);
        if relative.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(integrity_error(&artifact, "存储路径越出作品存储根目录"));
        }
        let root = tokio::fs::canonicalize(&self.storage_root)
            .await
            .map_err(|_| integrity_error(&artifact, "存储根目录不存在"))?;
        let absolute = tokio::fs::canonicalize(root.join(relative))
            .await
            .map_err(|_| integrity_error(&artifact, "登记文件不存在"))?;
        if !absolute.starts_with(&root) {
            return Err(integrity_error(&artifact, "存储路径越出作品存储根目录"));
        }
        let metadata = tokio::fs::metadata(&absolute)
            .await
            .map_err(|_| integrity_error(&artifact, "登记文件不存在"))?;
        if metadata.len() != artifact.size_bytes as u64 {
            return Err(integrity_error(&artifact, "文件大小与登记值不一致"));
        }
        let mut file = tokio::fs::File::open(&absolute)
            .await
            .map_err(|_| integrity_error(&artifact, "登记文件无法读取"))?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            let read = file
                .read(&mut buffer)
                .await
                .map_err(|_| integrity_error(&artifact, "读取文件时失败"))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        let actual = format!("{:x}", hasher.finalize());
        if actual != artifact.sha256 {
            return Err(integrity_error(&artifact, "SHA-256 与登记值不一致"));
        }
        Ok(ValidatedArtifact {
            artifact,
            absolute_path: absolute,
        })
    }
}

fn require_key(key: &str, operation: &str) -> Result<(), WorkLibraryApplicationError> {
    if key.trim().is_empty() {
        return Err(WorkLibraryApplicationError::Validation(format!(
            "{operation}必须提供 Idempotency-Key"
        )));
    }
    Ok(())
}

fn integrity_error(artifact: &WorkArtifactRecord, reason: &str) -> WorkLibraryApplicationError {
    WorkLibraryApplicationError::ArtifactIntegrity {
        artifact_id: artifact.id,
        reason: reason.into(),
    }
}

fn redact_sensitive_json(value: Value) -> Value {
    match value {
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .filter_map(|(key, value)| {
                    let normalized = key.to_ascii_lowercase().replace(['-', '_'], "");
                    let sensitive = ["cookie", "credentials", "headers"]
                        .contains(&normalized.as_str())
                        || [
                            "apikey",
                            "authorization",
                            "token",
                            "secret",
                            "password",
                            "privatekey",
                        ]
                        .iter()
                        .any(|suffix| normalized.ends_with(suffix));
                    (!sensitive).then(|| (key, redact_sensitive_json(value)))
                })
                .collect(),
        ),
        Value::Array(values) => {
            Value::Array(values.into_iter().map(redact_sensitive_json).collect())
        }
        value => value,
    }
}
