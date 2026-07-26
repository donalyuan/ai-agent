import { randomUUID } from "node:crypto";

import {
  AgentHarness,
  type AgentHarnessEvent,
  type AgentHarnessTool,
  type AgentMessage,
  type ExecutionToolContext,
  type Session,
} from "@earendil-works/pi-agent-core";
import type {
  AssistantMessage,
  Api,
  Context,
  Message,
  Model,
  ModelsSimpleStreamOptions,
} from "@earendil-works/pi-ai";

import type { CompleteSimpleNext } from "./audited-models.js";
import {
  ContextCompileError,
  compileContext,
  type ContextCompileRequest,
  type ContextSnapshot,
} from "./context.js";
import {
  definitionDigest,
  finalizePromptMessages,
  preparePrompt,
  preparePromptMessages,
  type DefinitionRegistry,
  type FinalizedPrompt,
  type PreparedPrompt,
} from "./definitions.js";
import { RuntimeError, type RuntimeErrorCode } from "./errors.js";
import type { PiModelRuntime } from "./models.js";
import {
  mapAgentMessages,
  selectAgentMessages,
  type AgentMessageContextMapping,
  type PendingContextInput,
  type PiContextPhase,
} from "./pi-context.js";
import type { SessionBinding } from "./persistence.js";
import { redactUnknown } from "./redaction.js";
import type { SessionStore, ToolProfile } from "./sessions.js";

type PiHarness = AgentHarness<ExecutionToolContext>;
type Phase = PiContextPhase;

interface NovexHarnessOptions {
  sessionId: string;
  session: Session;
  sessions: SessionStore;
  binding: SessionBinding;
  definitions: DefinitionRegistry;
  runtime: PiModelRuntime;
  profile: ToolProfile;
  tools: AgentHarnessTool<ExecutionToolContext>[];
  refreshRuntime: () => Promise<{ timeoutMs: number; maxRetries: 0 }>;
}

interface GovernedContextStep {
  mapping: AgentMessageContextMapping;
  finalized: FinalizedPrompt;
}

interface PendingSummaryCall {
  id: string;
  output: unknown;
  usage: unknown;
}

/** Public-API-only composition boundary; Pi remains the sole owner of Turn/Tool Loop lifecycle. */
export class NovexAgentHarness {
  private readonly harness: PiHarness;
  private phase: Phase = "turn";
  private phaseInput = "";
  private pendingInputs: PendingContextInput[] = [];
  private activeCallId: string | undefined;
  private rootCallId: string | undefined;
  private attempt = 0;
  private latestStep: GovernedContextStep | undefined;
  private pendingSummaryCalls: PendingSummaryCall[] = [];
  private executionError: RuntimeError | undefined;

  constructor(private readonly options: NovexHarnessOptions) {
    this.harness = new AgentHarness<ExecutionToolContext>({
      toolContext: { env: options.sessions.executionEnv },
      session: options.session,
      models: options.runtime.models,
      model: options.runtime.model,
      thinkingLevel: options.runtime.thinkingLevel,
      tools: options.tools,
      systemPrompt: this.systemPrompt(),
      streamOptions: options.runtime.streamOptions,
      steeringMode: "one-at-a-time",
      followUpMode: "one-at-a-time",
    });
    options.runtime.models.governCompleteSimple((model, context, streamOptions, next) =>
      this.completeGovernedSummary(model, context, streamOptions, next));
    this.installHooks();
  }

  prompt(text: string): Promise<AssistantMessage> {
    this.begin("turn", text);
    return this.execute(() => this.harness.prompt(text));
  }

  async steer(text: string): Promise<void> {
    this.pendingInputs.push({ kind: "steer", text });
    await this.harness.steer(text);
  }

  async followUp(text: string): Promise<void> {
    this.pendingInputs.push({ kind: "follow_up", text });
    await this.harness.followUp(text);
  }

  async compact(instructions?: string): Promise<unknown> {
    this.begin("compaction", instructions ?? "session compaction");
    await this.options.refreshRuntime();
    return this.execute(() => this.harness.compact(instructions));
  }

  async navigateTree(
    entryId: string,
    options?: { summarize?: boolean; customInstructions?: string; label?: string },
  ): Promise<unknown> {
    this.begin(options?.summarize ? "branch_summary" : "turn", options?.customInstructions ?? `navigate:${entryId}`);
    if (options?.summarize) await this.options.refreshRuntime();
    return this.execute(() => this.harness.navigateTree(entryId, options));
  }

  abort(): Promise<unknown> {
    return this.harness.abort();
  }

  subscribe(listener: (event: AgentHarnessEvent) => Promise<void> | void): () => void {
    return this.harness.subscribe(listener);
  }

