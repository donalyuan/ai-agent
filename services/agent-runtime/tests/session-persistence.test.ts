import { randomUUID } from "node:crypto";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { DatabaseSync } from "node:sqlite";

import type { AssistantMessage, UserMessage } from "@earendil-works/pi-ai";
import { NodeExecutionEnv } from "@earendil-works/pi-agent-core/node";
import { createNodeSqliteFactory, SqliteSessionRepo } from "@earendil-works/pi-storage-sqlite-node";
import { describe, expect, it } from "vitest";

import { cleanupSession, SessionStore } from "../src/sessions.js";
import type { ContextCompileAttempt, ContextSnapshot } from "../src/context.js";
import {
  activeAgent,
  canonicalJson,
  compilePrompt,
  definitionDigest,
  loadDefinitionRegistry,
  sha256Hex,
  type AgentDefinition,
  type DefinitionRegistry,
} from "../src/definitions.js";

const MODEL_ID = "11111111-1111-4111-8111-111111111111";

function user(text: string): UserMessage {
  return { role: "user", content: [{ type: "text", text }], timestamp: Date.now() };
}

function assistant(text: string): AssistantMessage {
  return {
    role: "assistant",
    content: [{ type: "text", text }],
    api: "faux",
    provider: "faux",
    model: "faux-1",
    stopReason: "stop",
    timestamp: Date.now(),
    usage: {
      input: 10,
      output: 5,
      cacheRead: 0,
      cacheWrite: 0,
      totalTokens: 15,
      cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
    },
  };
}

function governedEvidence(definitions: DefinitionRegistry, agent: AgentDefinition) {
  const profile = definitions.tokenizer_profiles.find((item) =>
    item.profile_key === "openai.o200k" && item.version === "1.0.0")!;
  return {
    prompt_bindings: Object.fromEntries(Object.entries(agent.nodes).map(([node, reference]) => [
      node,
      { key: reference.key, version: reference.version },
    ])),
    context_policy_bindings: Object.fromEntries(Object.entries(agent.nodes).map(([node, reference]) => {
      const policyReference = reference.context_policy!;
      const policy = definitions.context_policies.find((item) =>
        item.policy_key === policyReference.key && item.version === policyReference.version)!;
      return [node, { key: policy.policy_key, version: policy.version, digest: definitionDigest(policy) }];
    })),
    tokenizer_profile_key: profile.profile_key,
    tokenizer_profile_version: profile.version,
    tokenizer_profile_digest: definitionDigest(profile),
  };
}

function contextSnapshot(sessionId: string, binding: ReturnType<SessionStore["novex"]["binding"]>, text: string): ContextSnapshot {
  const policy = binding.context_policy_bindings["personal.turn"]!;
  return {
    schema_version: "2",
    owner: "pi",
    owner_id: sessionId,
    node_key: "personal.turn",
    compiled_at: "2026-07-25T00:00:00.000Z",
    policy_key: policy.key,
    policy_version: policy.version,
    tokenizer_profile_key: binding.tokenizer_profile_key,
    tokenizer_profile_version: binding.tokenizer_profile_version,
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
      candidate_id: `instruction-${text}`,
      source_kind: "session_message",
      source_id: `entry-${text}`,
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
    selected_order: [`instruction-${text}`],
    logical_input: {
      system: "system",
      messages: [{ role: "user", content: text }],
      tool_schema: null,
      output_schema: null,
    },
    digest: "f".repeat(64),
  };
}

function prepareGovernedCall(store: SessionStore, sessionId: string, text: string): string {
  const binding = store.novex.binding(sessionId);
  const snapshot = contextSnapshot(sessionId, binding, text);
  const snapshotId = randomUUID();
  const prompt = binding.prompt_bindings["personal.turn"]!;
  return store.novex.prepareModelCallWithContext({
    sessionId,
    phase: "turn",
    nodeKey: "personal.turn",
    attempt: 1,
    binding,
    promptSnapshot: {
      schema_version: "2",
      registry_digest: binding.registry_digest,
      agent_key: binding.agent_key,
      agent_version: binding.agent_version,
      prompt_key: prompt.key,
      prompt_version: prompt.version,
      node_key: "personal.turn",
      system: snapshot.logical_input.system,
      user: text,
      variables: {},
      fragments: [],
      tool_profile: binding.tool_profile,
      output_schema: null,
      tool_schema: null,
      max_output_tokens: 4096,
      context_snapshot_id: snapshotId,
      context_digest: snapshot.digest,
      logical_input: structuredClone(snapshot.logical_input),
    },
    modelSnapshot: binding.model_snapshot,
    contextSources: [],
    toolSchema: null,
    assetReferences: [],
    providerPayload: { input: snapshot.logical_input },
    contextSnapshot: snapshot,
  });
}

