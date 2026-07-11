//! Model registry, routing, provider capability, and usage boundaries for Novex.

pub mod llm;
pub mod registry;

pub use llm::{LLMClient, LLMError, LLMJsonSchema, LLMPrompt, OpenAIClient, OpenAIConfig};
pub use registry::{
    ApiProtocol, AuthScheme, ImageModelSettings, ModelExecutionSnapshot, ModelRuntimeConfig,
    ModelSettings, ModelSettingsError, ModelType, TextModelSettings, VideoModelSettings,
};

pub const CRATE_PURPOSE: &str = "novex-model";
