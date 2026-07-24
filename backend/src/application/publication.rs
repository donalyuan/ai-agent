use crate::application::work_library::{
    ValidatedArtifact, WorkLibraryApplicationError, WorkLibraryService,
};
use crate::domain::publication::{PublicationPlanStatus, PublicationTargetStatus};
use crate::repositories::{
    PostgresPublicationRepository, PublicationPackageRecord, PublicationPlanRecord,
    PublicationRepositoryError, PublicationTargetRecord, SavePublicationTarget,
};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::{Component, Path, PathBuf};
use url::Url;
use uuid::Uuid;

#[derive(Debug)]
pub enum PublicationApplicationError {
    Repository(PublicationRepositoryError),
    WorkLibrary(WorkLibraryApplicationError),
    Validation(String),
}
impl fmt::Display for PublicationApplicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Repository(e) => write!(f, "{e}"),
            Self::WorkLibrary(e) => write!(f, "{e}"),
            Self::Validation(v) => f.write_str(v),
        }
    }
}
impl std::error::Error for PublicationApplicationError {}
impl From<PublicationRepositoryError> for PublicationApplicationError {
    fn from(v: PublicationRepositoryError) -> Self {
        Self::Repository(v)
    }
}
impl From<WorkLibraryApplicationError> for PublicationApplicationError {
    fn from(v: WorkLibraryApplicationError) -> Self {
        Self::WorkLibrary(v)
    }
}

#[derive(Clone)]
pub struct PublicationService {
    repository: PostgresPublicationRepository,
    work_library: WorkLibraryService,
    storage_root: PathBuf,
}
impl PublicationService {
    pub fn new(
        repository: PostgresPublicationRepository,
        work_library: WorkLibraryService,
        storage_root: PathBuf,
    ) -> Self {
        Self {
            repository,
            work_library,
            storage_root,
        }
    }
    pub async fn plan(
        &self,
        handoff_id: Uuid,
    ) -> Result<PublicationPlanRecord, PublicationApplicationError> {
        Ok(self.repository.get_or_create_plan(handoff_id).await?)
    }
    pub async fn list(&self) -> Result<Value, PublicationApplicationError> {
        let mut value = self.repository.list().await?;
        if let Some(items) = value.get_mut("items").and_then(Value::as_array_mut) {
            for item in items {
                decorate_plan(item);
            }
        }
        Ok(value)
    }
    pub async fn details(&self, id: Uuid) -> Result<Value, PublicationApplicationError> {
        let mut value = self.repository.details(id).await?;
        decorate_plan(&mut value);
        Ok(value)
    }
    pub async fn save_target(
        &self,
        plan: Uuid,
        platform: &str,
        revision: Option<i32>,
        key: &str,
        input: SavePublicationTarget,
    ) -> Result<PublicationTargetRecord, PublicationApplicationError> {
        require_key(key)?;
        if !matches!(platform, "douyin" | "xiaohongshu") {
            return Err(PublicationApplicationError::Validation(
                "仅支持 douyin 或 xiaohongshu".into(),
            ));
        }
        if input.tags.as_array().is_none() {
            return Err(PublicationApplicationError::Validation(
                "tags 必须是数组".into(),
            ));
        }
        Ok(self
            .repository
            .save_target(plan, platform, revision, key, input)
            .await?)
    }
    pub async fn handoff(&self, id: Uuid, key: &str) -> Result<Value, PublicationApplicationError> {
        require_key(key)?;
        let target = self.repository.target(id).await?;
        let entrance = match target.platform.as_str() {
            "douyin" => "https://creator.douyin.com/",
            "xiaohongshu" => "https://creator.xiaohongshu.com/",
            _ => return Err(PublicationApplicationError::Validation("未知平台".into())),
        };
        let target = self
            .repository
            .transition(
                id,
                &["ready"],
                "handed_off",
                "handed_off",
                key,
                json!({"official_entrance":entrance}),
            )
            .await?;
        Ok(
            json!({"target":target,"official_entrance":entrance,"publication_confirmation":"manual_required"}),
        )
    }
    pub async fn needs_attention(
        &self,
        id: Uuid,
        key: &str,
    ) -> Result<PublicationTargetRecord, PublicationApplicationError> {
        require_key(key)?;
        Ok(self
            .repository
            .transition(
                id,
                &["handed_off"],
                "needs_attention",
                "needs_attention",
                key,
                json!({}),
            )
            .await?)
    }
    pub async fn cancel(
        &self,
        id: Uuid,
        key: &str,
    ) -> Result<PublicationTargetRecord, PublicationApplicationError> {
        require_key(key)?;
        Ok(self
            .repository
            .transition(
                id,
                &["draft", "ready", "handed_off", "needs_attention"],
                "cancelled",
                "cancelled",
                key,
                json!({}),
            )
            .await?)
    }
    pub async fn publish(
        &self,
        id: Uuid,
        url: &str,
        at: DateTime<Utc>,
        key: &str,
    ) -> Result<PublicationTargetRecord, PublicationApplicationError> {
        require_key(key)?;
        let target = self.repository.target(id).await?;
        validate_result_url(&target.platform, url)?;
        Ok(self
            .repository
            .transition(
                id,
                &["handed_off"],
                "published",
                "published",
                key,
                json!({"published_url":url,"published_at":at,"confirmation":"manual"}),
            )
            .await?)
    }
    pub async fn correct(
        &self,
        id: Uuid,
        url: &str,
        at: DateTime<Utc>,
        key: &str,
    ) -> Result<PublicationTargetRecord, PublicationApplicationError> {
        require_key(key)?;
        let target = self.repository.target(id).await?;
        validate_result_url(&target.platform, url)?;
        Ok(self.repository.correct_result(id, url, at, key).await?)
    }

