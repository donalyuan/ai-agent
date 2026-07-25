import { randomUUID } from "node:crypto";
import { backup, DatabaseSync } from "node:sqlite";

import type { PromptSnapshot } from "./definitions.js";
import { RuntimeError } from "./errors.js";
import type { ModelSnapshot } from "./models.js";
import { MODEL_CALL_SCHEMA_VERSION, redactForAudit } from "./redaction.js";
import type { ToolProfile } from "./sessions.js";

export interface SessionBinding {
  session_id: string;
  agent_key: string;
  agent_version: string;
  agent_digest: string;
  prompt_bindings: Record<string, { key: string; version: string }>;
  registry_digest: string;
  tool_profile: ToolProfile;
  model_id: string;
  behavior_fingerprint: string;
  model_snapshot: ModelSnapshot;
  binding_status: "executable" | "read_only" | "model_rebind_required";
  migration_source: string;
  parent_session_id: string | null;
  created_at: string;
}

interface BindingRow {
  session_id: string;
  agent_key: string;
  agent_version: string;
  agent_digest: string;
  prompt_bindings_json: string;
  registry_digest: string;
  tool_profile: ToolProfile;
  model_id: string;
  behavior_fingerprint: string;
  model_snapshot_json: string;
  binding_status: SessionBinding["binding_status"];
  migration_source: string;
  parent_session_id: string | null;
  created_at: string;
}

export interface PrepareModelCall {
  sessionId: string;
  entryId?: string;
  phase: "turn" | "tool_loop" | "compaction" | "branch_summary";
  nodeKey: string;
  attempt: number;
  rootCallId?: string;
  parentCallId?: string;
  binding: SessionBinding;
  promptSnapshot: PromptSnapshot;
  modelSnapshot: ModelSnapshot;
  contextSources: unknown[];
  toolSchema: unknown | null;
  assetReferences: unknown[];
  providerPayload: unknown;
}

export interface ModelCallSummary {
  id: string;
  session_id: string;
  entry_id: string | null;
  phase: string;
  node_key: string;
  attempt: number;
  status: string;
  agent_key: string;
  agent_version: string;
  prompt_key: string;
  prompt_version: string;
  registry_digest: string;
  model_id: string;
  behavior_fingerprint: string;
  usage: unknown | null;
  prepared_at: string;
  completed_at: string | null;
}

export interface ModelCallListFilter {
  sessionId?: string;
  nodeKey?: string;
  agentKey?: string;
  agentVersion?: string;
  promptKey?: string;
  promptVersion?: string;
  modelId?: string;
  status?: "prepared" | "succeeded" | "failed" | "aborted";
  preparedFrom?: string;
  preparedTo?: string;
}

export interface ModelCallPage {
  items: ModelCallSummary[];
  total: number;
}

export class NovexSqliteStore {
  private readonly database: DatabaseSync;

