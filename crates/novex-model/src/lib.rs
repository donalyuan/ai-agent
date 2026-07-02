//! Model registry, routing, provider capability, and usage boundaries for Novex.

pub mod llm;

pub use llm::{LLMClient, LLMError, LLMPrompt, OpenAIClient, OpenAIConfig};

pub const CRATE_PURPOSE: &str = "novex-model";
