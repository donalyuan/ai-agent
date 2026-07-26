import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const registryPath = resolve(root, "registry.json");
const releasePath = resolve(root, "release-index.json");
const publishedV1Path = resolve(root, "fixtures/registry-published-v1.json");
const deployedV1Path = resolve(root, "fixtures/registry-deployed-20260724-v1.json");
const baselinePath = resolve(root, "../backend/tests/fixtures/versioned_prompt_baseline.json");
const contextFixturePath = resolve(root, "fixtures/context-contract.json");
const tokenizerAssetPath = resolve(root, "tokenizers/encoding-contract-v1.json");

const canonicalJson = (value) => JSON.stringify(normalize(value));
function normalize(value) {
  if (Array.isArray(value)) return value.map(normalize);
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(Object.entries(value).sort(([left], [right]) => left.localeCompare(right, "en"))
      .map(([key, item]) => [key, normalize(item)]));
  }
  return value;
}
const sha256 = (value) => createHash("sha256").update(value).digest("hex");
const definitionDigest = (definition) => {
  const { status: _status, ...content } = definition;
  return sha256(canonicalJson(content));
};

const original = JSON.parse(await readFile(registryPath, "utf8"));
if (original.schema_version !== "1" && original.schema_version !== "2") throw new Error("unsupported source registry");
const publishedV1 = JSON.parse(await readFile(publishedV1Path, "utf8"));
const historicalV1Registries = [
  {
    reference: "agent-definitions/fixtures/registry-published-v1.json",
    digest: "a52b93f818d8d96767712ee413ee58e6d657cceeb588602808a646165536ceb3",
    registry: publishedV1,
  },
  {
    reference: "agent-definitions/fixtures/registry-deployed-20260724-v1.json",
    digest: "24530ff72d97caee088bd941acb0531840f0daa410df7f900be6ee0050c372bb",
    registry: JSON.parse(await readFile(deployedV1Path, "utf8")),
  },
];
for (const historical of historicalV1Registries) {
  if (historical.registry.schema_version !== "1" || sha256(canonicalJson(historical.registry)) !== historical.digest) {
    throw new Error(`historical v1 registry evidence changed: ${historical.reference}`);
  }
}
const v1Agents = publishedV1.agents.map((agent) => ({ ...agent, status: "supported" }));
const sourceAgents = original.agents.some((agent) => agent.version === "2.0.0")
  ? original.agents.filter((agent) => agent.version === "2.0.0")
  : original.agents.filter((agent) => agent.version === "1.0.0");
const sourcePrompts = original.prompts.some((prompt) => prompt.version === "2.0.0")
  ? original.prompts.filter((prompt) => prompt.version === "2.0.0")
  : original.prompts.filter((prompt) => prompt.version === "1.0.0");
