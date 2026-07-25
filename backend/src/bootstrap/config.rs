//! 从环境变量读取进程配置，不参与请求处理或业务状态管理。

/// 后端启动配置；只保存进程级配置，不保存请求或业务状态。
#[derive(Clone, Debug)]
pub struct AppConfig {
    pub environment: String,
    pub database_url: String,
    pub redis_url: String,
    pub openai_api_key: String,
    pub openai_base_url: String,
    pub openai_model: String,
    pub openai_timeout_seconds: u64,
    pub openai_reasoning_effort: Option<String>,
    pub openai_max_output_tokens: u32,
    pub asset_storage_root: String,
    pub asset_generation_providers: Vec<String>,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            environment: std::env::var("NOVEX_ENV")
                .or_else(|_| std::env::var("AI_AGENT_ENV"))
                .unwrap_or_else(|_| "development".to_string()),
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://postgres:postgres@biga-postgres:5432/video_agent".to_string()
            }),
            redis_url: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://bs-redis:6379/2".to_string()),
            openai_api_key: String::new(),
            openai_base_url: String::new(),
            openai_model: String::new(),
            openai_timeout_seconds: 0,
            openai_reasoning_effort: None,
            openai_max_output_tokens: 0,
            asset_storage_root: std::env::var("ASSET_STORAGE_ROOT")
                .unwrap_or_else(|_| "/app/storage/assets".to_string()),
            asset_generation_providers: Vec::new(),
        }
    }

    pub fn agent_definitions_dir(&self) -> String {
        std::env::var("NOVEX_AGENT_DEFINITIONS_DIR")
            .unwrap_or_else(|_| "/app/agent-definitions".to_string())
    }
}
