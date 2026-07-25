import {
  type AgentHarnessEvent,
  type AgentHarnessTool,
  type ExecutionToolContext,
} from "@earendil-works/pi-agent-core";
import type { AssistantMessage } from "@earendil-works/pi-ai";

import { RuntimeError } from "./errors.js";
import {
  activeAgent,
  canonicalJson,
  compilePromptForReplay,
  definitionDigest,
  sha256Hex,
  validateModelCapabilities as validateCapabilities,
  type AgentDefinition,
  type DefinitionRegistry,
  type PromptCompileInput,
  type PromptSnapshot,
} from "./definitions.js";
import {
  createPiModelRuntime,
  refreshPiModelRuntime,
  type ModelSnapshot,
  type PiModelRuntime,
  type ResolvedTextModel,
} from "./models.js";
import {
  cleanupSession,
  readSessionMetadata,
  SessionStore,
  type SessionView,
  type ToolProfile,
} from "./sessions.js";
import { redactUnknown } from "./redaction.js";
import { createWorkspaceTools } from "./tools.js";
import { NovexAgentHarness } from "./novex-harness.js";
import type { ModelCallListFilter, ModelCallSummary, SessionBinding } from "./persistence.js";

export interface TextModelResolver {
  resolveEnabledText(modelId: string): Promise<ResolvedTextModel>;
  ping(): Promise<void>;
}

export interface CreatedSession {
  session: SessionView;
  model: ModelSnapshot;
}

export interface UpgradeForkInput {
  agentKey: string;
  agentVersion: string;
  modelId: string;
  toolProfile: ToolProfile;
  legacyPromptDisposition?: "discard" | "user_instruction";
}

type RuntimeHarness = NovexAgentHarness;

interface ActiveRun {
  harness: RuntimeHarness;
  secrets: readonly string[];
}

export function toolsForProfile(profile: ToolProfile): AgentHarnessTool<ExecutionToolContext>[] {
  if (profile === "chat") return [];
  return createWorkspaceTools();
}

export class SessionCoordinator {
  private readonly activeRuns = new Map<string, ActiveRun>();
  private readonly reservations = new Set<string>();

  constructor(
    private readonly sessions: SessionStore,
    private readonly modelResolver: TextModelResolver,
    private readonly modelFactory: (config: ResolvedTextModel) => PiModelRuntime = createPiModelRuntime,
    private readonly definitions?: DefinitionRegistry,
  ) {}

  async createSession(input: {
    agentKey: string;
    modelId: string;
    toolProfile: ToolProfile;
    source: string;
  }): Promise<CreatedSession> {
    const definitions = this.requireDefinitions();
    const agent = activeAgent(definitions, input.agentKey);
    if (agent.executor_owner !== "pi") throw new RuntimeError("definition_contract_error", 422, "Agent executor owner 不是 Pi");
    if (!agent.tool_profiles.includes(input.toolProfile)) throw new RuntimeError("definition_contract_error", 422, "Agent 不允许所选 tool profile");
    const resolved = await this.modelResolver.resolveEnabledText(input.modelId);
    const runtime = this.modelFactory(resolved);
    validateModelCapabilities(agent, resolved);
    const session = await this.sessions.create({
      ...input,
      binding: {
        agent_key: agent.agent_key,
        agent_version: agent.version,
        agent_digest: definitionDigest(agent),
        prompt_bindings: structuredClone(agent.nodes),
        registry_digest: definitions.digest,
        tool_profile: input.toolProfile,
        model_id: input.modelId,
        behavior_fingerprint: runtime.snapshot.behavior_fingerprint,
        model_snapshot: runtime.snapshot,
        binding_status: "executable",
        migration_source: "created_versioned",
      },
    });
    const metadata = await session.getMetadata();
    await cleanupSession(session);
    return { session: await this.sessions.view(metadata.id), model: runtime.snapshot };
  }

