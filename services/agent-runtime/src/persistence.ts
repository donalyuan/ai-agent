import { randomUUID } from "node:crypto";
import { backup, DatabaseSync } from "node:sqlite";

import type { ContextCompileAttempt, ContextSnapshot } from "./context.js";
import { canonicalJson, readPromptSnapshot, type PromptSnapshot } from "./definitions.js";
import { RuntimeError } from "./errors.js";
import type { ModelSnapshot } from "./models.js";
import { GOVERNED_MODEL_CALL_SCHEMA_VERSION, MODEL_CALL_SCHEMA_VERSION, redactForAudit } from "./redaction.js";
import type { ToolProfile } from "./sessions.js";

export interface SessionBinding {
  session_id: string;
  agent_key: string;
  agent_version: string;
  agent_digest: string;
  prompt_bindings: Record<string, { key: string; version: string }>;
  context_policy_bindings: Record<string, { key: string; version: string; digest: string }>;
  tokenizer_profile_key: string;
  tokenizer_profile_version: string;
  tokenizer_profile_digest: string;
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
  context_policy_bindings_json: string | null;
  tokenizer_profile_key: string | null;
  tokenizer_profile_version: string | null;
  tokenizer_profile_digest: string | null;
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

export interface PersistContextSnapshot {
  id: string;
  sessionId: string;
  phase: PrepareModelCall["phase"];
  snapshot: ContextSnapshot;
}

export interface PersistContextCompileAttempt {
  sessionId: string;
  phase: PrepareModelCall["phase"];
  attempt: ContextCompileAttempt;
}

export interface PrepareModelCallWithContext extends PrepareModelCall {
  contextSnapshot: ContextSnapshot;
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

export interface ContextRecordListFilter {
  sessionId?: string;
  recordType?: "snapshot" | "compile_attempt";
  nodeKey?: string;
}

export interface ContextRecordSummary {
  id: string;
  record_type: "snapshot" | "compile_attempt";
  owner: { type: "session"; id: string };
  node_key: string;
  status: "succeeded" | "failed";
  compiled_at: string;
  policy: { key: string; version: string } | null;
  tokenizer_profile: { key: string; version: string } | null;
  digest: string;
  created_at: string;
}

export interface ContextRecordPage { items: ContextRecordSummary[]; total: number }

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
        context_policy_bindings_json TEXT,
        tokenizer_profile_key TEXT,
        tokenizer_profile_version TEXT,
        tokenizer_profile_digest TEXT,
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
        context_snapshot_id TEXT,
        context_digest TEXT,
        context_policy_key TEXT,
        context_policy_version TEXT,
        tokenizer_profile_key TEXT,
        tokenizer_profile_version TEXT,
        context_budget_summary_json TEXT,
        prepared_at TEXT NOT NULL,
        completed_at TEXT,
        FOREIGN KEY(session_id) REFERENCES novex_session_bindings(session_id) ON DELETE CASCADE,
        FOREIGN KEY(root_call_id) REFERENCES novex_model_calls(id) ON DELETE RESTRICT,
        FOREIGN KEY(parent_call_id) REFERENCES novex_model_calls(id) ON DELETE RESTRICT,
        FOREIGN KEY(context_snapshot_id) REFERENCES novex_context_snapshots(id) ON DELETE RESTRICT,
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
    this.ensureGovernedContextSchema();
  }

