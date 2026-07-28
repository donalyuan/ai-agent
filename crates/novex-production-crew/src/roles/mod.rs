//! 角色定义与注册模块

pub mod definition;
pub mod loader;
pub mod registry;

pub use definition::{Lifecycle, PromptRef, RoleDefinition};
pub use loader::RoleLoader;
pub use registry::RoleRegistry;
