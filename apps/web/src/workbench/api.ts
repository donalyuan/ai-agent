import type { z } from "zod";
import {
  creativeProjectionSchema,
  episodeSchema,
  projectSchema,
  runDetailSchema,
  runEventSchema,
  sceneProjectionSchema,
  skillRouteDecisionSchema,
  textReviewBatchSchema,
  workflowVersionSchema,
} from "./contracts";
import { runStartCommandSchema } from "./owner-contracts";
import { traceHeaders } from "./trace-context";

export class OwnerApiError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "OwnerApiError";
  }
}

const ownerHeaders = (projectId?: string): HeadersInit => ({
  "Content-Type": "application/json",
  ...traceHeaders(),
  ...(projectId ? { "X-Project-Scope": projectId } : {}),
});

async function request(path: string, init?: RequestInit): Promise<unknown> {
  const headers = new Headers(traceHeaders());
  new Headers(init?.headers).forEach((value, key) => headers.set(key, value));
  const response = await fetch(`/api${path}`, { ...init, headers });
  const payload = (await response.json().catch(() => null)) as {
    detail?: { type?: string; message?: string };
    message?: string;
  } | null;
  if (!response.ok) {
    throw new OwnerApiError(
      response.status,
      payload?.detail?.type ?? `http_${response.status}`,
      payload?.detail?.message ??
        payload?.message ??
        `请求失败（${response.status}）`,
    );
  }
  return payload;
}

function camelKey(key: string): string {
  return key.replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase());
}

function normalizeOwnerPayload(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(normalizeOwnerPayload);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, child]) => [
        camelKey(key),
        normalizeOwnerPayload(child),
      ]),
    );
  }
  return value;
}

function parse<T>(schema: z.ZodType<T>, payload: unknown): T {
  const parsed = schema.safeParse(payload);
  if (!parsed.success) {
    throw new OwnerApiError(
      502,
      "owner_contract_invalid",
      parsed.error.issues.map((issue) => issue.message).join("；"),
    );
  }
  return parsed.data;
}

export const queryKeys = {
  projects: ["projects"] as const,
  project: (projectId: string) => ["projects", projectId] as const,
  episodes: (projectId: string) => ["projects", projectId, "episodes"] as const,
  creative: (projectId: string) => ["projects", projectId, "creative"] as const,
  storyboard: (projectId: string, episodeId: string) =>
    ["projects", projectId, "episodes", episodeId, "storyboard"] as const,
  workflow: (projectId: string) =>
    ["projects", projectId, "workflow", "published"] as const,
  run: (projectId: string, runId: string) =>
    ["projects", projectId, "runs", runId] as const,
  textReview: (projectId: string) =>
    ["projects", projectId, "text-review"] as const,
  bible: (projectId: string) => ["projects", projectId, "asset-bible"] as const,
  skillRoutes: (projectId: string) =>
    ["projects", projectId, "skill-routes"] as const,
};

export function localOfflineSelection(projectId: string) {
  return {
    selectionSnapshotId: `local-workspace:${projectId}`,
    provider: "mock" as const,
    providerId: "mock-provider",
    profile: "local-test-offline" as const,
    profileId: "local-test-offline",
    modelId: "mock-model",
    adapterKey: "mock" as const,
    adapterIdentity: "local_workspace" as const,
    profileRevision: 1,
    capabilitySnapshotId: "mock-capability",
    capabilityRevision: 1,
    capabilityOperation: "text.generate" as const,
    capabilitySnapshots: {
      "text.generate": { id: "mock-capability", revision: 1 },
    },
    skills: ["novel-writing", "drama-skills"],
    skillRevisionIds: ["novel-writing@1.0.0", "drama-skills@1.0.0"],
    skillDigests: [
      "63ad1206cdcd49aad86a96ffbd2d49f1b0b56a45cceea7cc09b190ceef0cbcce",
      "a7217641a828b8c52778b9c2e0a19772c0faa0e830bc57ffa48e362a9a748b9f",
    ],
    decision: "fixed" as const,
    decisionRevision: 1,
    routeStatus: "selected" as const,
    source: "explicit-local-profile" as const,
  };
}

