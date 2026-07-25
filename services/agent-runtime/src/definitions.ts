import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { isAbsolute, join, normalize, sep } from "node:path";

export type DefinitionStatus = "candidate" | "active" | "supported" | "revoked";
export type ExecutorOwner = "rust" | "pi";
export type TrustLevel =
  | "platform"
  | "confirmed_fact"
  | "reference"
  | "user_instruction"
  | "steer"
  | "follow_up"
  | "candidate";

export interface ModelRequirements {
  text: boolean;
  tool_calling: boolean;
  structured_output: boolean;
  vision: boolean;
  reasoning: boolean;
  min_context_window: number;
}

export interface ModelCapabilities {
  text: boolean;
  tool_calling: boolean;
  structured_output: boolean;
  vision: boolean;
  reasoning: boolean;
  context_window: number;
}

export function validateModelCapabilities(requirements: ModelRequirements, capabilities: ModelCapabilities): void {
  const checks: Array<[boolean, boolean, string]> = [
    [requirements.text, capabilities.text, "text"],
    [requirements.tool_calling, capabilities.tool_calling, "tool_calling"],
    [requirements.structured_output, capabilities.structured_output, "structured_output"],
    [requirements.vision, capabilities.vision, "vision"],
    [requirements.reasoning, capabilities.reasoning, "reasoning"],
  ];
  const missing = checks.find(([required, available]) => required && !available);
  if (missing) throw new Error(`model capability mismatch: required capability ${missing[2]} is unavailable`);
  if (!positiveInteger(capabilities.context_window) || capabilities.context_window < requirements.min_context_window) {
    throw new Error(`model capability mismatch: context window ${capabilities.context_window} is below required ${requirements.min_context_window}`);
  }
}

export interface VersionedReference {
  key: string;
  version: string;
}

export interface AgentDefinition {
  agent_key: string;
  version: string;
  status: DefinitionStatus;
  executor_owner: ExecutorOwner;
  role: string;
  goals: string[];
  constraints: string[];
  model_requirements: ModelRequirements;
  tool_profiles: Array<"chat" | "workspace">;
  tools: Array<"read" | "write" | "edit" | "bash">;
  nodes: Record<string, VersionedReference>;
}

interface VariableDefinition {
  name: string;
  value_type: "string" | "string_list" | "integer" | "json" | "fragments";
  required: boolean;
  trust: TrustLevel;
  max_bytes: number;
}

export interface PromptDefinition {
  prompt_key: string;
  version: string;
  status: DefinitionStatus;
  executor_owner: ExecutorOwner;
  system_template: string;
  user_template: string;
  variables: VariableDefinition[];
  output_schema: unknown | null;
  tool_profile: "chat" | "workspace" | null;
  max_output_tokens: number | null;
}

export type ActivationEvidence =
  | { type: "golden_baseline"; reference: string; sha256: string }
  | { type: "eval_report"; report_id: string };

export interface DefinitionReleaseEvidence {
  definition_kind: "agent" | "prompt";
  definition_key: string;
  definition_version: string;
  definition_digest: string;
  activation_evidence: ActivationEvidence;
}

interface RegistryDocument {
  schema_version: "1";
  agents: AgentDefinition[];
  prompts: PromptDefinition[];
}

export interface DefinitionRegistry {
  readonly digest: string;
  readonly agents: readonly AgentDefinition[];
  readonly prompts: readonly PromptDefinition[];
  readonly templates: ReadonlyMap<string, string>;
  readonly releases: readonly DefinitionReleaseEvidence[];
}

export interface AssetReference {
  asset_id: string;
  version: string;
  sha256: string;
  mime: string;
  metadata?: Record<string, string>;
}

export interface DynamicFragment {
  id: string;
  trust: TrustLevel;
  source: string;
  content?: string;
  asset?: AssetReference;
}

export interface PromptCompileInput {
  schema_version: "1";
  variables?: Record<string, unknown>;
  fragments: DynamicFragment[];
}