    pub async fn generate_package(
        &self,
        id: Uuid,
        revision: i32,
        key: &str,
    ) -> Result<PublicationPackageRecord, PublicationApplicationError> {
        require_key(key)?;
        let context = self.repository.package_context(id).await?;
        if context.target.draft_revision != revision {
            return Err(PublicationApplicationError::Validation(
                "草稿 revision 已过期".into(),
            ));
        }
        if context.target.title.trim().is_empty() && context.target.body.trim().is_empty() {
            return Err(PublicationApplicationError::Validation(
                "发布文案不能为空".into(),
            ));
        }
        let video = self
            .work_library
            .validate_artifact(context.final_video_artifact_id)
            .await?;
        let cover = match context.target.cover_artifact_id {
            Some(id) => Some(self.work_library.validate_artifact(id).await?),
            None => None,
        };
        let platform = context.target.platform.clone();
        let rule_version = format!("manual-web-v1-{platform}-2026-07-23");
        let video_name = format!("{}-{}.mp4", safe_name(&context.work_title), platform);
        let cover_name = cover.as_ref().map(|a| {
            format!(
                "{}-{}-封面.{}",
                safe_name(&context.work_title),
                platform,
                Path::new(&a.artifact.file_name)
                    .extension()
                    .and_then(|v| v.to_str())
                    .unwrap_or("jpg")
            )
        });
        let manifest = json!({"schema":"novex-publication-package/v1","platform":platform,"platform_rule_version":rule_version,"rules_verified_at":"2026-07-23","platform_limits_checked":false,"work_id":context.work_id,"work_version_id":context.work_version_id,"publication_target_id":id,"draft_revision":revision,"files":{"video":{"name":video_name,"sha256":video.artifact.sha256,"size_bytes":video.artifact.size_bytes},"cover":cover.as_ref().zip(cover_name.as_ref()).map(|(a,n)|json!({"name":n,"sha256":a.artifact.sha256,"size_bytes":a.artifact.size_bytes}))},"copy":{"title":context.target.title,"body":context.target.body,"tags":context.target.tags},"checklist":["登录目标平台官方创作者中心","手工选择发布包中的文件","核对平台页面中的最终文案和封面","人工确认发布后返回工作台登记作品链接"]});
        let manifest_bytes = serde_json::to_vec_pretty(&manifest)
            .map_err(|e| PublicationApplicationError::Validation(e.to_string()))?;
        let hash = format!("{:x}", Sha256::digest(&manifest_bytes));
        let relative = format!("publication-packages/{id}/{hash}.zip");
        let output = self.storage_root.join(&relative);
        let created_file = tokio::fs::metadata(&output).await.is_err();
        if created_file {
            write_zip(
                output.clone(),
                video,
                video_name,
                cover,
                cover_name,
                context.target.title,
                context.target.body,
                context.target.tags.clone(),
                manifest_bytes,
            )
            .await?;
        }
        match self
            .repository
            .save_package(id, revision, &rule_version, manifest, &hash, &relative, key)
            .await
        {
            Ok(v) => Ok(v),
            Err(e) => {
                if created_file {
                    let _ = tokio::fs::remove_file(output).await;
                }
                Err(e.into())
            }
        }
    }

