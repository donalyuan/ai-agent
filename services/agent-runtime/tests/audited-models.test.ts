import {
  createModels,
  type Context,
  type ModelsSimpleStreamOptions,
} from "@earendil-works/pi-ai";
import {
  fauxAssistantMessage,
  fauxProvider,
  type FauxResponseFactory,
} from "@earendil-works/pi-ai/providers/faux";
import { describe, expect, it } from "vitest";

import { createAuditedModels } from "../src/audited-models.js";

const CONTEXT: Context = { systemPrompt: "fixture", messages: [] };

describe("audited Models", () => {
  it("forces every public provider call to disable transparent retries", async () => {
    const faux = fauxProvider();
    const inner = createModels();
    inner.setProvider(faux.provider);
    const models = createAuditedModels(inner);
    const seen: Array<number | undefined> = [];
    const response: FauxResponseFactory = (_context, options) => {
      seen.push(options?.maxRetries);
      return fauxAssistantMessage("ok");
    };
    const callerOptions = { maxRetries: 3 } satisfies ModelsSimpleStreamOptions;

    faux.setResponses([response]);
    await models.stream(faux.getModel(), CONTEXT, callerOptions).result();
    faux.setResponses([response]);
    await models.complete(faux.getModel(), CONTEXT, callerOptions);
    faux.setResponses([response]);
    await models.streamSimple(faux.getModel(), CONTEXT, callerOptions).result();
    faux.setResponses([response]);
    await models.completeSimple(faux.getModel(), CONTEXT, callerOptions);

    expect(seen).toEqual([0, 0, 0, 0]);
  });

  it("leaves a second response untouched after the first provider error", async () => {
    const faux = fauxProvider();
    const inner = createModels();
    inner.setProvider(faux.provider);
    const models = createAuditedModels(inner);
    let observedRetries: number | undefined;

    faux.setResponses([
      (_context, options) => {
        observedRetries = options?.maxRetries;
        return fauxAssistantMessage("", { stopReason: "error", errorMessage: "fixture failure" });
      },
      fauxAssistantMessage("must remain pending"),
    ]);

    const result = await models.completeSimple(faux.getModel(), CONTEXT, { maxRetries: 5 });

    expect(result.stopReason).toBe("error");
    expect(observedRetries).toBe(0);
    expect(faux.state.callCount).toBe(1);
    expect(faux.getPendingResponseCount()).toBe(1);
  });
});