export interface PromptSnapshot {
  readonly schema_version: "1";
  readonly registry_digest: string;
  readonly agent_key: string;
  readonly agent_version: string;
  readonly prompt_key: string;
  readonly prompt_version: string;
  readonly node_key: string;
  readonly system: string;
  readonly user: string;
  readonly variables: Readonly<Record<string, unknown>>;
  readonly fragments: readonly DynamicFragment[];
  readonly tool_profile: "chat" | "workspace";
  readonly output_schema: unknown | null;
  readonly tool_schema: unknown | null;
  readonly max_output_tokens: number | null;
}

export interface ModelBehavior {
  protocol: string;
  request_base_url: string;
  upstream_model: string;
  reasoning_effort: string | null;
  max_output_tokens: number;
  context_window: number;
  settings: unknown;
}

const AGENT_KEYS = [
  "agent_key", "version", "status", "executor_owner", "role", "goals", "constraints",
  "model_requirements", "tool_profiles", "tools", "nodes",
] as const;
const PROMPT_KEYS = [
  "prompt_key", "version", "status", "executor_owner", "system_template", "user_template",
  "variables", "output_schema", "tool_profile", "max_output_tokens",
] as const;

export async function loadDefinitionRegistry(directory: string): Promise<DefinitionRegistry> {
  const raw = await readFile(join(directory, "registry.json"), "utf8");
  const value: unknown = JSON.parse(raw);
  const document = parseRegistry(value);
  const templates = new Map<string, string>();
  for (const prompt of document.prompts) {
    for (const relative of [prompt.system_template, prompt.user_template]) {
      validateTemplatePath(relative);
      if (!templates.has(relative)) {
        const content = (await readFile(join(directory, relative), "utf8")).replace(/\r?\n$/, "");
        if (!content) throw new Error(`invalid definition registry: empty template ${relative}`);
        templates.set(relative, content);
      }
    }
  }
  validateRegistry(document, templates);
  const release = object(JSON.parse(await readFile(join(directory, "release-index.json"), "utf8")), "release-index");
  exactKeys(release, ["schema_version", "registry_digest", "releases"], "release-index");
  const digest = sha256Hex(canonicalJson(value));
  if (release.schema_version !== "1" || release.registry_digest !== digest || !Array.isArray(release.releases)) {
    throw new Error("invalid definition registry: release index does not match immutable registry digest");
  }
  const releases = parseReleaseEvidence(release.releases, document);
  return deepFreeze({
    digest,
    agents: Object.freeze(document.agents),
    prompts: Object.freeze(document.prompts),
    templates,
    releases: Object.freeze(releases),
  });
}

function parseReleaseEvidence(values: unknown[], document: RegistryDocument): DefinitionReleaseEvidence[] {
  const identities = new Set<string>();
  return values.map((value, index) => {
    const release = object(value, `release-index.releases[${index}]`);
    exactKeys(release, ["definition_kind", "definition_key", "definition_version", "definition_digest", "activation_evidence"], `release-index.releases[${index}]`);
    if (!(["agent", "prompt"] as unknown[]).includes(release.definition_kind)
      || typeof release.definition_key !== "string" || !release.definition_key
      || typeof release.definition_version !== "string" || !release.definition_version
      || typeof release.definition_digest !== "string" || !/^[0-9a-f]{64}$/.test(release.definition_digest)) {
      throw new Error("invalid definition registry: malformed release evidence identity");
    }
    const identity = `${release.definition_kind}:${release.definition_key}@${release.definition_version}`;
    if (identities.has(identity)) throw new Error(`invalid definition registry: duplicate release evidence for ${identity}`);
    identities.add(identity);
    const definition = release.definition_kind === "agent"
      ? document.agents.find((item) => item.agent_key === release.definition_key && item.version === release.definition_version)
      : document.prompts.find((item) => item.prompt_key === release.definition_key && item.version === release.definition_version);
    if (!definition || definitionDigest(definition) !== release.definition_digest) {
      throw new Error(`invalid definition registry: release evidence digest mismatch for ${identity}`);
    }
    const rawEvidence = object(release.activation_evidence, `${identity}.activation_evidence`);
    let activation_evidence: ActivationEvidence;
    if (rawEvidence.type === "golden_baseline") {
      exactKeys(rawEvidence, ["type", "reference", "sha256"], `${identity}.activation_evidence`);
      if (typeof rawEvidence.reference !== "string" || !rawEvidence.reference
        || typeof rawEvidence.sha256 !== "string" || !/^[0-9a-f]{64}$/.test(rawEvidence.sha256)) {
        throw new Error(`invalid definition registry: invalid activation evidence for ${identity}`);
      }
      activation_evidence = { type: "golden_baseline", reference: rawEvidence.reference, sha256: rawEvidence.sha256 };
    } else if (rawEvidence.type === "eval_report") {
      exactKeys(rawEvidence, ["type", "report_id"], `${identity}.activation_evidence`);
      if (typeof rawEvidence.report_id !== "string" || !rawEvidence.report_id) {
        throw new Error(`invalid definition registry: invalid activation evidence for ${identity}`);
      }
      activation_evidence = { type: "eval_report", report_id: rawEvidence.report_id };
    } else {
      throw new Error(`invalid definition registry: invalid activation evidence for ${identity}`);
    }
    return {
      definition_kind: release.definition_kind as "agent" | "prompt",
      definition_key: release.definition_key,
      definition_version: release.definition_version,
      definition_digest: release.definition_digest,
      activation_evidence,
    };
  });
}

