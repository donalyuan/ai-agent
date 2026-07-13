//! 组装配置、基础设施连接和应用依赖，不承载业务规则。

mod config;
mod runtime;
mod state;

pub use config::AppConfig;
pub use runtime::{build_runtime_state, connect_runtime_pg_pool};
pub use state::{AppState, AppStateError};
