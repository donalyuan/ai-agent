import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

import { describe, expect, it } from "vitest";

const ROOT = resolve(import.meta.dirname, "../../..");

describe("governed context migration baseline", () => {
  it("shares deterministic tokenizer, atomic tool and safety fixtures with Rust", async () => {
    const fixture = JSON.parse(await readFile(
      resolve(ROOT, "agent-definitions/fixtures/context-contract.json"), "utf8",
    )) as Record<string, any>;
    expect(fixture.schema_version).toBe("2");
    expect(fixture.tokenizer_cases.map((item: any) => item.id)).toEqual([
      "ascii", "chinese", "emoji", "json", "reasoning",
    ]);
    expect(fixture.context_cases.atomic_tool_group).toEqual({
      group_id: "tool-call-1", request_id: "tool-request-1", result_id: "tool-result-1",
    });
    expect(Object.values(fixture.external_effects)).toEqual([0, 0, 0, 0]);
  });

  it("shares the Context Eval candidate and zero-cost production-node contract with Rust", async () => {
    const contract = JSON.parse(await readFile(
      resolve(ROOT, "agent-definitions/fixtures/context-eval-contract.json"), "utf8",
    )) as Record<string, any>;
    expect(contract.schema_version).toBe("1");
    expect(contract.definition_kinds).toEqual(["context_policy", "tokenizer_profile"]);
    expect(contract.required_gates).toEqual([
      "schema", "cross_language_token", "determinism", "safety", "budget",
      "core_prompt", "business_output", "baseline_equivalence",
    ]);
    expect(contract.production_nodes).toHaveLength(18);
    expect(contract.baseline_report).toMatchObject({
      report_id: "context-production-baseline@1",
      mode: "golden_baseline",
      passed: true,
      actual_real_model_calls: 0,
    });
    expect(contract.baseline_report.node_results).toHaveLength(18);
    expect(contract.baseline_report.node_results.every((item: any) => item.equivalent)).toBe(true);
    expect(contract.tokenizer_metrics.rust_tokens).toBe(contract.tokenizer_metrics.typescript_tokens);
    expect(Object.values(contract.external_effects)).toEqual([0, 0, 0, 0]);
  });

  it("uses the public context hook without retaining the migration-era blob path", async () => {
    const wrapper = await readFile(resolve(ROOT, "services/agent-runtime/src/novex-harness.ts"), "utf8");
    expect(wrapper).toContain('this.harness.on("context"');
    expect(wrapper).toContain('this.harness.on("before_provider_request"');
    expect(wrapper).toContain("prepareModelCallWithContext");
    expect(wrapper).not.toContain("canonicalJson(redactUnknown(this.context");
    expect(wrapper).not.toContain("queuedFragments");
  });
});
