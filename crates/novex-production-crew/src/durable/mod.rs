//! Full Crew 持久工作流的纯领域核心。
//!
//! 本模块不执行数据库、Redis、模型或 provider I/O；Application/Repository
//! 边界必须先持久化命令和状态，再调用这里的确定性规则。

pub mod command_store;
pub mod media;
pub mod package;
pub mod plan;
pub mod production_input;
pub mod repository;
pub mod resource;
pub mod script;
pub mod state_machine;

use serde::Serialize;
use sha2::{Digest, Sha256};

/// 对 serde_json 的稳定对象键顺序做 SHA-256，供计划、命令、package 和证据共用。
pub fn canonical_digest<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(value)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub(crate) fn domain_error(message: impl Into<String>) -> crate::ProductionError {
    crate::ProductionError::InvalidArtifactSchema {
        details: message.into(),
    }
}
