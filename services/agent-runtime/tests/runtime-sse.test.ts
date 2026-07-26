import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { resolve } from "node:path";
import { DatabaseSync } from "node:sqlite";

import {
  createModels,
  fauxAssistantMessage,
  fauxProvider,
  fauxToolCall,
  type Api,
  type FauxProviderHandle,
  type Model,
} from "@earendil-works/pi-ai";
import { NodeExecutionEnv } from "@earendil-works/pi-agent-core/node";
import { createNodeSqliteFactory, SqliteSessionRepo } from "@earendil-works/pi-storage-sqlite-node";
import { afterEach, describe, expect, it } from "vitest";

import { SessionCoordinator, type TextModelResolver } from "../src/coordinator.js";
import { behaviorFingerprint, loadDefinitionRegistry } from "../src/definitions.js";
import type { PiModelRuntime, ResolvedTextModel } from "../src/models.js";
import { RuntimeError } from "../src/errors.js";
import { RuntimeHttpServer } from "../src/server.js";
import { cleanupSession, SessionStore } from "../src/sessions.js";
import { createAuditedModels } from "../src/audited-models.js";

const MODEL_ID = "11111111-1111-4111-8111-111111111111";
const SECRET = "runtime-test-secret";

interface Fixture {
  root: string;
  faux: FauxProviderHandle;
  server: RuntimeHttpServer;
  baseUrl: string;
  sessions: SessionStore;
  resolverCalls: () => number;
  updateModel: (patch: Partial<ResolvedTextModel>) => void;
  failModelResolution: (error?: RuntimeError) => void;
}

const fixtures: Fixture[] = [];

afterEach(async () => {
  await Promise.allSettled(fixtures.splice(0).map((fixture) => fixture.server.close()));
});

async function fixture(tokensPerSecond?: number): Promise<Fixture> {
  const root = await mkdtemp(join(tmpdir(), "novex-runtime-"));
  const sessions = new SessionStore(join(root, "sessions.sqlite"), root);
  let resolved: ResolvedTextModel = {
    id: MODEL_ID,
    providerName: "faux",
    protocol: "openai_responses",
    requestBaseUrl: "http://localhost:0",
    upstreamModel: "faux-1",
    apiKey: SECRET,
    timeoutMs: 10_000,
    maxOutputTokens: 4096,
    contextWindow: 128000,
    tokenizerProfileKey: "openai.o200k",
    tokenizerProfileVersion: "1.0.0",
    settings: {},
  };
  let resolveCalls = 0;
  let resolveError: RuntimeError | undefined;
  const resolver: TextModelResolver = {
    resolveEnabledText: async () => {
      resolveCalls += 1;
      if (resolveError) throw resolveError;
      return resolved;
    },
    ping: async () => undefined,
  };
  const faux = fauxProvider({
    provider: `faux-${Date.now()}-${Math.random()}`,
    ...(tokensPerSecond === undefined ? {} : { tokensPerSecond }),
  });
  const runtimeFor = (config: ResolvedTextModel): PiModelRuntime => {
    const models = createAuditedModels(createModels());
    models.setProvider(faux.provider);
    const fingerprint = behaviorFingerprint({
      protocol: config.protocol,
      request_base_url: config.requestBaseUrl,
      upstream_model: config.upstreamModel,
      reasoning_effort: config.reasoningEffort ?? null,
      max_output_tokens: config.maxOutputTokens,
      context_window: config.contextWindow,
      tokenizer_profile_key: config.tokenizerProfileKey,
      tokenizer_profile_version: config.tokenizerProfileVersion,
      settings: config.settings,
    });
    return {
      models,
      model: faux.getModel() as Model<Api>,
      streamOptions: { timeoutMs: config.timeoutMs, maxRetries: 0 },
      thinkingLevel: "off",
      snapshot: {
        model_id: config.id,
        provider: config.providerName,
        protocol: config.protocol,
        request_base_url: config.requestBaseUrl,
        upstream_model: config.upstreamModel,
        reasoning_effort: config.reasoningEffort ?? null,
        max_output_tokens: config.maxOutputTokens,
        timeout_seconds: config.timeoutMs / 1000,
        context_window: config.contextWindow,
        tokenizer_profile_key: config.tokenizerProfileKey,
        tokenizer_profile_version: config.tokenizerProfileVersion,
        behavior_settings: fingerprint.normalized.settings,
        behavior_fingerprint: fingerprint.digest,
      },
      secrets: [config.apiKey],
    };
  };
  const definitions = await loadDefinitionRegistry(resolve(import.meta.dirname, "../../../agent-definitions"));
  const coordinator = new SessionCoordinator(sessions, resolver, runtimeFor, definitions);
  const server = new RuntimeHttpServer({
    sessions,
    coordinator,
    models: resolver,
    pool: { end: async () => undefined },
  });
  await server.listen("127.0.0.1", 0);
  const created: Fixture = {
    root,
    faux,
    server,
    baseUrl: `http://127.0.0.1:${server.port}`,
    sessions,
    resolverCalls: () => resolveCalls,
    updateModel: (patch) => {
      resolved = { ...resolved, ...patch };
    },
    failModelResolution: (error = new RuntimeError("model_not_found", 404, "模型已停用或删除")) => {
      resolveError = error;
    },
  };
  fixtures.push(created);
  return created;
}

