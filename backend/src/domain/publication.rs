//! 人工发布运营的领域状态机；不保存平台凭据，也不控制第三方页面。

use std::fmt;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationPlatform {
    Douyin,
    Xiaohongshu,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationEventType {
    Created,
    DraftUpdated,
    PackageGenerated,
    Downloaded,
    Copied,
    HandedOff,
    NeedsAttention,
    Published,
    ResultCorrected,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationTargetStatus {
    Draft,
    Ready,
    HandedOff,
    NeedsAttention,
    Published,
    Cancelled,
}

impl PublicationTargetStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Ready => "ready",
            Self::HandedOff => "handed_off",
            Self::NeedsAttention => "needs_attention",
            Self::Published => "published",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Published | Self::Cancelled)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationPlanStatus {
    Draft,
    Ready,
    HandedOff,
    NeedsAttention,
    PartiallyPublished,
    Published,
    Cancelled,
}

impl PublicationPlanStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Ready => "ready",
            Self::HandedOff => "handed_off",
            Self::NeedsAttention => "needs_attention",
            Self::PartiallyPublished => "partially_published",
            Self::Published => "published",
            Self::Cancelled => "cancelled",
        }
    }

    /// 计划状态始终由平台目标投影而来，禁止独立写入。
    pub fn derive(targets: &[PublicationTargetStatus]) -> Self {
        if targets.is_empty()
            || targets
                .iter()
                .any(|status| *status == PublicationTargetStatus::Draft)
        {
            return Self::Draft;
        }
        if targets
            .iter()
            .all(|status| *status == PublicationTargetStatus::Cancelled)
        {
            return Self::Cancelled;
        }
        let published = targets
            .iter()
            .filter(|status| **status == PublicationTargetStatus::Published)
            .count();
        if published == targets.len() {
            return Self::Published;
        }
        if published > 0 {
            return Self::PartiallyPublished;
        }
        if targets
            .iter()
            .any(|status| *status == PublicationTargetStatus::NeedsAttention)
        {
            return Self::NeedsAttention;
        }
        if targets
            .iter()
            .any(|status| *status == PublicationTargetStatus::HandedOff)
        {
            return Self::HandedOff;
        }
        Self::Ready
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationTarget {
    pub id: Uuid,
    pub platform: PublicationPlatform,
    pub status: PublicationTargetStatus,
    pub draft_revision: i32,
}

/// 发布计划只绑定不可变 handoff；整体状态由 targets 派生。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationPlan {
    pub id: Uuid,
    pub handoff_id: Uuid,
    pub targets: Vec<PublicationTarget>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationPackage {
    pub id: Uuid,
    pub publication_target_id: Uuid,
    pub draft_revision: i32,
    pub manifest_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationEvent {
    pub id: Uuid,
    pub publication_target_id: Uuid,
    pub event_type: PublicationEventType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicationDomainError {
    UnsupportedPlatform,
    InvalidTransition {
        from: PublicationTargetStatus,
        action: &'static str,
    },
}

impl fmt::Display for PublicationDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => formatter.write_str("仅支持抖音和小红书人工发布目标"),
            Self::InvalidTransition { from, action } => {
                write!(formatter, "目标状态 {} 不允许执行 {action}", from.as_str())
            }
        }
    }
}

impl std::error::Error for PublicationDomainError {}

impl PublicationTarget {
    pub fn new(
        id: Uuid,
        platform: &str,
        status: PublicationTargetStatus,
    ) -> Result<Self, PublicationDomainError> {
        let platform = match platform {
            "douyin" => PublicationPlatform::Douyin,
            "xiaohongshu" => PublicationPlatform::Xiaohongshu,
            _ => return Err(PublicationDomainError::UnsupportedPlatform),
        };
        Ok(Self {
            id,
            platform,
            status,
            draft_revision: 1,
        })
    }

    pub fn revise(&mut self) -> Result<(), PublicationDomainError> {
        if self.status.is_terminal() {
            return Err(self.invalid("修改草稿"));
        }
        self.draft_revision += 1;
        self.status = PublicationTargetStatus::Draft;
        Ok(())
    }

    pub fn mark_ready(&mut self) -> Result<(), PublicationDomainError> {
        if !matches!(
            self.status,
            PublicationTargetStatus::Draft | PublicationTargetStatus::NeedsAttention
        ) {
            return Err(self.invalid("准备发布包"));
        }
        self.status = PublicationTargetStatus::Ready;
        Ok(())
    }

    pub fn handoff(&mut self) -> Result<(), PublicationDomainError> {
        if self.status != PublicationTargetStatus::Ready {
            return Err(self.invalid("人工交接"));
        }
        self.status = PublicationTargetStatus::HandedOff;
        Ok(())
    }

    pub fn needs_attention(&mut self) -> Result<(), PublicationDomainError> {
        if self.status != PublicationTargetStatus::HandedOff {
            return Err(self.invalid("标记需处理"));
        }
        self.status = PublicationTargetStatus::NeedsAttention;
        Ok(())
    }

    pub fn publish(&mut self) -> Result<(), PublicationDomainError> {
        if self.status != PublicationTargetStatus::HandedOff {
            return Err(self.invalid("人工确认发布"));
        }
        self.status = PublicationTargetStatus::Published;
        Ok(())
    }

    pub fn cancel(&mut self) -> Result<(), PublicationDomainError> {
        if self.status.is_terminal() {
            return Err(self.invalid("取消"));
        }
        self.status = PublicationTargetStatus::Cancelled;
        Ok(())
    }

    pub fn package_is_current(&self, package: &PublicationPackage) -> bool {
        package.publication_target_id == self.id && package.draft_revision == self.draft_revision
    }

    pub fn can_correct_result(&self) -> bool {
        self.status == PublicationTargetStatus::Published
    }

    fn invalid(&self, action: &'static str) -> PublicationDomainError {
        PublicationDomainError::InvalidTransition {
            from: self.status,
            action,
        }
    }
}
