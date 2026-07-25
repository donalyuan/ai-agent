import type { Session, SessionStorage, SessionTreeEntry } from "@earendil-works/pi-agent-core";
import { NodeExecutionEnv } from "@earendil-works/pi-agent-core/node";
import {
  createNodeSqliteFactory,
  SqliteSessionRepo,
  type SqliteSessionMetadata,
} from "@earendil-works/pi-storage-sqlite-node";

import { RuntimeError } from "./errors.js";
import { canonicalJson, sha256Hex } from "./definitions.js";
import type { ModelSnapshot } from "./models.js";
import {
  NovexSqliteStore,
  type PrepareModelCall,
  type SessionBinding,
} from "./persistence.js";

export type ToolProfile = "chat" | "workspace";

export interface NovexSessionMetadata {
  model_id: string;
  agent_key: string;
  tool_profile: ToolProfile;
  source: string;
  legacy_custom_system_prompt: boolean;
  legacy_system_prompt: string | null;
}

export interface LegacySessionMigrationItem {
  session_id: string;
  disposition: "auto_bind_personal_general" | "custom_prompt_read_only" | "unmapped";
  model_id: string | null;
  tool_profile: ToolProfile | null;
  evidence: Record<string, unknown>;
}

export interface SessionView {
  session_id: string;
  created_at: string;
  parent_session_id: string | null;
  active_leaf_id: string | null;
  cwd: string;
  model_id: string;
  agent_key: string;
  agent_version: string;
  registry_digest: string;
  behavior_fingerprint: string;
  tool_profile: ToolProfile;
  source: string;
}

export interface SequencedEntry {
  sequence: number;
  entry: SessionTreeEntry;
}

type PiSession = Session<SqliteSessionMetadata>;

function cleanupSession(session: PiSession): Promise<void> {
  const storage = session.getStorage() as SessionStorage<SqliteSessionMetadata> & {
    cleanup?: () => Promise<void>;
  };
  return storage.cleanup?.() ?? Promise.resolve();
}

function metadataOf(metadata: SqliteSessionMetadata): NovexSessionMetadata {
  const raw = metadata.metadata ?? {};
  const modelId = raw.model_id;
  const profile = raw.tool_profile;
  const source = raw.source;
  const agentKey = raw.agent_key;
  const systemPrompt = raw.system_prompt;
  if (
    typeof modelId !== "string" ||
    (profile !== "chat" && profile !== "workspace") ||
    typeof source !== "string" ||
    (agentKey !== undefined && typeof agentKey !== "string") ||
    (systemPrompt !== undefined && typeof systemPrompt !== "string")
  ) {
    throw new RuntimeError("storage_unavailable", 503, `会话 ${metadata.id} 的 metadata 无效`);
  }
  return {
    model_id: modelId,
    agent_key: agentKey ?? "",
    tool_profile: profile,
    source,
    legacy_custom_system_prompt: typeof systemPrompt === "string" && systemPrompt.length > 0,
    legacy_system_prompt: typeof systemPrompt === "string" && systemPrompt.length > 0 ? systemPrompt : null,
  };
}

export class SessionStore {
  private readonly env: NodeExecutionEnv;
  private readonly repo: SqliteSessionRepo;
  readonly novex: NovexSqliteStore;

  constructor(
    databasePath: string,
    private readonly workspaceRoot: string,
  ) {
    this.env = new NodeExecutionEnv({ cwd: workspaceRoot });
    this.repo = new SqliteSessionRepo({
      env: this.env,
      sqlite: createNodeSqliteFactory(),
      databasePath,
    });
    this.novex = new NovexSqliteStore(databasePath);
  }

  get executionEnv(): NodeExecutionEnv {
    return this.env;
  }

  async create(input: {
    modelId: string;
    agentKey: string;
    toolProfile: ToolProfile;
    source: string;
    parentSessionId?: string;
    binding: Omit<SessionBinding, "session_id" | "created_at" | "parent_session_id">;
  }): Promise<PiSession> {
    const session = await this.repo.create({
      cwd: this.workspaceRoot,
      ...(input.parentSessionId ? { parentSessionId: input.parentSessionId } : {}),
      metadata: {
        model_id: input.modelId,
        agent_key: input.agentKey,
        tool_profile: input.toolProfile,
        source: input.source,
      },
    });
    const metadata = await session.getMetadata();
    try {
      this.novex.createBinding({
        ...input.binding,
        session_id: metadata.id,
        parent_session_id: input.parentSessionId ?? null,
      });
      return session;
    } catch (error) {
      await cleanupSession(session);
      await this.repo.delete(metadata);
      throw error;
    }
  }

