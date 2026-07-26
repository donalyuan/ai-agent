import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

import { canonicalJson, type TrustLevel } from "../src/definitions.js";
import {
  compileContext, ContextCompileError, ENCODING_CONTRACT_V1_DIGEST, finalizeContext, ProfileTokenizer,
  type ContextCandidate, type ContextCompileRequest, type ContextPayload, type ContextPriority, type TokenizerMode,
} from "../src/context.js";

function hash(payload: ContextPayload): string { return createHash("sha256").update(canonicalJson(payload)).digest("hex"); }
function candidate(id: string, text: string, trust: TrustLevel = "reference", priority: ContextPriority = "p2", required = false): ContextCandidate {
  const payload: ContextPayload = { type: "text", text };
  const renderOrder = { p0: 0, p1: 1, p2: 2, p3: 3, p4: 4 }[priority];
  return { candidate_id: id, source_kind: "fixture", source_id: id, source_version: "1", trust, priority, required,
    render_order: renderOrder, observed_at: "2026-07-25T00:00:00Z", supersedes: [], content_hash: hash(payload), payload };
}
function request(candidates: ContextCandidate[], mode: TokenizerMode = { mode: "conservative", algorithm: "utf8-byte-upper-bound@1" }): ContextCompileRequest {
  return { schema_version: "2", owner: "pi", owner_id: "session-1", node_key: "personal.turn", compiled_at: "2026-07-25T00:00:00Z",
    model_context_window: 256, policy: { policy_key: "fixture.policy", version: "1.0.0", status: "active", executor_owners: ["pi"],
      allowed_sources: ["fixture"], required_sources: [], stable_sort: ["priority", "source_kind", "source_id", "source_version", "candidate_id"] },
    tokenizer_profile: { profile_key: "fixture.profile", version: "1.0.0", status: "active", implementation_version: "1", mode,
      applicable_protocols: ["openai_responses"], applicable_model_families: ["fixture"],
      framing: { per_message_tokens: 3, per_tool_tokens: 4, request_tokens: 3, reply_priming_tokens: 3 }, safety_reserve_tokens: 16 },
    prepared_prompt: { system: "fixed system", user_template_fixed: "", tool_schema: [], output_schema: null,
      protocol_envelope_tokens: 3, max_output_tokens: 32 }, candidates, atomic_groups: [] };
}