  constructor(databasePath: string) {
    this.database = new DatabaseSync(databasePath);
    this.database.exec("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;");
    this.database.exec(`
      CREATE TABLE IF NOT EXISTS novex_session_bindings (
        session_id TEXT PRIMARY KEY,
        agent_key TEXT NOT NULL,
        agent_version TEXT NOT NULL,
        agent_digest TEXT NOT NULL CHECK(length(agent_digest) = 64),
        prompt_bindings_json TEXT NOT NULL,
        registry_digest TEXT NOT NULL CHECK(length(registry_digest) = 64),
        tool_profile TEXT NOT NULL CHECK(tool_profile IN ('chat', 'workspace')),
        model_id TEXT NOT NULL,
        behavior_fingerprint TEXT NOT NULL CHECK(length(behavior_fingerprint) = 64),
        model_snapshot_json TEXT NOT NULL,
        binding_status TEXT NOT NULL CHECK(binding_status IN ('executable', 'read_only', 'model_rebind_required')),
        migration_source TEXT NOT NULL,
        parent_session_id TEXT,
        created_at TEXT NOT NULL
      );

      CREATE TRIGGER IF NOT EXISTS novex_session_bindings_immutable
      BEFORE UPDATE ON novex_session_bindings
      BEGIN SELECT RAISE(ABORT, 'session binding is immutable'); END;

      CREATE TABLE IF NOT EXISTS novex_model_calls (
        id TEXT PRIMARY KEY,
        schema_version TEXT NOT NULL DEFAULT '${MODEL_CALL_SCHEMA_VERSION}',
        source_runtime TEXT NOT NULL DEFAULT 'pi' CHECK(source_runtime = 'pi'),
        session_id TEXT NOT NULL,
        entry_id TEXT,
        phase TEXT NOT NULL CHECK(phase IN ('turn', 'tool_loop', 'compaction', 'branch_summary')),
        node_key TEXT NOT NULL,
        attempt INTEGER NOT NULL CHECK(attempt > 0),
        root_call_id TEXT,
        parent_call_id TEXT,
        status TEXT NOT NULL DEFAULT 'prepared' CHECK(status IN ('prepared', 'succeeded', 'failed', 'aborted')),
        agent_key TEXT NOT NULL,
        agent_version TEXT NOT NULL,
        prompt_key TEXT NOT NULL,
        prompt_version TEXT NOT NULL,
        registry_digest TEXT NOT NULL,
        prompt_snapshot_json TEXT NOT NULL,
        context_sources_json TEXT NOT NULL,
        memory_sources_json TEXT NOT NULL DEFAULT '[]',
        tool_schema_json TEXT,
        model_id TEXT NOT NULL,
        behavior_fingerprint TEXT NOT NULL,
        model_snapshot_json TEXT NOT NULL,
        parameters_json TEXT NOT NULL DEFAULT '{}',
        asset_references_json TEXT NOT NULL DEFAULT '[]',
        provider_payload_json TEXT NOT NULL,
        output_snapshot_json TEXT,
        usage_snapshot_json TEXT,
        error_snapshot_json TEXT,
        prepared_at TEXT NOT NULL,
        completed_at TEXT,
        FOREIGN KEY(session_id) REFERENCES novex_session_bindings(session_id) ON DELETE CASCADE,
        FOREIGN KEY(root_call_id) REFERENCES novex_model_calls(id) ON DELETE RESTRICT,
        FOREIGN KEY(parent_call_id) REFERENCES novex_model_calls(id) ON DELETE RESTRICT,
        UNIQUE(root_call_id, attempt)
      );
      CREATE INDEX IF NOT EXISTS novex_model_calls_session ON novex_model_calls(session_id, prepared_at DESC);
      CREATE INDEX IF NOT EXISTS novex_model_calls_filter ON novex_model_calls(status, node_key, model_id, prepared_at DESC);
      CREATE UNIQUE INDEX IF NOT EXISTS novex_model_calls_entry ON novex_model_calls(entry_id) WHERE entry_id IS NOT NULL;

      CREATE TRIGGER IF NOT EXISTS novex_model_calls_terminal_once
      BEFORE UPDATE ON novex_model_calls
      WHEN NOT (
        OLD.status = 'prepared'
        AND NEW.status IN ('succeeded', 'failed', 'aborted')
        AND NEW.id IS OLD.id
        AND NEW.schema_version IS OLD.schema_version
        AND NEW.source_runtime IS OLD.source_runtime
        AND NEW.session_id IS OLD.session_id
        AND NEW.phase IS OLD.phase
        AND NEW.node_key IS OLD.node_key
        AND NEW.attempt IS OLD.attempt
        AND NEW.root_call_id IS OLD.root_call_id
        AND NEW.parent_call_id IS OLD.parent_call_id
        AND NEW.agent_key IS OLD.agent_key
        AND NEW.agent_version IS OLD.agent_version
        AND NEW.prompt_key IS OLD.prompt_key
        AND NEW.prompt_version IS OLD.prompt_version
        AND NEW.registry_digest IS OLD.registry_digest
        AND NEW.prompt_snapshot_json IS OLD.prompt_snapshot_json
        AND NEW.context_sources_json IS OLD.context_sources_json
        AND NEW.memory_sources_json IS OLD.memory_sources_json
        AND NEW.tool_schema_json IS OLD.tool_schema_json
        AND NEW.model_id IS OLD.model_id
        AND NEW.behavior_fingerprint IS OLD.behavior_fingerprint
        AND NEW.model_snapshot_json IS OLD.model_snapshot_json
        AND NEW.parameters_json IS OLD.parameters_json
        AND NEW.asset_references_json IS OLD.asset_references_json
        AND NEW.provider_payload_json IS OLD.provider_payload_json
        AND NEW.prepared_at IS OLD.prepared_at
      )
      BEGIN SELECT RAISE(ABORT, 'model call prepared input is immutable and terminal is unique'); END;

      CREATE TABLE IF NOT EXISTS novex_session_deletion_intents (
        session_id TEXT PRIMARY KEY,
        created_at TEXT NOT NULL
      );

      CREATE TABLE IF NOT EXISTS novex_migration_events (
        id TEXT PRIMARY KEY,
        session_id TEXT NOT NULL,
        event_type TEXT NOT NULL,
        details_json TEXT NOT NULL,
        created_at TEXT NOT NULL,
        UNIQUE(session_id, event_type)
      );
    `);
  }

