import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import Ajv2020 from "ajv/dist/2020.js";
import addFormats from "ajv-formats";

const packageRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const schemaDirectory = join(packageRoot, "schemas");
const exampleDirectory = join(packageRoot, "examples");
const schemaNames = [
  "project",
  "episode",
  "scene",
  "shot",
  "asset",
  "asset-version",
  "workflow-draft",
  "workflow-version",
  "timeline-document",
];

const readJson = async (path) => JSON.parse(await readFile(path, "utf8"));
const objectKeyCorpus = await readJson(
  join(packageRoot, "tests/fixtures/object-key-contract-corpus.json"),
);
const createAjv = () => {
  const ajv = new Ajv2020({ allErrors: true, strict: true });
  addFormats(ajv);
  return ajv;
};
const loadSchemas = async () => {
  const ajv = createAjv();
  const schemas = await Promise.all(
    schemaNames.map((name) =>
      readJson(join(schemaDirectory, `${name}.schema.json`)),
    ),
  );
  for (const schema of schemas) {
    assert.equal(ajv.validateSchema(schema), true, JSON.stringify(ajv.errors));
    ajv.addSchema(schema);
  }
  return ajv;
};
const validateExample = async (name, kind) => {
  const ajv = await loadSchemas();
  const value = await readJson(join(exampleDirectory, `${name}.${kind}.json`));
  const valid = ajv.validate(
    `https://video-agent.local/schemas/${name}.schema.json`,
    value,
  );
  return { ajv, value, valid };
};

test("provides exactly nine Draft 2020-12 schemas", async () => {
  const files = (await readdir(schemaDirectory)).filter((file) =>
    file.endsWith(".schema.json"),
  );
  assert.deepEqual(
    files.sort(),
    schemaNames.map((name) => `${name}.schema.json`).sort(),
  );

  const ajv = await loadSchemas();
  assert.equal(
    ajv.schemas["https://video-agent.local/schemas/project.schema.json"].schema
      .$schema,
    "https://json-schema.org/draft/2020-12/schema",
  );
});

for (const name of schemaNames) {
  test(`${name} accepts its valid example`, async () => {
    const { valid } = await validateExample(name, "valid");
    assert.equal(valid, true);
  });

  test(`${name} rejects its invalid example`, async () => {
    const { valid } = await validateExample(name, "invalid");
    assert.equal(valid, false);
  });

  test(`${name} requires id and schema_version`, async () => {
    const { ajv, value } = await validateExample(name, "valid");
    const validate = ajv.getSchema(
      `https://video-agent.local/schemas/${name}.schema.json`,
    );
    const withoutId = structuredClone(value);
    delete withoutId.id;
    assert.equal(validate(withoutId), false);
    const withoutVersion = structuredClone(value);
    delete withoutVersion.schema_version;
    assert.equal(validate(withoutVersion), false);
  });
}

test("hierarchy references use stable UUID contracts", async () => {
  const { ajv, value: episode } = await validateExample("episode", "valid");
  const validate = ajv.getSchema(
    "https://video-agent.local/schemas/episode.schema.json",
  );
  const withoutProjectReference = structuredClone(episode);
  delete withoutProjectReference.projectId;
  assert.equal(validate(withoutProjectReference), false);
  const invalidProjectReference = structuredClone(episode);
  invalidProjectReference.projectId = "not-a-uuid";
  assert.equal(validate(invalidProjectReference), false);
});

test("WorkflowDraft requires explicit non-empty scope", async () => {
  const { ajv, value } = await validateExample("workflow-draft", "valid");
  const validate = ajv.getSchema(
    "https://video-agent.local/schemas/workflow-draft.schema.json",
  );
  const withoutScopeType = structuredClone(value);
  delete withoutScopeType.scopeType;
  assert.equal(validate(withoutScopeType), false);
  const emptyScopeIds = structuredClone(value);
  emptyScopeIds.scopeIds = [];
  assert.equal(validate(emptyScopeIds), false);
});

test("TimelineDocument rejects non-integer frame values", async () => {
  const { ajv, value } = await validateExample("timeline-document", "valid");
  const validate = ajv.getSchema(
    "https://video-agent.local/schemas/timeline-document.schema.json",
  );
  const floatingFrame = structuredClone(value);
  floatingFrame.clips[0].timelineStartFrame = 1.5;
  assert.equal(validate(floatingFrame), false);
});

test("AssetVersion reads the shared objectKey corpus", async () => {
  const { ajv, value } = await validateExample("asset-version", "valid");
  const validate = ajv.getSchema(
    "https://video-agent.local/schemas/asset-version.schema.json",
  );
  for (const property of ["binary", "base64", "blob", "content"]) {
    const withPayload = structuredClone(value);
    withPayload[property] = "payload";
    assert.equal(validate(withPayload), false, property);
  }
  for (const objectKey of objectKeyCorpus.canonicalObjectKeys) {
    const withCanonicalPath = structuredClone(value);
    withCanonicalPath.storageObject.objectKey = objectKey;
    assert.equal(validate(withCanonicalPath), true, objectKey);
  }
  for (const objectKey of objectKeyCorpus.invalidObjectKeys) {
    const withUnsafePath = structuredClone(value);
    withUnsafePath.storageObject.objectKey = objectKey;
    assert.equal(validate(withUnsafePath), false, objectKey);
  }
});
