import { createHash } from "node:crypto";
import { getEncoding, type Tiktoken } from "js-tiktoken";

import {
  canonicalJson,
  type AssetReference,
  type DefinitionStatus,
  type ExecutorOwner,
  type TrustLevel,
} from "./definitions.js";

export const CONTEXT_SCHEMA_VERSION = "2" as const;
export const ENCODING_CONTRACT_V1_DIGEST = "cfcf757abd2fceb98ca75ea57e0153123447dab26375938154c909640505f8bf";
const TOKENIZER_CACHE_LIMIT = 8;
export type ContextPriority = "p0" | "p1" | "p2" | "p3" | "p4";
export type ContextPayload =
  | { type: "text"; text: string }
  | { type: "message"; message: LogicalMessage }
  | { type: "asset"; asset: AssetReference };

export interface LogicalMessage {
  role: string;
  content: unknown;
  thinking?: string;
  tool_call_id?: string;
}

export interface ContextCandidate {
  candidate_id: string;
  source_kind: string;
  source_id: string;
  source_version: string;
  fact_key?: string;
  trust: TrustLevel;
  priority: ContextPriority;
  required: boolean;
  render_order: number;
  observed_at: string;
  valid_until?: string;
  supersedes: string[];
  content_hash: string;
  atomic_group_id?: string;
  payload: ContextPayload;
}

export interface ContextAtomicGroup { group_id: string; member_ids: string[] }
export interface FramingRules {
  per_message_tokens: number;
  per_tool_tokens: number;
  request_tokens: number;
  reply_priming_tokens: number;
}
export type TokenizerMode =
  | { mode: "exact"; encoding: "cl100k_base" | "o200k_base"; asset_digest: string }
  | { mode: "conservative"; algorithm: "utf8-byte-upper-bound@1" };
export interface TokenizerProfile {
  profile_key: string;
  version: string;
  status: DefinitionStatus;
  implementation_version: string;
  mode: TokenizerMode;
  applicable_protocols: string[];
  applicable_model_families: string[];
  framing: FramingRules;
  safety_reserve_tokens: number;
}
export interface ContextPolicyDefinition {
  policy_key: string;
  version: string;
  status: DefinitionStatus;
  executor_owners: ExecutorOwner[];
  allowed_sources: string[];
  required_sources: string[];
  stable_sort: string[];
}
export interface PreparedPromptEnvelope {
  system: string;
  user_template_fixed: string;
  tool_schema: unknown | null;
  output_schema: unknown | null;
  protocol_envelope_tokens: number;
  max_output_tokens: number;
}
export interface ContextCompileRequest {
  schema_version: "2";
  owner: ExecutorOwner;
  owner_id: string;
  node_key: string;
  compiled_at: string;
  model_context_window: number;
  policy: ContextPolicyDefinition;
  tokenizer_profile: TokenizerProfile;
  prepared_prompt: PreparedPromptEnvelope;
  candidates: ContextCandidate[];
  atomic_groups: ContextAtomicGroup[];
}
export interface BudgetLedger {
  model_context_window: number;
  system_prompt_tokens: number;
  user_template_fixed_tokens: number;
  tool_schema_tokens: number;
  output_schema_tokens: number;
  protocol_envelope_tokens: number;
  max_output_tokens: number;
  safety_reserve_tokens: number;
  dynamic_context_budget: number;
  selected_context_tokens: number;
  final_input_tokens: number;
}
export type ContextDecisionCode = "selected" | "expired" | "superseded" | "duplicate_identity" | "duplicate_content" | "atomic_group_excluded" | "budget_excluded";
export interface ContextDecision {
  candidate_id: string;
  source_kind: string;
  source_id: string;
  source_version: string;
  trust: TrustLevel;
  priority: ContextPriority;
  required: boolean;
  render_order: number;
  content_hash: string;
  token_count: number;
  decision: ContextDecisionCode;
  selected_payload?: ContextPayload;
}
export interface LogicalModelInput {
  system: string;
  messages: LogicalMessage[];
  tool_schema: unknown | null;
  output_schema: unknown | null;
}
export interface CompiledContext {
  schema_version: "2";
  owner: ExecutorOwner;
  owner_id: string;
  node_key: string;
  compiled_at: string;
  policy_key: string;
  policy_version: string;
  tokenizer_profile_key: string;
  tokenizer_profile_version: string;
  tokenizer_mode: "exact" | "conservative";
  budget: BudgetLedger;
  decisions: ContextDecision[];
  selected_order: string[];
  logical_input: LogicalModelInput;
  digest: string;
}
export interface ContextSnapshot {
  schema_version: "2";
  owner: ExecutorOwner;
  owner_id: string;
  node_key: string;
  compiled_at: string;
  policy_key: string;
  policy_version: string;
  tokenizer_profile_key: string;
  tokenizer_profile_version: string;
  tokenizer_mode: "exact" | "conservative";
  budget: BudgetLedger;
  decisions: ContextDecision[];
  selected_order: string[];
  logical_input: LogicalModelInput;
  digest: string;
}
export type CompileFailureStage = "schema" | "eligibility" | "conflict" | "tokenizer" | "budget" | "finalize";
export interface ContextCompileAttempt {
  schema_version: "2";
  owner: ExecutorOwner;
  owner_id: string;
  node_key: string;
  compiled_at: string;
  stage: CompileFailureStage;
  code: string;
  budget: BudgetLedger | null;
  decisions: ContextDecision[];
  digest: string;
}