export function activeAgent(registry: DefinitionRegistry, key: string): AgentDefinition {
  const matches = registry.agents.filter((item) => item.agent_key === key && item.status === "active");
  if (matches.length !== 1) throw new Error(`definition not found or not unique: ${key}`);
  return matches[0]!;
}

/** Fails startup when any Pi production phase is absent from the active code-owned definition. */
export function assertProductionExecutionIntegrity(registry: DefinitionRegistry): void {
  const agent = activeAgent(registry, "personal.general");
  if (agent.executor_owner !== "pi") throw new Error("production definition personal.general is not owned by Pi");
  const expected = ["personal.turn", "personal.tool_followup", "personal.compaction", "personal.branch_summary"];
  const actual = Object.keys(agent.nodes).sort();
  if (canonicalJson(actual) !== canonicalJson(expected.sort())) {
    throw new Error(`production definition personal.general has incomplete node inventory: ${canonicalJson(actual)}`);
  }
}

export function compilePrompt(
  registry: DefinitionRegistry,
  agentKey: string,
  agentVersion: string,
  nodeKey: string,
  input: PromptCompileInput,
  toolProfile: "chat" | "workspace",
  toolSchema: unknown | null = null,
): PromptSnapshot {
  return compilePromptInternal(registry, agentKey, agentVersion, nodeKey, input, toolProfile, toolSchema, false);
}

/** Recompiles immutable historical input without making the definition executable. */
export function compilePromptForReplay(
  registry: DefinitionRegistry,
  agentKey: string,
  agentVersion: string,
  nodeKey: string,
  input: PromptCompileInput,
  toolProfile: "chat" | "workspace",
  toolSchema: unknown | null = null,
): PromptSnapshot {
  return compilePromptInternal(registry, agentKey, agentVersion, nodeKey, input, toolProfile, toolSchema, true);
}