  private installHooks(): void {
    this.harness.on("before_agent_start", (event) => {
      this.phaseInput = event.prompt;
      return { systemPrompt: this.systemPrompt() };
    });
    this.harness.on("context", async (event) => {
      try {
        await this.options.refreshRuntime();
        this.assertBinding();
        const step = this.compileAgentContext(event.messages);
        this.latestStep = step;
        this.pendingInputs = [...step.mapping.remainingPendingInputs];
        return { messages: selectAgentMessages(step.mapping, step.finalized.contextSnapshot.selected_order) };
      } catch (error) {
        if (error instanceof RuntimeError) this.executionError = error;
        throw error;
      }
    });
    this.harness.on("before_provider_request", () => {
      try {
        this.assertBinding();
        const step = this.latestStep;
        if (!step) throw new RuntimeError("context_finalize_mismatch", 422, "模型请求缺少已治理 ContextSnapshot");
        this.latestStep = undefined;
        this.prepareCall(step.finalized, null);
        return { streamOptions: { ...this.options.runtime.streamOptions, maxRetries: 0 as const } };
      } catch (error) {
        if (error instanceof RuntimeError) this.executionError = error;
        throw error;
      }
    });
    this.harness.on("before_provider_payload", (event) => {
      this.assertBinding();
      return { payload: event.payload };
    });
    this.harness.on("after_provider_response", () => undefined);
    this.harness.on("tool_call", (event) => {
      const allowed = this.options.binding.tool_profile === "workspace"
        && ["read", "write", "edit", "bash"].includes(event.toolName);
      if (!allowed) return { block: true, reason: `tool ${event.toolName} is not allowed by binding` };
      return undefined;
    });
    this.harness.subscribe((event) => this.captureTerminal(event));
  }

  private compileAgentContext(messages: readonly AgentMessage[]): GovernedContextStep {
    const prepared = preparePrompt(
      this.options.definitions,
      this.options.binding.agent_key,
      this.options.binding.agent_version,
      nodeForPhase(this.phase),
      {},
      this.options.profile,
      this.toolSchema(),
      this.options.runtime.snapshot.max_output_tokens,
    );
    const request = this.contextRequest(prepared, []);
    try {
      const mapping = mapAgentMessages(messages, {
        sessionId: this.options.sessionId,
        phase: this.phase,
        phaseInput: this.phaseInput,
        compiledAt: request.compiled_at,
        pendingInputs: this.pendingInputs,
      });
      request.candidates = mapping.candidates;
      request.atomic_groups = mapping.atomicGroups;
      const finalized = finalizePromptMessages(
        prepared,
        randomUUID(),
        compileContext(request),
        request.tokenizer_profile,
      );
      return { mapping, finalized };
    } catch (error) {
      throw this.persistCompileFailure(error, request);
    }
  }

  private async completeGovernedSummary(
    _model: Model<Api>,
    context: Context,
    streamOptions: ModelsSimpleStreamOptions | undefined,
    next: CompleteSimpleNext,
  ): Promise<AssistantMessage> {
    if (this.phase !== "compaction" && this.phase !== "branch_summary") {
      throw new RuntimeError("context_schema_invalid", 422, "standalone 模型调用缺少 compaction/branch summary phase");
    }
    this.assertBinding();
    const maxOutputTokens = Math.min(
      streamOptions?.maxTokens ?? this.options.runtime.snapshot.max_output_tokens,
      this.options.runtime.snapshot.max_output_tokens,
    );
    const prepared = preparePromptMessages(
      this.options.definitions,
      this.options.binding.agent_key,
      this.options.binding.agent_version,
      nodeForPhase(this.phase),
      this.options.profile,
      context.systemPrompt ?? this.systemPrompt(),
      context.tools ?? null,
      maxOutputTokens,
    );
    const request = this.contextRequest(prepared, []);
    let mapping: AgentMessageContextMapping;
    let finalized: FinalizedPrompt;
    try {
      mapping = mapAgentMessages(context.messages, {
        sessionId: this.options.sessionId,
        phase: this.phase,
        phaseInput: this.phaseInput,
        compiledAt: request.compiled_at,
        pendingInputs: [],
        summaryProviderInput: true,
      });
      request.candidates = mapping.candidates;
      request.atomic_groups = mapping.atomicGroups;
      finalized = finalizePromptMessages(
        prepared,
        randomUUID(),
        compileContext(request),
        request.tokenizer_profile,
      );
    } catch (error) {
      throw this.persistCompileFailure(error, request);
    }

    const callId = this.prepareCall(finalized, null, false);
    const governedContext: Context = {
      ...context,
      messages: selectAgentMessages(mapping, finalized.contextSnapshot.selected_order) as Message[],
    };
    try {
      const response = await next(governedContext, streamOptions);
      const output = redactUnknown(response, this.options.runtime.secrets);
      if (response.stopReason === "error") {
        this.options.sessions.novex.finishModelCall(callId, "failed", output, response.usage, {
          code: "provider_error",
          message: redactUnknown(response.errorMessage ?? "摘要模型调用失败", this.options.runtime.secrets),
        });
      } else if (response.stopReason === "aborted") {
        this.options.sessions.novex.finishModelCall(callId, "aborted", output, response.usage, { code: "aborted" });
      } else {
        this.pendingSummaryCalls.push({ id: callId, output, usage: response.usage });
      }
      return response;
    } catch (error) {
      this.options.sessions.novex.finishModelCall(
        callId, "failed", undefined, undefined, redactUnknown(error, this.options.runtime.secrets),
      );
      throw error;
    }
  }