const exactCache = new Map<string, Tiktoken>();

export class ProfileTokenizer {
  private constructor(readonly profile: TokenizerProfile, private readonly encoding?: Tiktoken) {}

  static create(profile: TokenizerProfile): ProfileTokenizer {
    if (profile.status === "revoked" || profile.applicable_protocols.length === 0
      || !safeNonNegativeInteger(profile.safety_reserve_tokens)) {
      throw new ContextCompileError("tokenizer", "tokenizer_profile_unavailable");
    }
    if (profile.mode.mode === "exact") {
      if (profile.mode.asset_digest !== ENCODING_CONTRACT_V1_DIGEST || !["cl100k_base", "o200k_base"].includes(profile.mode.encoding)) {
        throw new ContextCompileError("tokenizer", "tokenizer_profile_unavailable");
      }
      const key = `${profile.version}:${profile.mode.asset_digest}:${profile.mode.encoding}`;
      let encoding = exactCache.get(key);
      if (!encoding) {
        encoding = getEncoding(profile.mode.encoding);
        if (exactCache.size >= TOKENIZER_CACHE_LIMIT) exactCache.delete(exactCache.keys().next().value!);
        exactCache.set(key, encoding);
      }
      return new ProfileTokenizer(profile, encoding);
    }
    if (profile.mode.algorithm !== "utf8-byte-upper-bound@1") {
      throw new ContextCompileError("tokenizer", "tokenizer_profile_unavailable");
    }
    return new ProfileTokenizer(profile);
  }

  countText(text: string): number {
    return this.encoding ? this.encoding.encode(text, "all").length : Buffer.byteLength(text, "utf8");
  }

  countJson(value: unknown): number { return this.countText(canonicalJson(value)); }

  countPayload(payload: ContextPayload): number {
    if (payload.type === "text") return this.countText(payload.text);
    if (payload.type === "asset") return this.countJson(payload.asset);
    return this.countMessage(payload.message, true);
  }

  countMessage(message: LogicalMessage, includeFraming: boolean): number {
    const content = typeof message.content === "string" ? this.countText(message.content) : this.countJson(message.content);
    return this.countText(message.role) + content
      + (message.thinking ? this.countText(message.thinking) : 0)
      + (message.tool_call_id ? this.countText(message.tool_call_id) : 0)
      + (includeFraming ? this.profile.framing.per_message_tokens : 0);
  }

  static cacheSize(): number { return exactCache.size; }
}