function compilePromptInternal(
  registry: DefinitionRegistry,
  agentKey: string,
  agentVersion: string,
  nodeKey: string,
  input: PromptCompileInput,
  toolProfile: "chat" | "workspace",
  toolSchema: unknown | null,
  allowHistorical: boolean,
): PromptSnapshot {
  if (input.schema_version !== "1") throw new Error("prompt compile error: unknown input schema");
  const agent = registry.agents.find((item) => item.agent_key === agentKey && item.version === agentVersion);
  if (!agent || (!allowHistorical && (agent.status === "candidate" || agent.status === "revoked"))) {
    throw new Error(`definition is not executable: ${agentKey}@${agentVersion}`);
  }
  if (!agent.tool_profiles.includes(toolProfile)) throw new Error(`prompt compile error: tool profile ${toolProfile} is not allowed`);
  const reference = agent.nodes[nodeKey];
  if (!reference) throw new Error(`prompt compile error: node ${nodeKey} is not declared`);
  const prompt = registry.prompts.find((item) => item.prompt_key === reference.key && item.version === reference.version);
  if (!prompt || (!allowHistorical && (prompt.status === "candidate" || prompt.status === "revoked"))) {
    throw new Error(`definition is not executable: ${reference.key}@${reference.version}`);
  }
  if (prompt.tool_profile !== null && prompt.tool_profile !== toolProfile) {
    throw new Error("prompt compile error: prompt requires a different tool profile");
  }
  validateToolSchema(agent, toolProfile, toolSchema);
  const declarations = new Map(prompt.variables.map((variable) => [variable.name, variable]));
  for (const name of Object.keys(input.variables ?? {})) {
    if (name === "fragments" || !declarations.has(name)) throw new Error(`prompt compile error: unknown variable ${name}`);
  }
  const fragmentVariable = declarations.get("fragments");
  if (!fragmentVariable || fragmentVariable.value_type !== "fragments") throw new Error("prompt compile error: fragments variable is not declared");
  if (fragmentVariable.required && input.fragments.length === 0) throw new Error("prompt compile error: fragments is required");
  const ids = new Set<string>();
  let totalBytes = 0;
  const content = input.fragments.map((fragment) => {
    if (!fragment.id.trim() || !fragment.source.trim() || ids.has(fragment.id)) {
      throw new Error("prompt compile error: fragment id/source is invalid or duplicated");
    }
    ids.add(fragment.id);
    const hasText = typeof fragment.content === "string" && fragment.content.length > 0;
    const hasAsset = fragment.asset !== undefined;
    if (hasText === hasAsset) throw new Error("prompt compile error: fragment must contain exactly one text or asset reference");
    if (hasAsset && !validAssetReference(fragment.asset)) {
      throw new Error("prompt compile error: asset reference format is invalid");
    }
    const serialized = hasText ? fragment.content! : canonicalJson(fragment.asset);
    totalBytes += Buffer.byteLength(serialized);
    return serialized;
  });
  if (totalBytes > fragmentVariable.max_bytes) throw new Error("prompt compile error: fragments exceed max_bytes");
  const system = template(registry, prompt.system_template);
  let user = template(registry, prompt.user_template).replace("{{fragments}}", content.join("\n"));
  const variables: Record<string, unknown> = { fragments: structuredClone(input.fragments) };
  for (const variable of prompt.variables.filter((item) => item.value_type !== "fragments")) {
    const value = input.variables?.[variable.name];
    if (variable.required && value === undefined) throw new Error(`prompt compile error: variable ${variable.name} is required`);
    const rendered = value === undefined ? "" : renderVariable(variable, value);
    if (Buffer.byteLength(rendered) > variable.max_bytes) throw new Error(`prompt compile error: variable ${variable.name} exceeds max_bytes`);
    user = user.replaceAll(`{{${variable.name}}}`, rendered);
    if (value !== undefined) variables[variable.name] = structuredClone(value);
  }
  if (system.includes("{{") || user.includes("{{")) throw new Error("prompt compile error: unresolved or dynamic System variable");
  return deepFreeze({
    schema_version: "1",
    registry_digest: registry.digest,
    agent_key: agent.agent_key,
    agent_version: agent.version,
    prompt_key: prompt.prompt_key,
    prompt_version: prompt.version,
    node_key: nodeKey,
    system,
    user,
    variables,
    fragments: structuredClone(input.fragments),
    tool_profile: toolProfile,
    output_schema: prompt.output_schema === null ? null : renderOutputSchema(prompt.output_schema, variables),
    tool_schema: structuredClone(toolSchema),
    max_output_tokens: prompt.max_output_tokens,
  });
}

