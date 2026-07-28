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
const productionCandidateContractPath = resolve(root, "fixtures/production-crew-candidate-registry-contract-v3.json");
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
const productionCandidateRoles = [
  "producer", "screenwriter", "character_critic", "director", "cinematographer",
  "performance_director", "sound_director", "editor", "qc",
];
const nonBlankString = { type: "string", minLength: 1 };
const uuidString = { type: "string", format: "uuid" };
const stringArray = (minItems = 0) => ({
  type: "array", minItems, uniqueItems: true, items: nonBlankString,
});
const strictObject = (required, properties) => ({
  type: "object", additionalProperties: false, required, properties,
});
const productionOutputSchemas = {
  producer: strictObject(["creative_brief"], {
    creative_brief: strictObject(
      ["target_audience", "tone", "key_messages", "constraints", "success_criteria"],
      {
        target_audience: nonBlankString, tone: stringArray(1), key_messages: stringArray(1),
        constraints: { type: "object" }, success_criteria: stringArray(1),
      },
    ),
  }),
  screenwriter: strictObject(["story_bible", "character_bibles", "script_draft"], {
    story_bible: strictObject(["premise", "theme", "narrative_structure", "world"], {
      premise: nonBlankString, theme: nonBlankString, narrative_structure: nonBlankString, world: nonBlankString,
    }),
    character_bibles: {
      type: "array", minItems: 1, items: strictObject(
        ["character_id", "name", "role", "personality", "motivation", "arc"],
        { character_id: nonBlankString, name: nonBlankString, role: nonBlankString, personality: nonBlankString, motivation: nonBlankString, arc: nonBlankString },
      ),
    },
    script_draft: strictObject(["title", "hook", "scenes"], {
      title: nonBlankString,
      hook: nonBlankString,
      scenes: {
        type: "array", minItems: 3, maxItems: 12,
        items: strictObject(
          ["sequence", "narration", "visual_description", "emotion", "duration_sec", "character_ids"],
          {
            sequence: { type: "integer", minimum: 1 }, narration: nonBlankString,
            visual_description: nonBlankString, emotion: nonBlankString,
            duration_sec: { type: "integer", minimum: 1, maximum: 30 }, character_ids: stringArray(),
          },
        ),
      },
    }),
  }),
  director: strictObject(["directorial_treatment", "shot_contracts"], {
    directorial_treatment: strictObject(
      ["visual_style", "pacing", "emotional_arc", "color_palette", "reference_works"],
      {
        visual_style: nonBlankString, pacing: nonBlankString, emotional_arc: nonBlankString,
        color_palette: stringArray(1), reference_works: stringArray(),
      },
    ),
    shot_contracts: {
      type: "array", minItems: 1,
      items: strictObject(
        ["shot_id", "sequence", "scene_id", "shot_type", "camera_movement", "duration_sec", "description", "character_ids"],
        {
          shot_id: nonBlankString, sequence: { type: "integer", minimum: 1 }, scene_id: uuidString,
          shot_type: nonBlankString, camera_movement: nonBlankString,
          duration_sec: { type: "integer", minimum: 1, maximum: 30 }, description: nonBlankString,
          character_ids: stringArray(),
        },
      ),
    },
  }),
  cinematographer: strictObject(["collaboration_suggestions"], {
    collaboration_suggestions: {
      type: "array",
      items: strictObject(
        ["target_artifact_id", "target_artifact_version", "suggestion_type", "content", "priority", "blocking", "rationale"],
        {
          target_artifact_id: uuidString, target_artifact_version: { type: "integer", minimum: 1 },
          suggestion_type: { enum: ["revision", "addition", "deletion"] }, content: nonBlankString,
          priority: { enum: ["low", "medium", "high"] }, blocking: { type: "boolean" }, rationale: nonBlankString,
        },
      ),
    },
  }),
  character_critic: strictObject(["collaboration_suggestions"], {
    collaboration_suggestions: {
      type: "array",
      items: strictObject(
        ["target_artifact_id", "target_artifact_version", "suggestion_type", "content", "priority", "blocking", "rationale"],
        {
          target_artifact_id: uuidString, target_artifact_version: { type: "integer", minimum: 1 },
          suggestion_type: { enum: ["revision", "addition", "deletion"] }, content: nonBlankString,
          priority: { enum: ["low", "medium", "high"] }, blocking: { type: "boolean" }, rationale: nonBlankString,
        },
      ),
    },
  }),
  performance_director: strictObject(["performance_briefs"], {
    performance_briefs: {
      type: "array", minItems: 1,
      items: strictObject(
        ["character_bible_id", "character_id", "script_id", "emotional_arc", "body_language", "vocal_direction"],
        {
          character_bible_id: uuidString, character_id: nonBlankString, script_id: uuidString,
          emotional_arc: {
            type: "array", minItems: 1,
            items: strictObject(["sequence", "scene_id", "emotion", "intensity", "notes"], {
              sequence: { type: "integer", minimum: 1 }, scene_id: uuidString, emotion: nonBlankString,
              intensity: { type: "integer", minimum: 1, maximum: 10 }, notes: nonBlankString,
            }),
          },
          body_language: nonBlankString, vocal_direction: nonBlankString,
        },
      ),
    },
  }),
  sound_director: strictObject(["sound_plan"], {
    sound_plan: strictObject(["script_id", "music_style", "scene_sound_notes"], {
      script_id: uuidString, music_style: nonBlankString,
      scene_sound_notes: {
        type: "array", minItems: 1,
        items: strictObject(["sequence", "scene_id", "music_cue", "sfx_notes", "dialogue_direction"], {
          sequence: { type: "integer", minimum: 1 }, scene_id: uuidString, music_cue: nonBlankString,
          sfx_notes: stringArray(), dialogue_direction: nonBlankString,
        }),
      },
    }),
  }),
  editor: strictObject(["continuity_ledgers"], {
    continuity_ledgers: {
      type: "array", minItems: 1,
      items: strictObject(
        ["order", "shot_contract_id", "work_version_id", "inventory_id", "evidence_snapshot_id", "visual_facts", "continuity_flags"],
        {
          order: { type: "integer", minimum: 1 }, shot_contract_id: uuidString, work_version_id: uuidString,
          inventory_id: uuidString, evidence_snapshot_id: uuidString, visual_facts: stringArray(1),
          continuity_flags: stringArray(),
        },
      ),
    },
  }),
  qc: strictObject(["take_reviews"], {
    take_reviews: {
      type: "array", minItems: 1,
      items: strictObject(
        ["required_take_id", "work_version_id", "inventory_id", "evidence_snapshot_id", "applicable_shot_contract_ids", "review_status", "quality_assessment", "issues", "suggestions"],
        {
          required_take_id: uuidString, work_version_id: uuidString, inventory_id: uuidString,
          evidence_snapshot_id: uuidString,
          applicable_shot_contract_ids: { type: "array", minItems: 1, uniqueItems: true, items: uuidString },
          review_status: { enum: ["approved", "needs_revision", "rejected"] },
          quality_assessment: { type: "object", minProperties: 1, additionalProperties: { type: "number", minimum: 0, maximum: 10 } },
          issues: stringArray(), suggestions: stringArray(),
        },
      ),
    },
  }),
};
const productionCandidatePoliciesV2 = productionCandidateRoles.map((role) => ({
  policy_key: `production.${role}.execute.baseline`,
  version: "2.0.0",
  status: "candidate",
  executor_owners: ["rust"],
  allowed_sources: ["project", "script_revision_command", "user_instruction"],
  required_sources: ["project", "script_revision_command", "user_instruction"],
  stable_sort: ["priority", "source_kind", "source_id", "source_version", "candidate_id"],
}));
const productionCandidatePolicies = productionCandidateRoles.map((role) => ({
  policy_key: `production.${role}.execute.baseline`,
  version: "3.0.0",
  status: "candidate",
  executor_owners: ["rust"],
  allowed_sources: ["project", "script_revision_command", "user_instruction"],
  required_sources: ["project", "user_instruction"],
  stable_sort: ["priority", "source_kind", "source_id", "source_version", "candidate_id"],
}));
const productionCandidatePrompts = productionCandidateRoles.map((role) => {
  const active = activePrompts.find((prompt) => prompt.prompt_key === `production.${role}.general`);
  if (!active) throw new Error(`missing active production prompt for ${role}`);
  return {
    ...active,
    version: "3.0.0",
    status: "candidate",
    system_template: `templates/production/${role}.full-crew-v3.system.txt`,
    output_schema: {
      name: `production_${role}_output_v3`,
      strict: true,
      schema: productionOutputSchemas[role],
    },
  };
});
const productionCandidateAgents = productionCandidateRoles.map((role) => {
  const active = activeAgents.find((agent) => agent.agent_key === `production.${role}`);
  if (!active) throw new Error(`missing active production agent for ${role}`);
  const nodes = Object.fromEntries(Object.keys(active.nodes).map((node) => [node, {
    key: `production.${role}.general`,
    version: "3.0.0",
    context_policy: { key: `production.${role}.execute.baseline`, version: "3.0.0" },
  }]));
  const mediaRequirements = ["editor", "qc"].includes(role)
    ? { ...active.model_requirements, vision: true }
    : active.model_requirements;
  return {
    ...active,
    version: "3.0.0",
    status: "candidate",
    constraints: [
      ...active.constraints,
      "只允许引用当前 ProductionRun 输入快照中的真实领域 ID 和版本",
      "不得绕过 package Gate、调用媒体 provider 或修改其他角色的产物",
    ],
    model_requirements: mediaRequirements,
    nodes,
  };
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
  agents: [...v1Agents, ...activeAgents, ...productionCandidateAgents],
  prompts: [...v1Prompts, ...activePrompts, ...productionCandidatePrompts],
  context_policies: [...policies, ...productionCandidatePoliciesV2, ...productionCandidatePolicies]
    .sort((left, right) => left.policy_key.localeCompare(right.policy_key, "en") || left.version.localeCompare(right.version, "en")),
  tokenizer_profiles: tokenizerProfiles,
};
const baselineDigest = sha256(await readFile(baselinePath));
const contextFixtureDigest = sha256(await readFile(contextFixturePath));
const productionCandidateContractDigest = sha256(await readFile(productionCandidateContractPath));
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
  ...productionCandidateAgents.map((agent) => evidence("agent", agent.agent_key, agent.version, agent,
    "agent-definitions/fixtures/production-crew-candidate-registry-contract-v3.json", productionCandidateContractDigest)),
  ...productionCandidatePrompts.map((prompt) => evidence("prompt", prompt.prompt_key, prompt.version, prompt,
    "agent-definitions/fixtures/production-crew-candidate-registry-contract-v3.json", productionCandidateContractDigest)),
  ...registry.context_policies.map((policy) => evidence("context_policy", policy.policy_key, policy.version, policy,
    policy.status === "candidate"
      ? "agent-definitions/fixtures/production-crew-candidate-registry-contract-v3.json"
      : "agent-definitions/fixtures/context-contract.json",
    policy.status === "candidate" ? productionCandidateContractDigest : contextFixtureDigest)),
  ...tokenizerProfiles.map((profile) => evidence("tokenizer_profile", profile.profile_key, profile.version, profile,
    "agent-definitions/tokenizers/encoding-contract-v1.json", tokenizerAssetDigest)),
];
const release = { schema_version: "2", registry_digest: sha256(canonicalJson(registry)), releases };
await writeFile(registryPath, `${JSON.stringify(registry, null, 2)}\n`);
await writeFile(releasePath, `${JSON.stringify(release, null, 2)}\n`);