  async prompt(
    sessionId: string,
    text: string,
    onAccepted: (model: ModelSnapshot) => Promise<void> | void,
    onEvent: (event: unknown) => Promise<void> | void,
  ): Promise<AssistantMessage> {
    this.reserve(sessionId);
    let opened: Awaited<ReturnType<SessionStore["open"]>> | undefined;
    let unsubscribe: (() => void) | undefined;
    try {
      await this.ensureSessionMigrated(sessionId);
      const metadata = await this.sessions.findMetadata(sessionId);
      const own = readSessionMetadata(metadata);
      const resolved = await this.modelResolver.resolveEnabledText(own.model_id);
      const runtime = this.modelFactory(resolved);
      const binding = this.validateBinding(sessionId, resolved, runtime);
      opened = await this.sessions.open(sessionId);
      const harness = this.createHarness(sessionId, opened, own.tool_profile, binding, runtime);
      unsubscribe = harness.subscribe(async (event: AgentHarnessEvent) => {
        await onEvent(redactUnknown(event, runtime.secrets));
      });
      this.activeRuns.set(sessionId, { harness, secrets: runtime.secrets });
      this.reservations.delete(sessionId);
      await onAccepted(runtime.snapshot);

      const response = await harness.prompt(text);
      if (response.stopReason === "error") {
        const message = String(redactUnknown(response.errorMessage ?? "模型执行失败", runtime.secrets));
        throw new RuntimeError("internal_error", 502, message);
      }
      return response;
    } finally {
      unsubscribe?.();
      this.activeRuns.delete(sessionId);
      this.reservations.delete(sessionId);
      if (opened) await cleanupSession(opened);
    }
  }

  async steer(sessionId: string, text: string): Promise<void> {
    await this.requireActive(sessionId).harness.steer(text);
  }

  async followUp(sessionId: string, text: string): Promise<void> {
    await this.requireActive(sessionId).harness.followUp(text);
  }

  async abort(sessionId: string): Promise<void> {
    await this.requireActive(sessionId).harness.abort();
  }

  async deleteSession(sessionId: string): Promise<void> {
    this.reserve(sessionId);
    try {
      await this.sessions.delete(sessionId);
    } finally {
      this.reservations.delete(sessionId);
    }
  }

  async forkSession(
    sessionId: string,
    entryId?: string,
    position: "before" | "at" = "at",
    upgrade?: UpgradeForkInput,
  ): Promise<SessionView> {
    this.reserve(sessionId);
    try {
      await this.ensureSessionMigrated(sessionId);
      const forked = upgrade
        ? await this.upgradeFork(sessionId, entryId, position, upgrade)
        : await this.sessions.fork(sessionId, entryId, position);
      const metadata = await forked.getMetadata();
      await cleanupSession(forked);
      return this.sessions.view(metadata.id);
    } finally {
      this.reservations.delete(sessionId);
    }
  }

  async sessionView(sessionId: string): Promise<SessionView> {
    await this.ensureSessionMigrated(sessionId);
    return this.sessions.view(sessionId);
  }

  async sessionEntries(sessionId: string, afterSequence: number, limit: number) {
    await this.ensureSessionMigrated(sessionId);
    return this.sessions.entries(sessionId, afterSequence, limit);
  }

  async listSessions(): Promise<SessionView[]> {
    const plan = await this.sessions.legacyMigrationPlan();
    for (const item of plan) {
      if (item.disposition !== "unmapped") await this.ensureSessionMigrated(item.session_id);
    }
    return this.sessions.list();
  }

  async legacyMigrationPlan() {
    return {
      schema_version: "1",
      dry_run: true,
      items: await this.sessions.legacyMigrationPlan(),
    };
  }

  async compact(sessionId: string, instructions?: string): Promise<unknown> {
    return this.withIdleHarness(sessionId, async ({ harness }) => harness.compact(instructions));
  }

  async navigateTree(
    sessionId: string,
    entryId: string,
    options?: { summarize?: boolean; instructions?: string; label?: string },
  ): Promise<unknown> {
    return this.withIdleHarness(sessionId, async ({ harness }) =>
      harness.navigateTree(entryId, {
        summarize: options?.summarize ?? false,
        ...(options?.instructions ? { customInstructions: options.instructions } : {}),
        ...(options?.label ? { label: options.label } : {}),
      }),
    );
  }

  async close(): Promise<void> {
    await Promise.allSettled([...this.activeRuns.values()].map(({ harness }) => harness.abort()));
    await this.sessions.close();
  }

  async listModelCalls(filter: ModelCallListFilter, limit: number, offset: number) {
    if (filter.sessionId) await this.sessions.findMetadata(filter.sessionId);
    const page = this.sessions.novex.queryModelCalls(filter, limit, offset);
    return { ...page, items: page.items.map(modelCallSummaryDto) };
  }

  modelCall(id: string): Record<string, unknown> {
    const record = modelCallRecordDto(this.sessions.novex.modelCall(id));
    return {
      schema_version: "1",
      source_runtime: "pi",
      record_hash: sha256Hex(canonicalJson(record)),
      record,
    };
  }

  exportModelCall(id: string): Record<string, unknown> {
    return this.modelCall(id);
  }

