use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};
use url::Url;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DefinitionStatus {
    Candidate,
    Active,
    Supported,
    Revoked,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutorOwner {
    Rust,
    Pi,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    Platform,
    ConfirmedFact,
    Reference,
    UserInstruction,
    Steer,
    FollowUp,
    Candidate,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRequirements {
    pub text: bool,
    pub tool_calling: bool,
    pub structured_output: bool,
    pub vision: bool,
    pub reasoning: bool,
    pub min_context_window: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelCapabilities {
    pub text: bool,
    pub tool_calling: bool,
    pub structured_output: bool,
    pub vision: bool,
    pub reasoning: bool,
    pub context_window: u64,
}

pub fn validate_model_capabilities(
    requirements: &ModelRequirements,
    capabilities: &ModelCapabilities,
) -> Result<(), DefinitionError> {
    let missing = [
        (requirements.text, capabilities.text, "text"),
        (
            requirements.tool_calling,
            capabilities.tool_calling,
            "tool_calling",
        ),
        (
            requirements.structured_output,
            capabilities.structured_output,
            "structured_output",
        ),
        (requirements.vision, capabilities.vision, "vision"),
        (requirements.reasoning, capabilities.reasoning, "reasoning"),
    ]
    .into_iter()
    .find_map(|(required, available, name)| (required && !available).then_some(name));
    if let Some(name) = missing {
        return Err(DefinitionError::Capability(format!(
            "required capability {name} is unavailable"
        )));
    }
    if capabilities.context_window < requirements.min_context_window {
        return Err(DefinitionError::Capability(format!(
            "context window {} is below required {}",
            capabilities.context_window, requirements.min_context_window
        )));
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VersionedReference {
    pub key: String,
    pub version: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentDefinition {
    pub agent_key: String,
    pub version: String,
    pub status: DefinitionStatus,
    pub executor_owner: ExecutorOwner,
    pub role: String,
    pub goals: Vec<String>,
    pub constraints: Vec<String>,
    pub model_requirements: ModelRequirements,
    pub tool_profiles: Vec<String>,
    pub tools: Vec<String>,
    pub nodes: BTreeMap<String, VersionedReference>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum VariableType {
    String,
    StringList,
    Integer,
    Json,
    Fragments,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct VariableDefinition {
    name: String,
    value_type: VariableType,
    required: bool,
    trust: TrustLevel,
    max_bytes: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromptDefinition {
    pub prompt_key: String,
    pub version: String,
    pub status: DefinitionStatus,
    pub executor_owner: ExecutorOwner,
    pub system_template: String,
    pub user_template: String,
    variables: Vec<VariableDefinition>,
    pub output_schema: Option<Value>,
    pub tool_profile: Option<String>,
    pub max_output_tokens: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RegistryDocument {
    schema_version: String,
    agents: Vec<AgentDefinition>,
    prompts: Vec<PromptDefinition>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DefinitionKind {
    Agent,
    Prompt,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActivationEvidence {
    GoldenBaseline { reference: String, sha256: String },
    EvalReport { report_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DefinitionReleaseEvidence {
    pub definition_kind: DefinitionKind,
    pub definition_key: String,
    pub definition_version: String,
    pub definition_digest: String,
    pub activation_evidence: ActivationEvidence,
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseIndex {
    schema_version: String,
    registry_digest: String,
    releases: Vec<DefinitionReleaseEvidence>,
}

#[derive(Clone, Debug)]
pub struct DefinitionRegistry {
    document: RegistryDocument,
    digest: String,
    templates: BTreeMap<String, String>,
    releases: Vec<DefinitionReleaseEvidence>,
}

impl DefinitionRegistry {
    pub fn load(directory: impl AsRef<Path>) -> Result<Self, DefinitionError> {
        let directory = directory.as_ref();
        let bytes = fs::read(directory.join("registry.json")).map_err(DefinitionError::Io)?;
        let document: RegistryDocument = serde_json::from_slice(&bytes)
            .map_err(|error| DefinitionError::InvalidRegistry(error.to_string()))?;
        if document.schema_version != "1" {
            return Err(DefinitionError::InvalidRegistry(
                "unsupported schema_version".into(),
            ));
        }

        let raw_value: Value = serde_json::from_slice(&bytes)
            .map_err(|error| DefinitionError::InvalidRegistry(error.to_string()))?;
        let digest = sha256_hex(canonical_json(&raw_value).as_bytes());
        let mut templates = BTreeMap::new();
        for prompt in &document.prompts {
            for relative in [&prompt.system_template, &prompt.user_template] {
                validate_relative_path(relative)?;
                templates
                    .entry(relative.clone())
                    .or_insert(read_template(directory.join(relative))?);
            }
        }
        let registry = Self {
            document,
            digest,
            templates,
            releases: Vec::new(),
        };
        registry.validate()?;
        let release: ReleaseIndex = serde_json::from_slice(
            &fs::read(directory.join("release-index.json")).map_err(DefinitionError::Io)?,
        )
        .map_err(|error| DefinitionError::InvalidRegistry(error.to_string()))?;
        if release.schema_version != "1" || release.registry_digest != registry.digest {
            return Err(DefinitionError::InvalidRegistry(
                "release index does not match immutable registry digest".into(),
            ));
        }
        registry.validate_release_evidence(&release.releases)?;
        let mut registry = registry;
        registry.releases = release.releases;
        Ok(registry)
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub fn agents(&self) -> &[AgentDefinition] {
        &self.document.agents
    }

    pub fn prompts(&self) -> &[PromptDefinition] {
        &self.document.prompts
    }

    pub fn release_evidence(&self) -> &[DefinitionReleaseEvidence] {
        &self.releases
    }

    pub fn active_agent(&self, key: &str) -> Result<&AgentDefinition, DefinitionError> {
        let matches = self
            .document
            .agents
            .iter()
            .filter(|item| item.agent_key == key && item.status == DefinitionStatus::Active)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [definition] => Ok(definition),
            [] => Err(DefinitionError::DefinitionNotFound(key.into())),
            _ => Err(DefinitionError::InvalidRegistry(format!(
                "multiple active versions for {key}"
            ))),
        }
    }

    pub fn agent(&self, key: &str, version: &str) -> Result<&AgentDefinition, DefinitionError> {
        self.document
            .agents
            .iter()
            .find(|item| item.agent_key == key && item.version == version)
            .ok_or_else(|| DefinitionError::DefinitionNotFound(format!("{key}@{version}")))
    }

    fn prompt(&self, key: &str, version: &str) -> Result<&PromptDefinition, DefinitionError> {
        self.document
            .prompts
            .iter()
            .find(|item| item.prompt_key == key && item.version == version)
            .ok_or_else(|| DefinitionError::DefinitionNotFound(format!("{key}@{version}")))
    }

    fn template(&self, path: &str) -> Result<&str, DefinitionError> {
        self.templates
            .get(path)
            .map(String::as_str)
            .ok_or_else(|| DefinitionError::InvalidRegistry(format!("missing template {path}")))
    }

    fn validate(&self) -> Result<(), DefinitionError> {
        let mut agent_versions = BTreeSet::new();
        let mut prompt_versions = BTreeSet::new();
        let mut active_agents = BTreeSet::new();
        for agent in &self.document.agents {
            validate_key_and_version(&agent.agent_key, &agent.version)?;
            if !agent_versions.insert((&agent.agent_key, &agent.version)) {
                return Err(DefinitionError::InvalidRegistry(format!(
                    "duplicate agent {}@{}",
                    agent.agent_key, agent.version
                )));
            }
            if agent.status == DefinitionStatus::Active && !active_agents.insert(&agent.agent_key) {
                return Err(DefinitionError::InvalidRegistry(format!(
                    "multiple active versions for {}",
                    agent.agent_key
                )));
            }
            if agent.nodes.is_empty()
                || agent.goals.is_empty()
                || agent.role.trim().is_empty()
                || agent.model_requirements.min_context_window == 0
            {
                return Err(DefinitionError::InvalidRegistry(format!(
                    "agent {} has an empty execution contract",
                    agent.agent_key
                )));
            }
            let profiles = agent.tool_profiles.iter().collect::<BTreeSet<_>>();
            if profiles.len() != agent.tool_profiles.len()
                || profiles.is_empty()
                || profiles
                    .iter()
                    .any(|profile| !matches!(profile.as_str(), "chat" | "workspace"))
            {
                return Err(DefinitionError::InvalidRegistry(format!(
                    "agent {} has invalid tool profiles",
                    agent.agent_key
                )));
            }
            let tools = agent.tools.iter().collect::<BTreeSet<_>>();
            if tools.len() != agent.tools.len()
                || tools
                    .iter()
                    .any(|tool| !matches!(tool.as_str(), "read" | "write" | "edit" | "bash"))
                || (!agent.tools.is_empty()
                    && !profiles.iter().any(|value| value.as_str() == "workspace"))
            {
                return Err(DefinitionError::InvalidRegistry(format!(
                    "agent {} has invalid tools",
                    agent.agent_key
                )));
            }
        }
        for prompt in &self.document.prompts {
            validate_key_and_version(&prompt.prompt_key, &prompt.version)?;
            if !prompt_versions.insert((&prompt.prompt_key, &prompt.version)) {
                return Err(DefinitionError::InvalidRegistry(format!(
                    "duplicate prompt {}@{}",
                    prompt.prompt_key, prompt.version
                )));
            }
            let system = self.template(&prompt.system_template)?;
            let user = self.template(&prompt.user_template)?;
            if system.contains("{{") {
                return Err(DefinitionError::InvalidRegistry(format!(
                    "invalid trust boundary in prompt {}",
                    prompt.prompt_key
                )));
            }
            if prompt
                .tool_profile
                .as_deref()
                .is_some_and(|profile| !matches!(profile, "chat" | "workspace"))
                || prompt.max_output_tokens == Some(0)
                || prompt
                    .output_schema
                    .as_ref()
                    .is_some_and(|schema| !valid_output_schema(schema))
            {
                return Err(DefinitionError::InvalidRegistry(format!(
                    "prompt {} has an invalid execution contract",
                    prompt.prompt_key
                )));
            }
            let mut variable_names = BTreeSet::new();
            for variable in &prompt.variables {
                if !valid_variable_name(&variable.name)
                    || variable.max_bytes == 0
                    || !variable_names.insert(&variable.name)
                    || (!user.contains(&format!("{{{{{}}}}}", variable.name))
                        && !prompt.output_schema.as_ref().is_some_and(|schema| {
                            contains_exact_placeholder(schema, &variable.name)
                        }))
                {
                    return Err(DefinitionError::InvalidRegistry(format!(
                        "prompt {} has an invalid variable {}",
                        prompt.prompt_key, variable.name
                    )));
                }
            }
            let fragment_variables = prompt.variables.iter().filter(|variable| {
                variable.name == "fragments" && variable.value_type == VariableType::Fragments
            });
            if fragment_variables.count() != 1 || !user.contains("{{fragments}}") {
                return Err(DefinitionError::InvalidRegistry(format!(
                    "prompt {} must declare exactly one fragments variable",
                    prompt.prompt_key
                )));
            }
        }
        for agent in &self.document.agents {
            for (node, reference) in &agent.nodes {
                let prompt = self.prompt(&reference.key, &reference.version)?;
                if prompt.executor_owner != agent.executor_owner {
                    return Err(DefinitionError::InvalidRegistry(format!(
                        "cross-owner prompt reference at {node}"
                    )));
                }
                if matches!(
                    agent.status,
                    DefinitionStatus::Active | DefinitionStatus::Supported
                ) && matches!(
                    prompt.status,
                    DefinitionStatus::Candidate | DefinitionStatus::Revoked
                ) {
                    return Err(DefinitionError::InvalidRegistry(format!(
                        "executable agent references unavailable prompt at {node}"
                    )));
                }
            }
        }
        Ok(())
    }

    fn validate_release_evidence(
        &self,
        releases: &[DefinitionReleaseEvidence],
    ) -> Result<(), DefinitionError> {
        let mut identities = BTreeSet::new();
        for release in releases {
            let identity = (
                release.definition_kind,
                release.definition_key.as_str(),
                release.definition_version.as_str(),
            );
            if !identities.insert(identity) {
                return Err(DefinitionError::InvalidRegistry(format!(
                    "duplicate release evidence for {}@{}",
                    release.definition_key, release.definition_version
                )));
            }
            let actual_digest = match release.definition_kind {
                DefinitionKind::Agent => self
                    .agent(&release.definition_key, &release.definition_version)
                    .and_then(definition_digest),
                DefinitionKind::Prompt => self
                    .prompt(&release.definition_key, &release.definition_version)
                    .and_then(definition_digest),
            }
            .map_err(|error| DefinitionError::InvalidRegistry(error.to_string()))?;
            if actual_digest != release.definition_digest {
                return Err(DefinitionError::InvalidRegistry(format!(
                    "release evidence digest mismatch for {}@{}: expected {}, actual {}",
                    release.definition_key,
                    release.definition_version,
                    release.definition_digest,
                    actual_digest
                )));
            }
            match &release.activation_evidence {
                ActivationEvidence::GoldenBaseline { reference, sha256 }
                    if !reference.is_empty() && is_sha256(sha256) => {}
                ActivationEvidence::EvalReport { report_id } if !report_id.is_empty() => {}
                _ => {
                    return Err(DefinitionError::InvalidRegistry(format!(
                        "invalid activation evidence for {}@{}",
                        release.definition_key, release.definition_version
                    )))
                }
            }
        }
        Ok(())
    }
}

fn render_variable(
    variable: &VariableDefinition,
    value: &Value,
) -> Result<String, DefinitionError> {
    match variable.value_type {
        VariableType::String => value.as_str().map(str::to_string).ok_or_else(|| {
            DefinitionError::Compile(format!("variable {} must be string", variable.name))
        }),
        VariableType::StringList => value
            .as_array()
            .filter(|items| items.iter().all(Value::is_string))
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .ok_or_else(|| {
                DefinitionError::Compile(format!("variable {} must be string_list", variable.name))
            }),
        VariableType::Integer => value
            .as_i64()
            .map(|value| value.to_string())
            .or_else(|| value.as_u64().map(|value| value.to_string()))
            .ok_or_else(|| {
                DefinitionError::Compile(format!("variable {} must be integer", variable.name))
            }),
        VariableType::Json => {
            if value.is_null() {
                Err(DefinitionError::Compile(format!(
                    "variable {} must be json",
                    variable.name
                )))
            } else {
                Ok(canonical_json(value))
            }
        }
        VariableType::Fragments => Err(DefinitionError::Compile(
            "fragments must use structured fragments input".into(),
        )),
    }
}

fn validate_tool_schema(
    agent: &AgentDefinition,
    profile: &str,
    schema: Option<&Value>,
) -> Result<(), DefinitionError> {
    let Some(schema) = schema else {
        if profile == "workspace" && !agent.tools.is_empty() {
            return Err(DefinitionError::Compile(
                "workspace tool schema is required".into(),
            ));
        }
        return Ok(());
    };
    let items = schema
        .as_array()
        .ok_or_else(|| DefinitionError::Compile("tool schema must be an array".into()))?;
    if profile == "chat" && !items.is_empty() {
        return Err(DefinitionError::Compile(
            "chat profile does not allow tools".into(),
        ));
    }
    let mut names = BTreeSet::new();
    for item in items {
        let name = item
            .as_object()
            .and_then(|object| object.get("name"))
            .and_then(Value::as_str)
            .ok_or_else(|| DefinitionError::Compile("tool schema name is required".into()))?;
        if !agent.tools.iter().any(|allowed| allowed == name) || !names.insert(name) {
            return Err(DefinitionError::Compile(format!(
                "tool {name} is not allowed or duplicated"
            )));
        }
    }
    let expected = agent
        .tools
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if profile == "workspace" && names != expected {
        return Err(DefinitionError::Compile(
            "workspace tool schema does not match definition".into(),
        ));
    }
    Ok(())
}

fn valid_variable_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn valid_output_schema(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.get("name").is_some_and(Value::is_string)
        && object.get("strict") == Some(&Value::Bool(true))
        && object.get("schema").is_some_and(Value::is_object)
}

fn contains_exact_placeholder(value: &Value, name: &str) -> bool {
    let placeholder = format!("{{{{{name}}}}}");
    match value {
        Value::String(value) => value == &placeholder,
        Value::Array(items) => items
            .iter()
            .any(|item| contains_exact_placeholder(item, name)),
        Value::Object(object) => object
            .values()
            .any(|item| contains_exact_placeholder(item, name)),
        _ => false,
    }
}

fn render_output_schema(
    value: &Value,
    variables: &BTreeMap<String, Value>,
) -> Result<Value, DefinitionError> {
    match value {
        Value::String(value) if value.starts_with("{{") && value.ends_with("}}") => {
            let name = &value[2..value.len() - 2];
            if !valid_variable_name(name) || format!("{{{{{name}}}}}") != *value {
                return Err(DefinitionError::Compile(
                    "output schema contains an invalid variable placeholder".into(),
                ));
            }
            variables.get(name).cloned().ok_or_else(|| {
                DefinitionError::Compile(format!(
                    "output schema variable {name} is missing or undeclared"
                ))
            })
        }
        Value::String(value) if value.contains("{{") => Err(DefinitionError::Compile(
            "output schema variables must occupy a complete JSON value".into(),
        )),
        Value::Array(items) => items
            .iter()
            .map(|item| render_output_schema(item, variables))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Value::Object(object) => object
            .iter()
            .map(|(key, item)| {
                render_output_schema(item, variables).map(|rendered| (key.clone(), rendered))
            })
            .collect::<Result<Map<_, _>, _>>()
            .map(Value::Object),
        other => Ok(other.clone()),
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetReference {
    pub asset_id: String,
    pub version: String,
    pub sha256: String,
    pub mime: String,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicFragment {
    pub id: String,
    pub trust: TrustLevel,
    pub source: String,
    pub content: Option<String>,
    pub asset: Option<AssetReference>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromptCompileInput {
    pub schema_version: String,
    #[serde(default)]
    pub variables: BTreeMap<String, Value>,
    pub fragments: Vec<DynamicFragment>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PromptSnapshot {
    pub schema_version: String,
    pub registry_digest: String,
    pub agent_key: String,
    pub agent_version: String,
    pub prompt_key: String,
    pub prompt_version: String,
    pub node_key: String,
    pub system: String,
    pub user: String,
    pub variables: BTreeMap<String, Value>,
    pub fragments: Vec<DynamicFragment>,
    pub tool_profile: String,
    pub output_schema: Option<Value>,
    pub tool_schema: Option<Value>,
    pub max_output_tokens: Option<u32>,
}

pub struct PromptCompiler<'a> {
    registry: &'a DefinitionRegistry,
}

impl<'a> PromptCompiler<'a> {
    pub fn new(registry: &'a DefinitionRegistry) -> Self {
        Self { registry }
    }

    pub fn compile(
        &self,
        agent_key: &str,
        agent_version: &str,
        node_key: &str,
        input: PromptCompileInput,
        tool_profile: &str,
        tool_schema: Option<Value>,
    ) -> Result<PromptSnapshot, DefinitionError> {
        self.compile_internal(
            agent_key,
            agent_version,
            node_key,
            input,
            tool_profile,
            tool_schema,
            false,
        )
    }

    /// Recompiles immutable historical input without making that version executable.
    pub fn compile_for_replay(
        &self,
        agent_key: &str,
        agent_version: &str,
        node_key: &str,
        input: PromptCompileInput,
        tool_profile: &str,
        tool_schema: Option<Value>,
    ) -> Result<PromptSnapshot, DefinitionError> {
        self.compile_internal(
            agent_key,
            agent_version,
            node_key,
            input,
            tool_profile,
            tool_schema,
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn compile_internal(
        &self,
        agent_key: &str,
        agent_version: &str,
        node_key: &str,
        input: PromptCompileInput,
        tool_profile: &str,
        tool_schema: Option<Value>,
        allow_historical: bool,
    ) -> Result<PromptSnapshot, DefinitionError> {
        if input.schema_version != "1" {
            return Err(DefinitionError::Compile("unknown input schema".into()));
        }
        let agent = self.registry.agent(agent_key, agent_version)?;
        if !allow_historical
            && matches!(
                agent.status,
                DefinitionStatus::Candidate | DefinitionStatus::Revoked
            )
        {
            return Err(DefinitionError::DefinitionUnavailable(format!(
                "{agent_key}@{agent_version}"
            )));
        }
        if !agent
            .tool_profiles
            .iter()
            .any(|profile| profile == tool_profile)
        {
            return Err(DefinitionError::Compile(format!(
                "tool profile {tool_profile} is not allowed"
            )));
        }
        let reference = agent
            .nodes
            .get(node_key)
            .ok_or_else(|| DefinitionError::Compile(format!("node {node_key} is not declared")))?;
        let prompt = self.registry.prompt(&reference.key, &reference.version)?;
        if !allow_historical
            && matches!(
                prompt.status,
                DefinitionStatus::Candidate | DefinitionStatus::Revoked
            )
        {
            return Err(DefinitionError::DefinitionUnavailable(format!(
                "{}@{}",
                prompt.prompt_key, prompt.version
            )));
        }
        if prompt
            .tool_profile
            .as_deref()
            .is_some_and(|profile| profile != tool_profile)
        {
            return Err(DefinitionError::Compile(
                "prompt requires a different tool profile".to_string(),
            ));
        }
        validate_tool_schema(agent, tool_profile, tool_schema.as_ref())?;
        let declared = prompt
            .variables
            .iter()
            .map(|variable| (variable.name.as_str(), variable))
            .collect::<BTreeMap<_, _>>();
        for name in input.variables.keys() {
            if name == "fragments" || !declared.contains_key(name.as_str()) {
                return Err(DefinitionError::Compile(format!("unknown variable {name}")));
            }
        }
        let fragment_variable = declared
            .get("fragments")
            .ok_or_else(|| DefinitionError::Compile("fragments variable is not declared".into()))?;
        if fragment_variable.required && input.fragments.is_empty() {
            return Err(DefinitionError::Compile("fragments is required".into()));
        }
        let mut total_bytes = 0usize;
        let mut fragment_ids = BTreeSet::new();
        let mut contents = Vec::with_capacity(input.fragments.len());
        for fragment in &input.fragments {
            if fragment.id.trim().is_empty()
                || fragment.source.trim().is_empty()
                || !fragment_ids.insert(&fragment.id)
            {
                return Err(DefinitionError::Compile(
                    "fragment id/source is invalid or duplicated".into(),
                ));
            }
            match (&fragment.content, &fragment.asset) {
                (Some(content), None) if !content.is_empty() => {
                    total_bytes = total_bytes.saturating_add(content.len());
                    contents.push(content.clone());
                }
                (None, Some(asset))
                    if !asset.asset_id.is_empty()
                        && !asset.version.is_empty()
                        && asset.sha256.len() == 64
                        && asset
                            .sha256
                            .bytes()
                            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
                        && matches!(
                            asset.mime.split_once('/'),
                            Some(("image" | "audio" | "video", subtype)) if !subtype.is_empty()
                        ) =>
                {
                    let value = serde_json::to_string(asset)
                        .map_err(|error| DefinitionError::Compile(error.to_string()))?;
                    total_bytes = total_bytes.saturating_add(value.len());
                    contents.push(value);
                }
                _ => {
                    return Err(DefinitionError::Compile(
                        "fragment must contain exactly one text or asset reference".into(),
                    ))
                }
            }
        }
        if total_bytes > fragment_variable.max_bytes {
            return Err(DefinitionError::Compile(
                "fragments exceed max_bytes".into(),
            ));
        }
        let mut rendered_variables = BTreeMap::new();
        rendered_variables.insert(
            "fragments".to_string(),
            serde_json::to_value(&input.fragments)
                .map_err(|error| DefinitionError::Compile(error.to_string()))?,
        );
        let mut user = self
            .registry
            .template(&prompt.user_template)?
            .replace("{{fragments}}", &contents.join("\n"));
        for variable in prompt
            .variables
            .iter()
            .filter(|variable| variable.value_type != VariableType::Fragments)
        {
            let value = input.variables.get(&variable.name);
            if variable.required && value.is_none() {
                return Err(DefinitionError::Compile(format!(
                    "variable {} is required",
                    variable.name
                )));
            }
            let rendered = match value {
                Some(value) => render_variable(variable, value)?,
                None => String::new(),
            };
            if rendered.len() > variable.max_bytes {
                return Err(DefinitionError::Compile(format!(
                    "variable {} exceeds max_bytes",
                    variable.name
                )));
            }
            user = user.replace(&format!("{{{{{}}}}}", variable.name), &rendered);
            if let Some(value) = value {
                rendered_variables.insert(variable.name.clone(), value.clone());
            }
        }
        if user.contains("{{") {
            return Err(DefinitionError::Compile(
                "unresolved template variable".into(),
            ));
        }
        let output_schema = prompt
            .output_schema
            .as_ref()
            .map(|schema| render_output_schema(schema, &rendered_variables))
            .transpose()?;
        Ok(PromptSnapshot {
            schema_version: "1".into(),
            registry_digest: self.registry.digest.clone(),
            agent_key: agent.agent_key.clone(),
            agent_version: agent.version.clone(),
            prompt_key: prompt.prompt_key.clone(),
            prompt_version: prompt.version.clone(),
            node_key: node_key.into(),
            system: self.registry.template(&prompt.system_template)?.into(),
            user,
            variables: rendered_variables,
            fragments: input.fragments,
            tool_profile: tool_profile.into(),
            output_schema,
            tool_schema,
            max_output_tokens: prompt.max_output_tokens,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelBehavior {
    pub protocol: String,
    pub request_base_url: String,
    pub upstream_model: String,
    pub reasoning_effort: Option<String>,
    pub max_output_tokens: u32,
    pub context_window: u64,
    pub settings: Value,
}

pub fn behavior_fingerprint(
    input: &ModelBehavior,
) -> Result<(String, ModelBehavior), DefinitionError> {
    let mut url = Url::parse(&input.request_base_url)
        .map_err(|_| DefinitionError::Fingerprint("request_base_url is invalid".into()))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(DefinitionError::Fingerprint(
            "request_base_url must be http(s)".into(),
        ));
    }
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_query(None);
    url.set_fragment(None);
    let trimmed_path = url.path().trim_end_matches('/').to_string();
    url.set_path(if trimmed_path.is_empty() {
        "/"
    } else {
        &trimmed_path
    });
    let mut request_base_url = url.to_string();
    if request_base_url.ends_with('/') && url.path() != "/" {
        request_base_url.pop();
    }
    if request_base_url.ends_with('/') && url.path() == "/" {
        request_base_url.pop();
    }
    let normalized = ModelBehavior {
        protocol: input.protocol.trim().to_ascii_lowercase(),
        request_base_url,
        upstream_model: input.upstream_model.trim().into(),
        reasoning_effort: input
            .reasoning_effort
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_ascii_lowercase),
        max_output_tokens: input.max_output_tokens,
        context_window: input.context_window,
        settings: remove_sensitive_fields(&input.settings),
    };
    if !matches!(
        normalized.protocol.as_str(),
        "openai_responses" | "openai_chat_completions"
    ) || normalized.upstream_model.is_empty()
        || normalized.context_window == 0
        || normalized.max_output_tokens == 0
        || normalized
            .reasoning_effort
            .as_deref()
            .is_some_and(|value| !matches!(value, "minimal" | "low" | "medium" | "high" | "xhigh"))
        || !normalized.settings.is_object()
    {
        return Err(DefinitionError::Fingerprint(
            "model behavior is incomplete".into(),
        ));
    }
    let value = serde_json::to_value(&normalized)
        .map_err(|error| DefinitionError::Fingerprint(error.to_string()))?;
    Ok((sha256_hex(canonical_json(&value).as_bytes()), normalized))
}

pub fn canonical_json(value: &Value) -> String {
    fn normalized(value: &Value) -> Value {
        match value {
            Value::Object(object) => {
                let mut keys = object.keys().collect::<Vec<_>>();
                keys.sort_unstable();
                let mut sorted = Map::new();
                for key in keys {
                    sorted.insert(key.clone(), normalized(&object[key]));
                }
                Value::Object(sorted)
            }
            Value::Array(items) => Value::Array(items.iter().map(normalized).collect()),
            other => other.clone(),
        }
    }
    serde_json::to_string(&normalized(value)).expect("JSON Value serialization cannot fail")
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn definition_digest(value: &impl Serialize) -> Result<String, DefinitionError> {
    let mut value = serde_json::to_value(value)
        .map_err(|error| DefinitionError::InvalidRegistry(error.to_string()))?;
    if let Value::Object(object) = &mut value {
        object.remove("status");
    }
    Ok(sha256_hex(canonical_json(&value).as_bytes()))
}

fn remove_sensitive_fields(value: &Value) -> Value {
    fn redact(value: &Value, preserve_empty: bool) -> Option<Value> {
        match value {
            Value::Object(object) => {
                let redacted = object
                    .iter()
                    .filter_map(|(key, value)| {
                        let normalized = key.to_ascii_lowercase().replace('-', "_");
                        let sensitive = normalized.contains("api_key")
                            || normalized.contains("api_secret")
                            || normalized.contains("authorization")
                            || normalized.contains("cookie")
                            || normalized.contains("credential")
                            || normalized.contains("signature")
                            || normalized.ends_with("_token")
                            || normalized == "token"
                            || normalized == "password"
                            || normalized == "secret";
                        (!sensitive)
                            .then(|| redact(value, false).map(|item| (key.clone(), item)))
                            .flatten()
                    })
                    .collect::<Map<_, _>>();
                (preserve_empty || object.is_empty() || !redacted.is_empty())
                    .then_some(Value::Object(redacted))
            }
            Value::Array(items) => Some(Value::Array(
                items
                    .iter()
                    .filter_map(|item| redact(item, false))
                    .collect(),
            )),
            other => Some(other.clone()),
        }
    }
    redact(value, true).unwrap_or_else(|| Value::Object(Map::new()))
}

fn validate_key_and_version(key: &str, version: &str) -> Result<(), DefinitionError> {
    let valid_key = !key.is_empty()
        && key.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        });
    let parts = version.split('.').collect::<Vec<_>>();
    let valid_version = parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()));
    if valid_key && valid_version {
        Ok(())
    } else {
        Err(DefinitionError::InvalidRegistry(format!(
            "invalid key/version {key}@{version}"
        )))
    }
}

fn validate_relative_path(path: &str) -> Result<(), DefinitionError> {
    let path = Path::new(path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(DefinitionError::InvalidRegistry(
            "template path must stay inside registry".into(),
        ));
    }
    Ok(())
}

fn read_template(path: PathBuf) -> Result<String, DefinitionError> {
    let mut value = fs::read_to_string(path).map_err(DefinitionError::Io)?;
    if value.ends_with("\r\n") {
        value.truncate(value.len() - 2);
    } else if value.ends_with('\n') {
        value.pop();
    }
    if value.is_empty() {
        return Err(DefinitionError::InvalidRegistry(
            "template must not be empty".into(),
        ));
    }
    Ok(value)
}

#[derive(Debug)]
pub enum DefinitionError {
    Io(std::io::Error),
    InvalidRegistry(String),
    DefinitionNotFound(String),
    DefinitionUnavailable(String),
    Compile(String),
    Fingerprint(String),
    Capability(String),
}

impl fmt::Display for DefinitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "definition registry I/O error: {error}"),
            Self::InvalidRegistry(message) => {
                write!(formatter, "invalid definition registry: {message}")
            }
            Self::DefinitionNotFound(value) => write!(formatter, "definition not found: {value}"),
            Self::DefinitionUnavailable(value) => {
                write!(formatter, "definition is not executable: {value}")
            }
            Self::Compile(message) => write!(formatter, "prompt compile error: {message}"),
            Self::Fingerprint(message) => write!(formatter, "model fingerprint error: {message}"),
            Self::Capability(message) => write!(formatter, "model capability mismatch: {message}"),
        }
    }
}

impl std::error::Error for DefinitionError {}