  async list(): Promise<SessionView[]> {
    const sessions = await this.repo.list();
    return Promise.all(sessions.map((metadata) => this.toView(metadata)));
  }

  async legacyMigrationPlan(): Promise<LegacySessionMigrationItem[]> {
    const sessions = await this.repo.list();
    const items: LegacySessionMigrationItem[] = [];
    for (const metadata of sessions) {
      if (this.novex.bindingOrNull(metadata.id)) continue;
      try {
        const own = metadataOf(metadata);
        items.push({
          session_id: metadata.id,
          disposition: own.legacy_custom_system_prompt
            ? "custom_prompt_read_only" as const
            : "auto_bind_personal_general" as const,
          model_id: own.model_id,
          tool_profile: own.tool_profile,
          evidence: {
            metadata_valid: true,
            custom_system_prompt: own.legacy_custom_system_prompt,
            legacy_text_exposed: false,
          },
        });
      } catch {
        items.push({
          session_id: metadata.id,
          disposition: "unmapped" as const,
          model_id: null,
          tool_profile: null,
          evidence: { metadata_valid: false, legacy_text_exposed: false },
        });
      }
    }
    return items;
  }

  async backupForHistoryMigration(destination: string): Promise<number> {
    return this.novex.backup(destination);
  }

  async findMetadata(sessionId: string): Promise<SqliteSessionMetadata> {
    const metadata = (await this.repo.list()).find((candidate) => candidate.id === sessionId);
    if (!metadata) throw new RuntimeError("session_not_found", 404, "会话不存在");
    return metadata;
  }

  async open(sessionId: string): Promise<PiSession> {
    this.novex.binding(sessionId);
    return this.repo.open(await this.findMetadata(sessionId));
  }

  async delete(sessionId: string): Promise<void> {
    const metadata = (await this.repo.list()).find((candidate) => candidate.id === sessionId);
    if (!metadata) {
      if (this.novex.pendingSessionDeletions().includes(sessionId)) {
        this.novex.completeSessionDeletion(sessionId);
        return;
      }
      throw new RuntimeError("session_not_found", 404, "会话不存在");
    }
    this.novex.beginSessionDeletion(sessionId);
    await this.repo.delete(metadata);
    this.novex.completeSessionDeletion(sessionId);
  }

  async reconcileSessionDeletions(): Promise<void> {
    const pending = this.novex.pendingSessionDeletions();
    if (pending.length === 0) return;
    const sessions = new Map((await this.repo.list()).map((metadata) => [metadata.id, metadata]));
    for (const sessionId of pending) {
      const metadata = sessions.get(sessionId);
      if (metadata) await this.repo.delete(metadata);
      this.novex.completeSessionDeletion(sessionId);
    }
  }

  async view(sessionId: string): Promise<SessionView> {
    return this.toView(await this.findMetadata(sessionId));
  }

  async entries(sessionId: string, afterSequence = 0, limit = 200): Promise<SequencedEntry[]> {
    const session = await this.open(sessionId);
    try {
      const entries = await session.getEntries();
      return entries
        .map((entry, index) => ({ sequence: index + 1, entry }))
        .filter(({ sequence }) => sequence > afterSequence)
        .slice(0, limit);
    } finally {
      await cleanupSession(session);
    }
  }

  async move(sessionId: string, entryId: string | null): Promise<void> {
    const session = await this.open(sessionId);
    try {
      if (entryId !== null && !(await session.getEntry(entryId))) {
        throw new RuntimeError("not_found", 404, "目标 entry 不存在");
      }
      await session.getStorage().setLeafId(entryId);
    } finally {
      await cleanupSession(session);
    }
  }

  async fork(sessionId: string, entryId?: string, position: "before" | "at" = "at"): Promise<PiSession> {
    const source = await this.findMetadata(sessionId);
    const binding = this.novex.binding(sessionId);
    if (binding.binding_status !== "executable") {
      throw new RuntimeError("session_migration_required", 409, "只读历史会话必须显式升级 fork");
    }
    return this.forkWithBinding(source, binding, {
      ...(entryId ? { entryId } : {}),
      position,
      metadata: source.metadata ?? {},
      migrationSource: "ordinary_fork",
    });
  }