export function compileContext(request: ContextCompileRequest): CompiledContext {
  validateRequest(request);
  const tokenizer = ProfileTokenizer.create(request.tokenizer_profile);
  const budget = fixedBudget(request, tokenizer);
  const candidates = structuredClone(request.candidates).sort(stableCandidateOrder);
  const groups = validateGroups(candidates, request.atomic_groups);
  const decisions: ContextDecision[] = [];
  const eligible: Array<[ContextCandidate, number]> = [];
  const identities = new Set<string>();
  const hashes = new Set<string>();
  const superseded = new Set(candidates.flatMap((candidate) => candidate.supersedes));
  for (const candidate of candidates) {
    const tokenCount = tokenizer.countPayload(candidate.payload);
    const identity = canonicalJson([candidate.source_kind, candidate.source_id, candidate.source_version, candidate.candidate_id]);
    let decision: ContextDecisionCode | undefined;
    if (candidate.valid_until !== undefined && candidate.valid_until < request.compiled_at) decision = "expired";
    else if (superseded.has(candidate.candidate_id)) decision = "superseded";
    else if (identities.has(identity)) decision = "duplicate_identity";
    else if (hashes.has(candidate.content_hash)) decision = "duplicate_content";
    else { identities.add(identity); hashes.add(candidate.content_hash); }
    if (decision) {
      if (candidate.required && (decision === "expired" || decision === "superseded")) {
        throw new ContextCompileError("eligibility", "required_context_unavailable");
      }
      decisions.push(minimalDecision(candidate, tokenCount, decision));
    } else eligible.push([candidate, tokenCount]);
  }
  excludeIncompleteGroups(eligible, decisions, groups);
  detectConfirmedFactConflicts(eligible);

  const selectedIds = new Set<string>();
  let selectedTokens = 0;
  for (const [candidate, tokenCount] of eligible) {
    if (selectedIds.has(candidate.candidate_id)) continue;
    const groupMembers = candidate.atomic_group_id ? groups.get(candidate.atomic_group_id) : undefined;
    const group = groupMembers
      ? eligible.filter(([item]) => groupMembers.has(item.candidate_id))
      : [[candidate, tokenCount] as [ContextCandidate, number]];
    const groupTokens = group.reduce((total, [, tokens]) => total + tokens, 0);
    const groupRequired = group.some(([item]) => item.required || item.priority === "p0");
    if (selectedTokens + groupTokens <= budget.dynamic_context_budget) {
      selectedTokens += groupTokens;
      for (const [item] of group) selectedIds.add(item.candidate_id);
    } else if (groupRequired) throw new ContextCompileError("budget", "context_budget_exceeded");
  }

  const selected = eligible.filter(([candidate]) => selectedIds.has(candidate.candidate_id)).sort(([left], [right]) => renderCandidateOrder(left, right));
  for (const [candidate, tokenCount] of eligible) {
    const chosen = selectedIds.has(candidate.candidate_id);
    decisions.push(minimalDecision(candidate, tokenCount, chosen ? "selected" : "budget_excluded", chosen ? candidate.payload : undefined));
  }
  decisions.sort((left, right) => left.candidate_id.localeCompare(right.candidate_id, "en"));
  budget.selected_context_tokens = selectedTokens;
  const compiled: CompiledContext = {
    schema_version: "2", owner: request.owner, owner_id: request.owner_id, node_key: request.node_key,
    compiled_at: request.compiled_at, policy_key: request.policy.policy_key, policy_version: request.policy.version,
    tokenizer_profile_key: request.tokenizer_profile.profile_key, tokenizer_profile_version: request.tokenizer_profile.version,
    tokenizer_mode: request.tokenizer_profile.mode.mode, budget, decisions,
    selected_order: selected.map(([candidate]) => candidate.candidate_id),
    logical_input: {
      system: request.prepared_prompt.system,
      messages: selected.map(([candidate]) => payloadMessage(candidate.payload)),
      tool_schema: request.prepared_prompt.tool_schema,
      output_schema: request.prepared_prompt.output_schema,
    },
    digest: "",
  };
  compiled.digest = sha256Hex(canonicalJson({ ...compiled, digest: undefined }));
  return deepFreeze(compiled);
}