  async backup(destination: string): Promise<number> {
    return backup(this.database, destination);
  }

  createBinding(binding: Omit<SessionBinding, "created_at">): SessionBinding {
    const createdAt = new Date().toISOString();
    try {
      this.database.prepare(`
        INSERT INTO novex_session_bindings (
          session_id, agent_key, agent_version, agent_digest, prompt_bindings_json,
          registry_digest, tool_profile, model_id, behavior_fingerprint, model_snapshot_json,
          binding_status, migration_source, parent_session_id, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
      `).run(
        binding.session_id, binding.agent_key, binding.agent_version, binding.agent_digest,
        JSON.stringify(binding.prompt_bindings), binding.registry_digest, binding.tool_profile,
        binding.model_id, binding.behavior_fingerprint, JSON.stringify(binding.model_snapshot),
        binding.binding_status, binding.migration_source, binding.parent_session_id, createdAt,
      );
    } catch (error) {
      throw new RuntimeError("storage_unavailable", 503, `无法保存不可变 Session binding: ${safeStorageMessage(error)}`);
    }
    return { ...binding, created_at: createdAt };
  }

  binding(sessionId: string): SessionBinding {
    const row = this.database.prepare("SELECT * FROM novex_session_bindings WHERE session_id = ?").get(sessionId) as unknown as BindingRow | undefined;
    if (!row) throw new RuntimeError("session_migration_required", 409, "会话缺少版本化执行 binding");
    return {
      session_id: row.session_id,
      agent_key: row.agent_key,
      agent_version: row.agent_version,
      agent_digest: row.agent_digest,
      prompt_bindings: JSON.parse(row.prompt_bindings_json) as SessionBinding["prompt_bindings"],
      registry_digest: row.registry_digest,
      tool_profile: row.tool_profile,
      model_id: row.model_id,
      behavior_fingerprint: row.behavior_fingerprint,
      model_snapshot: JSON.parse(row.model_snapshot_json) as ModelSnapshot,
      binding_status: row.binding_status,
      migration_source: row.migration_source,
      parent_session_id: row.parent_session_id,
      created_at: row.created_at,
    };
  }

  bindingOrNull(sessionId: string): SessionBinding | null {
    try {
      return this.binding(sessionId);
    } catch (error) {
      if (error instanceof RuntimeError && error.code === "session_migration_required") return null;
      throw error;
    }
  }

  createMigratedBinding(
    binding: Omit<SessionBinding, "created_at">,
    eventType: string,
    details: unknown,
  ): SessionBinding {
    const createdAt = new Date().toISOString();
    this.database.exec("BEGIN IMMEDIATE");
    try {
      this.database.prepare(`
        INSERT OR IGNORE INTO novex_session_bindings (
          session_id, agent_key, agent_version, agent_digest, prompt_bindings_json,
          registry_digest, tool_profile, model_id, behavior_fingerprint, model_snapshot_json,
          binding_status, migration_source, parent_session_id, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
      `).run(
        binding.session_id, binding.agent_key, binding.agent_version, binding.agent_digest,
        JSON.stringify(binding.prompt_bindings), binding.registry_digest, binding.tool_profile,
        binding.model_id, binding.behavior_fingerprint, JSON.stringify(binding.model_snapshot),
        binding.binding_status, binding.migration_source, binding.parent_session_id, createdAt,
      );
      this.database.prepare(`
        INSERT OR IGNORE INTO novex_migration_events (id, session_id, event_type, details_json, created_at)
        VALUES (?, ?, ?, ?, ?)
      `).run(randomUUID(), binding.session_id, eventType, JSON.stringify(details), createdAt);
      this.database.exec("COMMIT");
      return this.binding(binding.session_id);
    } catch (error) {
      this.database.exec("ROLLBACK");
      throw new RuntimeError("storage_unavailable", 503, `无法保存历史 Session migration: ${safeStorageMessage(error)}`);
    }
  }