  async upgradeFork(input: {
    sessionId: string;
    entryId?: string;
    position: "before" | "at";
    modelId: string;
    agentKey: string;
    toolProfile: ToolProfile;
    binding: Omit<SessionBinding, "session_id" | "created_at" | "parent_session_id">;
    legacyPromptDisposition?: "discard" | "user_instruction";
  }): Promise<PiSession> {
    const source = await this.findMetadata(input.sessionId);
    const legacy = metadataOf(source);
    if (legacy.legacy_custom_system_prompt && input.legacyPromptDisposition === undefined) {
      throw new RuntimeError("session_migration_required", 409, "必须明确丢弃旧 Prompt 或降级为 User instruction");
    }
    if (!legacy.legacy_custom_system_prompt && input.legacyPromptDisposition !== undefined) {
      throw new RuntimeError("bad_request", 400, "非自定义 Prompt 会话不得提交旧 Prompt 处置方式");
    }
    return this.forkWithBinding(source, input.binding, {
      ...(input.entryId ? { entryId: input.entryId } : {}),
      position: input.position,
      metadata: {
        model_id: input.modelId,
        agent_key: input.agentKey,
        tool_profile: input.toolProfile,
        source: "explicit_upgrade_fork",
      },
      migrationSource: "explicit_upgrade_fork",
      ...(input.legacyPromptDisposition
        ? { legacyPromptDisposition: input.legacyPromptDisposition, legacySystemPrompt: legacy.legacy_system_prompt }
        : {}),
    });
  }

  private async forkWithBinding(
    source: SqliteSessionMetadata,
    binding: Omit<SessionBinding, "session_id" | "created_at" | "parent_session_id">,
    options: {
      entryId?: string;
      position: "before" | "at";
      metadata: Record<string, unknown>;
      migrationSource: "ordinary_fork" | "explicit_upgrade_fork";
      legacyPromptDisposition?: "discard" | "user_instruction";
      legacySystemPrompt?: string | null;
    },
  ): Promise<PiSession> {
    const forked = await this.repo.fork(source, {
      cwd: source.cwd,
      parentSessionId: source.id,
      metadata: options.metadata,
      ...(options.entryId ? { entryId: options.entryId, position: options.position } : {}),
    });
    const metadata = await forked.getMetadata();
    try {
      const sourceBinding = this.novex.bindingOrNull(source.id);
      this.novex.createBinding({
        ...binding,
        session_id: metadata.id,
        parent_session_id: source.id,
        migration_source: options.migrationSource,
      });
      if (options.legacyPromptDisposition === "user_instruction") {
        if (!options.legacySystemPrompt) throw new RuntimeError("storage_unavailable", 503, "旧 Prompt 文本缺失");
        await forked.appendMessage({
          role: "user",
          content: [{ type: "text", text: options.legacySystemPrompt }],
          timestamp: Date.now(),
        });
      }
      this.novex.recordMigrationEvent(metadata.id, options.migrationSource, {
        parent_session_id: source.id,
        source_binding_digest: sourceBinding ? sha256Hex(canonicalJson(sourceBinding)) : null,
        legacy_prompt_disposition: options.legacyPromptDisposition ?? null,
      });
      return forked;
    } catch (error) {
      await cleanupSession(forked);
      await this.repo.delete(metadata);
      this.novex.deleteSessionAudit(metadata.id);
      throw error;
    }
  }

  async appendModelSnapshot(session: PiSession, snapshot: ModelSnapshot): Promise<string> {
    return session.appendCustomEntry("novex.model_snapshot", snapshot);
  }

  async ping(): Promise<void> {
    await this.reconcileSessionDeletions();
    this.novex.ping();
    const probe = await this.repo.create({
      cwd: this.workspaceRoot,
      metadata: {
        model_id: "00000000-0000-4000-8000-000000000000",
        tool_profile: "chat",
        source: "readiness_probe",
      },
    });
    const metadata = await probe.getMetadata();
    await cleanupSession(probe);
    await this.repo.delete(metadata);
  }

  async close(): Promise<void> {
    this.novex.close();
    await this.env.cleanup();
  }

  private async toView(metadata: SqliteSessionMetadata): Promise<SessionView> {
    const own = metadataOf(metadata);
    const binding = this.novex.binding(metadata.id);
    const session = await this.repo.open(metadata);
    try {
      return {
        session_id: metadata.id,
        created_at: metadata.createdAt,
        parent_session_id: metadata.parentSessionId ?? null,
        active_leaf_id: await session.getLeafId(),
        cwd: metadata.cwd,
        model_id: own.model_id,
        agent_key: binding.agent_key,
        agent_version: binding.agent_version,
        registry_digest: binding.registry_digest,
        behavior_fingerprint: binding.behavior_fingerprint,
        tool_profile: own.tool_profile,
        source: own.source,
      };
    } finally {
      await cleanupSession(session);
    }
  }
}

export type { PrepareModelCall, SessionBinding };

export function readSessionMetadata(metadata: SqliteSessionMetadata): NovexSessionMetadata {
  return metadataOf(metadata);
}

export { cleanupSession };