export function finalizeContext(
  compiled: CompiledContext,
  tokenizerProfile: TokenizerProfile,
  logicalInput: LogicalModelInput,
): ContextSnapshot {
  if (tokenizerProfile.profile_key !== compiled.tokenizer_profile_key
    || tokenizerProfile.version !== compiled.tokenizer_profile_version
    || logicalInput.system !== compiled.logical_input.system
    || canonicalJson(logicalInput.tool_schema) !== canonicalJson(compiled.logical_input.tool_schema)
    || canonicalJson(logicalInput.output_schema) !== canonicalJson(compiled.logical_input.output_schema)) {
    throw new ContextCompileError("finalize", "context_finalize_mismatch");
  }
  const tokenizer = ProfileTokenizer.create(tokenizerProfile);
  const budget = structuredClone(compiled.budget);
  const messageTokens = logicalInput.messages.reduce((total, message) => total + tokenizer.countMessage(message, false), 0);
  const toolTokens = logicalInput.tool_schema === null ? 0 : tokenizer.countJson(logicalInput.tool_schema);
  const outputTokens = logicalInput.output_schema === null ? 0 : tokenizer.countJson(logicalInput.output_schema);
  budget.final_input_tokens = tokenizer.countText(logicalInput.system) + messageTokens + toolTokens + outputTokens
    + budget.protocol_envelope_tokens;
  if (budget.final_input_tokens + budget.max_output_tokens + budget.safety_reserve_tokens > budget.model_context_window) {
    throw new ContextCompileError("finalize", "context_budget_exceeded");
  }
  const snapshot: ContextSnapshot = {
    schema_version: compiled.schema_version,
    owner: compiled.owner,
    owner_id: compiled.owner_id,
    node_key: compiled.node_key,
    compiled_at: compiled.compiled_at,
    policy_key: compiled.policy_key,
    policy_version: compiled.policy_version,
    tokenizer_profile_key: compiled.tokenizer_profile_key,
    tokenizer_profile_version: compiled.tokenizer_profile_version,
    tokenizer_mode: compiled.tokenizer_mode,
    budget,
    decisions: structuredClone(compiled.decisions),
    selected_order: [...compiled.selected_order],
    logical_input: structuredClone(logicalInput),
    digest: "",
  };
  snapshot.digest = sha256Hex(canonicalJson({ ...snapshot, digest: undefined }));
  return deepFreeze(snapshot);
}

export class ContextCompileError extends Error {
  constructor(readonly stage: CompileFailureStage, readonly code: string) { super(`${code} at ${stage}`); }

  attempt(request: ContextCompileRequest): ContextCompileAttempt {
    const attempt: ContextCompileAttempt = {
      schema_version: "2", owner: request.owner, owner_id: request.owner_id, node_key: request.node_key,
      compiled_at: request.compiled_at, stage: this.stage, code: this.code, budget: null, decisions: [], digest: "",
    };
    if (this.stage !== "schema" && this.stage !== "tokenizer") {
      try { attempt.budget = budgetEvidence(request, ProfileTokenizer.create(request.tokenizer_profile)); } catch { /* no trusted tokenizer evidence */ }
    }
    attempt.digest = sha256Hex(canonicalJson({ ...attempt, digest: undefined }));
    return deepFreeze(attempt);
  }
}

function validateRequest(request: ContextCompileRequest): void {
  if (request.schema_version !== "2" || !request.owner_id.trim() || !request.node_key.trim() || !validTimestamp(request.compiled_at)
    || !positiveInteger(request.model_context_window) || !request.policy.executor_owners.includes(request.owner)
    || !positiveInteger(request.prepared_prompt.max_output_tokens)
    || canonicalJson(request.policy.stable_sort) !== canonicalJson(["priority", "source_kind", "source_id", "source_version", "candidate_id"])
    || !["active", "supported"].includes(request.policy.status)) {
    throw new ContextCompileError("schema", "context_schema_invalid");
  }
  const allowed = new Set(request.policy.allowed_sources);
  const required = new Set(request.policy.required_sources);
  const ids = new Set<string>();
  for (const candidate of request.candidates) {
    if (!candidate.candidate_id.trim() || !candidate.source_kind.trim() || !candidate.source_id.trim() || !candidate.source_version.trim()
      || !safeNonNegativeInteger(candidate.render_order)
      || !validTimestamp(candidate.observed_at) || (candidate.valid_until !== undefined && !validTimestamp(candidate.valid_until))
      || !/^[0-9a-f]{64}$/.test(candidate.content_hash) || ids.has(candidate.candidate_id)
      || !allowed.has(candidate.source_kind) || (required.has(candidate.source_kind) && !candidate.required)
      || !validPayload(candidate.payload)) {
      throw new ContextCompileError("schema", "context_schema_invalid");
    }
    ids.add(candidate.candidate_id);
    if (sha256Hex(canonicalJson(candidate.payload)) !== candidate.content_hash) {
      throw new ContextCompileError("schema", "context_content_hash_mismatch");
    }
  }
  for (const candidate of request.candidates) {
    const supersedes = new Set(candidate.supersedes);
    if (supersedes.size !== candidate.supersedes.length
      || candidate.supersedes.some((id) => id === candidate.candidate_id || !ids.has(id))) {
      throw new ContextCompileError("schema", "context_schema_invalid");
    }
  }
}

