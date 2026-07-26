import type { Pool } from "pg";
import { describe, expect, it } from "vitest";

import { loadConfig } from "../src/config.js";
import { publicError, RuntimeError } from "../src/errors.js";
import { createPiModelRuntime, ModelConfigRepository } from "../src/models.js";
import { MODEL_CALL_SCHEMA_VERSION, REDACTED, redactForAudit, redactUnknown, safeJson } from "../src/redaction.js";
import { toolsForProfile } from "../src/coordinator.js";
import type { DefinitionRegistry } from "../src/definitions.js";

function row(protocol: string) {
  return {
    id: "11111111-1111-4111-8111-111111111111",
    provider_name: "local-openai",
    api_protocol: protocol,
    auth_scheme: "bearer",
    request_base_url: "https://example.test/v1/",
    upstream_model: "test-model",
    api_key: "top-secret-key",
    timeout_seconds: 12,
    reasoning_effort: "low",
    max_output_tokens: 2048,
    context_window: 64000,
    tokenizer_profile_key: "openai.o200k",
    tokenizer_profile_version: "1.0.0",
    settings: {},
  };
}

function repositoryWithRows(rows: Record<string, unknown>[]): ModelConfigRepository {
  const pool = { query: async () => ({ rows }) } as unknown as Pool;
  return new ModelConfigRepository(pool, {
    tokenizer_profiles: [{
      profile_key: "openai.o200k", version: "1.0.0", status: "active",
      applicable_protocols: ["openai_responses", "openai_chat_completions"],
    }],
  } as unknown as DefinitionRegistry);
}

describe("model routing and safety", () => {
  it.each([
    ["openai_responses", "openai-responses"],
    ["openai_chat_completions", "openai-completions"],
  ])("maps %s without URL guessing", async (protocol, expectedApi) => {
    const resolved = await repositoryWithRows([row(protocol)]).resolveEnabledText(row(protocol).id);
    const runtime = createPiModelRuntime(resolved);

    expect(runtime.model.api).toBe(expectedApi);
    expect(runtime.model.baseUrl).toBe("https://example.test/v1");
    expect(runtime.streamOptions.timeoutMs).toBe(12000);
    expect(runtime.snapshot).toMatchObject({ protocol, max_output_tokens: 2048 });
    expect(JSON.stringify(runtime.snapshot)).not.toContain("top-secret-key");
  });

  it("returns stable errors and never falls back for a missing model", async () => {
    await expect(repositoryWithRows([]).resolveEnabledText(row("openai_responses").id)).rejects.toMatchObject({
      code: "model_not_found",
      status: 404,
    });
    expect(publicError(new RuntimeError("session_busy", 409, "busy"))).toEqual({
      error: { code: "session_busy", message: "busy" },
    });
  });

  it("redacts credential fields, bearer values, query secrets and known values", () => {
    const output = redactUnknown(
      {
        api_key: "one",
        nested: { authorization: "Bearer abc", url: "https://x.test?a=1&token=abc", message: "leak known" },
      },
      ["known"],
    );
    const serialized = JSON.stringify(output);
    expect(serialized).not.toContain("abc");
    expect(serialized).not.toContain("known");
    expect(serialized).toContain(REDACTED);
    expect(safeJson(new Error("Bearer hidden"))).not.toContain("hidden");
    expect(MODEL_CALL_SCHEMA_VERSION).toBe("1");
    const structured = redactForAudit({
      headers: { accept: "application/json" },
      schema_secret: { secret: true, value: "schema-secret" },
      message: "Cookie: session=hidden-cookie",
    });
    expect(JSON.stringify(structured)).not.toMatch(/schema-secret|hidden-cookie|application\/json/);
    const cyclic: Record<string, unknown> = {};
    cyclic.self = cyclic;
    expect(() => redactForAudit(cyclic)).toThrow(/cycle/);
  });

  it("removes sensitive URL query values before a model snapshot is persisted", async () => {
    const modelRow = row("openai_responses");
    modelRow.request_base_url = "https://user:password@example.test/v1?token=query-secret&region=local";
    const runtime = createPiModelRuntime(await repositoryWithRows([modelRow]).resolveEnabledText(modelRow.id));

    expect(runtime.model.baseUrl).toContain("query-secret");
    expect(runtime.snapshot.request_base_url).not.toContain("query-secret");
    expect(runtime.snapshot.request_base_url).not.toContain("password");
    expect(runtime.snapshot.request_base_url).toContain("region=local");
  });

  it("enables local tools only for the workspace profile", () => {
    expect(toolsForProfile("chat")).toEqual([]);
    expect(toolsForProfile("workspace").map((tool) => tool.name)).toEqual(["read", "write", "edit", "bash"]);
  });

  it("rejects incomplete process configuration", () => {
    expect(() => loadConfig({})).toThrowError(/DATABASE_URL/);
    expect(() =>
      loadConfig({ DATABASE_URL: "postgres://db", AGENT_RUNTIME_SQLITE_PATH: "relative.sqlite" }),
    ).toThrowError(/绝对路径/);
  });
});