const v1Prompts = publishedV1.prompts.map((prompt) => ({ ...prompt, status: "supported" }));
const activePrompts = sourcePrompts.map((prompt) => ({ ...prompt, version: "2.0.0", status: "active" }));
const sources = [
  "account_strategy", "asset_store", "conversation_entry", "current_script", "current_work", "existing_topic",
  "migration_baseline_fixture", "pi_branch_entry", "pi_compaction", "pi_follow_up", "pi_steer", "pi_tool_exchange",
  "project", "script_scene", "topic_batch", "topic_candidate", "user_instruction", "voice_catalog", "work_manifest",
];
const policies = [];
const activeAgents = sourceAgents.map((agent) => {
  const nodes = Object.fromEntries(Object.entries(agent.nodes).map(([node, prompt]) => {
    const policyKey = `${node}.baseline`;
    policies.push({
      policy_key: policyKey,
      version: "1.0.0",
      status: "active",
      executor_owners: [agent.executor_owner],
      allowed_sources: sources,
      required_sources: ["user_instruction"],
      stable_sort: ["priority", "source_kind", "source_id", "source_version", "candidate_id"],
    });
    return [node, { key: prompt.key, version: "2.0.0", context_policy: { key: policyKey, version: "1.0.0" } }];
  }));
  return { ...agent, version: "2.0.0", status: "active", nodes };
});
const tokenizerAssetDigest = sha256(await readFile(tokenizerAssetPath));
const framing = { per_message_tokens: 3, per_tool_tokens: 4, request_tokens: 3, reply_priming_tokens: 3 };
const tokenizerProfiles = [
  {
    profile_key: "openai.cl100k", version: "1.0.0", status: "active", implementation_version: "tiktoken-rs=0.12.0;js-tiktoken=1.0.21",
    mode: { mode: "exact", encoding: "cl100k_base", asset_digest: tokenizerAssetDigest },
    applicable_protocols: ["openai_responses", "openai_chat_completions"], applicable_model_families: ["operator-confirmed-cl100k"], framing, safety_reserve_tokens: 256,
  },
  {
    profile_key: "openai.o200k", version: "1.0.0", status: "active", implementation_version: "tiktoken-rs=0.12.0;js-tiktoken=1.0.21",
    mode: { mode: "exact", encoding: "o200k_base", asset_digest: tokenizerAssetDigest },
    applicable_protocols: ["openai_responses", "openai_chat_completions"], applicable_model_families: ["operator-confirmed-o200k"], framing, safety_reserve_tokens: 256,
  },
  {
    profile_key: "byte-upper-bound", version: "1.0.0", status: "active", implementation_version: "utf8-byte-upper-bound@1",
    mode: { mode: "conservative", algorithm: "utf8-byte-upper-bound@1" },
    applicable_protocols: ["openai_responses", "openai_chat_completions"], applicable_model_families: ["operator-confirmed-byte-level-bpe"], framing, safety_reserve_tokens: 512,
  },
];
const registry = {
  schema_version: "2",
  agents: [...v1Agents, ...activeAgents],
  prompts: [...v1Prompts, ...activePrompts],
  context_policies: policies.sort((left, right) => left.policy_key.localeCompare(right.policy_key, "en")),
  tokenizer_profiles: tokenizerProfiles,
};
const baselineDigest = sha256(await readFile(baselinePath));
const contextFixtureDigest = sha256(await readFile(contextFixturePath));
const evidence = (definition_kind, definition_key, definition_version, definition, reference, digest, legacy_digests = []) => ({
  definition_kind, definition_key, definition_version, definition_digest: definitionDigest(definition),
  ...(legacy_digests.length > 0 ? { legacy_digests } : {}),
  activation_evidence: { type: "golden_baseline", reference, sha256: digest },
});
const publishedEvidence = (collection, keyField, definition) => {
  const legacyDigests = historicalV1Registries.map((historical) => {
    const published = historical.registry[collection].find((item) => item[keyField] === definition[keyField]
      && item.version === definition.version);
    if (!published) {
      throw new Error(`missing historical v1 evidence for ${collection} ${definition[keyField]}@${definition.version}`);
    }
    return {
      algorithm: "canonical-json-with-status@1",
      registry_digest: historical.digest,
      definition_digest: sha256(canonicalJson(published)),
    };
  });
  return evidence(collection.slice(0, -1), definition[keyField], definition.version, definition,
    "agent-definitions/fixtures/registry-published-v1.json", "a52b93f818d8d96767712ee413ee58e6d657cceeb588602808a646165536ceb3", legacyDigests);
};
const releases = [
  ...v1Agents.map((agent) => publishedEvidence("agents", "agent_key", agent)),
  ...v1Prompts.map((prompt) => publishedEvidence("prompts", "prompt_key", prompt)),
  ...activeAgents.map((agent) => evidence("agent", agent.agent_key, agent.version, agent,
    "backend/tests/fixtures/versioned_prompt_baseline.json", baselineDigest)),
  ...activePrompts.map((prompt) => evidence("prompt", prompt.prompt_key, prompt.version, prompt,
    "backend/tests/fixtures/versioned_prompt_baseline.json", baselineDigest)),
  ...registry.context_policies.map((policy) => evidence("context_policy", policy.policy_key, policy.version, policy,
    "agent-definitions/fixtures/context-contract.json", contextFixtureDigest)),
  ...tokenizerProfiles.map((profile) => evidence("tokenizer_profile", profile.profile_key, profile.version, profile,
    "agent-definitions/tokenizers/encoding-contract-v1.json", tokenizerAssetDigest)),
];
const release = { schema_version: "2", registry_digest: sha256(canonicalJson(registry)), releases };
await writeFile(registryPath, `${JSON.stringify(registry, null, 2)}\n`);
await writeFile(releasePath, `${JSON.stringify(release, null, 2)}\n`);
