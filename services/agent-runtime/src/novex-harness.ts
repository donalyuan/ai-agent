import { randomUUID } from "node:crypto";

import {
  AgentHarness,
  type AgentHarnessEvent,
  type AgentHarnessTool,
  type ExecutionToolContext,
  type Session,
} from "@earendil-works/pi-agent-core";
import type { AssistantMessage } from "@earendil-works/pi-ai";

import {
  canonicalJson,
  compilePrompt,
  type DefinitionRegistry,
  type DynamicFragment,
  type PromptSnapshot,
} from "./definitions.js";
import type { PiModelRuntime } from "./models.js";
import type { SessionBinding } from "./persistence.js";
import { redactUnknown } from "./redaction.js";
import { RuntimeError } from "./errors.js";
import type { SessionStore, ToolProfile } from "./sessions.js";

type PiHarness = AgentHarness<ExecutionToolContext>;
type Phase = "turn" | "tool_loop" | "compaction" | "branch_summary";

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

/** Public-API-only composition boundary; Pi remains the sole owner of Turn/Tool Loop lifecycle. */
export class NovexAgentHarness {
  private readonly harness: PiHarness;
  private phase: Phase = "turn";
  private phaseInput = "";
  private context: unknown[] = [];
  private queuedFragments: DynamicFragment[] = [];
  private activeCallId: string | undefined;
  private rootCallId: string | undefined;
  private attempt = 0;
  private latestSnapshot: PromptSnapshot | undefined;
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
    this.installHooks();
  }

  prompt(text: string): Promise<AssistantMessage> {
    this.begin("turn", text);
    return this.execute(() => this.harness.prompt(text));
  }

  async steer(text: string): Promise<void> {
    this.queuedFragments.push(fragment("steer", text));
    await this.harness.steer(text);
  }

  async followUp(text: string): Promise<void> {
    this.queuedFragments.push(fragment("follow_up", text));
    await this.harness.followUp(text);
  }

  async compact(instructions?: string): Promise<unknown> {
    this.begin("compaction", instructions ?? "session compaction");
    await this.options.refreshRuntime();
    this.prepareCall(null);
    return this.execute(() => this.harness.compact(instructions));
  }

  async navigateTree(
    entryId: string,
    options?: { summarize?: boolean; customInstructions?: string; label?: string },
  ): Promise<unknown> {
    this.begin(options?.summarize ? "branch_summary" : "turn", options?.customInstructions ?? `navigate:${entryId}`);
    if (options?.summarize) {
      await this.options.refreshRuntime();
      this.prepareCall(null);
    }
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
      const snapshot = this.compileSnapshot();
      return { systemPrompt: snapshot.system };
    });
    this.harness.on("context", (event) => {
      this.context = structuredClone(event.messages);
      return undefined;
    });
    this.harness.on("before_provider_request", async () => {
      try {
        const streamOptions = await this.options.refreshRuntime();
        this.assertBinding();
        if (!this.activeCallId) this.prepareCall(null);
        return { streamOptions: { ...streamOptions, maxRetries: 0 as const } };
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

  private compileSnapshot(): PromptSnapshot {
    const fragments: DynamicFragment[] = [];
    if (this.context.length > 0) {
      fragments.push({
        id: `context-${this.attempt + 1}`,
        trust: "reference",
        source: "pi_context_hook",
        content: canonicalJson(redactUnknown(this.context, this.options.runtime.secrets)),
      });
    } else if (this.phaseInput) {
      fragments.push(fragment("user_instruction", this.phaseInput));
    }
    fragments.push(...this.queuedFragments);
    if (fragments.length === 0) fragments.push(fragment("reference", this.phase));
    this.queuedFragments = [];
    const compiled = compilePrompt(
      this.options.definitions,
      this.options.binding.agent_key,
      this.options.binding.agent_version,
      nodeForPhase(this.phase),
      { schema_version: "1", fragments },
      this.options.profile,
      this.toolSchema(),
    );
    this.latestSnapshot = Object.freeze({ ...compiled, registry_digest: this.options.binding.registry_digest });
    return this.latestSnapshot;
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
    } else if (event.type === "session_compact") {
      const entryId = await this.options.session.getLeafId() ?? undefined;
      this.finishActive("succeeded", redactUnknown(event.compactionEntry, this.options.runtime.secrets), event.compactionEntry.usage, undefined, entryId);
    } else if (event.type === "session_tree" && event.summaryEntry) {
      const entryId = await this.options.session.getLeafId() ?? undefined;
      this.finishActive("succeeded", redactUnknown(event.summaryEntry, this.options.runtime.secrets), event.summaryEntry.usage, undefined, entryId);
    } else if (event.type === "abort") {
      this.finishActive("aborted", undefined, undefined, { code: "aborted" });
    }
  }

  private begin(phase: Phase, input: string): void {
    this.phase = phase;
    this.phaseInput = input;
    this.context = [];
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

  private prepareCall(providerPayload: unknown): void {
    if (this.activeCallId) throw new Error("previous ModelCall has no terminal state");
    const snapshot = this.compileSnapshot();
    this.attempt += 1;
    const id = this.options.sessions.novex.prepareModelCall({
      sessionId: this.options.sessionId,
      phase: this.phase,
      nodeKey: nodeForPhase(this.phase),
      attempt: this.attempt,
      ...(this.rootCallId ? { rootCallId: this.rootCallId } : {}),
      binding: this.options.binding,
      promptSnapshot: snapshot,
      modelSnapshot: this.options.runtime.snapshot,
      contextSources: snapshot.fragments.map(({ id: fragmentId, trust, source }) => ({ id: fragmentId, trust, source })),
      toolSchema: this.toolSchema(),
      assetReferences: snapshot.fragments.flatMap((item) => item.asset ? [item.asset] : []),
      providerPayload: redactUnknown(providerPayload, this.options.runtime.secrets),
    });
    this.rootCallId ??= id;
    this.activeCallId = id;
  }

  private assertBinding(): void {
    if (this.options.binding.behavior_fingerprint !== this.options.runtime.snapshot.behavior_fingerprint) {
      throw new Error("model_rebind_required");
    }
  }

  private systemPrompt(): string {
    const agent = this.options.definitions.agents.find((item) => item.agent_key === this.options.binding.agent_key && item.version === this.options.binding.agent_version);
    const reference = agent ? Object.values(agent.nodes)[0] : undefined;
    const prompt = reference
      ? this.options.definitions.prompts.find((item) => item.prompt_key === reference.key && item.version === reference.version)
      : undefined;
    const value = prompt ? this.options.definitions.templates.get(prompt.system_template) : undefined;
    if (!value) throw new Error("definition system template is missing");
    return value;
  }

  private toolSchema(): unknown[] {
    return this.harness.getActiveTools().map((tool) => ({ name: tool.name, description: tool.description, parameters: tool.parameters }));
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

function fragment(trust: DynamicFragment["trust"], content: string): DynamicFragment {
  return { id: `${trust}-${randomUUID()}`, trust, source: "pi_public_queue", content };
}
