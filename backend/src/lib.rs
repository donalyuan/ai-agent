//! Novex API crate 入口；业务实现分别位于 API、Application、Domain 和 Repository 模块。

pub mod agents;
pub mod api;
pub mod application;
pub mod bootstrap;
pub mod domain;
pub mod model_config_import;
pub mod model_routing;
pub mod repositories;

pub use api::router::{build_app, build_app_with_state};