  recordMigrationEvent(sessionId: string, eventType: string, details: unknown): void {
    try {
      this.database.prepare(`
        INSERT INTO novex_migration_events (id, session_id, event_type, details_json, created_at)
        VALUES (?, ?, ?, ?, ?)
      `).run(randomUUID(), sessionId, eventType, JSON.stringify(details), new Date().toISOString());
    } catch (error) {
      throw new RuntimeError("storage_unavailable", 503, `无法保存 Session 迁移事件: ${safeStorageMessage(error)}`);
    }
  }

  migrationEvent(sessionId: string, eventType: string): Record<string, unknown> {
    const row = this.database.prepare(`
      SELECT session_id, event_type, details_json, created_at
      FROM novex_migration_events WHERE session_id = ? AND event_type = ?
    `).get(sessionId, eventType) as Record<string, unknown> | undefined;
    if (!row) throw new RuntimeError("not_found", 404, "Session 迁移事件不存在");
    return parseJsonColumns(row);
  }

  ping(): void {
    const rows = this.database.prepare(`
      SELECT type, name FROM sqlite_master
      WHERE name IN (
        'novex_session_bindings', 'novex_session_bindings_immutable',
        'novex_model_calls', 'novex_model_calls_session',
        'novex_model_calls_filter', 'novex_model_calls_entry',
        'novex_model_calls_terminal_once', 'novex_session_deletion_intents'
      )
    `).all() as Array<{ type: string; name: string }>;
    const facts = new Set(rows.map(({ type, name }) => `${type}:${name}`));
    const required = [
      "table:novex_session_bindings",
      "trigger:novex_session_bindings_immutable",
      "table:novex_model_calls",
      "index:novex_model_calls_session",
      "index:novex_model_calls_filter",
      "index:novex_model_calls_entry",
      "trigger:novex_model_calls_terminal_once",
      "table:novex_session_deletion_intents",
    ];
    if (required.some((fact) => !facts.has(fact))) {
      throw new RuntimeError("storage_unavailable", 503, "Novex Session/audit schema 未就绪");
    }
  }

  prepareModelCall(input: PrepareModelCall): string {
    const id = randomUUID();
    const preparedAt = new Date().toISOString();
    let promptSnapshot: unknown;
    let contextSources: unknown;
    let modelSnapshot: unknown;
    let providerPayload: unknown;
    let assetReferences: unknown;
    let toolSchema: unknown;
    try {
      promptSnapshot = redactForAudit(input.promptSnapshot);
      contextSources = redactForAudit(input.contextSources);
      modelSnapshot = redactForAudit(input.modelSnapshot);
      providerPayload = redactForAudit(input.providerPayload);
      assetReferences = redactForAudit(input.assetReferences);
      toolSchema = input.toolSchema === null ? null : redactForAudit(input.toolSchema);
      [promptSnapshot, contextSources, modelSnapshot, providerPayload, toolSchema].forEach(assertPersistable);
      assertAssetReferences(assetReferences);
    } catch {
      throw new RuntimeError("audit_persistence_failed", 422, "模型调用审计输入无法安全脱敏或序列化");
    }
    try {
      this.database.prepare(`
        INSERT INTO novex_model_calls (
          id, session_id, entry_id, phase, node_key, attempt, root_call_id, parent_call_id,
          agent_key, agent_version, prompt_key, prompt_version, registry_digest,
          prompt_snapshot_json, context_sources_json, tool_schema_json, model_id,
          behavior_fingerprint, model_snapshot_json, asset_references_json, provider_payload_json, prepared_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
      `).run(
        id, input.sessionId, input.entryId ?? null, input.phase, input.nodeKey, input.attempt,
        input.rootCallId ?? null, input.parentCallId ?? null, input.binding.agent_key,
        input.binding.agent_version, input.promptSnapshot.prompt_key, input.promptSnapshot.prompt_version,
        input.binding.registry_digest, JSON.stringify(promptSnapshot), JSON.stringify(contextSources),
        toolSchema === null ? null : JSON.stringify(toolSchema), input.binding.model_id,
        input.binding.behavior_fingerprint, JSON.stringify(modelSnapshot), JSON.stringify(assetReferences), JSON.stringify(providerPayload),
        preparedAt,
      );
    } catch (error) {
      throw new RuntimeError("audit_persistence_failed", 503, `模型调用前审计持久化失败: ${safeStorageMessage(error)}`);
    }
    return id;
  }

