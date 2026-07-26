import { describe, expect, it } from "vitest";

import { ModelConfigRepository } from "../src/models.js";
import type { DefinitionRegistry } from "../src/definitions.js";

const MODEL_ID = "11111111-1111-4111-8111-111111111111";

function repository(
  overrides: Record<string, unknown> = {},
  profileStatus: "active" | "supported" | "candidate" | "revoked" = "active",
): ModelConfigRepository {
  const row = {
    id: MODEL_ID,
    provider_name: "fixture",
    api_protocol: "openai_responses",
    auth_scheme: "bearer",
    request_base_url: "https://example.test/v1",
    upstream_model: "fixture-1",
    api_key: "fixture-secret",
    timeout_seconds: 30,
    reasoning_effort: null,
    max_output_tokens: 4096,
    context_window: 128000,
    tokenizer_profile_key: "openai.o200k",
    tokenizer_profile_version: "1.0.0",
    settings: { temperature: 0 },
    ...overrides,
  };
  const pool = {
    query: async () => ({ rows: [row] }),
  };
  const definitions = {
    tokenizer_profiles: [{
      profile_key: "openai.o200k", version: "1.0.0", status: profileStatus,
      applicable_protocols: ["openai_responses", "openai_chat_completions"],
    }],
  } as unknown as DefinitionRegistry;
  return new ModelConfigRepository(pool as never, definitions);
}

describe("Pi text model resolution", () => {
  it("requires explicit output and context limits", async () => {
    await expect(repository({ max_output_tokens: null }).resolveEnabledText(MODEL_ID)).rejects.toMatchObject({
      code: "model_incompatible",
    });
    await expect(repository({ context_window: null }).resolveEnabledText(MODEL_ID)).rejects.toMatchObject({
      code: "model_incompatible",
    });
    await expect(repository({ tokenizer_profile_key: null }).resolveEnabledText(MODEL_ID)).rejects.toMatchObject({
      code: "tokenizer_profile_unavailable",
    });
  });

  it("returns the explicit behavior evidence without defaults", async () => {
    await expect(repository().resolveEnabledText(MODEL_ID)).resolves.toMatchObject({
      maxOutputTokens: 4096,
      contextWindow: 128000,
      tokenizerProfileKey: "openai.o200k",
      tokenizerProfileVersion: "1.0.0",
    });
  });

  it("does not infer context behavior from settings or an opaque model name", async () => {
    await expect(repository({
      context_window: null,
      upstream_model: "gpt-5.6-luna",
      settings: { context_window: 128000 },
    }).resolveEnabledText(MODEL_ID)).rejects.toMatchObject({ code: "model_incompatible" });
  });

  it("rejects unknown, revoked, and protocol-incompatible profiles", async () => {
    await expect(repository({ tokenizer_profile_key: "unknown" }).resolveEnabledText(MODEL_ID))
      .rejects.toMatchObject({ code: "tokenizer_profile_unavailable" });
    await expect(repository({}, "revoked").resolveEnabledText(MODEL_ID))
      .rejects.toMatchObject({ code: "tokenizer_profile_unavailable" });
    await expect(repository({ api_protocol: "unsupported" }).resolveEnabledText(MODEL_ID))
      .rejects.toMatchObject({ code: "model_incompatible" });
  });
});