  private ensureGovernedContextSchema(): void {
    this.addColumnIfMissing("novex_session_bindings", "context_policy_bindings_json", "TEXT");
    this.addColumnIfMissing("novex_session_bindings", "tokenizer_profile_key", "TEXT");
    this.addColumnIfMissing("novex_session_bindings", "tokenizer_profile_version", "TEXT");
    this.addColumnIfMissing("novex_session_bindings", "tokenizer_profile_digest", "TEXT");
    this.addColumnIfMissing("novex_model_calls", "context_snapshot_id", "TEXT");
    this.addColumnIfMissing("novex_model_calls", "context_digest", "TEXT");
    this.addColumnIfMissing("novex_model_calls", "context_policy_key", "TEXT");
    this.addColumnIfMissing("novex_model_calls", "context_policy_version", "TEXT");
    this.addColumnIfMissing("novex_model_calls", "tokenizer_profile_key", "TEXT");
    this.addColumnIfMissing("novex_model_calls", "tokenizer_profile_version", "TEXT");
    this.addColumnIfMissing("novex_model_calls", "context_budget_summary_json", "TEXT");
    this.database.exec(`
      CREATE TABLE IF NOT EXISTS novex_context_snapshots (
        id TEXT PRIMARY KEY,
        schema_version TEXT NOT NULL DEFAULT '2' CHECK(schema_version = '2'),
        source_runtime TEXT NOT NULL DEFAULT 'pi' CHECK(source_runtime = 'pi'),
        session_id TEXT NOT NULL,
        phase TEXT NOT NULL CHECK(phase IN ('turn', 'tool_loop', 'compaction', 'branch_summary')),
        node_key TEXT NOT NULL CHECK(length(trim(node_key)) > 0),
        status TEXT NOT NULL DEFAULT 'succeeded' CHECK(status = 'succeeded'),
        compiled_at TEXT NOT NULL,
        policy_key TEXT NOT NULL,
        policy_version TEXT NOT NULL,
        tokenizer_profile_key TEXT NOT NULL,
        tokenizer_profile_version TEXT NOT NULL,
        tokenizer_mode TEXT NOT NULL CHECK(tokenizer_mode IN ('exact', 'conservative')),
        model_context_window INTEGER NOT NULL CHECK(model_context_window > 0),
        budget_ledger_json TEXT NOT NULL CHECK(json_valid(budget_ledger_json) AND json_type(budget_ledger_json) = 'object'),
        decisions_json TEXT NOT NULL CHECK(json_valid(decisions_json) AND json_type(decisions_json) = 'array'),
        selected_order_json TEXT NOT NULL CHECK(json_valid(selected_order_json) AND json_type(selected_order_json) = 'array'),
        logical_input_json TEXT NOT NULL CHECK(json_valid(logical_input_json) AND json_type(logical_input_json) = 'object'),
        context_digest TEXT NOT NULL CHECK(length(context_digest) = 64 AND context_digest NOT GLOB '*[^0-9a-f]*'),
        created_at TEXT NOT NULL,
        FOREIGN KEY(session_id) REFERENCES novex_session_bindings(session_id) ON DELETE CASCADE
      );
      CREATE INDEX IF NOT EXISTS novex_context_snapshots_session
        ON novex_context_snapshots(session_id, compiled_at DESC);

      CREATE TRIGGER IF NOT EXISTS novex_context_snapshots_payload_guard
      BEFORE INSERT ON novex_context_snapshots
      WHEN EXISTS (
        SELECT 1 FROM json_each(NEW.decisions_json) decision
        WHERE (json_extract(decision.value, '$.decision') = 'selected'
               AND json_type(decision.value, '$.selected_payload') IS NULL)
           OR (json_extract(decision.value, '$.decision') <> 'selected'
               AND json_type(decision.value, '$.selected_payload') IS NOT NULL)
      )
      BEGIN SELECT RAISE(ABORT, 'ContextSnapshot decision payload contract violated'); END;

      CREATE TRIGGER IF NOT EXISTS novex_context_snapshots_immutable
      BEFORE UPDATE ON novex_context_snapshots
      BEGIN SELECT RAISE(ABORT, 'ContextSnapshot is immutable'); END;

      CREATE TRIGGER IF NOT EXISTS novex_context_snapshots_no_direct_delete
      BEFORE DELETE ON novex_context_snapshots
      WHEN NOT EXISTS (
        SELECT 1 FROM novex_session_deletion_intents intent WHERE intent.session_id = OLD.session_id
      )
      BEGIN SELECT RAISE(ABORT, 'ContextSnapshot is immutable'); END;

      CREATE TABLE IF NOT EXISTS novex_context_compile_attempts (
        id TEXT PRIMARY KEY,
        schema_version TEXT NOT NULL DEFAULT '2' CHECK(schema_version = '2'),
        source_runtime TEXT NOT NULL DEFAULT 'pi' CHECK(source_runtime = 'pi'),
        session_id TEXT NOT NULL,
        phase TEXT NOT NULL CHECK(phase IN ('turn', 'tool_loop', 'compaction', 'branch_summary')),
        node_key TEXT NOT NULL CHECK(length(trim(node_key)) > 0),
        status TEXT NOT NULL DEFAULT 'failed' CHECK(status = 'failed'),
        compiled_at TEXT NOT NULL,
        stage TEXT NOT NULL CHECK(stage IN ('schema', 'eligibility', 'conflict', 'tokenizer', 'budget', 'finalize')),
        code TEXT NOT NULL CHECK(length(trim(code)) > 0),
        budget_ledger_json TEXT CHECK(budget_ledger_json IS NULL OR (json_valid(budget_ledger_json) AND json_type(budget_ledger_json) = 'object')),
        decisions_json TEXT NOT NULL CHECK(json_valid(decisions_json) AND json_type(decisions_json) = 'array'),
        attempt_digest TEXT NOT NULL CHECK(length(attempt_digest) = 64 AND attempt_digest NOT GLOB '*[^0-9a-f]*'),
        created_at TEXT NOT NULL,
        FOREIGN KEY(session_id) REFERENCES novex_session_bindings(session_id) ON DELETE CASCADE
      );
      CREATE INDEX IF NOT EXISTS novex_context_compile_attempts_session
        ON novex_context_compile_attempts(session_id, compiled_at DESC);

      CREATE TRIGGER IF NOT EXISTS novex_context_compile_attempts_payload_guard
      BEFORE INSERT ON novex_context_compile_attempts
      WHEN EXISTS (
        SELECT 1 FROM json_each(NEW.decisions_json) decision
        WHERE json_type(decision.value, '$.selected_payload') IS NOT NULL
      )
      BEGIN SELECT RAISE(ABORT, 'ContextCompileAttempt payload is forbidden'); END;

      CREATE TRIGGER IF NOT EXISTS novex_context_compile_attempts_immutable
      BEFORE UPDATE ON novex_context_compile_attempts
      BEGIN SELECT RAISE(ABORT, 'ContextCompileAttempt is immutable'); END;

      CREATE TRIGGER IF NOT EXISTS novex_context_compile_attempts_no_direct_delete
      BEFORE DELETE ON novex_context_compile_attempts
      WHEN NOT EXISTS (
        SELECT 1 FROM novex_session_deletion_intents intent WHERE intent.session_id = OLD.session_id
      )
      BEGIN SELECT RAISE(ABORT, 'ContextCompileAttempt is immutable'); END;

      CREATE UNIQUE INDEX IF NOT EXISTS novex_model_calls_context_snapshot
        ON novex_model_calls(context_snapshot_id) WHERE context_snapshot_id IS NOT NULL;

      CREATE TRIGGER IF NOT EXISTS novex_model_calls_context_contract_insert
      BEFORE INSERT ON novex_model_calls
      WHEN (
        (NEW.context_snapshot_id IS NULL) <> (NEW.context_digest IS NULL)
        OR (NEW.context_snapshot_id IS NULL) <> (NEW.context_policy_key IS NULL)
        OR (NEW.context_snapshot_id IS NULL) <> (NEW.context_policy_version IS NULL)
        OR (NEW.context_snapshot_id IS NULL) <> (NEW.tokenizer_profile_key IS NULL)
        OR (NEW.context_snapshot_id IS NULL) <> (NEW.tokenizer_profile_version IS NULL)
        OR (NEW.context_snapshot_id IS NULL) <> (NEW.context_budget_summary_json IS NULL)
        OR (NEW.context_snapshot_id IS NOT NULL AND NOT EXISTS (
          SELECT 1 FROM novex_context_snapshots snapshot
          WHERE snapshot.id = NEW.context_snapshot_id AND snapshot.session_id = NEW.session_id
        ))
      )
      BEGIN SELECT RAISE(ABORT, 'ModelCall Context evidence is incomplete or mismatched'); END;
    `);
    this.ensureModelCallContextForeignKey();
    this.ensureModelCallIndexesAndContextTrigger();
    this.database.exec("DROP TRIGGER IF EXISTS novex_model_calls_terminal_once");
    this.database.exec(`
      CREATE TRIGGER novex_model_calls_terminal_once
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
        AND NEW.context_snapshot_id IS OLD.context_snapshot_id
        AND NEW.context_digest IS OLD.context_digest
        AND NEW.context_policy_key IS OLD.context_policy_key
        AND NEW.context_policy_version IS OLD.context_policy_version
        AND NEW.tokenizer_profile_key IS OLD.tokenizer_profile_key
        AND NEW.tokenizer_profile_version IS OLD.tokenizer_profile_version
        AND NEW.context_budget_summary_json IS OLD.context_budget_summary_json
        AND NEW.prepared_at IS OLD.prepared_at
      )
      BEGIN SELECT RAISE(ABORT, 'model call prepared input is immutable and terminal is unique'); END;
    `);
  }