  finishModelCall(
    id: string,
    status: "succeeded" | "failed" | "aborted",
    output: unknown,
    usage: unknown,
    error: unknown,
    entryId?: string,
  ): void {
    let safeOutput: unknown;
    let safeUsage: unknown;
    let safeError: unknown;
    try {
      safeOutput = output === undefined ? undefined : redactForAudit(output);
      safeUsage = usage === undefined ? undefined : redactForAudit(usage);
      safeError = error === undefined ? undefined : redactForAudit(error);
    } catch {
      throw new RuntimeError("audit_persistence_failed", 422, "模型调用终态证据无法安全脱敏或序列化");
    }
    const result = this.database.prepare(`
      UPDATE novex_model_calls
      SET status = ?, output_snapshot_json = ?, usage_snapshot_json = ?, error_snapshot_json = ?,
          entry_id = COALESCE(entry_id, ?), completed_at = ?
      WHERE id = ? AND status = 'prepared'
    `).run(
      status,
      safeOutput === undefined ? null : JSON.stringify(safeOutput),
      safeUsage === undefined ? null : JSON.stringify(safeUsage),
      safeError === undefined ? null : JSON.stringify(safeError),
      entryId ?? null,
      new Date().toISOString(),
      id,
    );
    if (result.changes !== 1) throw new RuntimeError("audit_terminal_conflict", 409, "ModelCall 已存在终态");
  }

  listModelCalls(sessionId: string, limit = 100, offset = 0): ModelCallSummary[] {
    return this.queryModelCalls({ sessionId }, limit, offset).items;
  }

  queryModelCalls(filter: ModelCallListFilter, limit = 100, offset = 0): ModelCallPage {
    const clauses: string[] = [];
    const parameters: Array<string | number> = [];
    const add = (column: string, operator: string, value: string | undefined): void => {
      if (value === undefined) return;
      clauses.push(`${column} ${operator} ?`);
      parameters.push(value);
    };
    add("session_id", "=", filter.sessionId);
    add("node_key", "=", filter.nodeKey);
    add("agent_key", "=", filter.agentKey);
    add("agent_version", "=", filter.agentVersion);
    add("prompt_key", "=", filter.promptKey);
    add("prompt_version", "=", filter.promptVersion);
    add("model_id", "=", filter.modelId);
    add("status", "=", filter.status);
    add("prepared_at", ">=", filter.preparedFrom);
    add("prepared_at", "<=", filter.preparedTo);
    const where = clauses.length === 0 ? "" : ` WHERE ${clauses.join(" AND ")}`;
    const totalRow = this.database.prepare(`SELECT COUNT(*) AS total FROM novex_model_calls${where}`)
      .get(...parameters) as { total: number };
    const rows = this.database.prepare(`
      SELECT id, session_id, entry_id, phase, node_key, attempt, status, agent_key, agent_version,
             prompt_key, prompt_version, registry_digest, model_id, behavior_fingerprint, usage_snapshot_json,
             prepared_at, completed_at
      FROM novex_model_calls${where} ORDER BY prepared_at DESC, id DESC LIMIT ? OFFSET ?
    `).all(...parameters, limit, offset) as Array<Record<string, unknown>>;
    return { total: Number(totalRow.total), items: rows.map((row) => ({
      id: String(row.id), session_id: String(row.session_id), entry_id: row.entry_id === null ? null : String(row.entry_id),
      phase: String(row.phase), node_key: String(row.node_key), attempt: Number(row.attempt), status: String(row.status),
      agent_key: String(row.agent_key), agent_version: String(row.agent_version), prompt_key: String(row.prompt_key),
      prompt_version: String(row.prompt_version), registry_digest: String(row.registry_digest),
      model_id: String(row.model_id), behavior_fingerprint: String(row.behavior_fingerprint),
      usage: row.usage_snapshot_json === null ? null : JSON.parse(String(row.usage_snapshot_json)),
      prepared_at: String(row.prepared_at), completed_at: row.completed_at === null ? null : String(row.completed_at),
    })) };
  }

  modelCall(id: string): Record<string, unknown> {
    const row = this.database.prepare("SELECT * FROM novex_model_calls WHERE id = ?").get(id) as Record<string, unknown> | undefined;
    if (!row) throw new RuntimeError("not_found", 404, "ModelCall 不存在");
    return parseJsonColumns(row);
  }

  deleteSessionAudit(sessionId: string): void {
    this.completeSessionDeletion(sessionId);
  }

