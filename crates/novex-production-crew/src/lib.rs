//! Novex 虚拟制作团队 crate
//!
//! 提供完整的虚拟制作团队编排能力，支持 Fast Lane（快速通道）和 Full Crew（完整团队）两种模式。
//! 角色通过结构化产物协作，由 Orchestrator 统一调度，质量闸门保障输出质量。

pub mod durable;
pub mod error;
pub mod executor;
pub mod gates;
pub mod orchestrator;
pub mod roles;
pub mod state;

pub use error::{ProductionError, ProductionResult};