  private ensureModelCallContextForeignKey(): void {
    const foreignKeys = this.database.prepare("PRAGMA foreign_key_list(novex_model_calls)").all() as unknown as Array<{
      table: string;
      from: string;
    }>;
    if (foreignKeys.some((foreignKey) =>
      foreignKey.table === "novex_context_snapshots" && foreignKey.from === "context_snapshot_id")) return;

    const invalidContext = this.database.prepare(`
      SELECT call.id
      FROM novex_model_calls call
      LEFT JOIN novex_context_snapshots snapshot ON snapshot.id = call.context_snapshot_id
      WHERE call.context_snapshot_id IS NOT NULL
        AND (snapshot.id IS NULL OR snapshot.session_id <> call.session_id)
      LIMIT 1
    `).get() as { id: string } | undefined;
    if (invalidContext) {
      throw new RuntimeError(
        "storage_unavailable",
        503,
        `ModelCall ${invalidContext.id} 的 ContextSnapshot 引用无效，拒绝迁移`,
      );
    }

    const columns = [
      "id", "schema_version", "source_runtime", "session_id", "entry_id", "phase", "node_key", "attempt",
      "root_call_id", "parent_call_id", "status", "agent_key", "agent_version", "prompt_key", "prompt_version",
      "registry_digest", "prompt_snapshot_json", "context_sources_json", "memory_sources_json", "tool_schema_json",
      "model_id", "behavior_fingerprint", "model_snapshot_json", "parameters_json", "asset_references_json",
      "provider_payload_json", "output_snapshot_json", "usage_snapshot_json", "error_snapshot_json",
      "context_snapshot_id", "context_digest", "context_policy_key", "context_policy_version",
      "tokenizer_profile_key", "tokenizer_profile_version", "context_budget_summary_json", "prepared_at", "completed_at",
    ];
    this.database.exec("PRAGMA foreign_keys = OFF");
    this.database.exec("BEGIN IMMEDIATE");
    try {
      this.database.exec(modelCallsTableSql("novex_model_calls_context_v2"));
      this.database.exec(`
        INSERT INTO novex_model_calls_context_v2 (${columns.join(", ")})
        SELECT ${columns.join(", ")} FROM novex_model_calls;
        DROP TABLE novex_model_calls;
        ALTER TABLE novex_model_calls_context_v2 RENAME TO novex_model_calls;
      `);
      this.database.exec("COMMIT");
    } catch (error) {
      this.database.exec("ROLLBACK");
      throw new RuntimeError("storage_unavailable", 503, `ModelCall Context FK 迁移失败: ${safeStorageMessage(error)}`);
    } finally {
      this.database.exec("PRAGMA foreign_keys = ON");
    }
    const violations = this.database.prepare("PRAGMA foreign_key_check").all();
    if (violations.length > 0) {
      throw new RuntimeError("storage_unavailable", 503, "ModelCall Context FK 迁移后完整性检查失败");
    }
  }

  private ensureModelCallIndexesAndContextTrigger(): void {
    this.database.exec(`
      CREATE INDEX IF NOT EXISTS novex_model_calls_session
        ON novex_model_calls(session_id, prepared_at DESC);
      CREATE INDEX IF NOT EXISTS novex_model_calls_filter
        ON novex_model_calls(status, node_key, model_id, prepared_at DESC);
      CREATE UNIQUE INDEX IF NOT EXISTS novex_model_calls_entry
        ON novex_model_calls(entry_id) WHERE entry_id IS NOT NULL;
      CREATE UNIQUE INDEX IF NOT EXISTS novex_model_calls_context_snapshot
        ON novex_model_calls(context_snapshot_id) WHERE context_snapshot_id IS NOT NULL;
      CREATE TRIGGER IF NOT EXISTS novex_model_calls_context_contract_insert
      BEFORE INSERT ON novex_model_calls
      WHEN (
        (NEW.context_snapshot_id IS NULL) <> (NEW.context_digest IS NULL)
        OR (NEW.context_snapshot_id IS NULL) <> (NEW.context_policy_key IS NULL)
        OR (NEW.context_snapshot_id IS NULL) <> (NEW.context_policy_version IS NULL)
        OR (NEW.context_snapshot_id IS NULL) <> (NEW.tokenizer_profile_key IS NULL)
        OR (NEW.context_snapshot_id IS NULL) <> (NEW.tokenizer_profile_version IS NULL)
        OR (NEW.context_snapshot_id IS NULL) <> (NEW.context_budget_summary_json IS NULL)
        OR (NEW.context_snapshot_id IS NOT NULL AND NOT EXISTS (
          SELECT 1 FROM novex_context_snapshots snapshot
          WHERE snapshot.id = NEW.context_snapshot_id AND snapshot.session_id = NEW.session_id
        ))
      )
      BEGIN SELECT RAISE(ABORT, 'ModelCall Context evidence is incomplete or mismatched'); END;
    `);
  }

