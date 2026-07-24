import {
  AgentHarness,
  type AgentHarnessEvent,
  type AgentHarnessTool,
  type ExecutionToolContext,
} from "@earendil-works/pi-agent-core";
import type { AssistantMessage } from "@earendil-works/pi-ai";

import { RuntimeError } from "./errors.js";
import {
  createPiModelRuntime,
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

export interface TextModelResolver {
  resolveEnabledText(modelId: string): Promise<ResolvedTextModel>;
  ping(): Promise<void>;
}

export interface CreatedSession {
  session: SessionView;
  model: ModelSnapshot;
}

type RuntimeHarness = AgentHarness<ExecutionToolContext>;

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
  ) {}

  async createSession(input: {
    modelId: string;
    toolProfile: ToolProfile;
    systemPrompt?: string;
    source: string;
  }): Promise<CreatedSession> {
    const resolved = await this.modelResolver.resolveEnabledText(input.modelId);
    const runtime = this.modelFactory(resolved);
    const session = await this.sessions.create(input);
    const metadata = await session.getMetadata();
    try {
      await this.sessions.appendModelSnapshot(session, runtime.snapshot);
    } catch (error) {
      await cleanupSession(session);
      await this.sessions.delete(metadata.id);
      throw error;
    }
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
      const metadata = await this.sessions.findMetadata(sessionId);
      const own = readSessionMetadata(metadata);
      const resolved = await this.modelResolver.resolveEnabledText(own.model_id);
      const runtime = this.modelFactory(resolved);
      opened = await this.sessions.open(sessionId);
      await this.sessions.appendModelSnapshot(opened, runtime.snapshot);
      const harness = this.createHarness(opened, own.tool_profile, own.system_prompt, runtime);
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
  ): Promise<SessionView> {
    this.reserve(sessionId);
    try {
      const forked = await this.sessions.fork(sessionId, entryId, position);
      const metadata = await forked.getMetadata();
      await cleanupSession(forked);
      return this.sessions.view(metadata.id);
    } finally {
      this.reservations.delete(sessionId);
    }
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

  private createHarness(
    session: Awaited<ReturnType<SessionStore["open"]>>,
    profile: ToolProfile,
    systemPrompt: string | undefined,
    runtime: PiModelRuntime,
  ): RuntimeHarness {
    return new AgentHarness<ExecutionToolContext>({
      toolContext: { env: this.sessions.executionEnv },
      session,
      models: runtime.models,
      model: runtime.model,
      thinkingLevel: runtime.thinkingLevel,
      tools: toolsForProfile(profile),
      systemPrompt: systemPrompt ?? "You are a helpful personal AI workbench assistant.",
      streamOptions: runtime.streamOptions,
      steeringMode: "one-at-a-time",
      followUpMode: "one-at-a-time",
    });
  }

  private reserve(sessionId: string): void {
    if (this.activeRuns.has(sessionId) || this.reservations.has(sessionId)) {
      throw new RuntimeError("session_busy", 409, "会话正在运行");
    }
    this.reservations.add(sessionId);
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
      const metadata = await this.sessions.findMetadata(sessionId);
      const own = readSessionMetadata(metadata);
      const runtime = this.modelFactory(await this.modelResolver.resolveEnabledText(own.model_id));
      session = await this.sessions.open(sessionId);
      await this.sessions.appendModelSnapshot(session, runtime.snapshot);
      return await operation({
        harness: this.createHarness(session, own.tool_profile, own.system_prompt, runtime),
        secrets: runtime.secrets,
      });
    } finally {
      this.reservations.delete(sessionId);
      if (session) await cleanupSession(session);
    }
  }
}
