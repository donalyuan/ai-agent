import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";

import { describe, expect, it } from "vitest";

import type { ContextCompileAttempt, ContextSnapshot } from "../src/context.js";
import type { PromptSnapshot } from "../src/definitions.js";
import { NovexSqliteStore, type PrepareModelCallWithContext } from "../src/persistence.js";

const MODEL_ID = "11111111-1111-4111-8111-111111111111";

function createBinding(store: NovexSqliteStore, sessionId: string): void {
  store.createBinding({
    session_id: sessionId,
    agent_key: "personal.general",
    agent_version: "2.0.0",
    agent_digest: "a".repeat(64),
    prompt_bindings: { "personal.turn": { key: "personal.turn", version: "2.0.0" } },
    context_policy_bindings: {
      "personal.turn": { key: "personal.turn.context", version: "1.0.0", digest: "d".repeat(64) },
    },
    tokenizer_profile_key: "openai.o200k",
    tokenizer_profile_version: "1.0.0",
    tokenizer_profile_digest: "e".repeat(64),
    registry_digest: "b".repeat(64),
    tool_profile: "chat",
    model_id: MODEL_ID,
    behavior_fingerprint: "c".repeat(64),
    model_snapshot: {
      model_id: MODEL_ID,
      provider: "fixture",
      protocol: "openai_responses",
      request_base_url: "https://example.invalid/v1",
      upstream_model: "fixture-model",
      reasoning_effort: null,
      max_output_tokens: 4096,
      timeout_seconds: 10,
      context_window: 128000,
      tokenizer_profile_key: "openai.o200k",
      tokenizer_profile_version: "1.0.0",
      behavior_settings: {},
      behavior_fingerprint: "c".repeat(64),
    },
    binding_status: "executable",
    migration_source: "context-test",
    parent_session_id: null,
  });
}

function selectedDecision(): string {
  return JSON.stringify([{
    candidate_id: "instruction",
    source_kind: "session_message",
    source_id: "entry-1",
    source_version: "1",
    content_hash: "d".repeat(64),
    token_count: 4,
    decision: "selected",
    selected_payload: { type: "text", text: "safe selected content" },
  }]);
}

function excludedDecision(payload = false): string {
  return JSON.stringify([{
    candidate_id: "reference",
    source_kind: "session_message",
    source_id: "entry-2",
    source_version: "1",
    content_hash: "e".repeat(64),
    token_count: 4096,
    decision: "budget_excluded",
    ...(payload ? { selected_payload: { type: "text", text: "must not persist" } } : {}),
  }]);
}

function insertSnapshot(database: DatabaseSync, sessionId: string, id: string): void {
  database.prepare(`
    INSERT INTO novex_context_snapshots (
      id, session_id, phase, node_key, status, compiled_at, policy_key, policy_version,
      tokenizer_profile_key, tokenizer_profile_version, tokenizer_mode, model_context_window,
      budget_ledger_json, decisions_json, selected_order_json, logical_input_json,
      context_digest, created_at
    ) VALUES (?, ?, 'turn', 'personal.turn', 'succeeded', ?, 'personal.turn.context',
              '1.0.0', 'openai.o200k', '1.0.0', 'exact', 128000,
              ?, ?, ?, ?, ?, ?)
  `).run(
    id,
    sessionId,
    "2026-07-25T00:00:00.000Z",
    JSON.stringify({ model_context_window: 128000, dynamic_context_budget: 120000 }),
    selectedDecision(),
    JSON.stringify(["instruction"]),
    JSON.stringify({
      system: "system",
      messages: [{ role: "user", content: "safe selected content" }],
      tool_schema: null,
      output_schema: null,
    }),
    "f".repeat(64),
    "2026-07-25T00:00:00.000Z",
  );
}

function contextSnapshot(sessionId: string, text = "safe selected content"): ContextSnapshot {
  return {
    schema_version: "2",
    owner: "pi",
    owner_id: sessionId,
    node_key: "personal.turn",
    compiled_at: "2026-07-25T00:00:00.000Z",
    policy_key: "personal.turn.context",
    policy_version: "1.0.0",
    tokenizer_profile_key: "openai.o200k",
    tokenizer_profile_version: "1.0.0",
    tokenizer_mode: "exact",
    budget: {
      model_context_window: 128000,
      system_prompt_tokens: 1,
      user_template_fixed_tokens: 0,
      tool_schema_tokens: 0,
      output_schema_tokens: 0,
      protocol_envelope_tokens: 1,
      max_output_tokens: 4096,
      safety_reserve_tokens: 16,
      dynamic_context_budget: 123886,
      selected_context_tokens: 4,
      final_input_tokens: 6,
    },
    decisions: [{
      candidate_id: "instruction",
      source_kind: "session_message",
      source_id: "entry-1",
      source_version: "1",
      trust: "reference",
      priority: "p2",
      required: false,
      render_order: 0,
      content_hash: "d".repeat(64),
      token_count: 4,
      decision: "selected",
      selected_payload: { type: "text", text },
    }],
    selected_order: ["instruction"],
    logical_input: {
      system: "system",
      messages: [{ role: "user", content: text }],
      tool_schema: null,
      output_schema: null,
    },
    digest: "f".repeat(64),
  };
}