  beginSessionDeletion(sessionId: string): void {
    this.database.prepare(`
      INSERT INTO novex_session_deletion_intents (session_id, created_at)
      VALUES (?, ?) ON CONFLICT(session_id) DO NOTHING
    `).run(sessionId, new Date().toISOString());
  }

  pendingSessionDeletions(): string[] {
    return (this.database.prepare(
      "SELECT session_id FROM novex_session_deletion_intents ORDER BY created_at, session_id",
    ).all() as Array<{ session_id: string }>).map(({ session_id }) => session_id);
  }

  completeSessionDeletion(sessionId: string): void {
    this.database.exec("BEGIN IMMEDIATE");
    try {
      while (true) {
        const remaining = this.database.prepare(
          "SELECT COUNT(*) AS total FROM novex_model_calls WHERE session_id = ?",
        ).get(sessionId) as { total: number };
        if (Number(remaining.total) === 0) break;
        const deleted = this.database.prepare(`
          DELETE FROM novex_model_calls AS current
          WHERE current.session_id = ?
            AND NOT EXISTS (
              SELECT 1 FROM novex_model_calls AS child
              WHERE child.root_call_id = current.id OR child.parent_call_id = current.id
            )
        `).run(sessionId);
        if (deleted.changes === 0) throw new Error("ModelCall lineage contains an undeletable cycle");
      }
      this.database.prepare("DELETE FROM novex_session_bindings WHERE session_id = ?").run(sessionId);
      this.database.prepare("DELETE FROM novex_migration_events WHERE session_id = ?").run(sessionId);
      this.database.prepare("DELETE FROM novex_session_deletion_intents WHERE session_id = ?").run(sessionId);
      this.database.exec("COMMIT");
    } catch (error) {
      this.database.exec("ROLLBACK");
      throw error;
    }
  }

  close(): void {
    this.database.close();
  }
}

function parseJsonColumns(row: Record<string, unknown>): Record<string, unknown> {
  return Object.fromEntries(Object.entries(row).map(([key, value]) => {
    if (key.endsWith("_json") && typeof value === "string") return [key.slice(0, -5), JSON.parse(value)];
    return [key, value];
  }));
}

function safeStorageMessage(error: unknown): string {
  return error instanceof Error ? error.message.replace(/[\r\n]/g, " ").slice(0, 240) : "unknown storage error";
}

function assertPersistable(value: unknown): void {
  const visit = (current: unknown): void => {
    if (typeof current === "string") {
      if (/^data:[^;,]+;base64,/i.test(current) || (current.length > 4096 && /^[A-Za-z0-9+/=\r\n]+$/.test(current))) {
        throw new RuntimeError("audit_persistence_failed", 422, "审计快照禁止保存 base64 大对象");
      }
      try {
        const url = new URL(current);
        const signed = [...url.searchParams.keys()].some((key) =>
          /^(x-amz-(signature|credential|expires)|x-tos-(signature|credential|expires)|signature)$/i.test(key));
        if (signed) throw new RuntimeError("audit_persistence_failed", 422, "审计快照禁止保存临时签名 URL");
      } catch (error) {
        if (error instanceof RuntimeError) throw error;
      }
      return;
    }
    if (Array.isArray(current)) {
      current.forEach(visit);
      return;
    }
    if (current !== null && typeof current === "object") Object.values(current).forEach(visit);
  };
  visit(value);
}

function assertAssetReferences(value: unknown): void {
  if (!Array.isArray(value)) throw new RuntimeError("audit_persistence_failed", 422, "asset_references 必须是数组");
  for (const reference of value) {
    if (reference === null || typeof reference !== "object" || Array.isArray(reference)) {
      throw new RuntimeError("audit_persistence_failed", 422, "资产引用必须是 object");
    }
    const object = reference as Record<string, unknown>;
    if (Object.keys(object).some((key) => !["asset_id", "version", "sha256", "mime", "metadata"].includes(key))
      || typeof object.asset_id !== "string" || !object.asset_id
      || typeof object.version !== "string" || !object.version
      || typeof object.sha256 !== "string" || !/^[0-9a-f]{64}$/.test(object.sha256)
      || typeof object.mime !== "string" || !/^(image|audio|video)\/[A-Za-z0-9.+-]+$/.test(object.mime)
      || (object.metadata !== undefined && (object.metadata === null || typeof object.metadata !== "object" || Array.isArray(object.metadata)))) {
      throw new RuntimeError("audit_persistence_failed", 422, "资产引用格式无效");
    }
    assertPersistable(reference);
  }
}
