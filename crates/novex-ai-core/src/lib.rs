//! Core run graph, trace, policy, and shared AI domain primitives for Novex.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

mod audit;
mod definitions;

pub use audit::{
    redact_audit_value, validate_asset_references, validate_audit_payload, AuditValidationError,
    AUDIT_REDACTED, MODEL_CALL_SCHEMA_VERSION,
};

pub use definitions::{
    behavior_fingerprint, canonical_json, definition_digest, sha256_hex,
    validate_model_capabilities, ActivationEvidence, AgentDefinition, AssetReference,
    DefinitionError, DefinitionKind, DefinitionRegistry, DefinitionReleaseEvidence,
    DefinitionStatus, DynamicFragment, ExecutorOwner, ModelBehavior, ModelCapabilities,
    ModelRequirements, PromptCompileInput, PromptCompiler, PromptDefinition, PromptSnapshot,
    TrustLevel,
};

pub const CRATE_PURPOSE: &str = "novex-ai-core";

/// Stable, transport-independent identifier used to register an Agent capability.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AgentKey(String);

impl AgentKey {
    pub fn new(value: impl Into<String>) -> Result<Self, AgentKeyError> {
        let value = value.into();
        if is_valid_agent_key(&value) {
            Ok(Self(value))
        } else {
            Err(AgentKeyError::Invalid(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_valid_agent_key(value: &str) -> bool {
    if value.is_empty() || value.len() > 64 {
        return false;
    }
    let mut previous_separator = false;
    for (index, byte) in value.bytes().enumerate() {
        let separator = matches!(byte, b'.' | b'_' | b'-');
        if index == 0 || index + 1 == value.len() {
            if !byte.is_ascii_lowercase() && !byte.is_ascii_digit() {
                return false;
            }
        } else if !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && !separator {
            return false;
        }
        if separator && previous_separator {
            return false;
        }
        previous_separator = separator;
    }
    true
}

impl fmt::Display for AgentKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for AgentKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for AgentKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentKeyError {
    Invalid(String),
}

impl fmt::Display for AgentKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(value) => write!(formatter, "invalid agent key: {value}"),
        }
    }
}

impl std::error::Error for AgentKeyError {}
