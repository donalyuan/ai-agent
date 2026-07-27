//! 角色定义数据结构：描述一个虚拟制作角色的职责、输入产物、输出产物和 Prompt 引用

use crate::state::artifacts::ArtifactType;
use serde::{Deserialize, Serialize};

/// 角色定义：从 YAML manifest 加载，描述角色的完整元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleDefinition {
    /// 角色唯一标识，如 "producer"
    pub role_key: String,
    /// 展示名称，如 "制片人"
    pub role_name: String,
    /// 职责描述列表
    pub responsibilities: Vec<String>,
    /// 依赖的输入产物类型（空表示由用户直接提供或无前置依赖）
    pub input_artifacts: Vec<ArtifactType>,
    /// 产出的产物类型（空表示仅产出协作建议，不落库）
    pub output_artifacts: Vec<ArtifactType>,
    /// 允许使用的工具（当前为空，未来扩展外部 API 调用权限）
    pub allowed_tools: Vec<String>,
    /// Prompt 定义引用，指向具体的 versioned prompt
    pub prompt_definition_ref: PromptRef,
    /// 角色生命周期状态，控制是否允许在新项目中使用
    pub lifecycle: Lifecycle,
}

/// Prompt 定义引用：指向具体的 Prompt 文件与版本
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptRef {
    /// Prompt key，如 "producer.general"
    pub key: String,
    /// Prompt 版本，如 "@1"
    pub version: String,
}

impl PromptRef {
    /// 构建完整引用字符串，如 "producer.general@1"
    pub fn full_ref(&self) -> String {
        format!("{}@{}", self.key, self.version.trim_start_matches('@'))
    }
}

/// 角色生命周期：控制角色在系统中的可用状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    /// 候选：尚未正式启用，仅用于测试验证
    Candidate,
    /// 活跃：当前推荐用于所有新项目
    Active,
    /// 支持中：可用但不推荐新项目使用，维护期
    Supported,
    /// 已撤销：不可再使用，历史归档
    Revoked,
}