  private addColumnIfMissing(table: string, column: string, definition: string): void {
    const columns = this.database.prepare(`PRAGMA table_info(${table})`).all() as unknown as Array<{ name: string }>;
    if (!columns.some(({ name }) => name === column)) {
      this.database.exec(`ALTER TABLE ${table} ADD COLUMN ${column} ${definition}`);
    }
  }

  async backup(destination: string): Promise<number> {
    return backup(this.database, destination);
  }

  createBinding(binding: Omit<SessionBinding, "created_at">): SessionBinding {
    validateSessionBinding(binding);
    const createdAt = new Date().toISOString();
    try {
      this.database.prepare(`
        INSERT INTO novex_session_bindings (
          session_id, agent_key, agent_version, agent_digest, prompt_bindings_json,
          context_policy_bindings_json, tokenizer_profile_key, tokenizer_profile_version,
          tokenizer_profile_digest, registry_digest, tool_profile, model_id,
          behavior_fingerprint, model_snapshot_json, binding_status, migration_source,
          parent_session_id, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
      `).run(
        binding.session_id, binding.agent_key, binding.agent_version, binding.agent_digest,
        JSON.stringify(binding.prompt_bindings), JSON.stringify(binding.context_policy_bindings),
        binding.tokenizer_profile_key, binding.tokenizer_profile_version, binding.tokenizer_profile_digest,
        binding.registry_digest, binding.tool_profile, binding.model_id, binding.behavior_fingerprint,
        JSON.stringify(binding.model_snapshot), binding.binding_status, binding.migration_source,
        binding.parent_session_id, createdAt,
      );
    } catch (error) {
      throw new RuntimeError("storage_unavailable", 503, `无法保存不可变 Session binding: ${safeStorageMessage(error)}`);
    }
    return { ...binding, created_at: createdAt };
  }