function promptSnapshot(snapshotId: string, context: ContextSnapshot): PromptSnapshot {
  return {
    schema_version: "2",
    registry_digest: "b".repeat(64),
    agent_key: "personal.general",
    agent_version: "2.0.0",
    prompt_key: "personal.turn",
    prompt_version: "2.0.0",
    node_key: context.node_key,
    system: context.logical_input.system,
    user: String(context.logical_input.messages[0]!.content),
    variables: {},
    fragments: [],
    tool_profile: "chat",
    output_schema: context.logical_input.output_schema,
    tool_schema: context.logical_input.tool_schema,
    max_output_tokens: 4096,
    context_snapshot_id: snapshotId,
    context_digest: context.digest,
    logical_input: structuredClone(context.logical_input),
  };
}

function governedModelCall(
  store: NovexSqliteStore,
  sessionId: string,
  snapshotId = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
  context = contextSnapshot(sessionId),
): PrepareModelCallWithContext {
  const binding = store.binding(sessionId);
  const prompt = promptSnapshot(snapshotId, context);
  return {
    sessionId,
    phase: "turn",
    nodeKey: "personal.turn",
    attempt: 1,
    binding,
    promptSnapshot: prompt,
    modelSnapshot: binding.model_snapshot,
    contextSources: [],
    toolSchema: null,
    assetReferences: [],
    providerPayload: { model: "fixture-model", input: prompt.logical_input },
    contextSnapshot: context,
  };
}

function compileAttempt(sessionId: string): ContextCompileAttempt {
  return {
    schema_version: "2",
    owner: "pi",
    owner_id: sessionId,
    node_key: "personal.turn",
    compiled_at: "2026-07-25T00:00:00.000Z",
    stage: "budget",
    code: "context_budget_exceeded",
    budget: contextSnapshot(sessionId).budget,
    decisions: [{
      candidate_id: "reference",
      source_kind: "session_message",
      source_id: "entry-2",
      source_version: "1",
      trust: "reference",
      priority: "p2",
      required: false,
      render_order: 1,
      content_hash: "e".repeat(64),
      token_count: 4096,
      decision: "budget_excluded",
    }],
    digest: "9".repeat(64),
  };
}