function validTimestamp(value: string): boolean {
  return value.endsWith("Z") && Number.isFinite(Date.parse(value));
}

function validPayload(payload: ContextPayload): boolean {
  if (payload.type === "text") return payload.text.length > 0;
  if (payload.type === "message") {
    return ["user", "assistant", "tool", "toolResult"].includes(payload.message.role)
      && payload.message.content !== null
      && (payload.message.tool_call_id === undefined || payload.message.tool_call_id.trim().length > 0);
  }
  return payload.asset.asset_id.trim().length > 0 && payload.asset.version.trim().length > 0
    && payload.asset.mime.trim().length > 0 && /^[0-9a-f]{64}$/.test(payload.asset.sha256)
    && Object.keys(payload.asset.metadata ?? {}).every((key) => key.trim().length > 0);
}

function fixedBudget(request: ContextCompileRequest, tokenizer: ProfileTokenizer): BudgetLedger {
  const budget = budgetEvidence(request, tokenizer);
  const fixed = budget.system_prompt_tokens + budget.user_template_fixed_tokens + budget.tool_schema_tokens
    + budget.output_schema_tokens + budget.protocol_envelope_tokens + budget.max_output_tokens + budget.safety_reserve_tokens;
  if (fixed > request.model_context_window) throw new ContextCompileError("budget", "context_budget_exceeded");
  return budget;
}

function budgetEvidence(request: ContextCompileRequest, tokenizer: ProfileTokenizer): BudgetLedger {
  const system = tokenizer.countText(request.prepared_prompt.system);
  const user = tokenizer.countText(request.prepared_prompt.user_template_fixed);
  const tools = request.prepared_prompt.tool_schema === null ? 0 : tokenizer.countJson(request.prepared_prompt.tool_schema);
  const output = request.prepared_prompt.output_schema === null ? 0 : tokenizer.countJson(request.prepared_prompt.output_schema);
  const toolCount = request.prepared_prompt.tool_schema === null ? 0
    : Array.isArray(request.prepared_prompt.tool_schema) ? request.prepared_prompt.tool_schema.length : 1;
  const framing = tokenizer.profile.framing;
  const protocol = request.prepared_prompt.protocol_envelope_tokens + framing.request_tokens + framing.reply_priming_tokens
    + framing.per_message_tokens * 2 + framing.per_tool_tokens * toolCount;
  const fixed = system + user + tools + output + protocol
    + request.prepared_prompt.max_output_tokens + tokenizer.profile.safety_reserve_tokens;
  return { model_context_window: request.model_context_window, system_prompt_tokens: system, user_template_fixed_tokens: user,
    tool_schema_tokens: tools, output_schema_tokens: output, protocol_envelope_tokens: protocol,
    max_output_tokens: request.prepared_prompt.max_output_tokens, safety_reserve_tokens: tokenizer.profile.safety_reserve_tokens,
    dynamic_context_budget: Math.max(0, request.model_context_window - fixed), selected_context_tokens: 0, final_input_tokens: 0 };
}