function validAssetReference(value: AssetReference | undefined): value is AssetReference {
  if (!value || !value.asset_id || !value.version || !/^[0-9a-f]{64}$/.test(value.sha256)) return false;
  if (!/^(image|audio|video)\/[A-Za-z0-9.+-]+$/.test(value.mime)) return false;
  return value.metadata === undefined || Object.values(value.metadata).every((item) => typeof item === "string");
}

function deepFreeze<T>(value: T): T {
  if (value !== null && typeof value === "object" && !Object.isFrozen(value)) {
    for (const item of Object.values(value as Record<string, unknown>)) deepFreeze(item);
    Object.freeze(value);
  }
  return value;
}

export function behaviorFingerprint(input: ModelBehavior): { digest: string; normalized: ModelBehavior } {
  const url = new URL(input.request_base_url);
  if (url.protocol !== "http:" && url.protocol !== "https:") throw new Error("model fingerprint error: request_base_url must be http(s)");
  url.username = "";
  url.password = "";
  url.search = "";
  url.hash = "";
  url.pathname = url.pathname.replace(/\/+$/, "") || "/";
  const normalizedUrl = url.toString().replace(/\/$/, "");
  const normalized: ModelBehavior = {
    protocol: input.protocol.trim().toLowerCase(),
    request_base_url: normalizedUrl,
    upstream_model: input.upstream_model.trim(),
    reasoning_effort: input.reasoning_effort?.trim().toLowerCase() || null,
    max_output_tokens: input.max_output_tokens,
    context_window: input.context_window,
    settings: removeSensitiveFields(input.settings),
  };
  if (!["openai_responses", "openai_chat_completions"].includes(normalized.protocol)
    || !normalized.upstream_model || !positiveInteger(normalized.max_output_tokens) || !positiveInteger(normalized.context_window)
    || (normalized.reasoning_effort !== null && !["minimal", "low", "medium", "high", "xhigh"].includes(normalized.reasoning_effort))
    || normalized.settings === null || typeof normalized.settings !== "object" || Array.isArray(normalized.settings)) {
    throw new Error("model fingerprint error: model behavior is incomplete");
  }
  return { digest: sha256Hex(canonicalJson(normalized)), normalized };
}

export function canonicalJson(value: unknown): string {
  const normalizeValue = (current: unknown): unknown => {
    if (Array.isArray(current)) return current.map(normalizeValue);
    if (current !== null && typeof current === "object") {
      return Object.fromEntries(
        Object.entries(current as Record<string, unknown>)
          .sort(([left], [right]) => (left < right ? -1 : left > right ? 1 : 0))
          .map(([key, item]) => [key, normalizeValue(item)]),
      );
    }
    return current;
  };
  return JSON.stringify(normalizeValue(value));
}

export function sha256Hex(value: string | Uint8Array): string {
  return createHash("sha256").update(value).digest("hex");
}

/** Lifecycle status is release metadata and must not invalidate an immutable binding. */
export function definitionDigest(definition: AgentDefinition | PromptDefinition): string {
  const { status: _status, ...content } = definition;
  return sha256Hex(canonicalJson(content));
}

function parseRegistry(value: unknown): RegistryDocument {
  const root = object(value, "registry");
  exactKeys(root, ["schema_version", "agents", "prompts"], "registry");
  if (root.schema_version !== "1" || !Array.isArray(root.agents) || !Array.isArray(root.prompts)) {
    throw new Error("invalid definition registry: invalid root contract");
  }
  const agents = root.agents.map((item, index) => parseAgent(item, index));
  const prompts = root.prompts.map((item, index) => parsePrompt(item, index));
  return { schema_version: "1", agents, prompts };
}