describe("Pi namespaced Context persistence schema", () => {
  it("creates Context binding, immutable audit tables and ModelCall Context FK summary", async () => {
    const root = await mkdtemp(join(tmpdir(), "novex-context-schema-"));
    const path = join(root, "sessions.sqlite");
    const store = new NovexSqliteStore(path);
    const database = new DatabaseSync(path);

    const objects = database.prepare(`
      SELECT type, name FROM sqlite_master
      WHERE name IN (
        'novex_context_snapshots', 'novex_context_snapshots_immutable',
        'novex_context_compile_attempts', 'novex_context_compile_attempts_immutable',
        'novex_context_snapshots_session', 'novex_context_compile_attempts_session',
        'novex_model_calls_context_snapshot'
      )
    `).all() as Array<{ type: string; name: string }>;
    expect(new Set(objects.map(({ type, name }) => `${type}:${name}`))).toEqual(new Set([
      "table:novex_context_snapshots",
      "trigger:novex_context_snapshots_immutable",
      "table:novex_context_compile_attempts",
      "trigger:novex_context_compile_attempts_immutable",
      "index:novex_context_snapshots_session",
      "index:novex_context_compile_attempts_session",
      "index:novex_model_calls_context_snapshot",
    ]));

    const columns = (table: string): Set<string> => new Set(
      (database.prepare(`PRAGMA table_info(${table})`).all() as Array<{ name: string }>).map(({ name }) => name),
    );
    const bindingColumns = columns("novex_session_bindings");
    for (const column of [
      "context_policy_bindings_json",
      "tokenizer_profile_key",
      "tokenizer_profile_version",
      "tokenizer_profile_digest",
    ]) expect(bindingColumns.has(column)).toBe(true);
    const modelCallColumns = columns("novex_model_calls");
    for (const column of [
      "context_snapshot_id",
      "context_digest",
      "context_policy_key",
      "context_policy_version",
      "tokenizer_profile_key",
      "tokenizer_profile_version",
      "context_budget_summary_json",
    ]) expect(modelCallColumns.has(column)).toBe(true);
    const foreignKeys = database.prepare("PRAGMA foreign_key_list(novex_model_calls)").all() as unknown as Array<{
      table: string;
      from: string;
    }>;
    expect(foreignKeys).toContainEqual(expect.objectContaining({
      table: "novex_context_snapshots",
      from: "context_snapshot_id",
    }));

    database.close();
    store.close();
  });

  it("enforces owner, digest, payload minimization and direct mutation rejection", async () => {
    const root = await mkdtemp(join(tmpdir(), "novex-context-guards-"));
    const path = join(root, "sessions.sqlite");
    const store = new NovexSqliteStore(path);
    const sessionId = "session-context-owner";
    createBinding(store, sessionId);
    const database = new DatabaseSync(path);
    database.exec("PRAGMA foreign_keys = ON");

    const snapshotId = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    insertSnapshot(database, sessionId, snapshotId);
    expect(() => database.prepare(
      "UPDATE novex_context_snapshots SET node_key='other.node' WHERE id=?",
    ).run(snapshotId)).toThrow(/immutable/);
    expect(() => database.prepare(
      "DELETE FROM novex_context_snapshots WHERE id=?",
    ).run(snapshotId)).toThrow(/immutable/);
    expect(() => insertSnapshot(database, "missing-session", "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"))
      .toThrow(/FOREIGN KEY/);

    const insertAttempt = (id: string, decisions: string, digest = "9".repeat(64)): void => {
      database.prepare(`
        INSERT INTO novex_context_compile_attempts (
          id, session_id, phase, node_key, status, compiled_at, stage, code,
          budget_ledger_json, decisions_json, attempt_digest, created_at
        ) VALUES (?, ?, 'turn', 'personal.turn', 'failed', ?, 'budget',
                  'context_budget_exceeded', NULL, ?, ?, ?)
      `).run(
        id,
        sessionId,
        "2026-07-25T00:00:00.000Z",
        decisions,
        digest,
        "2026-07-25T00:00:00.000Z",
      );
    };
    insertAttempt("cccccccc-cccc-4ccc-8ccc-cccccccccccc", excludedDecision());
    expect(() => insertAttempt(
      "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
      excludedDecision(true),
    )).toThrow(/payload/);
    expect(() => insertAttempt(
      "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee",
      excludedDecision(),
      "short",
    )).toThrow(/CHECK/);
    expect(() => database.prepare(
      "UPDATE novex_context_compile_attempts SET code='other' WHERE id=?",
    ).run("cccccccc-cccc-4ccc-8ccc-cccccccccccc")).toThrow(/immutable/);
    expect(database.prepare("SELECT COUNT(*) AS total FROM novex_model_calls").get())
      .toEqual({ total: 0 });

    database.close();
    store.close();
  });
});