function validateGroups(candidates: ContextCandidate[], input: ContextAtomicGroup[]): Map<string, Set<string>> {
  const candidateIds = new Set(candidates.map((item) => item.candidate_id));
  const groups = new Map<string, Set<string>>();
  for (const group of input) {
    const members = new Set(group.member_ids);
    if (!group.group_id || members.size < 2 || members.size !== group.member_ids.length
      || group.member_ids.some((id) => !candidateIds.has(id)) || groups.has(group.group_id)) {
      throw new ContextCompileError("schema", "context_atomic_group_invalid");
    }
    groups.set(group.group_id, members);
  }
  for (const candidate of candidates) {
    if (candidate.atomic_group_id && !groups.get(candidate.atomic_group_id)?.has(candidate.candidate_id)) {
      throw new ContextCompileError("schema", "context_atomic_group_invalid");
    }
  }
  for (const [groupId, members] of groups) {
    if ([...members].some((memberId) => candidates.find((candidate) => candidate.candidate_id === memberId)?.atomic_group_id !== groupId)) {
      throw new ContextCompileError("schema", "context_atomic_group_invalid");
    }
  }
  return groups;
}

function excludeIncompleteGroups(
  eligible: Array<[ContextCandidate, number]>,
  decisions: ContextDecision[],
  groups: Map<string, Set<string>>,
): void {
  const eligibleIds = new Set(eligible.map(([candidate]) => candidate.candidate_id));
  const incomplete = new Set([...groups.values()]
    .filter((members) => [...members].some((member) => !eligibleIds.has(member)))
    .flatMap((members) => [...members]));
  if (incomplete.size === 0) return;
  if (eligible.some(([candidate]) => incomplete.has(candidate.candidate_id) && (candidate.required || candidate.priority === "p0"))) {
    throw new ContextCompileError("eligibility", "required_context_unavailable");
  }
  for (let index = eligible.length - 1; index >= 0; index -= 1) {
    const [candidate, tokens] = eligible[index]!;
    if (incomplete.has(candidate.candidate_id)) {
      decisions.push(minimalDecision(candidate, tokens, "atomic_group_excluded"));
      eligible.splice(index, 1);
    }
  }
}

function detectConfirmedFactConflicts(eligible: Array<[ContextCandidate, number]>): void {
  const facts = new Map<string, string>();
  for (const [candidate] of eligible.filter(([item]) => item.trust === "confirmed_fact" && item.fact_key)) {
    const existing = facts.get(candidate.fact_key!);
    if (existing !== undefined && existing !== candidate.content_hash) throw new ContextCompileError("conflict", "context_conflict");
    facts.set(candidate.fact_key!, candidate.content_hash);
  }
}

function stableCandidateOrder(left: ContextCandidate, right: ContextCandidate): number {
  return left.priority.localeCompare(right.priority, "en") || Number(right.required) - Number(left.required)
    || left.source_kind.localeCompare(right.source_kind, "en") || left.source_id.localeCompare(right.source_id, "en")
    || left.source_version.localeCompare(right.source_version, "en") || left.candidate_id.localeCompare(right.candidate_id, "en");
}
function renderCandidateOrder(left: ContextCandidate, right: ContextCandidate): number {
  return left.render_order - right.render_order || stableCandidateOrder(left, right);
}
function payloadMessage(payload: ContextPayload): LogicalMessage {
  if (payload.type === "message") return structuredClone(payload.message);
  if (payload.type === "text") return { role: "user", content: payload.text };
  return { role: "user", content: structuredClone(payload.asset) };
}
function minimalDecision(candidate: ContextCandidate, token_count: number, decision: ContextDecisionCode, selected_payload?: ContextPayload): ContextDecision {
  return { candidate_id: candidate.candidate_id, source_kind: candidate.source_kind, source_id: candidate.source_id,
    source_version: candidate.source_version, trust: candidate.trust, priority: candidate.priority,
    required: candidate.required, render_order: candidate.render_order, content_hash: candidate.content_hash, token_count, decision,
    ...(selected_payload === undefined ? {} : { selected_payload: structuredClone(selected_payload) }) };
}
function sha256Hex(value: string): string { return createHash("sha256").update(value).digest("hex"); }
function positiveInteger(value: number): boolean { return Number.isSafeInteger(value) && value > 0; }
function safeNonNegativeInteger(value: number): boolean { return Number.isSafeInteger(value) && value >= 0; }
function deepFreeze<T>(value: T): T {
  if (value !== null && typeof value === "object" && !Object.isFrozen(value)) {
    Object.freeze(value);
    for (const child of Object.values(value as Record<string, unknown>)) deepFreeze(child);
  }
  return value;
}