    pub async fn downloads(&self, id: Uuid) -> Result<Value, PublicationApplicationError> {
        let context = self.repository.package_context(id).await?;
        let package = self.repository.current_package(id).await?;
        Ok(
            json!({"target_id":id,"draft_revision":context.target.draft_revision,"video":{"artifact_id":context.final_video_artifact_id,"download_url":format!("/api/work-artifacts/{}/download",context.final_video_artifact_id)},"cover":context.target.cover_artifact_id.map(|artifact_id|json!({"artifact_id":artifact_id,"download_url":format!("/api/work-artifacts/{artifact_id}/download")})),"package":{"id":package.id,"manifest_sha256":package.manifest_sha256,"download_url":format!("/api/publication-packages/{}/download",package.id)}}),
        )
    }

    pub async fn package_file(
        &self,
        id: Uuid,
    ) -> Result<(PublicationPackageRecord, PathBuf), PublicationApplicationError> {
        let package = self.repository.current_package_by_id(id).await?;
        let relative = Path::new(&package.package_storage_path);
        if relative.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(PublicationApplicationError::Validation(
                "发布包路径非法".into(),
            ));
        }
        let root = tokio::fs::canonicalize(&self.storage_root)
            .await
            .map_err(|_| PublicationApplicationError::Validation("存储根目录不存在".into()))?;
        let path = tokio::fs::canonicalize(root.join(relative))
            .await
            .map_err(|_| PublicationApplicationError::Validation("发布包文件不存在".into()))?;
        if !path.starts_with(root) {
            return Err(PublicationApplicationError::Validation(
                "发布包路径越界".into(),
            ));
        }
        Ok((package, path))
    }
    pub async fn audit(
        &self,
        id: Uuid,
        action: &str,
        key: &str,
    ) -> Result<(), PublicationApplicationError> {
        require_key(key)?;
        self.repository.target(id).await?;
        self.repository
            .record_event(
                id,
                action,
                key,
                json!({"source":"manual_publication_workbench"}),
            )
            .await?;
        Ok(())
    }
}