async function createSession(baseUrl: string, profile: "chat" | "workspace" = "chat"): Promise<string> {
  const response = await fetch(`${baseUrl}/sessions`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ agent_key: "personal.general", model_id: MODEL_ID, tool_profile: profile }),
  });
  expect(response.status).toBe(201);
  const body = (await response.json()) as { session: { session_id: string } };
  return body.session.session_id;
}

function post(baseUrl: string, path: string, body: object = {}) {
  return fetch(`${baseUrl}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
}

async function createLegacySession(root: string, systemPrompt?: string, agentKey?: string): Promise<string> {
  const env = new NodeExecutionEnv({ cwd: root });
  const repo = new SqliteSessionRepo({
    env,
    sqlite: createNodeSqliteFactory(),
    databasePath: join(root, "sessions.sqlite"),
  });
  const session = await repo.create({
    cwd: root,
    metadata: {
      model_id: MODEL_ID,
      tool_profile: "chat",
      source: "legacy_fixture",
      ...(agentKey ? { agent_key: agentKey } : {}),
      ...(systemPrompt ? { system_prompt: systemPrompt } : {}),
    },
  });
  await session.appendMessage({
    role: "user",
    content: [{ type: "text", text: "legacy history" }],
    timestamp: Date.now(),
  });
  const id = (await session.getMetadata()).id;
  await cleanupSession(session);
  await env.cleanup();
  return id;
}

function sortedKeys(value: unknown): string[] {
  return Object.keys(value as Record<string, unknown>).sort();
}

function contractFields(contract: Record<string, unknown>, key: string): string[] {
  return [...(contract[key] as string[])].sort();
}

describe("fake provider runtime and SSE", () => {
  it("plans and idempotently migrates legacy sessions without losing trees or reusing custom System text", async () => {
    const current = await fixture();
    const plainId = await createLegacySession(current.root);
    const customId = await createLegacySession(current.root, "legacy custom system");

    const plan = await fetch(`${current.baseUrl}/migration/plan`);
    expect(plan.status).toBe(200);
    const planned = await plan.json() as {
      schema_version: string;
      dry_run: boolean;
      items: Array<Record<string, unknown>>;
    };
    expect(planned.dry_run).toBe(true);
    expect(planned.schema_version).toBe("2");
    expect(planned.items).toEqual(expect.arrayContaining([
      expect.objectContaining({
        runtime: "pi", entity_type: "session", entity_id: plainId,
        agent_key: "personal.general", disposition: "equivalent", reason_code: "baseline_equivalent",
        node_keys: ["personal.branch_summary", "personal.compaction", "personal.tool_followup", "personal.turn"],
      }),
      expect.objectContaining({
        entity_id: customId, disposition: "context_migration_required",
        reason_code: "legacy_custom_system_prompt_not_equivalent",
      }),
    ]));
    expect(current.sessions.novex.listModelCalls(plainId)).toHaveLength(0);
    const backupPath = join(current.root, "pre-migration.sqlite");
    await current.sessions.backupForHistoryMigration(backupPath);
    const backup = new DatabaseSync(backupPath, { readOnly: true });
    expect(backup.prepare("SELECT COUNT(*) AS count FROM novex_session_bindings").get())
      .toMatchObject({ count: 0 });
    backup.close();

    const plain = await fetch(`${current.baseUrl}/sessions/${plainId}`);
    expect(plain.status).toBe(200);
    expect(await plain.json()).toMatchObject({ session_id: plainId, agent_key: "personal.general", agent_version: "2.0.0" });
    expect(current.sessions.novex.binding(plainId)).toMatchObject({
      binding_status: "executable",
      migration_source: "context_history_v2_equivalent",
    });
    expect(current.sessions.novex.migrationEvent(plainId, "context_history_v2_equivalent"))
      .toMatchObject({ details: { migration_plan: { reason_code: "baseline_equivalent" } } });
    expect((await current.sessions.entries(plainId)).map(({ entry }) => entry))
      .toEqual(expect.arrayContaining([expect.objectContaining({ type: "message" })]));

    const custom = await fetch(`${current.baseUrl}/sessions/${customId}`);
    expect(custom.status).toBe(200);
    expect(current.sessions.novex.binding(customId)).toMatchObject({
      binding_status: "read_only",
      migration_source: "context_history_v2_read_only",
    });
    expect(JSON.stringify(current.sessions.novex.migrationEvent(customId, "context_history_v2_context_migration_required")))
      .not.toContain("legacy custom system");
    expect((await post(current.baseUrl, `/sessions/${customId}/prompt`, { text: "continue" })).status).toBe(409);
    expect((await post(current.baseUrl, `/sessions/${customId}/fork`, {})).status).toBe(409);

    const upgrade = {
      agent_key: "personal.general",
      agent_version: "2.0.0",
      model_id: MODEL_ID,
      tool_profile: "chat",
    };
    expect((await post(current.baseUrl, `/sessions/${customId}/fork`, { upgrade })).status).toBe(409);
    const visibleFork = await post(current.baseUrl, `/sessions/${customId}/fork`, {
      upgrade: { ...upgrade, legacy_prompt_disposition: "user_instruction" },
    });
    expect(visibleFork.status).toBe(201);
    const visibleId = ((await visibleFork.json()) as { session_id: string }).session_id;
    const visibleEntries = await current.sessions.entries(visibleId);
    expect(JSON.stringify(visibleEntries)).toContain("legacy custom system");
    expect((await current.sessions.findMetadata(visibleId)).metadata).not.toHaveProperty("system_prompt");
    expect(current.sessions.novex.binding(customId).binding_status).toBe("read_only");
    expect(current.sessions.novex.binding(visibleId).binding_status).toBe("executable");

    const customDiscardId = await createLegacySession(current.root, "discard this legacy system");
    await fetch(`${current.baseUrl}/sessions/${customDiscardId}`);
    const discardFork = await post(current.baseUrl, `/sessions/${customDiscardId}/fork`, {
      upgrade: { ...upgrade, legacy_prompt_disposition: "discard" },
    });
    expect(discardFork.status).toBe(201);
    const discardId = ((await discardFork.json()) as { session_id: string }).session_id;
    expect(JSON.stringify(await current.sessions.entries(discardId))).not.toContain("discard this legacy system");
    expect((await current.sessions.findMetadata(customDiscardId)).id).toBe(customDiscardId);

    const after = await fetch(`${current.baseUrl}/migration/plan`);
    expect(((await after.json()) as { items: unknown[] }).items).toHaveLength(0);
    expect(current.sessions.novex.listModelCalls(plainId)).toHaveLength(0);
  });

  it("keeps missing-model and unmappable legacy sessions readable while blocking execution", async () => {
    const current = await fixture();
    const missingModelId = await createLegacySession(current.root);
    const unmappableId = await createLegacySession(current.root, undefined, "legacy.unknown");
    current.failModelResolution(new RuntimeError("tokenizer_profile_unavailable", 422, "Profile 缺失"));

    const plan = await fetch(`${current.baseUrl}/migration/plan`);
    const planned = await plan.json() as { items: Array<Record<string, unknown>> };
    expect(planned.items).toEqual(expect.arrayContaining([
      expect.objectContaining({
        entity_id: missingModelId,
        disposition: "model_configuration_missing",
        reason_code: "model_configuration_missing",
      }),
      expect.objectContaining({
        entity_id: unmappableId,
        disposition: "unmappable",
        reason_code: "unknown_agent_key",
      }),
    ]));

    expect((await post(current.baseUrl, `/sessions/${missingModelId}/prompt`, { text: "blocked" })).status).toBe(409);
    expect(((await (await fetch(`${current.baseUrl}/migration/plan`)).json()) as { items: unknown[] }).items)
      .toHaveLength(2);

    await current.sessions.backupForHistoryMigration(join(current.root, "blocked-migration.sqlite"));
    expect((await fetch(`${current.baseUrl}/sessions/${missingModelId}`)).status).toBe(200);
    expect((await fetch(`${current.baseUrl}/sessions/${unmappableId}`)).status).toBe(200);
    expect((await current.sessions.entries(missingModelId)).map(({ entry }) => entry))
      .toEqual(expect.arrayContaining([expect.objectContaining({ type: "message" })]));
    expect(current.sessions.novex.migrationEvent(
      missingModelId,
      "context_history_v2_model_configuration_missing",
    )).toMatchObject({ details: { migration_plan: { reason_code: "model_configuration_missing" } } });
    expect(current.sessions.novex.migrationEvent(
      unmappableId,
      "context_history_v2_unmappable",
    )).toMatchObject({ details: { migration_plan: { reason_code: "unknown_agent_key" } } });
    expect(((await (await fetch(`${current.baseUrl}/migration/plan`)).json()) as { items: unknown[] }).items)
      .toHaveLength(0);
    expect(current.sessions.novex.bindingOrNull(missingModelId)).toBeNull();
    expect(current.sessions.novex.bindingOrNull(unmappableId)).toBeNull();
  });

  it("serves the shared ModelCall read contract and recompiles dry-run without side effects", async () => {
    const current = await fixture();
    current.faux.setResponses([fauxAssistantMessage("audited response")]);
    const id = await createSession(current.baseUrl);
    const prompt = await post(current.baseUrl, `/sessions/${id}/prompt`, { text: "audit this" });
    expect(await prompt.text()).toContain("event: run_completed");

    const contract = JSON.parse(await readFile(
      resolve(import.meta.dirname, "../../../agent-definitions/fixtures/model-call-read-api.json"),
      "utf8",
    )) as Record<string, unknown>;
    const raw = current.sessions.novex.listModelCalls(id);
    expect(raw).toHaveLength(1);
    const call = raw[0]!;
    const query = new URLSearchParams({
      owner_type: "session",
      owner_id: id,
      node_key: call.node_key,
      agent_version: call.agent_version,
      prompt_version: call.prompt_version,
      model_id: call.model_id,
      status: call.status,
      prepared_from: call.prepared_at,
      prepared_to: new Date(Date.now() + 60_000).toISOString(),
      limit: "20",
      offset: "0",
    });
    const list = await fetch(`${current.baseUrl}/model-calls?${query}`).then((item) => item.json()) as Record<string, unknown>;
    expect(sortedKeys(list)).toEqual(contractFields(contract, "list_envelope_fields"));
    expect(list.source_runtime).toBe("pi");
    expect(list.total).toBe(1);
    const summary = (list.items as Array<Record<string, unknown>>)[0]!;
    expect(sortedKeys(summary)).toEqual(contractFields(contract, "summary_fields"));
    expect(sortedKeys(summary.owner)).toEqual(contractFields(contract, "owner_fields"));
    expect(sortedKeys(summary.execution)).toEqual(contractFields(contract, "execution_fields"));
    expect(sortedKeys(summary.definition)).toEqual(contractFields(contract, "definition_fields"));
    expect(sortedKeys(summary.model)).toEqual(contractFields(contract, "summary_model_fields"));
    expect(sortedKeys(summary.usage)).toEqual(contractFields(contract, "usage_fields"));

    const detail = await fetch(`${current.baseUrl}/model-calls/${call.id}`).then((item) => item.json()) as Record<string, unknown>;
    expect(sortedKeys(detail)).toEqual(contractFields(contract, "detail_envelope_fields"));
    expect(sortedKeys(detail.record)).toEqual(contractFields(contract, "record_fields"));
    expect(String(detail.record_hash)).toHaveLength(64);
    expect(await fetch(`${current.baseUrl}/model-calls/${call.id}/export`).then((item) => item.json())).toEqual(detail);

    const contextContract = JSON.parse(await readFile(
      resolve(import.meta.dirname, "../../../agent-definitions/fixtures/context-audit-read-api.json"),
      "utf8",
    )) as Record<string, unknown>;
    const contextList = await fetch(
      `${current.baseUrl}/sessions/${id}/contexts?record_type=snapshot&limit=20&offset=0`,
    ).then((item) => item.json()) as Record<string, unknown>;
    expect(sortedKeys(contextList)).toEqual(contractFields(contextContract, "list_envelope_fields"));
    expect(contextList).toMatchObject({ schema_version: "2", source_runtime: "pi", total: 1 });
    const contextSummary = (contextList.items as Array<Record<string, unknown>>)[0]!;
    expect(sortedKeys(contextSummary)).toEqual(contractFields(contextContract, "summary_fields"));
    expect(sortedKeys(contextSummary.owner)).toEqual(contractFields(contextContract, "owner_fields"));
    expect(contextSummary).not.toHaveProperty("decisions");
    const contextId = String(contextSummary.id);
    const contextDetail = await fetch(`${current.baseUrl}/contexts/${contextId}`).then((item) => item.json()) as
      Record<string, unknown>;
    expect(sortedKeys(contextDetail)).toEqual(contractFields(contextContract, "detail_envelope_fields"));
    expect(sortedKeys(contextDetail.record)).toEqual(contractFields(contextContract, "snapshot_record_fields"));
    expect(String(contextDetail.record_hash)).toHaveLength(64);
    expect(await fetch(`${current.baseUrl}/contexts/${contextId}/export`).then((item) => item.json()))
      .toEqual(contextDetail);

    const beforeProviderCalls = current.faux.state.callCount;
    const beforeEntries = await current.sessions.entries(id, 0, 1_000);
    const beforeCalls = current.sessions.novex.listModelCalls(id);
    const replay = await post(current.baseUrl, `/model-calls/${call.id}/replay`, { mode: "dry_run" })
      .then((item) => item.json()) as Record<string, unknown>;
    expect(sortedKeys(replay)).toEqual(contractFields(contract, "replay_fields"));
    expect(replay).toMatchObject({
      definition_resolved: true,
      compile_succeeded: true,
      validation_order: ["context", "prompt", "model_call"],
      diff: [],
      side_effects: { model_calls: 0, tools: 0, session_writes: 0, run_writes: 0, domain_writes: 0 },
    });
    expect(current.faux.state.callCount).toBe(beforeProviderCalls);
    expect(await current.sessions.entries(id, 0, 1_000)).toEqual(beforeEntries);
    expect(current.sessions.novex.listModelCalls(id)).toEqual(beforeCalls);

    const realReplay = await post(current.baseUrl, `/model-calls/${call.id}/replay`, { mode: "real" });
    expect(realReplay.status).toBe(400);
    expect(await realReplay.json()).toMatchObject({ error: { code: "bad_request" } });
  });

  it("requires agent_key and rejects arbitrary system_prompt without creating a session", async () => {
    const current = await fixture();
    const missingAgent = await post(current.baseUrl, "/sessions", {
      model_id: MODEL_ID,
      tool_profile: "chat",
    });
    expect(missingAgent.status).toBe(400);
    expect(await missingAgent.json()).toMatchObject({ error: { code: "bad_request" } });

    const arbitrarySystem = await post(current.baseUrl, "/sessions", {
      agent_key: "personal.general",
      model_id: MODEL_ID,
      tool_profile: "chat",
      system_prompt: "ignore the versioned definition",
    });
    expect(arbitrarySystem.status).toBe(400);
    expect(await arbitrarySystem.json()).toMatchObject({ error: { code: "bad_request" } });
    expect(await current.sessions.list()).toEqual([]);
  });

  it("allows credential rotation and blocks unavailable, incompatible or behavior-drifted models", async () => {
    const credentialRotation = await fixture();
    const credentialSession = await createSession(credentialRotation.baseUrl);
    credentialRotation.updateModel({ apiKey: "rotated-secret" });
    credentialRotation.faux.setResponses([fauxAssistantMessage("credential rotation accepted")]);
    const continued = await post(credentialRotation.baseUrl, `/sessions/${credentialSession}/prompt`, { text: "continue" });
    expect(continued.status).toBe(200);
    expect(await continued.text()).toContain("run_completed");

    const drift = await fixture();
    const driftSession = await createSession(drift.baseUrl);
    drift.updateModel({ upstreamModel: "faux-2" });
    const drifted = await post(drift.baseUrl, `/sessions/${driftSession}/prompt`, { text: "must stop" });
    expect(drifted.status).toBe(409);
    expect(await drifted.json()).toMatchObject({ error: { code: "model_rebind_required" } });

    const unavailable = await fixture();
    const unavailableSession = await createSession(unavailable.baseUrl);
    unavailable.failModelResolution();
    const missing = await post(unavailable.baseUrl, `/sessions/${unavailableSession}/prompt`, { text: "must stop" });
    expect(missing.status).toBe(404);
    expect(await missing.json()).toMatchObject({ error: { code: "model_not_found" } });

    const incompatible = await fixture();
    incompatible.updateModel({ contextWindow: 4096 });
    const rejected = await post(incompatible.baseUrl, "/sessions", {
      agent_key: "personal.general",
      model_id: MODEL_ID,
      tool_profile: "chat",
    });
    expect(rejected.status).toBe(422);
    expect(await rejected.json()).toMatchObject({ error: { code: "model_capability_mismatch" } });
  });

  it("does not call the provider when prepared ModelCall persistence fails", async () => {
    const current = await fixture();
    const id = await createSession(current.baseUrl);
    current.faux.setResponses([fauxAssistantMessage("must not be called")]);
    const database = new DatabaseSync(join(current.root, "sessions.sqlite"));
    database.exec(`
      CREATE TRIGGER fail_model_call_prepare
      BEFORE INSERT ON novex_model_calls
      BEGIN SELECT RAISE(ABORT, 'injected audit failure'); END;
    `);
    database.close();

    const response = await post(current.baseUrl, `/sessions/${id}/prompt`, { text: "must stop before provider" });
    const sse = await response.text();

    expect(current.faux.state.callCount).toBe(0);
    expect(current.faux.getPendingResponseCount()).toBe(1);
    expect(sse).toContain('"code":"audit_persistence_failed"');
    expect(sse).toContain("event: run_failed");
    expect(sse).not.toContain("event: run_completed");
    expect(current.sessions.novex.listModelCalls(id)).toEqual([]);
    const entries = await current.sessions.entries(id, 0, 20);
    expect(entries.some(({ entry }) => entry.type === "message"
      && entry.message.role === "assistant"
      && entry.message.stopReason !== "error")).toBe(false);
  });

  it("persists an oversized Context failure without provider, Tool or canary leakage", async () => {
    const current = await fixture();
    current.updateModel({ contextWindow: 8192 });
    const id = await createSession(current.baseUrl);
    const canary = "NOVEX_CANARY_SECRET_DO_NOT_PERSIST_runtime_context";
    const oversized = `${canary}-${"x ".repeat(5_000)}`;
    const response = await post(current.baseUrl, `/sessions/${id}/prompt`, { text: oversized });
    const sse = await response.text();

    expect((sse.match(/event: run_failed/g) ?? [])).toHaveLength(1);
    expect(sse).toContain("context_budget_exceeded");
    expect(sse).not.toContain(canary);
    expect(current.faux.state.callCount).toBe(0);
    expect(current.sessions.novex.listModelCalls(id)).toEqual([]);
    const attempts = current.sessions.novex.queryContextRecords(
      { sessionId: id, recordType: "compile_attempt" }, 20, 0,
    );
    expect(attempts).toMatchObject({
      total: 1,
      items: [expect.objectContaining({ record_type: "compile_attempt", status: "failed" })],
    });
    const attemptId = attempts.items[0]!.id;
    expect(JSON.stringify(current.sessions.novex.contextRecord(attemptId))).not.toContain(canary);
    const detail = await fetch(`${current.baseUrl}/contexts/${attemptId}`).then((item) => item.json());
    const exported = await fetch(`${current.baseUrl}/contexts/${attemptId}/export`).then((item) => item.json());
    expect(exported).toEqual(detail);
    expect(JSON.stringify(exported)).not.toContain(canary);
    expect(JSON.stringify(exported)).not.toContain("selected_payload");
  }, 30_000);

  it("does not silently retry after partial stream output", async () => {
    const current = await fixture();
    let observedMaxRetries: number | undefined;
    current.faux.setResponses([
      (_context, options) => {
        observedMaxRetries = options?.maxRetries;
        return fauxAssistantMessage("partial output", {
          stopReason: "error",
          errorMessage: "stream failed after output",
        });
      },
      fauxAssistantMessage("unapproved retry response"),
    ]);
    const id = await createSession(current.baseUrl);
    const response = await post(current.baseUrl, `/sessions/${id}/prompt`, { text: "stream once" });
    const sse = await response.text();

    expect(observedMaxRetries).toBe(0);
    expect(current.faux.state.callCount).toBe(1);
    expect(current.faux.getPendingResponseCount()).toBe(1);
    expect(sse).toContain("partial output");
    expect(sse).toContain("event: run_failed");
    expect(sse).not.toContain("unapproved retry response");
    const calls = current.sessions.novex.listModelCalls(id);
    expect(calls).toHaveLength(1);
    expect(calls[0]).toMatchObject({ attempt: 1, status: "failed" });
    expect(calls[0]?.entry_id).toBeTruthy();
    expect(current.sessions.novex.modelCall(calls[0]!.id)).toMatchObject({
      output_snapshot: { stopReason: "error" },
      error_snapshot: { code: "provider_error", message: "stream failed after output" },
    });
  });

  it("streams compatible write and edit tool loops before the persisted terminal event", async () => {
    const current = await fixture();
    current.faux.setResponses([
      fauxAssistantMessage(fauxToolCall("write", { path: "result.txt", content: "before" }), {
        stopReason: "toolUse",
      }),
      fauxAssistantMessage(fauxToolCall("edit", {
        path: "result.txt",
        old_text: "before",
        new_text: "after",
      }), {
        stopReason: "toolUse",
      }),
      fauxAssistantMessage("finished"),
    ]);
    const id = await createSession(current.baseUrl, "workspace");
    const response = await post(current.baseUrl, `/sessions/${id}/prompt`, { text: "write then edit the result" });
    const sse = await response.text();

    expect(response.headers.get("content-type")).toContain("text/event-stream");
    expect(sse.indexOf("event: run_started")).toBeLessThan(sse.indexOf("event: tool_execution_start"));
    expect(sse.indexOf("event: tool_execution_end")).toBeLessThan(sse.indexOf("event: run_completed"));
    expect((sse.match(/event: tool_execution_start/g) ?? [])).toHaveLength(2);
    expect((sse.match(/event: tool_execution_end/g) ?? [])).toHaveLength(2);
    expect((sse.match(/event: run_completed/g) ?? [])).toHaveLength(1);
    expect(sse).not.toContain("event: run_failed");
    expect(sse).not.toContain(SECRET);
    expect(await readFile(join(current.root, "result.txt"), "utf8")).toBe("after");

    const entries = await fetch(`${current.baseUrl}/sessions/${id}/entries`).then((item) => item.json()) as {
      entries: Array<{ entry: { id: string } }>;
    };
    expect(JSON.stringify(entries)).toContain("finished");
    expect(JSON.stringify(entries)).not.toContain(SECRET);

    const calls = (await fetch(`${current.baseUrl}/sessions/${id}/model-calls`).then((item) => item.json())) as {
      items: Array<{
        id: string;
        execution: { entry_id: string | null };
        node_key: string;
        status: string;
      }>;
    };
    expect(calls.items).toHaveLength(3);
    expect(calls.items.every((call) => call.status === "succeeded")).toBe(true);
    expect(calls.items.every((call) => typeof call.execution.entry_id === "string")).toBe(true);
    expect(current.resolverCalls()).toBeGreaterThanOrEqual(4);
    expect(calls.items.filter((call) => call.node_key === "personal.tool_followup")).toHaveLength(2);
    const entryIds = new Set(entries.entries.map(({ entry }) => entry.id));
    expect(calls.items.every((call) => call.execution.entry_id !== null && entryIds.has(call.execution.entry_id))).toBe(true);
    const detail = await fetch(`${current.baseUrl}/model-calls/${calls.items[0]!.id}`).then((item) => item.json());
    expect(JSON.stringify(detail)).not.toContain(SECRET);
    const details = calls.items.map((call) => current.sessions.novex.modelCall(call.id));
    const toolFollowups = details.filter((call) => call.node_key === "personal.tool_followup");
    expect(toolFollowups).toHaveLength(2);
    for (const call of toolFollowups) {
      expect(call.prompt_snapshot).toMatchObject({
        schema_version: "2",
        agent_key: "personal.general",
        agent_version: "2.0.0",
        node_key: "personal.tool_followup",
        fragments: [],
        context_snapshot_id: expect.stringMatching(/^[0-9a-f-]{36}$/),
      });
      expect(call.context_sources).toEqual(expect.arrayContaining([
        expect.objectContaining({ source_kind: "pi_tool_exchange", trust: "reference" }),
      ]));
      const snapshot = call.prompt_snapshot as { context_snapshot_id: string; context_digest: string };
      expect(call.context_snapshot_id).toBe(snapshot.context_snapshot_id);
      expect(call.context_digest).toBe(snapshot.context_digest);
      expect(JSON.stringify(call.prompt_snapshot)).not.toContain("You are a helpful assistant.");
    }
    const exported = await fetch(`${current.baseUrl}/model-calls/${calls.items[0]!.id}/export`).then((item) => item.json());
    expect(exported).toMatchObject({ schema_version: "1", source_runtime: "pi" });
    const replay = await post(current.baseUrl, `/model-calls/${calls.items[0]!.id}/replay`, { mode: "dry_run" }).then((item) => item.json());
    expect(replay).toMatchObject({
      mode: "dry_run",
      side_effects: { model_calls: 0, tools: 0, session_writes: 0, domain_writes: 0 },
    });
  });

  it("rejects concurrent prompt and accepts steer plus follow-up during an active run", async () => {
    const current = await fixture(20);
    current.faux.setResponses([
      fauxAssistantMessage("first response stays active long enough"),
      fauxAssistantMessage("steered response"),
      fauxAssistantMessage("follow-up response"),
    ]);
    const id = await createSession(current.baseUrl);
    const firstResponse = await post(current.baseUrl, `/sessions/${id}/prompt`, { text: "start" });

    const concurrent = await post(current.baseUrl, `/sessions/${id}/prompt`, { text: "duplicate" });
    expect(concurrent.status).toBe(409);
    expect(await concurrent.json()).toMatchObject({ error: { code: "session_busy" } });
    expect((await post(current.baseUrl, `/sessions/${id}/steer`, { text: "change direction" })).status).toBe(202);
    expect((await post(current.baseUrl, `/sessions/${id}/follow-up`, { text: "then continue" })).status).toBe(202);

    const sse = await firstResponse.text();
    expect(sse).toContain("event: queue_update");
    expect((sse.match(/event: run_completed/g) ?? [])).toHaveLength(1);
    expect(sse).not.toContain("event: run_failed");
    const details = current.sessions.novex.listModelCalls(id).map((call) => current.sessions.novex.modelCall(call.id));
    expect(details).toHaveLength(3);
    expect(details.some((call) => JSON.stringify(call.context_sources).includes('"trust":"steer"')
      && JSON.stringify(call.prompt_snapshot).includes("change direction"))).toBe(true);
    expect(details.some((call) => JSON.stringify(call.context_sources).includes('"trust":"follow_up"')
      && JSON.stringify(call.prompt_snapshot).includes("then continue"))).toBe(true);
    expect(details.every((call) => {
      const snapshot = call.prompt_snapshot as { system: string };
      return !snapshot.system.includes("change direction") && !snapshot.system.includes("then continue");
    })).toBe(true);
  }, 15_000);

  it("blocks model-requested tools outside the bound workspace profile", async () => {
    const current = await fixture();
    current.faux.setResponses([
      fauxAssistantMessage(fauxToolCall("write", { path: "blocked.txt", content: "must not exist" }), {
        stopReason: "toolUse",
      }),
      fauxAssistantMessage("tool request was blocked"),
    ]);
    const id = await createSession(current.baseUrl, "chat");
    const response = await post(current.baseUrl, `/sessions/${id}/prompt`, { text: "try a tool" });
    const sse = await response.text();

    expect(sse).toContain("Tool write not found");
    await expect(readFile(join(current.root, "blocked.txt"), "utf8")).rejects.toMatchObject({ code: "ENOENT" });
    expect(current.sessions.novex.listModelCalls(id)).toHaveLength(2);
  });

  it("aborts a slow run and emits exactly one completed terminal state", async () => {
    const current = await fixture(20);
    current.faux.setResponses([fauxAssistantMessage("x".repeat(200))]);
    const id = await createSession(current.baseUrl);
    const response = await post(current.baseUrl, `/sessions/${id}/prompt`, { text: "slow" });
    expect((await post(current.baseUrl, `/sessions/${id}/abort`)).status).toBe(202);
    const sse = await response.text();

    expect(sse).toContain('"status":"aborted"');
    expect((sse.match(/event: run_completed/g) ?? [])).toHaveLength(1);
    expect(sse).not.toContain("event: run_failed");
  }, 15_000);

  it("keeps compaction, summarized navigation and fork on the persisted session tree", async () => {
    const current = await fixture();
    current.faux.setResponses([
      fauxAssistantMessage("initial answer"),
      fauxAssistantMessage("compact summary"),
      fauxAssistantMessage("branch summary"),
    ]);
    const id = await createSession(current.baseUrl);
    const promptResponse = await post(current.baseUrl, `/sessions/${id}/prompt`, { text: "establish context" });
    expect((await promptResponse.text()).match(/event: run_completed/g)).toHaveLength(1);

    const before = (await fetch(`${current.baseUrl}/sessions/${id}/entries`).then((item) => item.json())) as {
      entries: Array<{ entry: { id: string; type: string } }>;
    };
    const userEntry = before.entries.find(({ entry }) => entry.type === "message")?.entry.id;
    expect(userEntry).toBeTruthy();

    const compact = await post(current.baseUrl, `/sessions/${id}/compact`, { instructions: "保留已确认事实" });
    expect(compact.status).toBe(200);
    const navigate = await post(current.baseUrl, `/sessions/${id}/tree`, {
      entry_id: userEntry,
      summarize: true,
      instructions: "总结被切换的分支",
    });
    expect(navigate.status).toBe(200);

    const fork = await post(current.baseUrl, `/sessions/${id}/fork`, { entry_id: userEntry, position: "at" });
    expect(fork.status).toBe(201);
    const forkView = await fork.json() as { session_id: string; parent_session_id: string };
    expect(forkView).toMatchObject({ parent_session_id: id });
    const sourceBinding = current.sessions.novex.binding(id);
    const forkBinding = current.sessions.novex.binding(forkView.session_id);
    expect(Object.keys(sourceBinding.context_policy_bindings).sort()).toEqual([
      "personal.branch_summary",
      "personal.compaction",
      "personal.tool_followup",
      "personal.turn",
    ]);
    expect(Object.values(sourceBinding.context_policy_bindings))
      .toEqual(expect.arrayContaining([expect.objectContaining({ digest: expect.stringMatching(/^[0-9a-f]{64}$/) })]));
    expect(sourceBinding).toMatchObject({
      tokenizer_profile_key: "openai.o200k",
      tokenizer_profile_version: "1.0.0",
      tokenizer_profile_digest: expect.stringMatching(/^[0-9a-f]{64}$/),
    });
    expect(forkBinding).toMatchObject({
      agent_key: sourceBinding.agent_key,
      agent_version: sourceBinding.agent_version,
      agent_digest: sourceBinding.agent_digest,
      prompt_bindings: sourceBinding.prompt_bindings,
      context_policy_bindings: sourceBinding.context_policy_bindings,
      tokenizer_profile_key: sourceBinding.tokenizer_profile_key,
      tokenizer_profile_version: sourceBinding.tokenizer_profile_version,
      tokenizer_profile_digest: sourceBinding.tokenizer_profile_digest,
      registry_digest: sourceBinding.registry_digest,
      tool_profile: sourceBinding.tool_profile,
      model_id: sourceBinding.model_id,
      behavior_fingerprint: sourceBinding.behavior_fingerprint,
      migration_source: "ordinary_fork",
      parent_session_id: id,
    });

    const upgraded = await post(current.baseUrl, `/sessions/${id}/fork`, {
      entry_id: userEntry,
      position: "at",
      upgrade: {
        agent_key: "personal.general",
        agent_version: "2.0.0",
        model_id: MODEL_ID,
        tool_profile: "workspace",
      },
    });
    expect(upgraded.status).toBe(201);
    const upgradedView = await upgraded.json() as { session_id: string; parent_session_id: string; tool_profile: string };
    expect(upgradedView).toMatchObject({ parent_session_id: id, tool_profile: "workspace" });
    expect(current.sessions.novex.binding(upgradedView.session_id)).toMatchObject({
      agent_key: "personal.general",
      agent_version: "2.0.0",
      model_id: MODEL_ID,
      tool_profile: "workspace",
      migration_source: "explicit_upgrade_fork",
      parent_session_id: id,
    });
    expect(current.sessions.novex.binding(id)).toEqual(sourceBinding);

    const after = await fetch(`${current.baseUrl}/sessions/${id}/entries`).then((item) => item.json());
    expect(JSON.stringify(after)).toContain("compaction");
    expect(JSON.stringify(after)).toContain("branch");
    expect(JSON.stringify(after)).not.toContain(SECRET);
    const calls = (await fetch(`${current.baseUrl}/sessions/${id}/model-calls`).then((item) => item.json())) as {
      items: Array<{ execution: { entry_id: string | null; phase: string }; status: string }>;
    };
    expect(calls.items.map((item) => item.execution.phase).sort()).toEqual(["branch_summary", "compaction", "turn"]);
    expect(calls.items.every((item) => item.status === "succeeded")).toBe(true);
    expect(calls.items.every((item) => typeof item.execution.entry_id === "string")).toBe(true);
  });
});