function parseAgent(value: unknown, index: number): AgentDefinition {
  const item = object(value, `agent[${index}]`);
  exactKeys(item, AGENT_KEYS, `agent[${index}]`);
  const requirements = object(item.model_requirements, "model_requirements");
  exactKeys(requirements, ["text", "tool_calling", "structured_output", "vision", "reasoning", "min_context_window"], "model_requirements");
  const nodes = object(item.nodes, "nodes");
  for (const [node, reference] of Object.entries(nodes)) {
    const versioned = object(reference, node);
    exactKeys(versioned, ["key", "version"], node);
    if (!validKey(versioned.key) || !validVersion(versioned.version)) throw new Error(`invalid definition registry: invalid reference ${node}`);
  }
  const statuses: DefinitionStatus[] = ["candidate", "active", "supported", "revoked"];
  const owners: ExecutorOwner[] = ["rust", "pi"];
  if (!validKey(item.agent_key) || !validVersion(item.version)
    || !statuses.includes(item.status as DefinitionStatus) || !owners.includes(item.executor_owner as ExecutorOwner)
    || typeof item.role !== "string" || !item.role.trim()
    || !stringArray(item.goals, true) || !stringArray(item.constraints, false)
    || !booleanRequirements(requirements) || !positiveInteger(requirements.min_context_window)
    || !stringArray(item.tool_profiles, true) || !(item.tool_profiles as string[]).every((profile) => profile === "chat" || profile === "workspace")
    || new Set(item.tool_profiles as string[]).size !== (item.tool_profiles as string[]).length
    || !stringArray(item.tools, false) || !(item.tools as string[]).every((tool) => ["read", "write", "edit", "bash"].includes(tool))
    || new Set(item.tools as string[]).size !== (item.tools as string[]).length
    || Object.keys(nodes).length === 0) {
    throw new Error(`invalid definition registry: invalid agent[${index}] contract`);
  }
  if ((item.tools as string[]).length > 0 && !(item.tool_profiles as string[]).includes("workspace")) {
    throw new Error(`invalid definition registry: agent[${index}] tools require workspace`);
  }
  return item as unknown as AgentDefinition;
}

function parsePrompt(value: unknown, index: number): PromptDefinition {
  const item = object(value, `prompt[${index}]`);
  exactKeys(item, PROMPT_KEYS, `prompt[${index}]`);
  if (!Array.isArray(item.variables)) throw new Error(`invalid definition registry: prompt[${index}].variables`);
  for (const rawVariable of item.variables) {
    const variable = object(rawVariable, "variable");
    exactKeys(variable, ["name", "value_type", "required", "trust", "max_bytes"], "variable");
    if (!validVariableName(variable.name)
      || !["string", "string_list", "integer", "json", "fragments"].includes(String(variable.value_type))
      || typeof variable.required !== "boolean"
      || !["platform", "confirmed_fact", "reference", "user_instruction", "steer", "follow_up", "candidate"].includes(String(variable.trust))
      || !positiveInteger(variable.max_bytes)) {
      throw new Error(`invalid definition registry: invalid variable ${String(variable.name)}`);
    }
  }
  if (!validKey(item.prompt_key) || !validVersion(item.version)
    || !["candidate", "active", "supported", "revoked"].includes(String(item.status))
    || !["rust", "pi"].includes(String(item.executor_owner))
    || typeof item.system_template !== "string" || typeof item.user_template !== "string"
    || (item.output_schema !== null && !validOutputSchema(item.output_schema))
    || (item.tool_profile !== null && item.tool_profile !== "chat" && item.tool_profile !== "workspace")
    || (item.max_output_tokens !== null && !positiveInteger(item.max_output_tokens))) {
    throw new Error(`invalid definition registry: invalid prompt[${index}] contract`);
  }
  return item as unknown as PromptDefinition;
}

