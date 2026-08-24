import assert from "node:assert/strict";
import { createHash } from "node:crypto";
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
  "asset-center",
  "workflow-draft",
  "workflow-version",
  "published-workflow-version",
  "timeline-document",
  "creative-configuration",
  "asset-bible",
  "asset-edit",
  "timeline-current",
  "timeline-version",
  "project-package",
  "export-artifact",
  "export-diagnostic-target",
  "episode-export-batch",
  "media-inspection",
  "media-derivative",
  "preview-artifact",
];

const readJson = async (path) => JSON.parse(await readFile(path, "utf8"));
const canonicalize = (value) => {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, canonicalize(value[key])]),
    );
  }
  return value;
};
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

test("provides the phase-one Draft 2020-12 schemas", async () => {
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

  test(`${name} requires canonical identity fields`, async () => {
    const { ajv, value } = await validateExample(name, "valid");
    const validate = ajv.getSchema(
      `https://video-agent.local/schemas/${name}.schema.json`,
    );
    if (name !== "asset-center") {
      const withoutId = structuredClone(value);
      delete withoutId.id;
      assert.equal(validate(withoutId), false);
    }
    const withoutVersion = structuredClone(value);
    delete withoutVersion.schema_version;
    assert.equal(validate(withoutVersion), false);
  });
}

test("AssetCenter rejects storage locations and a second schema source", async () => {
  const { ajv, value } = await validateExample("asset-center", "valid");
  const validate = ajv.getSchema(
    "https://video-agent.local/schemas/asset-center.schema.json",
  );
  for (const forbidden of [
    "objectKey",
    "workspaceUri",
    "presignedUrl",
    "bytes",
    "base64",
  ]) {
    const leaked = structuredClone(value);
    leaked[forbidden] = "forbidden";
    assert.equal(validate(leaked), false, forbidden);
  }
  const alias = structuredClone(value);
  alias.schemaVersion = alias.schema_version;
  assert.equal(validate(alias), false);
});

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

test("PublishedWorkflowVersion fixes the default graph and explicit ports", async () => {
  const { ajv, value } = await validateExample(
    "published-workflow-version",
    "valid",
  );
  const validate = ajv.getSchema(
    "https://video-agent.local/schemas/published-workflow-version.schema.json",
  );
  assert.equal(value.templateKey, "drama-mvp-a-default");
  assert.equal(
    value.contentHash,
    createHash("sha256")
      .update(JSON.stringify(canonicalize(value.definition)))
      .digest("hex"),
  );
  assert.deepEqual(
    value.definition.nodes.map((node) => node.key),
    [
      "text.generate",
      "text.review",
      "media.generate.image",
      "media.review.image",
      "media.generate.video",
      "media.review.video",
      "media.inspect",
      "timeline.handoff",
    ],
  );
  for (const property of ["ports", "key"]) {
    const inferredGraph = structuredClone(value);
    delete inferredGraph.definition.nodes[0][property];
    assert.equal(validate(inferredGraph), false, property);
  }
  const editableGraph = structuredClone(value);
  editableGraph.definition.nodes[0].ports.output = "client-inferred.output";
  assert.equal(validate(editableGraph), false);
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

test("ProjectPackage rejects missing audit facts, aliases, and a second schema source", async () => {
  const { ajv, value } = await validateExample("project-package", "valid");
  const validate = ajv.getSchema(
    "https://video-agent.local/schemas/project-package.schema.json",
  );
  for (const property of [
    "authorization",
    "license",
    "loudness",
    "models",
    "skillRevisions",
    "parameters",
    "usage",
    "cost",
  ]) {
    const missing = structuredClone(value);
    delete missing[property];
    assert.equal(validate(missing), false, property);
  }
  const unknownWithoutSource = structuredClone(value);
  unknownWithoutSource.cost.source = "";
  assert.equal(validate(unknownWithoutSource), false);
  for (const property of ["models", "skillRevisions"]) {
    const empty = structuredClone(value);
    empty[property] = [];
    assert.equal(validate(empty), false, `${property} empty`);
  }
  for (const alias of ["profile", "export_profile", "schemaVersion"]) {
    const withAlias = structuredClone(value);
    withAlias[alias] = alias === "schemaVersion" ? "2.0.0" : "light";
    assert.equal(validate(withAlias), false, alias);
  }
  const portable = structuredClone(value);
  portable.exportProfile = "portable";
  assert.equal(validate(portable), false);
  for (const payloadField of ["bytes", "base64", "blob", "mediaPayload"]) {
    const embedded = structuredClone(value);
    embedded[payloadField] = "forbidden";
    assert.equal(validate(embedded), false, payloadField);
  }
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

test("AssetBible covers every owner fact and rejects copied owner payload", async () => {
  const { ajv, value } = await validateExample("asset-bible", "valid");
  const validate = ajv.getSchema(
    "https://video-agent.local/schemas/asset-bible.schema.json",
  );
  for (const property of ["objectKey", "url", "promptText", "bytes"]) {
    const copiedOwnerPayload = structuredClone(value);
    copiedOwnerPayload.versions[0][property] = "forbidden";
    assert.equal(validate(copiedOwnerPayload), false, property);
  }
  const staleHash = structuredClone(value);
  staleHash.assignments[0].entryVersionHash = "not-a-hash";
  assert.equal(validate(staleHash), false);
  const unknownTaskState = structuredClone(value);
  unknownTaskState.revisionTasks[0].status = "deleted";
  assert.equal(validate(unknownTaskState), false);
});