  dryRunReplay(id: string): Record<string, unknown> {
    const raw = this.sessions.novex.modelCall(id);
    const detail = this.modelCall(id);
    const snapshot = raw.prompt_snapshot as Partial<PromptSnapshot> | undefined;
    const promptKey = typeof snapshot?.prompt_key === "string" ? snapshot.prompt_key : "";
    const promptVersion = typeof snapshot?.prompt_version === "string" ? snapshot.prompt_version : "";
    const definitions = this.requireDefinitions();
    const definitionResolved = definitions.agents.some(
      (item) => item.agent_key === raw.agent_key && item.version === raw.agent_version,
    ) && definitions.prompts.some(
      (item) => item.prompt_key === promptKey && item.version === promptVersion,
    );
    let compileSucceeded = false;
    let diff: unknown[] = [];
    if (!definitionResolved) {
      diff = [{ path: "prompt_definition", kind: "missing" }];
    } else {
      try {
        if (!snapshot || !Array.isArray(snapshot.fragments) || typeof snapshot.variables !== "object"
          || (snapshot.tool_profile !== "chat" && snapshot.tool_profile !== "workspace")) {
          throw new Error("historical prompt snapshot has no compile input");
        }
        const variables = Object.fromEntries(
          Object.entries(snapshot.variables ?? {}).filter(([key]) => key !== "fragments"),
        );
        const input: PromptCompileInput = {
          schema_version: "1",
          variables,
          fragments: structuredClone(snapshot.fragments),
        };
        const recompiled = compilePromptForReplay(
          definitions,
          String(raw.agent_key),
          String(raw.agent_version),
          String(raw.node_key),
          input,
          snapshot.tool_profile,
          snapshot.tool_schema ?? null,
        );
        compileSucceeded = true;
        diff = structuredDiff(snapshot, recompiled);
      } catch (error) {
        diff = [{ path: "compile", kind: "error", message: error instanceof Error ? error.message : "compile failed" }];
      }
    }
    return {
      schema_version: "1",
      mode: "dry_run",
      source_model_call_id: id,
      source_record_hash: detail.record_hash,
      definition_resolved: definitionResolved,
      compile_succeeded: compileSucceeded,
      side_effects: { model_calls: 0, tools: 0, session_writes: 0, run_writes: 0, domain_writes: 0 },
      diff,
    };
  }

  private createHarness(
    sessionId: string,
    session: Awaited<ReturnType<SessionStore["open"]>>,
    profile: ToolProfile,
    binding: SessionBinding,
    runtime: PiModelRuntime,
  ): RuntimeHarness {
    return new NovexAgentHarness({
      sessionId,
      session,
      sessions: this.sessions,
      binding,
      definitions: this.requireDefinitions(),
      runtime,
      profile,
      tools: toolsForProfile(profile),
      refreshRuntime: async () => {
        const resolved = await this.modelResolver.resolveEnabledText(binding.model_id);
        const refreshed = this.modelFactory(resolved);
        this.validateBinding(sessionId, resolved, refreshed);
        refreshPiModelRuntime(runtime, refreshed);
        return runtime.streamOptions;
      },
    });
  }

  private async upgradeFork(
    sessionId: string,
    entryId: string | undefined,
    position: "before" | "at",
    upgrade: UpgradeForkInput,
  ) {
    const definitions = this.requireDefinitions();
    const agent = definitions.agents.find(
      (item) => item.agent_key === upgrade.agentKey && item.version === upgrade.agentVersion,
    );
    if (!agent || agent.executor_owner !== "pi" || agent.status === "candidate" || agent.status === "revoked") {
      throw new RuntimeError("definition_contract_error", 422, "目标 Agent Definition 不可执行");
    }
    if (!agent.tool_profiles.includes(upgrade.toolProfile)) {
      throw new RuntimeError("definition_contract_error", 422, "Agent 不允许目标 tool profile");
    }
    const resolved = await this.modelResolver.resolveEnabledText(upgrade.modelId);
    const runtime = this.modelFactory(resolved);
    validateModelCapabilities(agent, resolved);
    return this.sessions.upgradeFork({
      sessionId,
      ...(entryId ? { entryId } : {}),
      position,
      modelId: upgrade.modelId,
      agentKey: upgrade.agentKey,
      toolProfile: upgrade.toolProfile,
      ...(upgrade.legacyPromptDisposition
        ? { legacyPromptDisposition: upgrade.legacyPromptDisposition }
        : {}),
      binding: {
        agent_key: agent.agent_key,
        agent_version: agent.version,
        agent_digest: definitionDigest(agent),
        prompt_bindings: structuredClone(agent.nodes),
        registry_digest: definitions.digest,
        tool_profile: upgrade.toolProfile,
        model_id: upgrade.modelId,
        behavior_fingerprint: runtime.snapshot.behavior_fingerprint,
        model_snapshot: runtime.snapshot,
        binding_status: "executable",
        migration_source: "explicit_upgrade_fork",
      },
    });
  }