function validateRegistry(document: RegistryDocument, templates: ReadonlyMap<string, string>): void {
  const agents = new Set<string>();
  const active = new Set<string>();
  const prompts = new Map<string, PromptDefinition>();
  for (const prompt of document.prompts) {
    const id = `${prompt.prompt_key}@${prompt.version}`;
    if (prompts.has(id)) throw new Error(`invalid definition registry: duplicate prompt ${id}`);
    prompts.set(id, prompt);
    const system = templates.get(prompt.system_template);
    const user = templates.get(prompt.user_template);
    if (!system || system.includes("{{") || !user?.includes("{{fragments}}")) throw new Error(`invalid definition registry: invalid trust boundary in ${id}`);
    const variableNames = new Set<string>();
    for (const variable of prompt.variables) {
      if (variableNames.has(variable.name)
        || (!user.includes(`{{${variable.name}}}`) && !containsExactPlaceholder(prompt.output_schema, variable.name))) {
        throw new Error(`invalid definition registry: invalid variable ${variable.name} in ${id}`);
      }
      variableNames.add(variable.name);
    }
    if (prompt.variables.filter((item) => item.name === "fragments" && item.value_type === "fragments").length !== 1) {
      throw new Error(`invalid definition registry: ${id} must declare fragments`);
    }
  }
  for (const agent of document.agents) {
    const id = `${agent.agent_key}@${agent.version}`;
    if (agents.has(id)) throw new Error(`invalid definition registry: duplicate agent ${id}`);
    agents.add(id);
    if (agent.status === "active" && active.has(agent.agent_key)) throw new Error(`invalid definition registry: multiple active ${agent.agent_key}`);
    if (agent.status === "active") active.add(agent.agent_key);
    for (const [node, reference] of Object.entries(agent.nodes)) {
      const prompt = prompts.get(`${reference.key}@${reference.version}`);
      if (!prompt) throw new Error(`invalid definition registry: missing prompt for ${node}`);
      if (prompt.executor_owner !== agent.executor_owner) throw new Error(`invalid definition registry: cross-owner prompt reference at ${node}`);
      if ((agent.status === "active" || agent.status === "supported")
        && (prompt.status === "candidate" || prompt.status === "revoked")) {
        throw new Error(`invalid definition registry: executable agent references unavailable prompt at ${node}`);
      }
    }
  }
}

function renderVariable(variable: VariableDefinition, value: unknown): string {
  switch (variable.value_type) {
    case "string":
      if (typeof value !== "string") throw new Error(`prompt compile error: variable ${variable.name} must be string`);
      return value;
    case "string_list":
      if (!Array.isArray(value) || !value.every((item) => typeof item === "string")) throw new Error(`prompt compile error: variable ${variable.name} must be string_list`);
      return value.join("\n");
    case "integer":
      if (typeof value !== "number" || !Number.isSafeInteger(value)) throw new Error(`prompt compile error: variable ${variable.name} must be integer`);
      return String(value);
    case "json":
      if (value === null || value === undefined) throw new Error(`prompt compile error: variable ${variable.name} must be json`);
      return canonicalJson(value);
    case "fragments":
      throw new Error("prompt compile error: fragments must use structured fragments input");
  }
}

function containsExactPlaceholder(value: unknown, name: string): boolean {
  const placeholder = `{{${name}}}`;
  if (typeof value === "string") return value === placeholder;
  if (Array.isArray(value)) return value.some((item) => containsExactPlaceholder(item, name));
  if (value !== null && typeof value === "object") {
    return Object.values(value as Record<string, unknown>).some((item) => containsExactPlaceholder(item, name));
  }
  return false;
}

function renderOutputSchema(value: unknown, variables: Readonly<Record<string, unknown>>): unknown {
  if (typeof value === "string") {
    if (/^{{[a-z][a-z0-9_]*}}$/.test(value)) {
      const name = value.slice(2, -2);
      if (!(name in variables)) throw new Error(`prompt compile error: output schema variable ${name} is missing or undeclared`);
      return structuredClone(variables[name]);
    }
    if (value.includes("{{")) throw new Error("prompt compile error: output schema variables must occupy a complete JSON value");
    return value;
  }
  if (Array.isArray(value)) return value.map((item) => renderOutputSchema(item, variables));
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .map(([key, item]) => [key, renderOutputSchema(item, variables)]),
    );
  }
  return value;
}