export const workbenchApi = {
  async listProjects() {
    return parse(projectSchema.array(), await request("/v1/projects"));
  },
  async getProject(projectId: string) {
    return parse(projectSchema, await request(`/v1/projects/${projectId}`));
  },
  async createProject(name: string) {
    return parse(
      projectSchema,
      await request("/v1/projects", {
        method: "POST",
        headers: ownerHeaders(),
        body: JSON.stringify({ name }),
      }),
    );
  },
  async updateProject(
    projectId: string,
    name: string,
    expectedRevision: number,
  ) {
    return parse(
      projectSchema,
      await request(`/v1/projects/${projectId}`, {
        method: "PATCH",
        headers: { ...ownerHeaders(), "If-Match": String(expectedRevision) },
        body: JSON.stringify({ name }),
      }),
    );
  },
  async listEpisodes(projectId: string) {
    return parse(
      episodeSchema.array(),
      await request(`/v1/projects/${projectId}/episodes`, {
        headers: ownerHeaders(projectId),
      }),
    );
  },
  async getCreative(projectId: string) {
    return parse(
      creativeProjectionSchema,
      normalizeOwnerPayload(
        await request(`/v1/projects/${projectId}/creative`, {
          headers: ownerHeaders(projectId),
        }),
      ),
    );
  },
  async createSourceMaterial(
    projectId: string,
    materialType: string,
    inputMode: string,
  ) {
    return normalizeOwnerPayload(
      await request(`/v1/projects/${projectId}/source-materials`, {
        method: "POST",
        headers: ownerHeaders(projectId),
        body: JSON.stringify({ materialType, inputMode }),
      }),
    );
  },
  async appendSourceMaterial(
    sourceMaterialId: string,
    expectedRevision: number,
    inputMode: string,
    content: string | null,
    assetVersionId: string | null,
  ) {
    return normalizeOwnerPayload(
      await request(`/v1/source-materials/${sourceMaterialId}/versions`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          expectedRevision,
          inputMode,
          content,
          assetVersionId,
        }),
      }),
    );
  },
  async saveBrief(
    projectId: string,
    payload: Record<string, unknown>,
    expectedRevision: number,
  ) {
    return request(`/v1/projects/${projectId}/creative/brief`, {
      method: "PUT",
      headers: {
        ...ownerHeaders(projectId),
        "If-Match": String(expectedRevision),
      },
      body: JSON.stringify(payload),
    });
  },
  async getStoryboard(projectId: string, episodeId: string) {
    return parse(
      sceneProjectionSchema.array(),
      await request(
        `/v1/projects/${projectId}/episodes/${episodeId}/storyboard`,
        {
          headers: ownerHeaders(projectId),
        },
      ),
    );
  },
  async getWorkflow(projectId: string) {
    return parse(
      workflowVersionSchema,
      await request(`/v1/projects/${projectId}/workflow/default`, {
        headers: ownerHeaders(projectId),
      }),
    );
  },
  async reorderScenes(
    projectId: string,
    episodeId: string,
    sceneIds: string[],
    expectedRevision: number,
  ) {
    await request(
      `/v1/projects/${projectId}/episodes/${episodeId}/scenes/reorder`,
      {
        method: "POST",
        headers: ownerHeaders(projectId),
        body: JSON.stringify({ sceneIds, expectedRevision }),
      },
    );
    return this.getStoryboard(projectId, episodeId);
  },
  async reorderShots(
    projectId: string,
    episodeId: string,
    sceneId: string,
    shotIds: string[],
    expectedRevision: number,
  ) {
    await request(
      `/v1/projects/${projectId}/episodes/${episodeId}/scenes/${sceneId}/shots/reorder`,
      {
        method: "POST",
        headers: ownerHeaders(projectId),
        body: JSON.stringify({ shotIds, expectedRevision }),
      },
    );
    return this.getStoryboard(projectId, episodeId);
  },
  async ensureWorkflow(projectId: string) {
    return parse(
      workflowVersionSchema,
      await request(`/v1/projects/${projectId}/workflow/default/ensure`, {
        method: "POST",
        headers: ownerHeaders(projectId),
        body: JSON.stringify({ schemaVersion: "1.0.0" }),
      }),
    );
  },
  async startRun(
    projectId: string,
    workflowVersionId: string,
    bindingRevision: number,
    idempotencyKey: string,
    routeDecisionId: string,
  ) {
    const command = runStartCommandSchema.parse({
      workflowVersionId,
      nodeKeys: ["text.generate"],
      scopeRefs: [],
      ownerRefs: [],
      idempotencyKey,
      routeDecisionId,
      expectedBindingRevision: bindingRevision,
      schemaVersion: "1.0.0",
    });
    const created = normalizeOwnerPayload(
      await request(`/v1/projects/${projectId}/runs`, {
        method: "POST",
        headers: {
          ...ownerHeaders(projectId),
          "If-Match": String(bindingRevision),
        },
        body: JSON.stringify(command),
      }),
    ) as { id?: unknown };
    if (typeof created.id !== "string" || !created.id) {
      throw new OwnerApiError(
        502,
        "owner_contract_invalid",
        "Run create response 缺少稳定 id",
      );
    }
    return this.getRun(projectId, created.id);
  },
  async getRun(projectId: string, runId: string) {
    return parse(
      runDetailSchema,
      normalizeOwnerPayload(
        await request(`/v1/runs/${runId}`, {
          headers: ownerHeaders(projectId),
        }),
      ),
    );
  },
  async cancelRun(projectId: string, runId: string, expectedRevision: number) {
    await request(`/v1/runs/${runId}/cancel`, {
      method: "POST",
      headers: {
        ...ownerHeaders(projectId),
        "If-Match": String(expectedRevision),
      },
      body: JSON.stringify({ expectedRevision, schemaVersion: "1.0.0" }),
    });
    return this.getRun(projectId, runId);
  },
  async getRunEvents(projectId: string, runId: string, lastEventId = 0) {
    const response = await fetch(`/api/v1/runs/${runId}/events`, {
      headers: {
        "X-Project-Scope": projectId,
        "Last-Event-ID": String(lastEventId),
      },
    });
    if (!response.ok) {
      throw new OwnerApiError(
        response.status,
        "run_event_replay_failed",
        await response.text(),
      );
    }
    const events: unknown[] = [];
    for (const block of (await response.text()).split("\n\n")) {
      const data = block
        .split("\n")
        .find((line) => line.startsWith("data: "))
        ?.slice(6);
      if (data) events.push(JSON.parse(data) as unknown);
    }
    return parse(runEventSchema.array(), events);
  },
  async listTextReviews(projectId: string) {
    return parse(
      textReviewBatchSchema.array(),
      await request(`/v1/projects/${projectId}/text-review-batches`, {
        headers: ownerHeaders(projectId),
      }),
    );
  },
  async decideTextReview(
    batchId: string,
    expectedRevision: number,
    action: "accept" | "reject" | "retake",
  ) {
    return normalizeOwnerPayload(
      await request(`/v1/text-review-batches/${batchId}/decision`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ expectedRevision, action }),
      }),
    );
  },
  async getMediaGate(handoffId: string) {
    return normalizeOwnerPayload(
      await request(`/v1/text-handoffs/${handoffId}/media-gate`),
    ) as {
      status: "ready" | "blocked";
      handoffId: string;
      missingOwners: string[];
    };
  },
  async listSkillRoutes(projectId: string) {
    return parse(
      skillRouteDecisionSchema.array(),
      normalizeOwnerPayload(
        await request(`/v1/projects/${projectId}/skill-route-decisions`, {
          headers: ownerHeaders(projectId),
        }),
      ),
    );
  },
  async resolveSkillRoute(projectId: string) {
    return parse(
      skillRouteDecisionSchema,
      normalizeOwnerPayload(
        await request(`/v1/projects/${projectId}/skill-route-decisions`, {
          method: "POST",
          headers: ownerHeaders(projectId),
          body: JSON.stringify({
            nodeKey: "text.generate",
            launchId: `workbench:${projectId}`,
            projectType: "short_drama",
            stage: "text.generate",
            targetModel: "mock-model",
            query: "story script scene shot continuity",
            allowedTools: ["text_model"],
            allowedLicenses: ["MIT"],
            allowedSkills: ["novel-writing", "drama-skills"],
            requiredCapabilities: ["story_spec"],
            selectionMode: "fixed",
          }),
        }),
      ),
    );
  },
  async selectSkillRoute(
    projectId: string,
    decisionId: string,
    skillName: string,
    skillVersion: string,
    expectedRevision: number,
  ) {
    return normalizeOwnerPayload(
      await request(`/v1/skill-route-decisions/${decisionId}/selection`, {
        method: "POST",
        headers: ownerHeaders(projectId),
        body: JSON.stringify({
          skillName,
          skillVersion,
          actorUuid: "11111111-1111-4111-8111-111111111111",
          expectedRevision,
        }),
      }),
    );
  },
  async listRunInputSnapshots(projectId: string) {
    return request(`/v1/projects/${projectId}/run-input-snapshots`, {
      headers: ownerHeaders(projectId),
    });
  },
  async rerunHistorical(
    projectId: string,
    snapshotId: string,
    expectedSnapshotRevision: number,
  ) {
    const created = normalizeOwnerPayload(
      await request(
        `/v1/projects/${projectId}/run-input-snapshots/${snapshotId}/rerun`,
        {
          method: "POST",
          headers: {
            ...ownerHeaders(projectId),
            "If-Match": String(expectedSnapshotRevision),
          },
          body: JSON.stringify({
            expectedSnapshotRevision,
            schemaVersion: "1.0.0",
          }),
        },
      ),
    ) as { id?: unknown };
    if (typeof created.id !== "string" || !created.id) {
      throw new OwnerApiError(
        502,
        "owner_contract_invalid",
        "historical rerun response 缺少稳定 id",
      );
    }
    return this.getRun(projectId, created.id);
  },
  async createSuccessorRun(
    projectId: string,
    runId: string,
    expectedRevision: number,
  ) {
    const created = normalizeOwnerPayload(
      await request(`/v1/projects/${projectId}/runs/${runId}/successor`, {
        method: "POST",
        headers: {
          ...ownerHeaders(projectId),
          "If-Match": String(expectedRevision),
        },
        body: JSON.stringify({
          expectedPredecessorRevision: expectedRevision,
          reuseNodeIds: [],
          schemaVersion: "1.0.0",
        }),
      }),
    ) as { id?: unknown };
    if (typeof created.id !== "string" || !created.id) {
      throw new OwnerApiError(
        502,
        "owner_contract_invalid",
        "successor Run response 缺少稳定 id",
      );
    }
    return this.getRun(projectId, created.id);
  },
};