  private contextRequest(prepared: PreparedPrompt, candidates: ContextCompileRequest["candidates"]): ContextCompileRequest {
    const nodeKey = nodeForPhase(this.phase);
    const policyBinding = this.options.binding.context_policy_bindings[nodeKey];
    const policy = policyBinding === undefined ? undefined : this.options.definitions.context_policies.find((item) =>
      item.policy_key === policyBinding.key && item.version === policyBinding.version);
    const profile = this.options.definitions.tokenizer_profiles.find((item) =>
      item.profile_key === this.options.binding.tokenizer_profile_key
      && item.version === this.options.binding.tokenizer_profile_version);
    if (!policyBinding || !policy || definitionDigest(policy) !== policyBinding.digest) {
      throw new RuntimeError("definition_rebind_required", 409, `Context Policy binding ${nodeKey} 不可用`);
    }
    if (!profile || definitionDigest(profile) !== this.options.binding.tokenizer_profile_digest) {
      throw new RuntimeError("tokenizer_profile_unavailable", 422, "Tokenizer Profile binding 不可用");
    }
    return {
      schema_version: "2",
      owner: "pi",
      owner_id: this.options.sessionId,
      node_key: nodeKey,
      compiled_at: new Date().toISOString(),
      model_context_window: this.options.runtime.snapshot.context_window,
      policy,
      tokenizer_profile: profile,
      prepared_prompt: prepared.envelope,
      candidates,
      atomic_groups: [],
    };
  }

  private persistCompileFailure(error: unknown, request: ContextCompileRequest): RuntimeError {
    if (!(error instanceof ContextCompileError)) {
      return error instanceof RuntimeError
        ? error
        : new RuntimeError("context_schema_invalid", 422, error instanceof Error ? error.message : "Context 编译失败");
    }
    const attemptId = this.options.sessions.novex.persistContextCompileAttempt({
      sessionId: this.options.sessionId,
      phase: this.phase,
      attempt: error.attempt(request),
    });
    return new RuntimeError(
      contextErrorCode(error.code),
      422,
      `Context 编译失败（attempt_id=${attemptId}）: ${error.code}`,
      { cause: error },
    );
  }

  private async captureTerminal(event: AgentHarnessEvent): Promise<void> {
    if (event.type === "tool_execution_start") this.phase = "tool_loop";
    if (event.type === "message_end" && event.message.role === "assistant") {
      const message = redactUnknown(event.message, this.options.runtime.secrets) as Record<string, unknown>;
      const entryId = await this.options.session.getLeafId() ?? undefined;
      if (event.message.stopReason === "error") {
        this.finishActive("failed", message, message.usage, {
          code: "provider_error",
          message: redactUnknown(event.message.errorMessage ?? "模型流处理失败", this.options.runtime.secrets),
        }, entryId);
      } else if (event.message.stopReason === "aborted") {
        this.finishActive("aborted", message, message.usage, { code: "aborted" }, entryId);
      } else {
        this.finishActive("succeeded", message, message.usage, undefined, entryId);
      }
    } else if (event.type === "session_compact" || (event.type === "session_tree" && event.summaryEntry)) {
      const entryId = await this.options.session.getLeafId() ?? undefined;
      this.finishPendingSummaries("succeeded", undefined, entryId);
    } else if (event.type === "abort") {
      this.finishActive("aborted", undefined, undefined, { code: "aborted" });
      this.finishPendingSummaries("aborted", { code: "aborted" });
    }
  }

  private begin(phase: Phase, input: string): void {
    this.phase = phase;
    this.phaseInput = input;
    this.pendingInputs = [];
    this.latestStep = undefined;
    this.pendingSummaryCalls = [];
    this.activeCallId = undefined;
    this.rootCallId = undefined;
    this.attempt = 0;
    this.executionError = undefined;
  }

