import { readFile, readdir } from "node:fs/promises";
import { resolve } from "node:path";

import { describe, expect, it } from "vitest";

const SOURCE = resolve(import.meta.dirname, "../src");
const DEFINITIONS = resolve(import.meta.dirname, "../../../agent-definitions/registry.json");

describe("Pi public composition boundary", () => {
  it("keeps AgentHarness construction inside the Novex wrapper and rejects private integration patterns", async () => {
    const files = (await readdir(SOURCE)).filter((name) => name.endsWith(".ts"));
    const sources = await Promise.all(files.map(async (name) => [name, await readFile(resolve(SOURCE, name), "utf8")] as const));
    const harnessConstructors = sources.filter(([, source]) => source.includes("new AgentHarness"));
    expect(harnessConstructors.map(([name]) => name)).toEqual(["novex-harness.ts"]);
    expect(sources.find(([name]) => name === "audited-models.ts")?.[1]).toContain("class AuditedModels");
    expect(sources.find(([name]) => name === "models.ts")?.[1]).toContain("createAuditedModels(createModels())");

    for (const [name, source] of sources) {
      for (const forbidden of [
        "@earendil-works/pi-agent-core/dist/",
        "@earendil-works/pi-ai/dist/",
        ".prototype.",
        "monkeyPatch",
        "privateFields",
        "copyAgentLoop",
        "extends AgentHarness",
      ]) {
        expect(source, `${name} contains forbidden Pi integration token ${forbidden}`).not.toContain(forbidden);
      }
    }
  });

  it("uses only the approved public hook allowlist", async () => {
    const wrapper = await readFile(resolve(SOURCE, "novex-harness.ts"), "utf8");
    const hooks = [...wrapper.matchAll(/\.on\("([a-z_]+)"/g)].map((match) => match[1]);
    expect(hooks.sort()).toEqual([
      "after_provider_response",
      "before_agent_start",
      "before_provider_payload",
      "before_provider_request",
      "context",
      "tool_call",
    ]);
  });

  it("keeps one versioned and audited production path for every Pi node", async () => {
    const files = (await readdir(SOURCE)).filter((name) => name.endsWith(".ts"));
    const sources = await Promise.all(files.map(async (name) => [name, await readFile(resolve(SOURCE, name), "utf8")] as const));
    const registry = JSON.parse(await readFile(DEFINITIONS, "utf8")) as {
      agents: Array<{ agent_key: string; executor_owner: string; status: string; nodes: Record<string, unknown> }>;
    };
    const personal = registry.agents.find((agent) => agent.agent_key === "personal.general" && agent.status === "active");
    expect(personal?.executor_owner).toBe("pi");
    expect(Object.keys(personal?.nodes ?? {}).sort()).toEqual([
      "personal.branch_summary",
      "personal.compaction",
      "personal.tool_followup",
      "personal.turn",
    ]);

    const wrapper = sources.find(([name]) => name === "novex-harness.ts")?.[1] ?? "";
    for (const nodeKey of Object.keys(personal?.nodes ?? {})) expect(wrapper).toContain(`"${nodeKey}"`);
    expect(wrapper).toContain("prepareModelCallWithContext");
    expect(wrapper).toContain("compileContext(");
    expect(wrapper).toContain("return { messages:");
    for (const forbidden of [
      "queuedFragments",
      "canonicalJson(redactUnknown(this.context",
      "compilePrompt(",
      "prepareModelCall({",
      "pi_context_hook",
    ]) expect(wrapper).not.toContain(forbidden);

    for (const [name, source] of sources) {
      if (name !== "sessions.ts" && name !== "coordinator.ts") expect(source).not.toMatch(/\bsystem_prompt\b/);
      for (const forbidden of [
        "USE_LEGACY_PROMPT",
        "ENABLE_LEGACY_LLM",
        "USE_AUDITED_MODEL",
        "ENABLE_VERSIONED_PROMPT",
      ]) expect(source, `${name} contains a forbidden dual-track flag`).not.toContain(forbidden);
    }
  });
});
