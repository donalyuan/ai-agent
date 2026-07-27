use serde::{Deserialize, Serialize};

/// 产物类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    CreativeBrief,
    StoryBible,
    CharacterBible,
    ScriptDraft,
    DirectorialTreatment,
    ShotContract,
    PerformanceBrief,
    SoundPlan,
    ContinuityLedger,
    TakeReview,
}

impl ArtifactType {
    /// 获取产物类型的表名
    pub fn table_name(&self) -> &'static str {
        match self {
            Self::CreativeBrief => "creative_briefs",
            Self::StoryBible => "story_bibles",
            Self::CharacterBible => "character_bibles",
            Self::ScriptDraft => "script_drafts",
            Self::DirectorialTreatment => "directorial_treatments",
            Self::ShotContract => "shot_contracts",
            Self::PerformanceBrief => "performance_briefs",
            Self::SoundPlan => "sound_plans",
            Self::ContinuityLedger => "continuity_ledgers",
            Self::TakeReview => "take_reviews",
        }
    }

    /// 获取产物类型的显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::CreativeBrief => "创意简报",
            Self::StoryBible => "故事圣经",
            Self::CharacterBible => "角色圣经",
            Self::ScriptDraft => "剧本草稿",
            Self::DirectorialTreatment => "导演阐述",
            Self::ShotContract => "镜头合约",
            Self::PerformanceBrief => "表演简报",
            Self::SoundPlan => "声音计划",
            Self::ContinuityLedger => "连续性台账",
            Self::TakeReview => "镜头评审",
        }
    }
}

/// 产物状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactStatus {
    Draft,
    Approved,
    Superseded,
}

impl ArtifactStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Approved => "approved",
            Self::Superseded => "superseded",
        }
    }
}

// 导出各产物模块
pub mod creative_brief;
pub mod story_bible;
pub mod character_bible;
pub mod script_draft;
pub mod directorial_treatment;
pub mod shot_contract;
pub mod performance_brief;
pub mod sound_plan;
pub mod continuity_ledger;
pub mod take_review;
