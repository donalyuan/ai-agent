import { describe, expect, it } from "vitest";

import { ModelConfigRepository } from "../src/models.js";

const MODEL_ID = "11111111-1111-4111-8111-111111111111";

function repository(overrides: Record<string, unknown> = {}): ModelConfigRepository {
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
    settings: { context_window: 128000 },
    ...overrides,
  };
  const pool = {
    query: async () => ({ rows: [row] }),
  };
  return new ModelConfigRepository(pool as never);
}

describe("Pi text model resolution", () => {
  it("requires explicit output and context limits", async () => {
    await expect(repository({ max_output_tokens: null }).resolveEnabledText(MODEL_ID)).rejects.toMatchObject({
      code: "model_incompatible",
    });
    await expect(repository({ settings: {} }).resolveEnabledText(MODEL_ID)).rejects.toMatchObject({
      code: "model_incompatible",
    });
  });

  it("returns the explicit behavior evidence without defaults", async () => {
    await expect(repository().resolveEnabledText(MODEL_ID)).resolves.toMatchObject({
      maxOutputTokens: 4096,
      contextWindow: 128000,
    });
  });
});