  private reserve(sessionId: string): void {
    if (this.activeRuns.has(sessionId) || this.reservations.has(sessionId)) {
      throw new RuntimeError("session_busy", 409, "会话正在运行");
    }
    this.reservations.add(sessionId);
  }

  private async ensureSessionMigrated(sessionId: string): Promise<SessionBinding> {
    const existing = this.sessions.novex.bindingOrNull(sessionId);
    if (existing) return existing;
    const metadata = await this.sessions.findMetadata(sessionId);
    const own = readSessionMetadata(metadata);
    const definitions = this.requireDefinitions();
    const agent = activeAgent(definitions, "personal.general");
    const resolved = await this.modelResolver.resolveEnabledText(own.model_id);
    const runtime = this.modelFactory(resolved);
    validateModelCapabilities(agent, resolved);
    const readOnly = own.legacy_custom_system_prompt;
    return this.sessions.novex.createMigratedBinding({
      session_id: sessionId,
      agent_key: agent.agent_key,
      agent_version: agent.version,
      agent_digest: definitionDigest(agent),
      prompt_bindings: structuredClone(agent.nodes),
      registry_digest: definitions.digest,
      tool_profile: own.tool_profile,
      model_id: own.model_id,
      behavior_fingerprint: runtime.snapshot.behavior_fingerprint,
      model_snapshot: runtime.snapshot,
      binding_status: readOnly ? "read_only" : "executable",
      migration_source: readOnly ? "legacy_custom_prompt" : "legacy_default_prompt",
      parent_session_id: metadata.parentSessionId ?? null,
    }, readOnly ? "legacy_custom_prompt_read_only" : "legacy_default_prompt_bound", {
      agent_key: agent.agent_key,
      agent_version: agent.version,
      custom_system_prompt: readOnly,
      legacy_text_exposed: false,
    });
  }

  private requireActive(sessionId: string): ActiveRun {
    const active = this.activeRuns.get(sessionId);
    if (!active) throw new RuntimeError("session_not_running", 409, "会话当前没有活动运行");
    return active;
  }

  private async withIdleHarness<T>(
    sessionId: string,
    operation: (active: ActiveRun) => Promise<T>,
  ): Promise<T> {
    this.reserve(sessionId);
    let session: Awaited<ReturnType<SessionStore["open"]>> | undefined;
    try {
      await this.ensureSessionMigrated(sessionId);
      const metadata = await this.sessions.findMetadata(sessionId);
      const own = readSessionMetadata(metadata);
      const resolved = await this.modelResolver.resolveEnabledText(own.model_id);
      const runtime = this.modelFactory(resolved);
      const binding = this.validateBinding(sessionId, resolved, runtime);
      session = await this.sessions.open(sessionId);
      return await operation({
        harness: this.createHarness(sessionId, session, own.tool_profile, binding, runtime),
        secrets: runtime.secrets,
      });
    } finally {
      this.reservations.delete(sessionId);
      if (session) await cleanupSession(session);
    }
  }

  private requireDefinitions(): DefinitionRegistry {
    if (!this.definitions) throw new RuntimeError("config_invalid", 500, "Definition Registry 未配置");
    return this.definitions;
  }

  private validateBinding(sessionId: string, resolved: ResolvedTextModel, runtime: PiModelRuntime) {
    const binding = this.sessions.novex.binding(sessionId);
    if (binding.binding_status !== "executable") throw new RuntimeError("session_migration_required", 409, "会话当前不可执行");
    if (binding.model_id !== resolved.id || binding.behavior_fingerprint !== runtime.snapshot.behavior_fingerprint) {
      throw new RuntimeError("model_rebind_required", 409, "模型行为配置已变化，需要显式 fork/rebind");
    }
    const agent = this.requireDefinitions().agents.find((item) => item.agent_key === binding.agent_key && item.version === binding.agent_version);
    if (!agent || agent.status === "revoked" || definitionDigest(agent) !== binding.agent_digest) {
      throw new RuntimeError("definition_rebind_required", 409, "会话绑定的 Agent Definition 不可继续执行");
    }
    validateModelCapabilities(agent, resolved);
    return binding;
  }

}