  binding(sessionId: string): SessionBinding {
    const row = this.database.prepare("SELECT * FROM novex_session_bindings WHERE session_id = ?").get(sessionId) as unknown as BindingRow | undefined;
    if (!row) throw new RuntimeError("session_migration_required", 409, "会话缺少版本化执行 binding");
    if (row.context_policy_bindings_json === null || row.tokenizer_profile_key === null
      || row.tokenizer_profile_version === null || row.tokenizer_profile_digest === null) {
      throw new RuntimeError("session_migration_required", 409, "会话缺少固定 Context Policy/Tokenizer Profile binding");
    }
    const binding: SessionBinding = {
      session_id: row.session_id,
      agent_key: row.agent_key,
      agent_version: row.agent_version,
      agent_digest: row.agent_digest,
      prompt_bindings: JSON.parse(row.prompt_bindings_json) as SessionBinding["prompt_bindings"],
      context_policy_bindings: JSON.parse(row.context_policy_bindings_json) as SessionBinding["context_policy_bindings"],
      tokenizer_profile_key: row.tokenizer_profile_key,
      tokenizer_profile_version: row.tokenizer_profile_version,
      tokenizer_profile_digest: row.tokenizer_profile_digest,
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
    validateSessionBinding(binding);
    return binding;
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
    validateSessionBinding(binding);
    const createdAt = new Date().toISOString();
    this.database.exec("BEGIN IMMEDIATE");
    try {
      this.database.prepare(`
        INSERT OR IGNORE INTO novex_session_bindings (
          session_id, agent_key, agent_version, agent_digest, prompt_bindings_json,
          context_policy_bindings_json, tokenizer_profile_key, tokenizer_profile_version,
          tokenizer_profile_digest, registry_digest, tool_profile, model_id,
          behavior_fingerprint, model_snapshot_json, binding_status, migration_source,
          parent_session_id, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
      `).run(
        binding.session_id, binding.agent_key, binding.agent_version, binding.agent_digest,
        JSON.stringify(binding.prompt_bindings), JSON.stringify(binding.context_policy_bindings),
        binding.tokenizer_profile_key, binding.tokenizer_profile_version, binding.tokenizer_profile_digest,
        binding.registry_digest, binding.tool_profile, binding.model_id, binding.behavior_fingerprint,
        JSON.stringify(binding.model_snapshot), binding.binding_status, binding.migration_source,
        binding.parent_session_id, createdAt,
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

  hasHistoryMigrationOutcome(sessionId: string): boolean {
    const row = this.database.prepare(`
      SELECT 1 AS present FROM novex_migration_events
      WHERE session_id = ? AND event_type LIKE 'context_history_v2_%'
      LIMIT 1
    `).get(sessionId) as { present: number } | undefined;
    return row?.present === 1;
  }

  ping(): void {
    const rows = this.database.prepare(`
      SELECT type, name FROM sqlite_master
      WHERE name IN (
        'novex_session_bindings', 'novex_session_bindings_immutable',
        'novex_model_calls', 'novex_model_calls_session',
        'novex_model_calls_filter', 'novex_model_calls_entry',
        'novex_model_calls_terminal_once', 'novex_session_deletion_intents',
        'novex_context_snapshots', 'novex_context_snapshots_immutable',
        'novex_context_compile_attempts', 'novex_context_compile_attempts_immutable',
        'novex_context_snapshots_session', 'novex_context_compile_attempts_session',
        'novex_model_calls_context_snapshot'
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
      "table:novex_context_snapshots",
      "trigger:novex_context_snapshots_immutable",
      "table:novex_context_compile_attempts",
      "trigger:novex_context_compile_attempts_immutable",
      "index:novex_context_snapshots_session",
      "index:novex_context_compile_attempts_session",
      "index:novex_model_calls_context_snapshot",
    ];
    if (required.some((fact) => !facts.has(fact))) {
      throw new RuntimeError("storage_unavailable", 503, "Novex Session/audit schema 未就绪");
    }
  }

  persistContextSnapshot(input: PersistContextSnapshot): string {
    if (!validUuid(input.id)) invalidContextRecord("ContextSnapshot ID 必须是 UUID");
    const snapshot = sanitizeContextSnapshot(input.sessionId, input.snapshot);
    try {
      this.insertContextSnapshot(input.id, input.sessionId, input.phase, snapshot);
    } catch (error) {
      throw new RuntimeError("audit_persistence_failed", 503, `ContextSnapshot 持久化失败: ${safeStorageMessage(error)}`);
    }
    return input.id;
  }

  persistContextCompileAttempt(input: PersistContextCompileAttempt): string {
    const id = randomUUID();
    const attempt = sanitizeContextCompileAttempt(input.sessionId, input.attempt);
    try {
      this.database.prepare(`
        INSERT INTO novex_context_compile_attempts (
          id, session_id, phase, node_key, compiled_at, stage, code,
          budget_ledger_json, decisions_json, attempt_digest, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
      `).run(
        id,
        input.sessionId,
        input.phase,
        attempt.node_key,
        attempt.compiled_at,
        attempt.stage,
        attempt.code,
        attempt.budget === null ? null : JSON.stringify(attempt.budget),
        JSON.stringify(attempt.decisions),
        attempt.digest,
        new Date().toISOString(),
      );
    } catch (error) {
      throw new RuntimeError("audit_persistence_failed", 503, `ContextCompileAttempt 持久化失败: ${safeStorageMessage(error)}`);
    }
    return id;
  }

  prepareModelCallWithContext(input: PrepareModelCallWithContext): string {
    const contextSnapshotId = validateGovernedModelCall(input);
    const snapshot = sanitizeContextSnapshot(input.sessionId, input.contextSnapshot);
    const safeInput = sanitizeModelCallInput(input);
    const id = randomUUID();
    const preparedAt = new Date().toISOString();

    this.database.exec("BEGIN IMMEDIATE");
    try {
      this.insertContextSnapshot(contextSnapshotId, input.sessionId, input.phase, snapshot);
      this.insertPreparedModelCall(input, safeInput, id, preparedAt, {
        snapshotId: contextSnapshotId,
        snapshot,
      });
      this.database.exec("COMMIT");
    } catch (error) {
      this.database.exec("ROLLBACK");
      throw new RuntimeError("audit_persistence_failed", 503, `Context/ModelCall 事务持久化失败: ${safeStorageMessage(error)}`);
    }
    return id;
  }

  private insertContextSnapshot(
    id: string,
    sessionId: string,
    phase: PrepareModelCall["phase"],
    snapshot: ContextSnapshot,
  ): void {
    this.database.prepare(`
      INSERT INTO novex_context_snapshots (
        id, session_id, phase, node_key, compiled_at, policy_key, policy_version,
        tokenizer_profile_key, tokenizer_profile_version, tokenizer_mode,
        model_context_window, budget_ledger_json, decisions_json, selected_order_json,
        logical_input_json, context_digest, created_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `).run(
      id,
      sessionId,
      phase,
      snapshot.node_key,
      snapshot.compiled_at,
      snapshot.policy_key,
      snapshot.policy_version,
      snapshot.tokenizer_profile_key,
      snapshot.tokenizer_profile_version,
      snapshot.tokenizer_mode,
      snapshot.budget.model_context_window,
      JSON.stringify(snapshot.budget),
      JSON.stringify(snapshot.decisions),
      JSON.stringify(snapshot.selected_order),
      JSON.stringify(snapshot.logical_input),
      snapshot.digest,
      new Date().toISOString(),
    );
  }

  private insertPreparedModelCall(
    input: PrepareModelCall,
    safe: SanitizedModelCallInput,
    id: string,
    preparedAt: string,
    context?: { snapshotId: string; snapshot: ContextSnapshot },
  ): void {
    this.database.prepare(`
      INSERT INTO novex_model_calls (
        id, schema_version, session_id, entry_id, phase, node_key, attempt, root_call_id, parent_call_id,
        agent_key, agent_version, prompt_key, prompt_version, registry_digest,
        prompt_snapshot_json, context_sources_json, tool_schema_json, model_id,
        behavior_fingerprint, model_snapshot_json, asset_references_json, provider_payload_json,
        context_snapshot_id, context_digest, context_policy_key, context_policy_version,
        tokenizer_profile_key, tokenizer_profile_version, context_budget_summary_json, prepared_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `).run(
      id,
      context === undefined ? MODEL_CALL_SCHEMA_VERSION : GOVERNED_MODEL_CALL_SCHEMA_VERSION,
      input.sessionId,
      input.entryId ?? null,
      input.phase,
      input.nodeKey,
      input.attempt,
      input.rootCallId ?? null,
      input.parentCallId ?? null,
      input.binding.agent_key,
      input.binding.agent_version,
      input.promptSnapshot.prompt_key,
      input.promptSnapshot.prompt_version,
      input.binding.registry_digest,
      JSON.stringify(safe.promptSnapshot),
      JSON.stringify(safe.contextSources),
      safe.toolSchema === null ? null : JSON.stringify(safe.toolSchema),
      input.binding.model_id,
      input.binding.behavior_fingerprint,
      JSON.stringify(safe.modelSnapshot),
      JSON.stringify(safe.assetReferences),
      JSON.stringify(safe.providerPayload),
      context?.snapshotId ?? null,
      context?.snapshot.digest ?? null,
      context?.snapshot.policy_key ?? null,
      context?.snapshot.policy_version ?? null,
      context?.snapshot.tokenizer_profile_key ?? null,
      context?.snapshot.tokenizer_profile_version ?? null,
      context === undefined ? null : JSON.stringify(context.snapshot.budget),
      preparedAt,
    );
  }

  prepareModelCall(input: PrepareModelCall): string {
    const id = randomUUID();
    const preparedAt = new Date().toISOString();
    let safeInput: SanitizedModelCallInput;
    try {
      safeInput = sanitizeModelCallInput(input);
    } catch {
      throw new RuntimeError("audit_persistence_failed", 422, "模型调用审计输入无法安全脱敏或序列化");
    }
    try {
      this.insertPreparedModelCall(input, safeInput, id, preparedAt);
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

  queryContextRecords(
    filter: ContextRecordListFilter,
    limit: number,
    offset: number,
  ): ContextRecordPage {
    const conditions: string[] = [];
    const parameters: Array<string | number> = [];
    if (filter.sessionId) { conditions.push("session_id = ?"); parameters.push(filter.sessionId); }
    if (filter.recordType) { conditions.push("record_type = ?"); parameters.push(filter.recordType); }
    if (filter.nodeKey) { conditions.push("node_key = ?"); parameters.push(filter.nodeKey); }
    const where = conditions.length === 0 ? "" : ` WHERE ${conditions.join(" AND ")}`;
    const union = `
      SELECT id, 'snapshot' AS record_type, session_id, node_key, 'succeeded' AS status,
             compiled_at, policy_key, policy_version, tokenizer_profile_key,
             tokenizer_profile_version, context_digest AS digest, created_at
        FROM novex_context_snapshots
      UNION ALL
      SELECT id, 'compile_attempt' AS record_type, session_id, node_key, 'failed' AS status,
             compiled_at, NULL AS policy_key, NULL AS policy_version, NULL AS tokenizer_profile_key,
             NULL AS tokenizer_profile_version, attempt_digest AS digest, created_at
        FROM novex_context_compile_attempts`;
    const total = this.database.prepare(`SELECT COUNT(*) AS total FROM (${union})${where}`)
      .get(...parameters) as { total: number };
    const rows = this.database.prepare(`
      SELECT * FROM (${union})${where}
      ORDER BY compiled_at DESC, id DESC LIMIT ? OFFSET ?
    `).all(...parameters, limit, offset) as Array<Record<string, unknown>>;
    return { total: Number(total.total), items: rows.map(contextSummaryFromRow) };
  }

  contextRecord(id: string): ContextRecordSummary & Record<string, unknown> {
    const snapshot = this.database.prepare("SELECT * FROM novex_context_snapshots WHERE id = ?").get(id) as
      Record<string, unknown> | undefined;
    if (snapshot) {
      return {
        ...contextSummaryFromRow({ ...snapshot, record_type: "snapshot", status: "succeeded", digest: snapshot.context_digest }),
        tokenizer_mode: String(snapshot.tokenizer_mode),
        budget: JSON.parse(String(snapshot.budget_ledger_json)),
        decisions: JSON.parse(String(snapshot.decisions_json)),
        selected_order: JSON.parse(String(snapshot.selected_order_json)),
        logical_input: JSON.parse(String(snapshot.logical_input_json)),
      };
    }
    const attempt = this.database.prepare("SELECT * FROM novex_context_compile_attempts WHERE id = ?").get(id) as
      Record<string, unknown> | undefined;
    if (!attempt) throw new RuntimeError("not_found", 404, "Context 审计记录不存在");
    return {
      ...contextSummaryFromRow({ ...attempt, record_type: "compile_attempt", status: "failed", digest: attempt.attempt_digest }),
      stage: String(attempt.stage),
      code: String(attempt.code),
      budget: attempt.budget_ledger_json === null ? null : JSON.parse(String(attempt.budget_ledger_json)),
      decisions: JSON.parse(String(attempt.decisions_json)),
    };
  }

  contextSnapshot(id: string): ContextSnapshot {
    const row = this.database.prepare("SELECT * FROM novex_context_snapshots WHERE id = ?").get(id) as
      Record<string, unknown> | undefined;
    if (!row) throw new RuntimeError("not_found", 404, "ContextSnapshot 不存在");
    return {
      schema_version: "2",
      owner: "pi",
      owner_id: String(row.session_id),
      node_key: String(row.node_key),
      compiled_at: String(row.compiled_at),
      policy_key: String(row.policy_key),
      policy_version: String(row.policy_version),
      tokenizer_profile_key: String(row.tokenizer_profile_key),
      tokenizer_profile_version: String(row.tokenizer_profile_version),
      tokenizer_mode: row.tokenizer_mode === "conservative" ? "conservative" : "exact",
      budget: JSON.parse(String(row.budget_ledger_json)) as ContextSnapshot["budget"],
      decisions: JSON.parse(String(row.decisions_json)) as ContextSnapshot["decisions"],
      selected_order: JSON.parse(String(row.selected_order_json)) as string[],
      logical_input: JSON.parse(String(row.logical_input_json)) as ContextSnapshot["logical_input"],
      digest: String(row.context_digest),
    };
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

function validateSessionBinding(binding: Omit<SessionBinding, "created_at"> | SessionBinding): void {
  const promptNodes = Object.keys(binding.prompt_bindings).sort();
  const contextNodes = Object.keys(binding.context_policy_bindings).sort();
  const invalidReference = (reference: { key: string; version: string; digest?: string }): boolean =>
    !reference.key.trim() || !reference.version.trim()
    || (reference.digest !== undefined && !validDigest(reference.digest));
  if (!binding.session_id.trim() || !binding.agent_key.trim() || !binding.agent_version.trim()
    || !validDigest(binding.agent_digest) || !validDigest(binding.registry_digest)
    || !binding.model_id.trim() || !validDigest(binding.behavior_fingerprint)
    || promptNodes.length === 0 || canonicalJson(promptNodes) !== canonicalJson(contextNodes)
    || Object.values(binding.prompt_bindings).some(invalidReference)
    || Object.values(binding.context_policy_bindings).some(invalidReference)
    || !binding.tokenizer_profile_key.trim() || !binding.tokenizer_profile_version.trim()
    || !validDigest(binding.tokenizer_profile_digest)
    || binding.model_snapshot.model_id !== binding.model_id
    || binding.model_snapshot.behavior_fingerprint !== binding.behavior_fingerprint
    || binding.model_snapshot.tokenizer_profile_key !== binding.tokenizer_profile_key
    || binding.model_snapshot.tokenizer_profile_version !== binding.tokenizer_profile_version) {
    throw new RuntimeError("session_migration_required", 409, "Session execution binding 不完整或不一致");
  }
}

interface SanitizedModelCallInput {
  promptSnapshot: unknown;
  contextSources: unknown;
  modelSnapshot: unknown;
  providerPayload: unknown;
  assetReferences: unknown;
  toolSchema: unknown | null;
}

function sanitizeModelCallInput(input: PrepareModelCall): SanitizedModelCallInput {
  const safe: SanitizedModelCallInput = {
    promptSnapshot: redactForAudit(input.promptSnapshot),
    contextSources: redactForAudit(input.contextSources),
    modelSnapshot: redactForAudit(input.modelSnapshot),
    providerPayload: redactForAudit(input.providerPayload),
    assetReferences: redactForAudit(input.assetReferences),
    toolSchema: input.toolSchema === null ? null : redactForAudit(input.toolSchema),
  };
  [safe.promptSnapshot, safe.contextSources, safe.modelSnapshot, safe.providerPayload, safe.toolSchema]
    .forEach(assertPersistable);
  assertAssetReferences(safe.assetReferences);
  return safe;
}

function validateGovernedModelCall(input: PrepareModelCallWithContext): string {
  let prompt: PromptSnapshot;
  try {
    prompt = readPromptSnapshot(input.promptSnapshot);
  } catch (error) {
    throw new RuntimeError("audit_persistence_failed", 422, `PromptSnapshot v2 非法: ${safeStorageMessage(error)}`);
  }
  const promptBinding = input.binding.prompt_bindings[input.nodeKey];
  if (prompt.schema_version !== "2"
    || !validUuid(prompt.context_snapshot_id ?? "")
    || input.binding.session_id !== input.sessionId
    || input.binding.registry_digest !== prompt.registry_digest
    || input.binding.agent_key !== prompt.agent_key
    || input.binding.agent_version !== prompt.agent_version
    || promptBinding?.key !== prompt.prompt_key
    || promptBinding.version !== prompt.prompt_version
    || input.nodeKey !== prompt.node_key
    || input.nodeKey !== input.contextSnapshot.node_key
    || prompt.context_digest !== input.contextSnapshot.digest
    || canonicalJson(prompt.logical_input) !== canonicalJson(input.contextSnapshot.logical_input)
    || canonicalJson(input.toolSchema) !== canonicalJson(prompt.tool_schema)
    || input.binding.model_id !== input.modelSnapshot.model_id
    || input.binding.behavior_fingerprint !== input.modelSnapshot.behavior_fingerprint
    || canonicalJson(input.binding.model_snapshot) !== canonicalJson(input.modelSnapshot)) {
    throw new RuntimeError(
      "audit_persistence_failed",
      422,
      "Context、Prompt、ModelCall 与 Session binding 不一致",
    );
  }
  return prompt.context_snapshot_id!;
}

function sanitizeContextSnapshot(sessionId: string, snapshot: ContextSnapshot): ContextSnapshot {
  validateContextRecord(sessionId, snapshot);
  const selected = new Set<string>();
  for (const decision of snapshot.decisions) {
    validateContextDecision(decision);
    if (decision.decision === "selected") {
      if (decision.selected_payload === undefined) invalidContextRecord("selected decision 缺少 payload");
      validateContextPayload(decision.selected_payload);
      selected.add(decision.candidate_id);
    } else if (decision.selected_payload !== undefined) {
      invalidContextRecord("排除 decision 禁止保存 payload");
    }
  }
  if (selected.size !== snapshot.selected_order.length
    || snapshot.selected_order.some((id) => !selected.has(id))
    || new Set(snapshot.selected_order).size !== snapshot.selected_order.length) {
    invalidContextRecord("selected_order 与 selected decisions 不一致");
  }
  validateLogicalInput(snapshot.logical_input);
  const safe = redactContextRecord(snapshot) as ContextSnapshot;
  const assets = safe.decisions.flatMap((decision) =>
    decision.selected_payload?.type === "asset" ? [decision.selected_payload.asset] : []);
  assertAssetReferences(assets);
  return safe;
}

function sanitizeContextCompileAttempt(sessionId: string, attempt: ContextCompileAttempt): ContextCompileAttempt {
  validateContextRecord(sessionId, attempt);
  if (!attempt.code.trim() || !["schema", "eligibility", "conflict", "tokenizer", "budget", "finalize"].includes(attempt.stage)) {
    invalidContextRecord("CompileAttempt stage 或 code 非法");
  }
  for (const decision of attempt.decisions) {
    validateContextDecision(decision);
    if (decision.selected_payload !== undefined) invalidContextRecord("CompileAttempt 禁止保存候选 payload");
  }
  return redactContextRecord(attempt) as ContextCompileAttempt;
}

function validateContextRecord(
  sessionId: string,
  record: ContextSnapshot | ContextCompileAttempt,
): void {
  if (record.schema_version !== "2"
    || record.owner !== "pi"
    || record.owner_id !== sessionId
    || !record.node_key.trim()
    || !validTimestamp(record.compiled_at)
    || !validDigest(record.digest)) {
    invalidContextRecord("Context owner/schema/node/time/digest 非法");
  }
  if ("tokenizer_mode" in record) {
    if (!record.policy_key.trim() || !record.policy_version.trim()
      || !record.tokenizer_profile_key.trim() || !record.tokenizer_profile_version.trim()
      || !["exact", "conservative"].includes(record.tokenizer_mode)) {
      invalidContextRecord("Context Policy/Profile 非法");
    }
  }
  validateBudget("budget" in record ? record.budget : null);
}

function validateBudget(budget: ContextSnapshot["budget"] | null): void {
  if (budget === null) return;
  const values = Object.values(budget);
  if (!values.every((value) => Number.isSafeInteger(value) && value >= 0)
    || budget.model_context_window <= 0
    || budget.dynamic_context_budget + budget.system_prompt_tokens + budget.user_template_fixed_tokens
      + budget.tool_schema_tokens + budget.output_schema_tokens + budget.protocol_envelope_tokens
      + budget.max_output_tokens + budget.safety_reserve_tokens !== budget.model_context_window
    || budget.selected_context_tokens > budget.dynamic_context_budget
    || budget.final_input_tokens + budget.max_output_tokens + budget.safety_reserve_tokens > budget.model_context_window) {
    invalidContextRecord("Context BudgetLedger 非法");
  }
}

function validateContextDecision(decision: ContextSnapshot["decisions"][number]): void {
  if (![decision.candidate_id, decision.source_kind, decision.source_id, decision.source_version]
    .every((value) => typeof value === "string" && value.trim().length > 0)
    || !validDigest(decision.content_hash)
    || !Number.isSafeInteger(decision.token_count)
    || decision.token_count < 0
    || !["selected", "expired", "superseded", "duplicate_identity", "duplicate_content", "atomic_group_excluded", "budget_excluded"]
      .includes(decision.decision)) {
    invalidContextRecord("Context decision 非法");
  }
}

function validateContextPayload(payload: NonNullable<ContextSnapshot["decisions"][number]["selected_payload"]>): void {
  if (payload.type === "text") {
    if (typeof payload.text !== "string") invalidContextRecord("Context text payload 非法");
    return;
  }
  if (payload.type === "message") {
    validateLogicalMessage(payload.message);
    return;
  }
  if (payload.type === "asset") {
    assertAssetReferences([payload.asset]);
    return;
  }
  invalidContextRecord("Context payload type 非法");
}

function validateLogicalInput(input: ContextSnapshot["logical_input"]): void {
  if (input === null || typeof input !== "object" || Array.isArray(input)
    || Object.keys(input).some((key) => !["system", "messages", "tool_schema", "output_schema"].includes(key))
    || typeof input.system !== "string"
    || !Array.isArray(input.messages)) {
    invalidContextRecord("Context logical_input 非法");
  }
  input.messages.forEach(validateLogicalMessage);
}

function validateLogicalMessage(message: ContextSnapshot["logical_input"]["messages"][number]): void {
  if (message === null || typeof message !== "object" || Array.isArray(message)
    || Object.keys(message).some((key) => !["role", "content", "thinking", "tool_call_id"].includes(key))
    || typeof message.role !== "string" || !message.role.trim()
    || !("content" in message)
    || (message.thinking !== undefined && typeof message.thinking !== "string")
    || (message.tool_call_id !== undefined && typeof message.tool_call_id !== "string")) {
    invalidContextRecord("Context logical message 非法");
  }
}

function redactContextRecord(value: ContextSnapshot | ContextCompileAttempt): unknown {
  try {
    const safe = redactForAudit(value);
    assertPersistable(safe);
    return safe;
  } catch (error) {
    if (error instanceof RuntimeError) throw error;
    throw new RuntimeError("audit_persistence_failed", 422, "Context 审计输入无法安全脱敏或序列化");
  }
}

function invalidContextRecord(message: string): never {
  throw new RuntimeError("audit_persistence_failed", 422, message);
}

function validTimestamp(value: string): boolean {
  return value.trim().length > 0 && !Number.isNaN(Date.parse(value));
}

function validDigest(value: string): boolean {
  return /^[0-9a-f]{64}$/.test(value);
}

function validUuid(value: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);
}

function contextSummaryFromRow(row: Record<string, unknown>): ContextRecordSummary {
  const snapshot = row.record_type === "snapshot";
  return {
    id: String(row.id),
    record_type: snapshot ? "snapshot" : "compile_attempt",
    owner: { type: "session", id: String(row.session_id) },
    node_key: String(row.node_key),
    status: snapshot ? "succeeded" : "failed",
    compiled_at: String(row.compiled_at),
    policy: snapshot ? { key: String(row.policy_key), version: String(row.policy_version) } : null,
    tokenizer_profile: snapshot
      ? { key: String(row.tokenizer_profile_key), version: String(row.tokenizer_profile_version) }
      : null,
    digest: String(row.digest),
    created_at: String(row.created_at),
  };
}

function modelCallsTableSql(tableName: "novex_model_calls_context_v2"): string {
  return `
    CREATE TABLE ${tableName} (
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
      context_snapshot_id TEXT,
      context_digest TEXT,
      context_policy_key TEXT,
      context_policy_version TEXT,
      tokenizer_profile_key TEXT,
      tokenizer_profile_version TEXT,
      context_budget_summary_json TEXT,
      prepared_at TEXT NOT NULL,
      completed_at TEXT,
      FOREIGN KEY(session_id) REFERENCES novex_session_bindings(session_id) ON DELETE CASCADE,
      FOREIGN KEY(root_call_id) REFERENCES ${tableName}(id) ON DELETE RESTRICT,
      FOREIGN KEY(parent_call_id) REFERENCES ${tableName}(id) ON DELETE RESTRICT,
      FOREIGN KEY(context_snapshot_id) REFERENCES novex_context_snapshots(id) ON DELETE RESTRICT,
      UNIQUE(root_call_id, attempt)
    );
  `;
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
