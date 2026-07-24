import { mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  createModels,
  fauxAssistantMessage,
  fauxProvider,
  fauxToolCall,
  type Api,
  type FauxProviderHandle,
  type Model,
} from "@earendil-works/pi-ai";
import { afterEach, describe, expect, it } from "vitest";

import { SessionCoordinator, type TextModelResolver } from "../src/coordinator.js";
import type { PiModelRuntime, ResolvedTextModel } from "../src/models.js";
import { RuntimeHttpServer } from "../src/server.js";
import { SessionStore } from "../src/sessions.js";

const MODEL_ID = "11111111-1111-4111-8111-111111111111";
const SECRET = "runtime-test-secret";

interface Fixture {
  root: string;
  faux: FauxProviderHandle;
  server: RuntimeHttpServer;
  baseUrl: string;
}

const fixtures: Fixture[] = [];

afterEach(async () => {
  await Promise.allSettled(fixtures.splice(0).map((fixture) => fixture.server.close()));
});

async function fixture(tokensPerSecond?: number): Promise<Fixture> {
  const root = await mkdtemp(join(tmpdir(), "novex-runtime-"));
  const sessions = new SessionStore(join(root, "sessions.sqlite"), root);
  const resolved: ResolvedTextModel = {
    id: MODEL_ID,
    providerName: "faux",
    protocol: "openai_responses",
    requestBaseUrl: "http://localhost:0",
    upstreamModel: "faux-1",
    apiKey: SECRET,
    timeoutMs: 10_000,
    maxOutputTokens: 4096,
    contextWindow: 128000,
  };
  const resolver: TextModelResolver = {
    resolveEnabledText: async () => resolved,
    ping: async () => undefined,
  };
  const faux = fauxProvider({
    provider: `faux-${Date.now()}-${Math.random()}`,
    ...(tokensPerSecond === undefined ? {} : { tokensPerSecond }),
  });
  const models = createModels();
  models.setProvider(faux.provider);
  const runtime: PiModelRuntime = {
    models,
    model: faux.getModel() as Model<Api>,
    streamOptions: { timeoutMs: 10_000 },
    thinkingLevel: "off",
    snapshot: {
      model_id: MODEL_ID,
      provider: "faux",
      protocol: "openai_responses",
      request_base_url: "http://localhost:0",
      upstream_model: "faux-1",
      reasoning_effort: null,
      max_output_tokens: 4096,
      timeout_seconds: 10,
    },
    secrets: [SECRET],
  };
  const coordinator = new SessionCoordinator(sessions, resolver, () => runtime);
  const server = new RuntimeHttpServer({
    sessions,
    coordinator,
    models: resolver,
    pool: { end: async () => undefined },
  });
  await server.listen("127.0.0.1", 0);
  const created = { root, faux, server, baseUrl: `http://127.0.0.1:${server.port}` };
  fixtures.push(created);
  return created;
}

async function createSession(baseUrl: string, profile: "chat" | "workspace" = "chat"): Promise<string> {
  const response = await fetch(`${baseUrl}/sessions`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ model_id: MODEL_ID, tool_profile: profile }),
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

describe("fake provider runtime and SSE", () => {
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

    const entries = await fetch(`${current.baseUrl}/sessions/${id}/entries`).then((item) => item.json());
    expect(JSON.stringify(entries)).toContain("finished");
    expect(JSON.stringify(entries)).not.toContain(SECRET);
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
  }, 15_000);

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
});