function validateToolSchema(agent: AgentDefinition, profile: "chat" | "workspace", schema: unknown | null): void {
  if (schema === null) {
    if (profile === "workspace" && agent.tools.length > 0) throw new Error("prompt compile error: workspace tool schema is required");
    return;
  }
  if (!Array.isArray(schema)) throw new Error("prompt compile error: tool schema must be an array");
  if (profile === "chat" && schema.length > 0) throw new Error("prompt compile error: chat profile does not allow tools");
  const names = new Set<string>();
  for (const rawTool of schema) {
    const tool = object(rawTool, "tool schema");
    if (typeof tool.name !== "string" || !agent.tools.includes(tool.name as AgentDefinition["tools"][number]) || names.has(tool.name)) {
      throw new Error(`prompt compile error: tool ${String(tool.name)} is not allowed or duplicated`);
    }
    names.add(tool.name);
  }
  if (profile === "workspace" && (names.size !== agent.tools.length || agent.tools.some((tool) => !names.has(tool)))) {
    throw new Error("prompt compile error: workspace tool schema does not match definition");
  }
}

function validKey(value: unknown): boolean {
  return typeof value === "string" && /^[a-z][a-z0-9._-]*$/.test(value);
}

function validVersion(value: unknown): boolean {
  return typeof value === "string" && /^\d+\.\d+\.\d+$/.test(value);
}

function validVariableName(value: unknown): boolean {
  return typeof value === "string" && /^[a-z][a-z0-9_]*$/.test(value);
}

function positiveInteger(value: unknown): boolean {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0;
}

function stringArray(value: unknown, nonEmpty: boolean): boolean {
  return Array.isArray(value) && (!nonEmpty || value.length > 0) && value.every((item) => typeof item === "string" && item.length > 0);
}

function booleanRequirements(value: Record<string, unknown>): boolean {
  return ["text", "tool_calling", "structured_output", "vision", "reasoning"].every((key) => typeof value[key] === "boolean");
}

function validOutputSchema(value: unknown): boolean {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const schema = value as Record<string, unknown>;
  return typeof schema.name === "string" && schema.name.length > 0 && schema.strict === true
    && schema.schema !== null && typeof schema.schema === "object" && !Array.isArray(schema.schema);
}

function template(registry: DefinitionRegistry, path: string): string {
  const value = registry.templates.get(path);
  if (value === undefined) throw new Error(`invalid definition registry: missing template ${path}`);
  return value;
}

function validateTemplatePath(value: string): void {
  const normalized = normalize(value);
  if (isAbsolute(value) || normalized.startsWith(`..${sep}`) || normalized === ".." || !normalized.startsWith(`templates${sep}`)) {
    throw new Error("invalid definition registry: template path escapes registry");
  }
}

function object(value: unknown, label: string): Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) throw new Error(`invalid definition registry: ${label} must be an object`);
  return value as Record<string, unknown>;
}

function exactKeys(value: Record<string, unknown>, expected: readonly string[], label: string): void {
  const allowed = new Set(expected);
  const unknown = Object.keys(value).find((key) => !allowed.has(key));
  const missing = expected.find((key) => !(key in value));
  if (unknown) throw new Error(`invalid definition registry: unknown field ${label}.${unknown}`);
  if (missing) throw new Error(`invalid definition registry: missing field ${label}.${missing}`);
}

function removeSensitiveFields(value: unknown, preserveEmpty = true): unknown {
  if (Array.isArray(value)) return value.map((item) => removeSensitiveFields(item, false)).filter((item) => item !== undefined);
  if (value !== null && typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>).flatMap(([key, item]) => {
      const normalized = key.toLowerCase().replaceAll("-", "_");
      const sensitive = normalized.includes("api_key") || normalized.includes("api_secret")
        || normalized.includes("authorization") || normalized.includes("cookie")
        || normalized.includes("credential") || normalized.includes("signature")
        || normalized.endsWith("_token") || ["token", "password", "secret"].includes(normalized);
      if (sensitive) return [];
      const redacted = removeSensitiveFields(item, false);
      return redacted === undefined ? [] : [[key, redacted] as const];
    });
    if (!preserveEmpty && Object.keys(value as Record<string, unknown>).length > 0 && entries.length === 0) return undefined;
    return Object.fromEntries(entries);
  }
  return value;
}
