import type { Session, SessionStorage, SessionTreeEntry } from "@earendil-works/pi-agent-core";
import { NodeExecutionEnv } from "@earendil-works/pi-agent-core/node";
import {
  createNodeSqliteFactory,
  SqliteSessionRepo,
  type SqliteSessionMetadata,
} from "@earendil-works/pi-storage-sqlite-node";

import { RuntimeError } from "./errors.js";
import type { ModelSnapshot } from "./models.js";

export type ToolProfile = "chat" | "workspace";

export interface NovexSessionMetadata {
  model_id: string;
  tool_profile: ToolProfile;
  source: string;
  system_prompt?: string;
}

export interface SessionView {
  session_id: string;
  created_at: string;
  parent_session_id: string | null;
  active_leaf_id: string | null;
  cwd: string;
  model_id: string;
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
  const systemPrompt = raw.system_prompt;
  if (
    typeof modelId !== "string" ||
    (profile !== "chat" && profile !== "workspace") ||
    typeof source !== "string" ||
    (systemPrompt !== undefined && typeof systemPrompt !== "string")
  ) {
    throw new RuntimeError("storage_unavailable", 503, `会话 ${metadata.id} 的 metadata 无效`);
  }
  return {
    model_id: modelId,
    tool_profile: profile,
    source,
    ...(systemPrompt === undefined ? {} : { system_prompt: systemPrompt }),
  };
}

export class SessionStore {
  private readonly env: NodeExecutionEnv;
  private readonly repo: SqliteSessionRepo;

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
  }

  get executionEnv(): NodeExecutionEnv {
    return this.env;
  }

  async create(input: {
    modelId: string;
    toolProfile: ToolProfile;
    source: string;
    systemPrompt?: string;
    parentSessionId?: string;
  }): Promise<PiSession> {
    return this.repo.create({
      cwd: this.workspaceRoot,
      ...(input.parentSessionId ? { parentSessionId: input.parentSessionId } : {}),
      metadata: {
        model_id: input.modelId,
        tool_profile: input.toolProfile,
        source: input.source,
        ...(input.systemPrompt ? { system_prompt: input.systemPrompt } : {}),
      },
    });
  }

  async list(): Promise<SessionView[]> {
    const sessions = await this.repo.list();
    return Promise.all(sessions.map((metadata) => this.toView(metadata)));
  }

  async findMetadata(sessionId: string): Promise<SqliteSessionMetadata> {
    const metadata = (await this.repo.list()).find((candidate) => candidate.id === sessionId);
    if (!metadata) throw new RuntimeError("session_not_found", 404, "会话不存在");
    return metadata;
  }

  async open(sessionId: string): Promise<PiSession> {
    return this.repo.open(await this.findMetadata(sessionId));
  }

  async delete(sessionId: string): Promise<void> {
    await this.repo.delete(await this.findMetadata(sessionId));
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
    return this.repo.fork(source, {
      cwd: source.cwd,
      parentSessionId: source.id,
      ...(source.metadata ? { metadata: source.metadata } : {}),
      ...(entryId ? { entryId, position } : {}),
    });
  }

  async appendModelSnapshot(session: PiSession, snapshot: ModelSnapshot): Promise<string> {
    return session.appendCustomEntry("novex.model_snapshot", snapshot);
  }

  async ping(): Promise<void> {
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
    await this.env.cleanup();
  }

  private async toView(metadata: SqliteSessionMetadata): Promise<SessionView> {
    const own = metadataOf(metadata);
    const session = await this.repo.open(metadata);
    try {
      return {
        session_id: metadata.id,
        created_at: metadata.createdAt,
        parent_session_id: metadata.parentSessionId ?? null,
        active_leaf_id: await session.getLeafId(),
        cwd: metadata.cwd,
        model_id: own.model_id,
        tool_profile: own.tool_profile,
        source: own.source,
      };
    } finally {
      await cleanupSession(session);
    }
  }
}

export function readSessionMetadata(metadata: SqliteSessionMetadata): NovexSessionMetadata {
  return metadataOf(metadata);
}

export { cleanupSession };