function modelCallSummaryDto(call: ModelCallSummary): Record<string, unknown> {
  return {
    id: call.id,
    owner: { type: "session", id: call.session_id },
    execution: { phase: call.phase, entry_id: call.entry_id, step_id: null },
    node_key: call.node_key,
    attempt: call.attempt,
    status: call.status,
    definition: {
      agent_key: call.agent_key,
      agent_version: call.agent_version,
      prompt_key: call.prompt_key,
      prompt_version: call.prompt_version,
      registry_digest: call.registry_digest,
    },
    model: { id: call.model_id, behavior_fingerprint: call.behavior_fingerprint },
    usage: usageSummary(call.usage),
    prepared_at: call.prepared_at,
    completed_at: call.completed_at,
  };
}

function modelCallRecordDto(call: Record<string, unknown>): Record<string, unknown> {
  return {
    id: call.id,
    owner: { type: "session", id: call.session_id },
    execution: { phase: call.phase, entry_id: call.entry_id ?? null, step_id: null },
    root_call_id: call.root_call_id ?? null,
    parent_call_id: call.parent_call_id ?? null,
    node_key: call.node_key,
    attempt: call.attempt,
    status: call.status,
    definition: {
      agent_key: call.agent_key,
      agent_version: call.agent_version,
      prompt_key: call.prompt_key,
      prompt_version: call.prompt_version,
      registry_digest: call.registry_digest,
    },
    prompt_snapshot: call.prompt_snapshot,
    context_sources: call.context_sources,
    memory_sources: call.memory_sources,
    tool_schema: call.tool_schema ?? null,
    model: {
      id: call.model_id,
      behavior_fingerprint: call.behavior_fingerprint,
      snapshot: call.model_snapshot,
    },
    parameters: call.parameters,
    asset_references: call.asset_references,
    output_snapshot: call.output_snapshot ?? null,
    usage_snapshot: call.usage_snapshot ?? null,
    error_snapshot: call.error_snapshot ?? null,
    structured_parse_status: call.structured_parse_status ?? null,
    prepared_at: call.prepared_at,
    completed_at: call.completed_at ?? null,
  };
}

function usageSummary(value: unknown): Record<string, number | null> {
  const usage = value !== null && typeof value === "object" ? value as Record<string, unknown> : {};
  const numeric = (...keys: string[]): number | null => {
    for (const key of keys) {
      if (typeof usage[key] === "number" && Number.isFinite(usage[key])) return usage[key];
    }
    return null;
  };
  const cost = usage.cost !== null && typeof usage.cost === "object"
    ? (usage.cost as Record<string, unknown>).total
    : undefined;
  return {
    input_tokens: numeric("input_tokens", "input"),
    output_tokens: numeric("output_tokens", "output"),
    total_tokens: numeric("total_tokens", "totalTokens"),
    cost_usd: typeof cost === "number" && Number.isFinite(cost) ? cost : numeric("cost_usd", "cost"),
  };
}

function structuredDiff(historical: unknown, current: unknown): unknown[] {
  const diff: unknown[] = [];
  const visit = (path: string, left: unknown, right: unknown): void => {
    if (Array.isArray(left) && Array.isArray(right) && left.length === right.length) {
      left.forEach((item, index) => visit(`${path}[${index}]`, item, right[index]));
      return;
    }
    if (isRecord(left) && isRecord(right)) {
      const keys = [...new Set([...Object.keys(left), ...Object.keys(right)])].sort();
      for (const key of keys) {
        const next = path ? `${path}.${key}` : key;
        if (!(key in right)) diff.push({ path: next, kind: "removed", historical: left[key] });
        else if (!(key in left)) diff.push({ path: next, kind: "added", current: right[key] });
        else visit(next, left[key], right[key]);
      }
      return;
    }
    if (canonicalJson(left) !== canonicalJson(right)) {
      diff.push({ path, kind: "changed", historical: left, current: right });
    }
  };
  visit("", historical, current);
  return diff;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function validateModelCapabilities(agent: AgentDefinition, model: ResolvedTextModel): void {
  try {
    validateCapabilities(agent.model_requirements, {
      text: true,
      tool_calling: true,
      structured_output: true,
      vision: false,
      reasoning: model.reasoningEffort !== undefined,
      context_window: model.contextWindow,
    });
  } catch {
    throw new RuntimeError("model_capability_mismatch", 422, "模型能力不满足 Agent Definition");
  }
}