  private async execute<T>(operation: () => Promise<T>): Promise<T> {
    try {
      const result = await operation();
      if (this.executionError) throw this.executionError;
      return result;
    } catch (error) {
      this.finishActive("failed", undefined, undefined, redactUnknown(error, this.options.runtime.secrets));
      this.finishPendingSummaries("failed", redactUnknown(error, this.options.runtime.secrets));
      throw error;
    }
  }

  private finishActive(
    status: "succeeded" | "failed" | "aborted",
    output: unknown,
    usage: unknown,
    error: unknown,
    entryId?: string,
  ): void {
    if (!this.activeCallId) return;
    const id = this.activeCallId;
    this.activeCallId = undefined;
    this.options.sessions.novex.finishModelCall(id, status, output, usage, error, entryId);
  }

  private finishPendingSummaries(
    status: "succeeded" | "failed" | "aborted",
    error: unknown,
    entryId?: string,
  ): void {
    for (const call of this.pendingSummaryCalls.splice(0)) {
      this.options.sessions.novex.finishModelCall(
        call.id,
        status,
        status === "succeeded" ? call.output : undefined,
        call.usage,
        error,
        entryId,
      );
    }
  }

  private prepareCall(finalized: FinalizedPrompt, providerPayload: unknown, active = true): string {
    if (active && this.activeCallId) throw new Error("previous ModelCall has no terminal state");
    this.attempt += 1;
    const snapshot = finalized.contextSnapshot;
    const id = this.options.sessions.novex.prepareModelCallWithContext({
      sessionId: this.options.sessionId,
      phase: this.phase,
      nodeKey: nodeForPhase(this.phase),
      attempt: this.attempt,
      ...(this.rootCallId ? { rootCallId: this.rootCallId } : {}),
      binding: this.options.binding,
      promptSnapshot: finalized.promptSnapshot,
      contextSnapshot: snapshot,
      modelSnapshot: this.options.runtime.snapshot,
      contextSources: snapshot.decisions.map(({ selected_payload: _payload, ...decision }) => decision),
      toolSchema: finalized.promptSnapshot.tool_schema,
      assetReferences: selectedAssets(snapshot),
      providerPayload: redactUnknown(providerPayload, this.options.runtime.secrets),
    });
    this.rootCallId ??= id;
    if (active) this.activeCallId = id;
    return id;
  }

  private assertBinding(): void {
    if (this.options.binding.behavior_fingerprint !== this.options.runtime.snapshot.behavior_fingerprint
      || this.options.binding.model_id !== this.options.runtime.snapshot.model_id
      || this.options.binding.tokenizer_profile_key !== this.options.runtime.snapshot.tokenizer_profile_key
      || this.options.binding.tokenizer_profile_version !== this.options.runtime.snapshot.tokenizer_profile_version) {
      throw new RuntimeError("model_rebind_required", 409, "模型行为配置已变化，需要显式 fork/rebind");
    }
  }

  private systemPrompt(): string {
    const agent = this.options.definitions.agents.find((item) =>
      item.agent_key === this.options.binding.agent_key && item.version === this.options.binding.agent_version);
    const reference = agent?.nodes[nodeForPhase(this.phase)] ?? (agent ? Object.values(agent.nodes)[0] : undefined);
    const prompt = reference
      ? this.options.definitions.prompts.find((item) => item.prompt_key === reference.key && item.version === reference.version)
      : undefined;
    const value = prompt ? this.options.definitions.templates.get(prompt.system_template) : undefined;
    if (!value) throw new RuntimeError("definition_contract_error", 422, "Definition system template 缺失");
    return value;
  }

  private toolSchema(): unknown[] {
    return this.harness.getActiveTools().map((tool) => ({
      name: tool.name,
      description: tool.description,
      parameters: tool.parameters,
    }));
  }
}

function nodeForPhase(phase: Phase): string {
  switch (phase) {
    case "turn": return "personal.turn";
    case "tool_loop": return "personal.tool_followup";
    case "compaction": return "personal.compaction";
    case "branch_summary": return "personal.branch_summary";
  }
}

function selectedAssets(snapshot: ContextSnapshot): unknown[] {
  return snapshot.decisions.flatMap((decision) =>
    decision.selected_payload?.type === "asset" ? [decision.selected_payload.asset] : []);
}

function contextErrorCode(code: string): RuntimeErrorCode {
  const known: RuntimeErrorCode[] = [
    "context_atomic_group_invalid",
    "context_budget_exceeded",
    "context_conflict",
    "context_content_hash_mismatch",
    "context_finalize_mismatch",
    "context_schema_invalid",
    "required_context_unavailable",
    "tokenizer_profile_unavailable",
  ];
  return known.includes(code as RuntimeErrorCode) ? code as RuntimeErrorCode : "context_schema_invalid";
}
