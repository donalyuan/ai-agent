import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

import { describe, expect, it } from "vitest";

describe("shared Context audit read API contract", () => {
  it("fixes cross-runtime pagination, summary, detail and export fields", async () => {
    const contract = JSON.parse(await readFile(resolve(
      import.meta.dirname,
      "../../../agent-definitions/fixtures/context-audit-read-api.json",
    ), "utf8")) as Record<string, string[]> & { schema_version: string };
    expect(contract.schema_version).toBe("2");
    expect(contract.source_runtimes).toEqual(["rust", "pi"]);
    expect(contract.record_types).toEqual(["snapshot", "compile_attempt"]);
    expect(contract.list_envelope_fields).toEqual([
      "schema_version", "source_runtime", "items", "total", "limit", "offset",
    ]);
    expect(contract.detail_envelope_fields).toEqual([
      "schema_version", "source_runtime", "record_hash", "record",
    ]);
    expect(contract.summary_fields).not.toContain("decisions");
    expect(contract.summary_fields).not.toContain("logical_input");
    expect(contract.snapshot_record_fields).toEqual(expect.arrayContaining(["decisions", "logical_input"]));
  });
});