function persistCompileAttempt(store: SessionStore, sessionId: string, text: string): string {
  const snapshot = contextSnapshot(sessionId, store.novex.binding(sessionId), text);
  const attempt: ContextCompileAttempt = {
    schema_version: "2",
    owner: "pi",
    owner_id: sessionId,
    node_key: snapshot.node_key,
    compiled_at: snapshot.compiled_at,
    stage: "budget",
    code: "context_budget_exceeded",
    budget: snapshot.budget,
    decisions: [],
    digest: "9".repeat(64),
  };
  return store.novex.persistContextCompileAttempt({ sessionId, phase: "turn", attempt });
}

function contextAuditCounts(databasePath: string, sessionId: string): Record<string, number> {
  const database = new DatabaseSync(databasePath);
  try {
    const count = (table: string): number => Number((database.prepare(
      `SELECT COUNT(*) AS total FROM ${table} WHERE session_id = ?`,
    ).get(sessionId) as { total: number }).total);
    return {
      snapshots: count("novex_context_snapshots"),
      attempts: count("novex_context_compile_attempts"),
      model_calls: count("novex_model_calls"),
      bindings: count("novex_session_bindings"),
    };
  } finally {
    database.close();
  }
}

describe("Pi SQLite session persistence", () => {
  it("deletes only the explicit owner and reconciles an interrupted public Session deletion", async () => {
    const root = await mkdtemp(join(tmpdir(), "novex-session-delete-"));
    const database = join(root, "sessions.sqlite");
    let store = new SessionStore(database, root);
    const definitions = await loadDefinitionRegistry(resolve(import.meta.dirname, "../../../agent-definitions"));
    const agent = activeAgent(definitions, "personal.general");
    const modelSnapshot = {
      model_id: MODEL_ID,
      provider: "faux",
      protocol: "openai_responses" as const,
      request_base_url: "http://localhost:0",
      upstream_model: "faux-1",
      reasoning_effort: null,
      max_output_tokens: 4096,
      timeout_seconds: 10,
      context_window: 128000,
      tokenizer_profile_key: "openai.o200k",
      tokenizer_profile_version: "1.0.0",
      behavior_settings: {},
      behavior_fingerprint: "a".repeat(64),
    };
    const create = async () => {
      const session = await store.create({
        agentKey: agent.agent_key,
        modelId: MODEL_ID,
        toolProfile: "chat",
        source: "delete-test",
        binding: {
          agent_key: agent.agent_key,
          agent_version: agent.version,
          agent_digest: definitionDigest(agent),
          ...governedEvidence(definitions, agent),
          registry_digest: definitions.digest,
          tool_profile: "chat" as const,
          model_id: MODEL_ID,
          behavior_fingerprint: modelSnapshot.behavior_fingerprint,
          model_snapshot: modelSnapshot,
          binding_status: "executable" as const,
          migration_source: "test",
        },
      });
      const id = (await session.getMetadata()).id;
      return { session, id };
    };
    const promptSnapshot = compilePrompt(definitions, agent.agent_key, agent.version, "personal.turn", {
      schema_version: "1",
      fragments: [{ id: "delete-1", trust: "user_instruction", source: "test", content: "delete" }],
    }, "chat");
    const prepare = (sessionId: string) => store.novex.prepareModelCall({
      sessionId,
      phase: "turn",
      nodeKey: "personal.turn",
      attempt: 1,
      binding: store.novex.binding(sessionId),
      promptSnapshot,
      modelSnapshot,
      contextSources: [],
      toolSchema: null,
      assetReferences: [],
      providerPayload: { messages: [] },
    });

    const source = await create();
    const sourceEntry = await source.session.appendMessage(user("fork source"));
    const sourceCall = prepare(source.id);
    store.novex.finishModelCall(sourceCall, "succeeded", { text: "source" }, {}, undefined);
    const sourceGovernedCall = prepareGovernedCall(store, source.id, "source-context");
    store.novex.finishModelCall(sourceGovernedCall, "succeeded", { text: "source governed" }, {}, undefined);
    persistCompileAttempt(store, source.id, "source-attempt");
    const fork = await store.fork(source.id, sourceEntry, "at");
    const forkId = (await fork.getMetadata()).id;
    const forkCall = prepare(forkId);
    store.novex.finishModelCall(forkCall, "succeeded", { text: "fork" }, {}, undefined);
    const forkGovernedCall = prepareGovernedCall(store, forkId, "fork-context");
    store.novex.finishModelCall(forkGovernedCall, "succeeded", { text: "fork governed" }, {}, undefined);
    persistCompileAttempt(store, forkId, "fork-attempt");
    await cleanupSession(source.session);
    await cleanupSession(fork);

    expect(contextAuditCounts(database, source.id)).toEqual({ snapshots: 1, attempts: 1, model_calls: 2, bindings: 1 });
    expect(contextAuditCounts(database, forkId)).toEqual({ snapshots: 1, attempts: 1, model_calls: 2, bindings: 1 });
    await store.delete(source.id);
    expect(contextAuditCounts(database, source.id)).toEqual({ snapshots: 0, attempts: 0, model_calls: 0, bindings: 0 });
    expect(contextAuditCounts(database, forkId)).toEqual({ snapshots: 1, attempts: 1, model_calls: 2, bindings: 1 });
    expect(store.novex.listModelCalls(forkId).map(({ id }) => id)).toEqual(expect.arrayContaining([forkCall, forkGovernedCall]));
    await expect(store.view(forkId)).resolves.toMatchObject({ session_id: forkId, parent_session_id: source.id });

    const interrupted = await create();
    const interruptedCall = prepareGovernedCall(store, interrupted.id, "interrupted-context");
    persistCompileAttempt(store, interrupted.id, "interrupted-attempt");
    await cleanupSession(interrupted.session);
    store.novex.beginSessionDeletion(interrupted.id);
    const publicRepo = new SqliteSessionRepo({
      env: new NodeExecutionEnv({ cwd: root }),
      sqlite: createNodeSqliteFactory(),
      databasePath: database,
    });
    const metadata = (await publicRepo.list()).find(({ id }) => id === interrupted.id)!;
    await publicRepo.delete(metadata);
    await store.close();

    store = new SessionStore(database, root);
    expect(store.novex.pendingSessionDeletions()).toEqual([interrupted.id]);
    await store.reconcileSessionDeletions();
    expect(store.novex.pendingSessionDeletions()).toEqual([]);
    expect(contextAuditCounts(database, interrupted.id)).toEqual({ snapshots: 0, attempts: 0, model_calls: 0, bindings: 0 });
    expect(() => store.novex.binding(interrupted.id)).toThrow(/缺少版本化/);
    expect(() => store.novex.modelCall(interruptedCall)).toThrow(/不存在/);
    expect(store.novex.listModelCalls(forkId).map(({ id }) => id)).toEqual(expect.arrayContaining([forkCall, forkGovernedCall]));

    const interruptedBeforePublicDelete = await create();
    const interruptedBeforePublicDeleteCall = prepare(interruptedBeforePublicDelete.id);
    await cleanupSession(interruptedBeforePublicDelete.session);
    store.novex.beginSessionDeletion(interruptedBeforePublicDelete.id);
    await store.reconcileSessionDeletions();
    expect(store.novex.pendingSessionDeletions()).toEqual([]);
    expect(store.novex.listModelCalls(interruptedBeforePublicDelete.id)).toEqual([]);
    expect(() => store.novex.modelCall(interruptedBeforePublicDeleteCall)).toThrow(/不存在/);
    await expect(store.view(interruptedBeforePublicDelete.id)).rejects.toMatchObject({ code: "session_not_found" });

    const interruptedRetry = await create();
    const interruptedRetryCall = prepare(interruptedRetry.id);
    await cleanupSession(interruptedRetry.session);
    store.novex.beginSessionDeletion(interruptedRetry.id);
    const retryMetadata = (await publicRepo.list()).find(({ id }) => id === interruptedRetry.id)!;
    await publicRepo.delete(retryMetadata);
    await store.delete(interruptedRetry.id);
    expect(store.novex.pendingSessionDeletions()).toEqual([]);
    expect(() => store.novex.modelCall(interruptedRetryCall)).toThrow(/不存在/);
    await store.close();
  });

  it("restores entries, cursor, active leaf, fork and compaction after restart", async () => {
    const root = await mkdtemp(join(tmpdir(), "novex-session-"));
    const database = join(root, "sessions.sqlite");
    let store = new SessionStore(database, root);
    const definitions = await loadDefinitionRegistry(resolve(import.meta.dirname, "../../../agent-definitions"));
    const agent = activeAgent(definitions, "personal.general");
    const modelSnapshot = {
      model_id: MODEL_ID,
      provider: "faux",
      protocol: "openai_responses" as const,
      request_base_url: "http://localhost:0",
      upstream_model: "faux-1",
      reasoning_effort: null,
      max_output_tokens: 4096,
      timeout_seconds: 10,
      context_window: 128000,
      tokenizer_profile_key: "openai.o200k",
      tokenizer_profile_version: "1.0.0",
      behavior_settings: {},
      behavior_fingerprint: "a".repeat(64),
    };
    const session = await store.create({
      agentKey: agent.agent_key,
      modelId: MODEL_ID,
      toolProfile: "chat",
      source: "test",
      binding: {
        agent_key: agent.agent_key,
        agent_version: agent.version,
        agent_digest: definitionDigest(agent),
        ...governedEvidence(definitions, agent),
        registry_digest: definitions.digest,
        tool_profile: "chat",
        model_id: MODEL_ID,
        behavior_fingerprint: modelSnapshot.behavior_fingerprint,
        model_snapshot: modelSnapshot,
        binding_status: "executable",
        migration_source: "test",
      },
    });
    const sessionId = (await session.getMetadata()).id;
    const direct = new DatabaseSync(database);
    expect(() => direct.prepare("UPDATE novex_session_bindings SET model_id = ? WHERE session_id = ?").run(
      "22222222-2222-4222-8222-222222222222",
      sessionId,
    )).toThrow(/session binding is immutable/);
    direct.close();
    const binding = store.novex.binding(sessionId);
    const promptSnapshot = compilePrompt(
      definitions,
      agent.agent_key,
      agent.version,
      "personal.turn",
      {
        schema_version: "1",
        fragments: [{ id: "input-1", trust: "user_instruction", source: "test", content: "hello" }],
      },
      "chat",
    );
    const prepare = (attempt: number, rootCallId?: string) => store.novex.prepareModelCall({
      sessionId,
      phase: "turn",
      nodeKey: "personal.turn",
      attempt,
      ...(rootCallId ? { rootCallId } : {}),
      binding,
      promptSnapshot,
      modelSnapshot,
      contextSources: [{ id: "input-1", trust: "user_instruction", source: "test" }],
      toolSchema: null,
      assetReferences: [],
      providerPayload: { messages: [] },
    });
    const rootCallId = prepare(1);
    store.novex.finishModelCall(rootCallId, "succeeded", { text: "world" }, { total: 15 }, undefined);
    expect(() => store.novex.finishModelCall(rootCallId, "failed", undefined, undefined, { code: "late" }))
      .toThrow(/终态/);
    const retryCallId = prepare(2, rootCallId);
    expect(retryCallId).not.toBe(rootCallId);
    expect(() => prepare(2, rootCallId)).toThrow(/持久化失败/);
    store.novex.finishModelCall(retryCallId, "failed", undefined, undefined, { code: "fixture" });
    const asset = {
      asset_id: "image-fixture",
      version: "1",
      sha256: "c7d09a3d7f1f0f7b80f149553682168e1a8182478fba69963f8f54d9f9a714f0",
      mime: "image/png",
    };
    const assetCallId = store.novex.prepareModelCall({
      sessionId,
      phase: "turn",
      nodeKey: "personal.turn",
      attempt: 3,
      rootCallId,
      binding,
      promptSnapshot,
      modelSnapshot,
      contextSources: [],
      toolSchema: null,
      assetReferences: [asset],
      providerPayload: { messages: [] },
    });
    expect(store.novex.modelCall(assetCallId)).toMatchObject({ asset_references: [asset] });
    expect(() => store.novex.prepareModelCall({
      sessionId, phase: "turn", nodeKey: "personal.turn", attempt: 4, rootCallId,
      binding, promptSnapshot, modelSnapshot, contextSources: [], toolSchema: null,
      assetReferences: [], providerPayload: { image: "data:image/png;base64,AAAA" },
    })).toThrow(/安全脱敏|base64/);
    expect(() => store.novex.prepareModelCall({
      sessionId, phase: "turn", nodeKey: "personal.turn", attempt: 4, rootCallId,
      binding, promptSnapshot, modelSnapshot, contextSources: [], toolSchema: null,
      assetReferences: [{ ...asset, url: "https://assets.invalid/a.png" }], providerPayload: {},
    })).toThrow(/安全脱敏|资产引用/);
    store.novex.finishModelCall(assetCallId, "succeeded", { text: "asset accepted" }, {}, undefined);
    const userId = await session.appendMessage(user("hello"));
    const assistantId = await session.appendMessage(assistant("world"));
    const compactId = await session.appendCompaction("summary", assistantId, 15, { memory: false });
    expect(await session.getLeafId()).toBe(compactId);
    await cleanupSession(session);
    await store.close();

    store = new SessionStore(database, root);
    await store.ping();
    expect(store.novex.binding(sessionId)).toEqual(binding);
    const restored = await store.open(sessionId);
    expect(await restored.getLeafId()).toBe(compactId);
    await cleanupSession(restored);

    const all = await store.entries(sessionId, 0, 20);
    expect(all.map(({ sequence }) => sequence)).toEqual([1, 2, 3]);
    expect((await store.entries(sessionId, 1, 20)).map(({ sequence }) => sequence)).toEqual([2, 3]);
    expect(all[2]?.entry).toMatchObject({ type: "compaction", summary: "summary", details: { memory: false } });
    expect(all[0]?.entry).toMatchObject({ id: userId, type: "message" });

    const fork = await store.fork(sessionId, userId, "at");
    const forkId = (await fork.getMetadata()).id;
    await cleanupSession(fork);
    expect((await store.view(forkId)).parent_session_id).toBe(sessionId);
    expect(store.novex.binding(forkId)).toMatchObject({
      context_policy_bindings: binding.context_policy_bindings,
      tokenizer_profile_key: binding.tokenizer_profile_key,
      tokenizer_profile_version: binding.tokenizer_profile_version,
      tokenizer_profile_digest: binding.tokenizer_profile_digest,
    });
    expect(store.novex.migrationEvent(forkId, "ordinary_fork")).toMatchObject({
      details: {
        parent_session_id: sessionId,
        source_binding_digest: expect.stringMatching(/^[0-9a-f]{64}$/),
      },
    });
    expect(await store.entries(forkId, 0, 20)).toHaveLength(1);

    await store.move(sessionId, assistantId);
    expect((await store.view(sessionId)).active_leaf_id).toBe(assistantId);
    const movedEntries = await store.entries(sessionId, 0, 20);
    expect(movedEntries).toHaveLength(4);
    expect(movedEntries[3]?.entry).toMatchObject({ type: "leaf", targetId: assistantId });

    const missingBinding = new DatabaseSync(database);
    missingBinding.prepare("DELETE FROM novex_session_bindings WHERE session_id = ?").run(forkId);
    missingBinding.close();
    await expect(store.open(forkId)).rejects.toMatchObject({ code: "session_migration_required" });
    await store.close();
  });
});