describe("Pi Context repository", () => {
  it("persists ContextSnapshot and governed prepared ModelCall atomically", async () => {
    const root = await mkdtemp(join(tmpdir(), "novex-context-transaction-"));
    const path = join(root, "sessions.sqlite");
    const store = new NovexSqliteStore(path);
    const sessionId = "session-context-transaction";
    createBinding(store, sessionId);

    const callId = store.prepareModelCallWithContext(governedModelCall(store, sessionId));
    const database = new DatabaseSync(path);
    const call = database.prepare(`
      SELECT schema_version, context_snapshot_id, context_digest, context_policy_key,
             context_policy_version, tokenizer_profile_key, tokenizer_profile_version,
             context_budget_summary_json
      FROM novex_model_calls WHERE id = ?
    `).get(callId) as Record<string, unknown>;
    expect(call).toMatchObject({
      schema_version: "2",
      context_snapshot_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
      context_digest: "f".repeat(64),
      context_policy_key: "personal.turn.context",
      context_policy_version: "1.0.0",
      tokenizer_profile_key: "openai.o200k",
      tokenizer_profile_version: "1.0.0",
    });
    expect(JSON.parse(String(call.context_budget_summary_json))).toEqual(contextSnapshot(sessionId).budget);
    expect(database.prepare("SELECT COUNT(*) AS total FROM novex_context_snapshots").get()).toEqual({ total: 1 });

    database.close();
    store.close();
  });

  it("rejects Prompt/Context mismatches before writing either record", async () => {
    const root = await mkdtemp(join(tmpdir(), "novex-context-mismatch-"));
    const path = join(root, "sessions.sqlite");
    const store = new NovexSqliteStore(path);
    const sessionId = "session-context-mismatch";
    createBinding(store, sessionId);
    const input = governedModelCall(store, sessionId);
    input.promptSnapshot = { ...input.promptSnapshot, context_digest: "0".repeat(64) };

    expect(() => store.prepareModelCallWithContext(input)).toThrow(/Context.*Prompt|不一致/);
    const database = new DatabaseSync(path);
    expect(database.prepare("SELECT COUNT(*) AS total FROM novex_context_snapshots").get()).toEqual({ total: 0 });
    expect(database.prepare("SELECT COUNT(*) AS total FROM novex_model_calls").get()).toEqual({ total: 0 });

    database.close();
    store.close();
  });

  it("rolls back ContextSnapshot when prepared ModelCall persistence fails", async () => {
    const root = await mkdtemp(join(tmpdir(), "novex-context-rollback-"));
    const path = join(root, "sessions.sqlite");
    const store = new NovexSqliteStore(path);
    const sessionId = "session-context-rollback";
    createBinding(store, sessionId);
    const database = new DatabaseSync(path);
    database.exec(`
      CREATE TRIGGER fail_governed_model_call
      BEFORE INSERT ON novex_model_calls
      WHEN NEW.context_snapshot_id IS NOT NULL
      BEGIN SELECT RAISE(ABORT, 'forced ModelCall failure'); END;
    `);

    expect(() => store.prepareModelCallWithContext(governedModelCall(store, sessionId)))
      .toThrow(/持久化失败|forced ModelCall failure/);
    expect(database.prepare("SELECT COUNT(*) AS total FROM novex_context_snapshots").get()).toEqual({ total: 0 });
    expect(database.prepare("SELECT COUNT(*) AS total FROM novex_model_calls").get()).toEqual({ total: 0 });

    database.close();
    store.close();
  });

  it("persists failed ContextCompileAttempt without creating a ModelCall", async () => {
    const root = await mkdtemp(join(tmpdir(), "novex-context-attempt-"));
    const path = join(root, "sessions.sqlite");
    const store = new NovexSqliteStore(path);
    const sessionId = "session-context-attempt";
    createBinding(store, sessionId);

    const id = store.persistContextCompileAttempt({ sessionId, phase: "turn", attempt: compileAttempt(sessionId) });
    const database = new DatabaseSync(path);
    expect(database.prepare("SELECT id, code FROM novex_context_compile_attempts WHERE id = ?").get(id))
      .toEqual({ id, code: "context_budget_exceeded" });
    expect(store.queryContextRecords({ sessionId, recordType: "compile_attempt" }, 20, 0))
      .toMatchObject({ total: 1, items: [expect.objectContaining({ id, record_type: "compile_attempt", status: "failed" })] });
    expect(store.contextRecord(id)).toMatchObject({
      id,
      record_type: "compile_attempt",
      code: "context_budget_exceeded",
      decisions: [expect.not.objectContaining({ selected_payload: expect.anything() })],
    });
    expect(database.prepare("SELECT COUNT(*) AS total FROM novex_model_calls").get()).toEqual({ total: 0 });

    database.close();
    store.close();
  });

  it("redacts canaries and rejects base64, signed URLs and forbidden decision payloads", async () => {
    const root = await mkdtemp(join(tmpdir(), "novex-context-safety-"));
    const path = join(root, "sessions.sqlite");
    const store = new NovexSqliteStore(path);
    const sessionId = "session-context-safety";
    createBinding(store, sessionId);

    const canary = contextSnapshot(sessionId, "NOVEX_CANARY_SECRET_DO_NOT_PERSIST_context");
    store.persistContextSnapshot({
      id: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
      sessionId,
      phase: "turn",
      snapshot: canary,
    });
    const database = new DatabaseSync(path);
    const persisted = database.prepare(
      "SELECT decisions_json, logical_input_json FROM novex_context_snapshots WHERE id = ?",
    ).get("bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb") as Record<string, unknown>;
    expect(JSON.stringify(persisted)).toContain("[REDACTED]");
    expect(JSON.stringify(persisted)).not.toContain("NOVEX_CANARY_SECRET_DO_NOT_PERSIST");

    const unsafeValues = [
      `data:image/png;base64,${"A".repeat(128)}`,
      "https://example.invalid/file?X-Amz-Signature=secret",
    ];
    for (const [index, value] of unsafeValues.entries()) {
      const snapshot = contextSnapshot(sessionId, value);
      expect(() => store.persistContextSnapshot({
        id: `cccccccc-cccc-4ccc-8ccc-ccccccccccc${index}`,
        sessionId,
        phase: "turn",
        snapshot,
      })).toThrow(/base64|签名 URL|安全脱敏/);
    }
    const invalidAttempt = compileAttempt(sessionId);
    invalidAttempt.decisions[0]!.selected_payload = { type: "text", text: "forbidden" };
    expect(() => store.persistContextCompileAttempt({ sessionId, phase: "turn", attempt: invalidAttempt }))
      .toThrow(/payload|安全脱敏/);
    expect(database.prepare("SELECT COUNT(*) AS total FROM novex_context_snapshots").get()).toEqual({ total: 1 });
    expect(database.prepare("SELECT COUNT(*) AS total FROM novex_context_compile_attempts").get()).toEqual({ total: 0 });

    database.close();
    store.close();
  });

  it("rebuilds an upgraded ModelCall table with a real Context FK idempotently", async () => {
    const root = await mkdtemp(join(tmpdir(), "novex-context-upgrade-"));
    const path = join(root, "sessions.sqlite");
    let store = new NovexSqliteStore(path);
    const sessionId = "session-context-upgrade";
    createBinding(store, sessionId);
    const callId = store.prepareModelCallWithContext(governedModelCall(store, sessionId));
    store.close();

    const legacy = new DatabaseSync(path);
    legacy.exec("PRAGMA foreign_keys = OFF");
    legacy.exec(`
      CREATE TABLE novex_model_calls_without_context_fk AS SELECT * FROM novex_model_calls;
      DROP TABLE novex_model_calls;
      ALTER TABLE novex_model_calls_without_context_fk RENAME TO novex_model_calls;
    `);
    expect(legacy.prepare("PRAGMA foreign_key_list(novex_model_calls)").all()).toEqual([]);
    legacy.close();

    store = new NovexSqliteStore(path);
    expect(store.modelCall(callId)).toMatchObject({
      id: callId,
      context_snapshot_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
    });
    let upgraded = new DatabaseSync(path);
    expect(upgraded.prepare("PRAGMA foreign_key_list(novex_model_calls)").all())
      .toContainEqual(expect.objectContaining({
        table: "novex_context_snapshots",
        from: "context_snapshot_id",
      }));
    expect(upgraded.prepare("PRAGMA foreign_key_check").all()).toEqual([]);
    upgraded.close();
    store.close();

    store = new NovexSqliteStore(path);
    expect(store.modelCall(callId)).toMatchObject({ id: callId, schema_version: "2" });
    upgraded = new DatabaseSync(path);
    expect(upgraded.prepare("SELECT COUNT(*) AS total FROM novex_model_calls").get()).toEqual({ total: 1 });
    expect(upgraded.prepare("SELECT COUNT(*) AS total FROM novex_context_snapshots").get()).toEqual({ total: 1 });
    expect(upgraded.prepare("PRAGMA foreign_key_check").all()).toEqual([]);
    upgraded.close();
    store.close();
  });

  it("keeps an owner-scoped orphan Snapshot across restart and removes it only by deletion intent", async () => {
    const root = await mkdtemp(join(tmpdir(), "novex-context-orphan-"));
    const path = join(root, "sessions.sqlite");
    let store = new NovexSqliteStore(path);
    const sessionId = "session-context-orphan";
    createBinding(store, sessionId);
    const snapshotId = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
    store.persistContextSnapshot({
      id: snapshotId,
      sessionId,
      phase: "turn",
      snapshot: contextSnapshot(sessionId),
    });
    store.close();

    store = new NovexSqliteStore(path);
    let database = new DatabaseSync(path);
    expect(database.prepare("SELECT id FROM novex_context_snapshots WHERE id = ?").get(snapshotId))
      .toEqual({ id: snapshotId });
    expect(database.prepare("SELECT COUNT(*) AS total FROM novex_model_calls WHERE context_snapshot_id = ?").get(snapshotId))
      .toEqual({ total: 0 });
    database.close();

    store.beginSessionDeletion(sessionId);
    store.completeSessionDeletion(sessionId);
    database = new DatabaseSync(path);
    expect(database.prepare("SELECT COUNT(*) AS total FROM novex_context_snapshots WHERE id = ?").get(snapshotId))
      .toEqual({ total: 0 });
    expect(database.prepare("SELECT COUNT(*) AS total FROM novex_session_bindings WHERE session_id = ?").get(sessionId))
      .toEqual({ total: 0 });
    database.close();
    store.close();
  });
});