describe("governed ContextCompiler", () => {
  it("is deterministic across input order and keeps fixed output reserve", () => {
    const p0 = candidate("instruction", "current user instruction", "user_instruction", "p0", true);
    const p1 = candidate("fact", "confirmed project fact", "confirmed_fact", "p1");
    const p2 = candidate("p2-candidate", "candidate at p2", "candidate", "p2");
    const p3 = candidate("p3-platform", "platform at p3", "platform", "p3");
    const p4 = candidate("candidate", "untrusted draft", "candidate", "p4");
    const first = compileContext(request([p4, p3, p1, p2, p0]));
    const second = compileContext(request([p0, p2, p4, p1, p3]));
    expect(first.digest).toBe(second.digest);
    expect(first.selected_order).toEqual(["instruction", "fact", "p2-candidate", "p3-platform", "candidate"]);
    expect(first.budget.final_input_tokens + first.budget.max_output_tokens + first.budget.safety_reserve_tokens)
      .toBeLessThanOrEqual(first.budget.model_context_window);
  });

  it("keeps rendering order independent from budget priority", () => {
    const instruction = { ...candidate("instruction", "current user instruction", "user_instruction", "p0", true), render_order: 1 };
    const fact = { ...candidate("fact", "confirmed project fact", "confirmed_fact", "p1", true), render_order: 0 };
    const compiled = compileContext(request([instruction, fact]));
    expect(compiled.selected_order).toEqual(["fact", "instruction"]);
    expect(compiled.logical_input.messages.map((message) => message.content))
      .toEqual(["confirmed project fact", "current user instruction"]);
  });

  it("accounts for every fixed budget component and profile framing", () => {
    const compiled = compileContext(request([]));
    expect(compiled.budget).toMatchObject({
      system_prompt_tokens: 12,
      user_template_fixed_tokens: 0,
      tool_schema_tokens: 2,
      output_schema_tokens: 0,
      protocol_envelope_tokens: 15,
      max_output_tokens: 32,
      safety_reserve_tokens: 16,
      dynamic_context_budget: 179,
      final_input_tokens: 0,
    });
    const withTools = request([]);
    withTools.prepared_prompt.tool_schema = [{ name: "read" }, { name: "write" }];
    expect(compileContext(withTools).budget.protocol_envelope_tokens)
      .toBe(compiled.budget.protocol_envelope_tokens + 8);
  });

  it("rejects a real BPE boundary overflow during final LogicalModelInput recheck without reselection", () => {
    const input = request([candidate("required", "A", "user_instruction", "p0", true)], {
      mode: "exact", encoding: "cl100k_base", asset_digest: ENCODING_CONTRACT_V1_DIGEST,
    });
    input.model_context_window = 15;
    input.prepared_prompt = { system: "", user_template_fixed: "aa", tool_schema: null, output_schema: null,
      protocol_envelope_tokens: 0, max_output_tokens: 1 };
    input.tokenizer_profile.safety_reserve_tokens = 0;
    const compiled = compileContext(input);
    expect(compiled.selected_order).toEqual(["required"]);
    expect(() => finalizeContext(compiled, input.tokenizer_profile, {
      system: "", messages: [{ role: "user", content: "aAa" }], tool_schema: null, output_schema: null,
    })).toThrow(expect.objectContaining({ stage: "finalize", code: "context_budget_exceeded" }));
    expect(compiled.selected_order).toEqual(["required"]);
  });

  it("fails closed for confirmed fact conflicts, required overflow and invalid hash", () => {
    const left = { ...candidate("fact-a", "A", "confirmed_fact", "p1", true), fact_key: "project.target" };
    const right = { ...candidate("fact-b", "B", "confirmed_fact", "p1", true), fact_key: "project.target" };
    expect(() => compileContext(request([left, right]))).toThrow(expect.objectContaining({ code: "context_conflict" }));
    const oversized = candidate("required", "中".repeat(200), "user_instruction", "p0", true);
    const oversizedRequest = request([oversized]);
    let overflow: ContextCompileError | undefined;
    try { compileContext(oversizedRequest); } catch (error) { overflow = error as ContextCompileError; }
    expect(overflow).toMatchObject({ code: "context_budget_exceeded" });
    expect(overflow!.attempt(oversizedRequest)).toMatchObject({ budget: expect.any(Object), decisions: [] });
    expect(JSON.stringify(overflow!.attempt(oversizedRequest))).not.toContain("中");
    oversized.content_hash = "0".repeat(64);
    expect(() => compileContext(request([oversized]))).toThrow(expect.objectContaining({ code: "context_content_hash_mismatch" }));
  });

  it("keeps tool request/result atomic and failure attempts payload-free", () => {
    const toolRequest = { ...candidate("tool-request", '{"call":"1"}', "reference", "p0", true), atomic_group_id: "tool-1" };
    const toolResult = { ...candidate("tool-result", '{"result":"ok"}', "reference", "p0", true), atomic_group_id: "tool-1" };
    const input = request([toolRequest, toolResult]);
    input.atomic_groups = [{ group_id: "tool-1", member_ids: ["tool-request", "tool-result"] }];
    expect(compileContext(input).selected_order).toEqual(["tool-request", "tool-result"]);
    input.atomic_groups[0]!.member_ids.pop();
    try { compileContext(input); } catch (error) {
      expect(error).toBeInstanceOf(ContextCompileError);
      const attempt = (error as ContextCompileError).attempt(input);
      expect(attempt).toMatchObject({ code: "context_atomic_group_invalid", decisions: [] });
      expect(JSON.stringify(attempt)).not.toContain("result");
    }
  });

  it("excludes incomplete atomic groups together and fails when the remainder is required", () => {
    const left = { ...candidate("tool-request", "request"), atomic_group_id: "tool-1", valid_until: "2026-07-24T00:00:00Z" };
    const right = { ...candidate("tool-result", "result"), atomic_group_id: "tool-1" };
    const input = request([left, right]);
    input.atomic_groups = [{ group_id: "tool-1", member_ids: ["tool-request", "tool-result"] }];
    const snapshot = compileContext(input);
    expect(snapshot.selected_order).toEqual([]);
    expect(snapshot.decisions).toContainEqual(expect.objectContaining({
      candidate_id: "tool-result", decision: "atomic_group_excluded",
    }));

    expect(() => compileContext({ ...input, candidates: [left, { ...right, required: true }] }))
      .toThrow(expect.objectContaining({ code: "required_context_unavailable" }));
    const { atomic_group_id: _omitted, ...withoutGroup } = right;
    expect(() => compileContext({ ...input, candidates: [left, withoutGroup] }))
      .toThrow(expect.objectContaining({ code: "context_atomic_group_invalid" }));
  });

  it("validates assets, timestamps, supersedes and canonical object order", () => {
    const assetPayload: ContextPayload = { type: "asset", asset: {
      asset_id: "asset-1", version: "1", sha256: "a".repeat(64), mime: "image/png", metadata: {},
    } };
    const asset = { ...candidate("asset", "placeholder"), payload: assetPayload, content_hash: hash(assetPayload) };
    expect(() => compileContext(request([asset]))).not.toThrow();
    const invalidPayload: ContextPayload = { type: "asset", asset: { ...assetPayload.asset, mime: "" } };
    expect(() => compileContext(request([{ ...asset, payload: invalidPayload, content_hash: hash(invalidPayload) }])))
      .toThrow(expect.objectContaining({ code: "context_schema_invalid" }));
    expect(() => compileContext(request([{ ...asset, observed_at: "not-a-time" }])))
      .toThrow(expect.objectContaining({ code: "context_schema_invalid" }));
    expect(() => compileContext(request([{ ...asset, supersedes: ["missing"] }])))
      .toThrow(expect.objectContaining({ code: "context_schema_invalid" }));
    expect(() => compileContext({ ...request([asset]), owner: "rust" }))
      .toThrow(expect.objectContaining({ code: "context_schema_invalid" }));
    expect(() => compileContext(request([{ ...asset, source_kind: "denied" }])))
      .toThrow(expect.objectContaining({ code: "context_schema_invalid" }));
    const requiredSource = request([asset]);
    requiredSource.policy.required_sources = ["fixture"];
    expect(() => compileContext(requiredSource)).toThrow(expect.objectContaining({ code: "context_schema_invalid" }));

    const firstPayload: ContextPayload = { type: "message", message: { role: "user", content: { a: 1, b: 2 } } };
    const secondPayload: ContextPayload = { type: "message", message: { role: "user", content: { b: 2, a: 1 } } };
    const first = { ...candidate("message", "placeholder"), payload: firstPayload, content_hash: hash(firstPayload) };
    const second = { ...first, payload: secondPayload, content_hash: hash(secondPayload) };
    expect(compileContext(request([first])).digest).toBe(compileContext(request([second])).digest);
  });

  it("uses exact tokenizers and a byte upper bound without chars/4 fallback", () => {
    const exact = ProfileTokenizer.create(request([], { mode: "exact", encoding: "cl100k_base", asset_digest: ENCODING_CONTRACT_V1_DIGEST }).tokenizer_profile);
    expect(exact.countText("中文 🔒 JSON")).toBeGreaterThan(0);
    const conservative = ProfileTokenizer.create(request([]).tokenizer_profile);
    expect(conservative.countText("中")).toBe(3);
    const invalid = request([]).tokenizer_profile as any;
    invalid.mode = { mode: "conservative", algorithm: "chars/4" };
    expect(() => ProfileTokenizer.create(invalid)).toThrow("tokenizer_profile_unavailable");
  });

  it("matches every token count in the shared Rust/TypeScript encoding asset", () => {
    const fixture = JSON.parse(readFileSync(resolve(
      import.meta.dirname, "../../../agent-definitions/tokenizers/encoding-contract-v1.json",
    ), "utf8")) as { cases: Array<{ id: string; text: string; cl100k_base: number; o200k_base: number }> };
    for (const encoding of ["cl100k_base", "o200k_base"] as const) {
      const tokenizer = ProfileTokenizer.create(request([], { mode: "exact", encoding, asset_digest: ENCODING_CONTRACT_V1_DIGEST }).tokenizer_profile);
      for (const item of fixture.cases) expect(tokenizer.countText(item.text), `${encoding}/${item.id}`).toBe(item[encoding]);
    }
  });

  it("bounds the exact tokenizer cache by profile version and asset digest", () => {
    for (let version = 0; version < 12; version += 1) {
      const profile = request([], { mode: "exact", encoding: "cl100k_base", asset_digest: ENCODING_CONTRACT_V1_DIGEST }).tokenizer_profile;
      profile.version = `1.0.${version}`;
      ProfileTokenizer.create(profile);
    }
    expect(ProfileTokenizer.cacheSize()).toBeLessThanOrEqual(8);
    const invalid = request([], { mode: "exact", encoding: "cl100k_base", asset_digest: "0".repeat(64) }).tokenizer_profile;
    expect(() => ProfileTokenizer.create(invalid)).toThrow("tokenizer_profile_unavailable");
  });

  it("matches the shared Rust/TypeScript ContextSnapshot digest", () => {
    const fixture = JSON.parse(readFileSync(resolve(
      import.meta.dirname, "../../../agent-definitions/fixtures/context-compile-contract-v1.json",
    ), "utf8")) as { request: ContextCompileRequest; final_logical_input: import("../src/context.js").LogicalModelInput;
      expected_digest: string; expected_snapshot_digest: string; expected_schema_attempt_digest: string };
    const compiled = compileContext(fixture.request);
    expect(compiled.digest).toBe(fixture.expected_digest);
    expect(finalizeContext(compiled, fixture.request.tokenizer_profile, fixture.final_logical_input).digest)
      .toBe(fixture.expected_snapshot_digest);
    const invalid = structuredClone(fixture.request);
    invalid.candidates[0]!.content_hash = "0".repeat(64);
    let failure: ContextCompileError | undefined;
    try { compileContext(invalid); } catch (error) { failure = error as ContextCompileError; }
    expect(failure!.attempt(invalid).digest).toBe(fixture.expected_schema_attempt_digest);
  });

  it("has no model, Tool or business repository dependency", () => {
    const source = readFileSync(resolve(import.meta.dirname, "../src/context.ts"), "utf8");
    for (const forbidden of ["LLMClient", "ModelCallRepository", "TopicRepository", "ProjectRepository", "executeTool("]) {
      expect(source).not.toContain(forbidden);
    }
  });
});