async fn write_zip(
    output: PathBuf,
    video: ValidatedArtifact,
    video_name: String,
    cover: Option<ValidatedArtifact>,
    cover_name: Option<String>,
    title: String,
    body: String,
    tags: Value,
    manifest: Vec<u8>,
) -> Result<(), PublicationApplicationError> {
    let parent = output.parent().unwrap().to_path_buf();
    tokio::fs::create_dir_all(&parent)
        .await
        .map_err(|e| PublicationApplicationError::Validation(e.to_string()))?;
    let temp = output.with_extension(format!("{}.tmp", Uuid::new_v4()));
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        use std::io::Write;
        let file = std::fs::File::create(&temp).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file(video_name, options).map_err(|e| e.to_string())?;
        let mut source = std::fs::File::open(video.absolute_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut source, &mut zip).map_err(|e| e.to_string())?;
        if let (Some(cover), Some(name)) = (cover, cover_name) {
            zip.start_file(name, options).map_err(|e| e.to_string())?;
            let mut source = std::fs::File::open(cover.absolute_path).map_err(|e| e.to_string())?;
            std::io::copy(&mut source, &mut zip).map_err(|e| e.to_string())?;
        }
        zip.start_file("发布文案.txt", options).map_err(|e| e.to_string())?;
        writeln!(zip, "标题：{title}\n\n{body}\n\n标签：{}", tags.as_array().map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")).unwrap_or_default()).map_err(|e| e.to_string())?;
        zip.start_file("发布检查清单.txt", options).map_err(|e| e.to_string())?;
        zip.write_all("1. 登录目标平台官方创作者中心\n2. 手工选择视频和封面\n3. 核对最终文案\n4. 发布后返回工作台登记官方作品链接\n".as_bytes()).map_err(|e| e.to_string())?;
        zip.start_file("manifest.json", options).map_err(|e| e.to_string())?;
        zip.write_all(&manifest).map_err(|e| e.to_string())?;
        zip.finish().map_err(|e| e.to_string())?;
        std::fs::rename(&temp, &output).map_err(|e| e.to_string())?;
        Ok(())
    }).await.map_err(|e| PublicationApplicationError::Validation(e.to_string()))?
      .map_err(PublicationApplicationError::Validation)
}
fn safe_name(value: &str) -> String {
    let clean = value.replace(['/', '\\', '\r', '\n'], "_");
    if clean.trim().is_empty() {
        "作品".into()
    } else {
        clean
    }
}
fn require_key(key: &str) -> Result<(), PublicationApplicationError> {
    if key.trim().is_empty() {
        Err(PublicationApplicationError::Validation(
            "必须提供 Idempotency-Key".into(),
        ))
    } else {
        Ok(())
    }
}
fn validate_result_url(platform: &str, value: &str) -> Result<(), PublicationApplicationError> {
    let url = Url::parse(value)
        .map_err(|_| PublicationApplicationError::Validation("作品链接格式无效".into()))?;
    if url.scheme() != "https" || url.query().is_some() {
        return Err(PublicationApplicationError::Validation(
            "作品链接必须是无查询参数的 HTTPS 官方链接".into(),
        ));
    }
    let host = url.host_str().unwrap_or("");
    let root = if platform == "douyin" {
        "douyin.com"
    } else {
        "xiaohongshu.com"
    };
    if host != root && !host.ends_with(&format!(".{root}")) {
        return Err(PublicationApplicationError::Validation(
            "作品链接不是目标平台官方域名".into(),
        ));
    }
    Ok(())
}
fn decorate_plan(value: &mut Value) {
    let statuses = value
        .get("targets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|target| target.get("status").and_then(Value::as_str))
        .filter_map(|status| match status {
            "draft" => Some(PublicationTargetStatus::Draft),
            "ready" => Some(PublicationTargetStatus::Ready),
            "handed_off" => Some(PublicationTargetStatus::HandedOff),
            "needs_attention" => Some(PublicationTargetStatus::NeedsAttention),
            "published" => Some(PublicationTargetStatus::Published),
            "cancelled" => Some(PublicationTargetStatus::Cancelled),
            _ => None,
        })
        .collect::<Vec<_>>();
    value["status"] = json!(PublicationPlanStatus::derive(&statuses).as_str());
    if let Some(targets) = value.get_mut("targets").and_then(Value::as_array_mut) {
        for target in targets {
            let overdue = target
                .get("planned_at")
                .and_then(Value::as_str)
                .and_then(|v| v.parse::<DateTime<Utc>>().ok())
                .is_some_and(|at| at < Utc::now())
                && !matches!(
                    target.get("status").and_then(Value::as_str),
                    Some("published" | "cancelled")
                );
            target["overdue"] = json!(overdue);
        }
    }
}
