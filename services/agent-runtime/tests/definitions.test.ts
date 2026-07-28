import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";

import { describe, expect, it } from "vitest";

import {
  activeAgent,
  assertProductionExecutionIntegrity,
  behaviorFingerprint,
  canonicalJson,
  compilePrompt,
  compilePromptForReplay,
  definitionDigest,
  finalizePrompt,
  loadDefinitionRegistry,
  preparePrompt,
  readPromptSnapshot,
  sha256Hex,
  validateModelCapabilities,
  type ModelBehavior,
} from "../src/definitions.js";
import { compileContext, type ContextPayload } from "../src/context.js";

const ROOT = resolve(import.meta.dirname, "../../..");
const DEFINITIONS = resolve(ROOT, "agent-definitions");

async function fixture(name: string): Promise<Record<string, unknown>> {
  return JSON.parse(await readFile(resolve(DEFINITIONS, "fixtures", name), "utf8")) as Record<string, unknown>;
}

async function loadFixtureRegistry(
  name: string,
  mutate?: (document: Record<string, unknown>) => void,
  releaseDigest?: string,
) {
  const document = structuredClone(await fixture(name));
  mutate?.(document);
  const prompts = document.prompts as Array<{ system_template: string; user_template: string }>;
  const directory = await mkdtemp(resolve(tmpdir(), "novex-definition-fixture-"));
  await mkdir(resolve(directory, "templates"));
  await mkdir(resolve(directory, "fixtures"));
  await writeFile(resolve(directory, "registry.json"), JSON.stringify(document));
  await copyFile(
    resolve(DEFINITIONS, "fixtures/context-eval-contract.json"),
    resolve(directory, "fixtures/context-eval-contract.json"),
  );
  for (const prompt of prompts) {
    for (const relative of [prompt.system_template, prompt.user_template]) {
      await copyFile(resolve(DEFINITIONS, relative), resolve(directory, relative));
    }
  }
  await writeFile(resolve(directory, "release-index.json"), JSON.stringify({
    schema_version: document.schema_version, registry_digest: releaseDigest ?? sha256Hex(canonicalJson(document)), releases: [],
  }));
  try {
    return await loadDefinitionRegistry(directory);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}

describe("versioned definition contracts", () => {
  it("loads the same registry and canonical digest as Rust", async () => {
    const registry = await loadDefinitionRegistry(DEFINITIONS);
    expect(registry.agents).toHaveLength(30);
    expect(registry.prompts).toHaveLength(44);
    expect(registry.context_policies).toHaveLength(45);
    expect(registry.tokenizer_profiles).toHaveLength(3);
    expect(registry.releases).toHaveLength(122);
    expect(registry.releases).toHaveLength(
      registry.agents.length
        + registry.prompts.length
        + registry.context_policies.length
        + registry.tokenizer_profiles.length,
    );
    expect(activeAgent(registry, "personal.general").executor_owner).toBe("pi");
    expect(Object.values(activeAgent(registry, "personal.general").nodes).every((node) => node.context_policy)).toBe(true);
    expect(() => assertProductionExecutionIntegrity(registry)).not.toThrow();
    const release = JSON.parse(await readFile(resolve(DEFINITIONS, "release-index.json"), "utf8")) as { registry_digest: string };
    expect(registry.digest).toBe(release.registry_digest);

    const canonical = await fixture("canonical.json");
    expect(canonicalJson(canonical.input)).toBe(canonical.canonical);
    expect(sha256Hex(canonicalJson(canonical.input))).toBe(canonical.sha256);
  });

  it("compiles dynamic content only into User and rejects undeclared nodes", async () => {
    const registry = await loadDefinitionRegistry(DEFINITIONS);
    const snapshot = compilePrompt(registry, "personal.general", "1.0.0", "personal.turn", {
      schema_version: "1",
      fragments: [{ id: "message-1", trust: "user_instruction", source: "pi_entry", content: "dynamic user text" }],
    }, "chat");
    expect(snapshot.system).not.toContain("dynamic user text");
    expect(snapshot.user).toBe("dynamic user text");
    expect(snapshot.fragments[0]).toMatchObject({ id: "message-1", source: "pi_entry" });
    const asset = {
      asset_id: "image-fixture",
      version: "1",
      sha256: "c7d09a3d7f1f0f7b80f149553682168e1a8182478fba69963f8f54d9f9a714f0",
      mime: "image/png",
    };
    expect(compilePrompt(registry, "personal.general", "1.0.0", "personal.turn", {
      schema_version: "1", fragments: [{ id: "asset-1", trust: "reference", source: "asset_store", asset }],
    }, "chat").fragments[0]?.asset).toEqual(asset);
    expect(() => compilePrompt(registry, "personal.general", "1.0.0", "personal.turn", {
      schema_version: "1", fragments: [{ id: "asset-2", trust: "reference", source: "asset_store", asset: { ...asset, sha256: "short" } }],
    }, "chat")).toThrow("asset reference format is invalid");
    expect(() => compilePrompt(registry, "personal.general", "1.0.0", "personal.unknown", {
      schema_version: "1", fragments: [],
    }, "chat")).toThrow("node personal.unknown is not declared");
  });

  it("allows revoked definitions only through the dry-run replay compiler", async () => {
    const registry = await loadFixtureRegistry("registry-valid.json", (document) => {
      (document.agents as Array<Record<string, unknown>>)[0]!.status = "revoked";
      (document.prompts as Array<Record<string, unknown>>)[0]!.status = "revoked";
    });
    const input = {
      schema_version: "1" as const,
      fragments: [{ id: "history-1", trust: "user_instruction" as const, source: "model_call", content: "history" }],
    };
    expect(() => compilePrompt(registry, "fixture.agent", "1.0.0", "fixture.node", input, "chat"))
      .toThrow("not executable");
    expect(compilePromptForReplay(registry, "fixture.agent", "1.0.0", "fixture.node", input, "chat"))
      .toMatchObject({ user: "history", agent_key: "fixture.agent", prompt_key: "fixture.prompt" });
    await expect(loadFixtureRegistry("registry-valid.json", (document) => {
      (document.prompts as Array<Record<string, unknown>>)[0]!.status = "revoked";
    })).rejects.toThrow("executable agent references unavailable prompt");
  });

  it("keeps content digests stable across lifecycle switches and never upgrades bound versions", async () => {
    const source = await fixture("registry-valid.json");
    const original = (source.agents as Array<Record<string, unknown>>)[0]!;
    const activeDigest = definitionDigest(original as any);
    const registry = await loadFixtureRegistry("registry-valid.json", (document) => {
      const agents = document.agents as Array<Record<string, any>>;
      const prompts = document.prompts as Array<Record<string, any>>;
      agents[0]!.status = "supported";
      prompts[0]!.status = "supported";
      agents.push(structuredClone(agents[0]!));
      prompts.push(structuredClone(prompts[0]!));
      agents[1]!.version = "2.0.0";
      agents[1]!.status = "active";
      agents[1]!.nodes["fixture.node"].version = "2.0.0";
      prompts[1]!.version = "2.0.0";
      prompts[1]!.status = "active";
    });
    expect(activeAgent(registry, "fixture.agent").version).toBe("2.0.0");
    const input = {
      schema_version: "1" as const,
      fragments: [{ id: "bound-v1", trust: "user_instruction" as const, source: "session_binding", content: "keep v1" }],
    };
    expect(compilePrompt(registry, "fixture.agent", "1.0.0", "fixture.node", input, "chat"))
      .toMatchObject({ agent_version: "1.0.0", prompt_version: "1.0.0" });

    const revokedRegistry = await loadFixtureRegistry("registry-valid.json", (document) => {
      (document.agents as Array<Record<string, unknown>>)[0]!.status = "revoked";
      (document.prompts as Array<Record<string, unknown>>)[0]!.status = "revoked";
    });
    expect(definitionDigest(revokedRegistry.agents[0]!)).toBe(activeDigest);
    expect(() => compilePrompt(revokedRegistry, "fixture.agent", "1.0.0", "fixture.node", input, "chat"))
      .toThrow("not executable");
    expect(compilePromptForReplay(revokedRegistry, "fixture.agent", "1.0.0", "fixture.node", input, "chat"))
      .toMatchObject({ agent_version: "1.0.0" });
  });

  it("normalizes model behavior and excludes credential rotation", async () => {
    const current = await fixture("fingerprint.json");
    const input = current.input as ModelBehavior;
    const result = behaviorFingerprint(input);
    expect(result.normalized).toEqual(current.normalized);
    expect(result.digest).toBe(current.sha256);
    expect(behaviorFingerprint({
      ...input,
      settings: { temperature: 0, api_key: "rotated", nested: { authorization: "Bearer secret", access_token: "secret" } },
    }).digest).toBe(result.digest);
    const mutations: ModelBehavior[] = [
      { ...input, protocol: "openai_chat_completions" },
      { ...input, request_base_url: "https://example.com/v2" },
      { ...input, upstream_model: "different-model" },
      { ...input, reasoning_effort: "high" },
      { ...input, max_output_tokens: input.max_output_tokens + 1 },
      { ...input, context_window: input.context_window + 1 },
      { ...input, tokenizer_profile_key: "openai.cl100k" },
      { ...input, tokenizer_profile_version: "2.0.0" },
      { ...input, settings: { temperature: 0.5 } },
    ];
    for (const changed of mutations) expect(behaviorFingerprint(changed).digest).not.toBe(result.digest);
    expect(() => behaviorFingerprint({ ...input, protocol: "unknown" })).toThrow("incomplete");
  });

  it("fails closed when required model capabilities cannot be proven", () => {
    const requirements = {
      text: true, tool_calling: true, structured_output: true,
      vision: false, reasoning: false, min_context_window: 8192,
    };
    const available = {
      text: true, tool_calling: true, structured_output: true,
      vision: false, reasoning: false, context_window: 128000,
    };
    expect(() => validateModelCapabilities(requirements, available)).not.toThrow();
    expect(() => validateModelCapabilities(requirements, { ...available, tool_calling: false })).toThrow("tool_calling");
    expect(() => validateModelCapabilities(requirements, { ...available, context_window: 4096 })).toThrow("context window");
  });

  it("validates shared success/failure schema fixtures", async () => {
    await expect(loadFixtureRegistry("registry-valid.json")).resolves.toMatchObject({ agents: [{ agent_key: "fixture.agent" }] });
    await expect(loadFixtureRegistry("registry-invalid-owner.json")).rejects.toThrow("cross-owner");
    await expect(loadFixtureRegistry("registry-invalid-unknown-field.json")).rejects.toThrow("unknown field");
    await expect(loadFixtureRegistry("registry-valid.json", (document) => {
      const agents = document.agents as unknown[];
      agents.push(structuredClone(agents[0]));
    })).rejects.toThrow("duplicate agent");
    await expect(loadFixtureRegistry("registry-valid.json", undefined, "0".repeat(64))).rejects.toThrow("release index does not match");
  });

  it("loads Registry v2 and rejects missing, cross-owner, duplicate and unknown Context contracts", async () => {
    await expect(loadFixtureRegistry("registry-valid-v2.json")).resolves.toMatchObject({
      context_policies: [{ policy_key: "fixture.policy" }], tokenizer_profiles: [{ profile_key: "fixture.profile" }],
    });
    await expect(loadFixtureRegistry("registry-valid-v2.json", (document) => {
      delete (document.agents as Array<any>)[0]!.nodes["fixture.node"].context_policy;
    })).rejects.toThrow("missing context policy");
    await expect(loadFixtureRegistry("registry-valid-v2.json", (document) => {
      (document.context_policies as Array<any>)[0]!.executor_owners = ["pi"];
    })).rejects.toThrow("incompatible context policy");
    await expect(loadFixtureRegistry("registry-valid-v2.json", (document) => {
      const profiles = document.tokenizer_profiles as unknown[]; profiles.push(structuredClone(profiles[0]));
    })).rejects.toThrow("duplicate tokenizer profile");
    await expect(loadFixtureRegistry("registry-valid-v2.json", (document) => {
      (document.context_policies as Array<any>)[0]!.unknown = true;
    })).rejects.toThrow("unknown field");
  });

  it("supports declared variable types and rejects invalid compiler inputs", async () => {
    const registry = await loadFixtureRegistry("registry-valid-variables.json");
    const input = {
      schema_version: "1" as const,
      variables: { title: "合同测试", tags: ["alpha", "beta"], item_count: 2, metadata: { z: 2, a: 1 } },
      fragments: [{ id: "reference-1", trust: "reference" as const, source: "shared_fixture", content: "动态参考" }],
    };
    const tools = [{ name: "read" }, { name: "write" }, { name: "edit" }, { name: "bash" }];
    const snapshot = compilePrompt(registry, "fixture.variables", "1.0.0", "fixture.variables", input, "workspace", tools);
    expect(snapshot.system).toBe("固定 System，不允许动态变量。");
    expect(snapshot.user).toContain("标题：合同测试");
    expect(snapshot.user).toContain("标签：alpha\nbeta");
    expect(snapshot.user).toContain('元数据：{"a":1,"z":2}');
    expect(snapshot.user).toContain("片段：动态参考");
    expect(snapshot.fragments[0]).toMatchObject({ trust: "reference", source: "shared_fixture" });
    expect(snapshot.output_schema).toMatchObject({ strict: true });
    expect((snapshot.output_schema as any).schema.properties.items).toMatchObject({ minItems: 2, maxItems: 2 });
    expect(snapshot.tool_schema).toEqual(tools);
    expect(snapshot.tool_profile).toBe("workspace");
    expect(Object.isFrozen(snapshot)).toBe(true);
    expect(Object.isFrozen(snapshot.fragments)).toBe(true);
    expect(Object.isFrozen(snapshot.variables.metadata)).toBe(true);

    expect(() => compilePrompt(registry, "fixture.variables", "1.0.0", "fixture.variables", {
      ...input, variables: { tags: ["alpha"] },
    }, "workspace", tools)).toThrow("title is required");
    expect(() => compilePrompt(registry, "fixture.variables", "1.0.0", "fixture.variables", {
      ...input, variables: { ...input.variables, unknown: true },
    }, "workspace", tools)).toThrow("unknown variable");
    expect(() => compilePrompt(registry, "fixture.variables", "1.0.0", "fixture.variables", {
      ...input, variables: { ...input.variables, tags: ["ok", 2] },
    }, "workspace", tools)).toThrow("string_list");
    expect(() => compilePrompt(registry, "fixture.variables", "1.0.0", "fixture.variables", {
      ...input, variables: { ...input.variables, item_count: 2.5 },
    }, "workspace", tools)).toThrow("integer");
    expect(() => compilePrompt(registry, "fixture.variables", "1.0.0", "fixture.variables", {
      ...input, variables: { ...input.variables, title: "x".repeat(33) },
    }, "workspace", tools)).toThrow("max_bytes");
    expect(() => compilePrompt(registry, "fixture.variables", "1.0.0", "fixture.variables", input, "workspace", [{ name: "read" }])).toThrow("does not match");
    expect(() => compilePrompt(registry, "fixture.variables", "9.0.0", "fixture.variables", input, "workspace", tools)).toThrow("not executable");
  });

  it("preserves an under-budget legacy compile output through prepare, ContextCompiler and finalize", async () => {
    const registry = await loadDefinitionRegistry(DEFINITIONS);
    const legacy = compilePrompt(registry, "video.project-strategy", "2.0.0", "project.strategy_draft", {
      schema_version: "1", fragments: [{ id: "legacy", trust: "reference", source: "migration_baseline_fixture", content: "golden-user-input" }],
    }, "chat");
    const prepared = preparePrompt(registry, "video.project-strategy", "2.0.0", "project.strategy_draft", {}, "chat", null, 2_000);
    const payload: ContextPayload = { type: "text", text: "golden-user-input" };
    const context = compileContext({
      schema_version: "2", owner: "rust", owner_id: "owner", node_key: "project.strategy_draft", compiled_at: "2026-07-25T00:00:00Z",
      model_context_window: 8192,
      policy: registry.context_policies.find((item) => item.policy_key === "project.strategy_draft.baseline")!,
      tokenizer_profile: registry.tokenizer_profiles.find((item) => item.profile_key === "byte-upper-bound")!,
      prepared_prompt: prepared.envelope,
      candidates: [{ candidate_id: "golden", source_kind: "user_instruction", source_id: "golden", source_version: "1",
        trust: "user_instruction", priority: "p0", required: true, render_order: 0, observed_at: "2026-07-25T00:00:00Z", supersedes: [],
        content_hash: sha256Hex(canonicalJson(payload)), payload }], atomic_groups: [],
    });
    const finalized = finalizePrompt(prepared, "context-snapshot-1", context,
      registry.tokenizer_profiles.find((item) => item.profile_key === "byte-upper-bound")!);
    expect(finalized.promptSnapshot).toMatchObject({ schema_version: "2", system: legacy.system, user: legacy.user,
      output_schema: legacy.output_schema, fragments: [], context_digest: finalized.contextSnapshot.digest });
    expect(finalized.promptSnapshot.context_digest).toBe(finalized.contextSnapshot.digest);
    expect(finalized.promptSnapshot.logical_input?.messages[0]?.content).toBe(legacy.user);
    expect(finalized.promptSnapshot.logical_input).toEqual(finalized.contextSnapshot.logical_input);
    expect(readPromptSnapshot(JSON.parse(JSON.stringify(legacy)))).toEqual(legacy);
    expect(readPromptSnapshot(JSON.parse(JSON.stringify(finalized.promptSnapshot)))).toEqual(finalized.promptSnapshot);
  });

  it("prepares fixed templates, workspace tools and output contracts without dynamic content", async () => {
    const registry = await loadDefinitionRegistry(DEFINITIONS);
    const tools = [{ name: "read" }, { name: "write" }, { name: "edit" }, { name: "bash" }];
    const prepared = preparePrompt(registry, "personal.general", "2.0.0", "personal.turn", {}, "workspace", tools, 2_048);
    expect(prepared.envelope).toMatchObject({
      system: expect.any(String),
      user_template_fixed: "",
      tool_schema: tools,
      output_schema: null,
      max_output_tokens: 2_048,
    });
    expect(prepared.envelope.system.length).toBeGreaterThan(0);
  });

  it("preserves every active under-budget node through the governed two-phase compiler", async () => {
    const registry = await loadDefinitionRegistry(DEFINITIONS);
    const tokenizer = registry.tokenizer_profiles.find((item) => item.profile_key === "byte-upper-bound")!;
    let checked = 0;
    for (const agent of registry.agents.filter((item) => item.status === "active")) {
      for (const [nodeKey, reference] of Object.entries(agent.nodes)) {
        const prompt = registry.prompts.find((item) => item.prompt_key === reference.key && item.version === reference.version)!;
        const variables = nodeKey === "script.complete" ? { scene_count: 3 } : {};
        const toolProfile = prompt.tool_profile ?? "chat";
        const prepared = preparePrompt(registry, agent.agent_key, agent.version, nodeKey, variables, toolProfile, null, 4_096);
        const trust = prompt.variables.find((item) => item.value_type === "fragments")!.trust;
        const legacy = compilePrompt(registry, agent.agent_key, agent.version, nodeKey, {
          schema_version: "1", variables,
          fragments: [{ id: "legacy", trust, source: "migration_baseline_fixture", content: "golden-user-input" }],
        }, toolProfile);
        const policyRef = reference.context_policy!;
        const policy = registry.context_policies.find((item) => item.policy_key === policyRef.key && item.version === policyRef.version)!;
        const payload: ContextPayload = { type: "text", text: "golden-user-input" };
        const compiled = compileContext({
          schema_version: "2", owner: agent.executor_owner, owner_id: `golden-${nodeKey}`, node_key: nodeKey,
          compiled_at: "2026-07-25T00:00:00Z", model_context_window: 1_000_000, policy,
          tokenizer_profile: tokenizer, prepared_prompt: prepared.envelope,
          candidates: (policy.required_sources.length > 0 ? policy.required_sources : [policy.allowed_sources[0]!])
            .map((sourceKind, index) => ({
              candidate_id: `golden-${sourceKind}`, source_kind: sourceKind, source_id: `golden-${sourceKind}`, source_version: "1",
              trust, priority: "p0" as const, required: true, render_order: index,
              observed_at: "2026-07-25T00:00:00Z", supersedes: [],
              content_hash: sha256Hex(canonicalJson(payload)), payload,
            })), atomic_groups: [],
        });
        const finalized = finalizePrompt(prepared, `snapshot-${nodeKey}`, compiled, tokenizer);
        expect(finalized.promptSnapshot, nodeKey).toMatchObject({
          system: legacy.system,
          user: legacy.user,
          output_schema: legacy.output_schema,
          max_output_tokens: legacy.max_output_tokens,
          fragments: [],
        });
        checked += 1;
      }
    }
    expect(checked).toBe(27);
  });
});
