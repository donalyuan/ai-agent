import { z } from "zod";
import { OwnerApiError } from "../workbench/api";
import { traceHeaders } from "../workbench/trace-context";
import {
  acceptCommandSchema,
  assetVersionOwnerSchema,
  assetVersionRefSchema,
  reviewSessionSchema,
  sessionIndexSchema,
  turnPlanCommandSchema,
  type AssetVersionRef,
} from "./contracts";

const commandResultSchema = z.object({
  id: z.string().min(1),
  revision: z.number().int().positive(),
  status: z.string().min(1),
});

async function request(path: string, init?: RequestInit) {
  const headers = new Headers(traceHeaders());
  headers.set("Accept", "application/json");
  new Headers(init?.headers).forEach((value, key) => headers.set(key, value));
  const response = await fetch(`/api${path}`, { ...init, headers });
  const payload = (await response.json().catch(() => null)) as {
    detail?: { type?: string; message?: string };
  } | null;
  if (!response.ok) {
    throw new OwnerApiError(
      response.status,
      payload?.detail?.type ?? `asset_edit_http_${response.status}`,
      payload?.detail?.message ??
        `AssetEdit owner 请求失败（${response.status}）`,
    );
  }
  return payload;
}

function parse<T>(schema: z.ZodType<T>, payload: unknown): T {
  const result = schema.safeParse(payload);
  if (!result.success) {
    throw new OwnerApiError(
      502,
      "asset_edit_contract_invalid",
      result.error.issues.map((item) => item.message).join("；"),
    );
  }
  return result.data;
}

const headers = (projectId?: string) => ({
  "Content-Type": "application/json",
  ...(projectId ? { "X-Project-Scope": projectId } : {}),
});

const ownerVersion = (value: AssetVersionRef) => ({
  id: value.assetVersionId,
  revision: value.revision,
  contentHash: value.contentHash,
  kind: value.kind,
  projectId: value.projectId,
  mimeType: value.mimeType,
});

export const assetEditReviewQueryKeys = {
  sessions: (projectId: string, episodeId: string) =>
    ["projects", projectId, "asset-edit", "sessions", { episodeId }] as const,
  session: (projectId: string, sessionId: string) =>
    ["projects", projectId, "asset-edit", "sessions", sessionId] as const,
  plan: (projectId: string, planId: string) =>
    ["projects", projectId, "asset-edit", "plans", planId] as const,
  candidate: (projectId: string, planId: string, candidateId: string) =>
    [
      "projects",
      projectId,
      "asset-edit",
      "plans",
      planId,
      "candidates",
      candidateId,
    ] as const,
  version: (projectId: string, versionId: string) =>
    ["projects", projectId, "asset-edit", "versions", versionId] as const,
};

export const assetEditReviewApi = {
  async getAssetVersion(projectId: string, versionId: string) {
    const value = parse(
      assetVersionOwnerSchema,
      await request(`/v1/asset-versions/${versionId}`),
    );
    if (
      value.projectId !== projectId ||
      !/^(image|video)\//.test(value.mimeType)
    ) {
      throw new OwnerApiError(
        409,
        "base_version_conflict",
        "asset version is stale or foreign",
      );
    }
    return assetVersionRefSchema.parse({
      assetVersionId: value.id,
      revision: value.revision,
      contentHash: value.contentHash,
      kind: value.mimeType.startsWith("image/") ? "image" : "video",
      projectId: value.projectId,
      mimeType: value.mimeType,
    });
  },
  async listSessions(projectId: string, episodeId: string) {
    const params = new URLSearchParams({ episodeId });
    return parse(
      sessionIndexSchema,
      await request(`/v1/projects/${projectId}/asset-edit-sessions?${params}`),
    );
  },
  async getSession(projectId: string, sessionId: string) {
    return parse(
      reviewSessionSchema,
      await request(
        `/v1/projects/${projectId}/asset-edit-sessions/${sessionId}`,
      ),
    );
  },
  async createSession(
    projectId: string,
    episodeId: string,
    targetId: string,
    primary: AssetVersionRef,
    references: AssetVersionRef[],
    continuity: { id: string; revision: number; contentHash: string },
  ) {
    assetVersionRefSchema.parse(primary);
    const payload = await request(
      `/v1/projects/${projectId}/asset-edit-sessions`,
      {
        method: "POST",
        headers: headers(projectId),
        body: JSON.stringify({
          episodeId,
          targetId,
          primary: ownerVersion(primary),
          references: references.map(ownerVersion),
          continuity: { ...continuity, targetId },
          schemaVersion: "1.0.0",
        }),
      },
    );
    return parse(commandResultSchema, payload);
  },
  async appendMessage(
    sessionId: string,
    contentHash: string,
    correlationId: string,
    expectedRevision: number,
  ) {
    return parse(
      commandResultSchema,
      await request(`/v1/asset-edit-sessions/${sessionId}/messages`, {
        method: "POST",
        headers: headers(),
        body: JSON.stringify({ contentHash, correlationId, expectedRevision }),
      }),
    );
  },
  async generatePlan(input: z.input<typeof turnPlanCommandSchema>) {
    const value = turnPlanCommandSchema.parse(input);
    return parse(
      commandResultSchema,
      await request(
        `/v1/asset-edit-sessions/${value.sessionId}/turns/${value.turnId}/asset-edit-plans`,
        {
          method: "POST",
          headers: headers(value.base.projectId),
          body: JSON.stringify({
            ...value,
            base: ownerVersion(value.base),
            references: value.references.map(ownerVersion),
          }),
        },
      ),
    );
  },
  async executePlan(
    planId: string,
    input: {
      planRevision: number;
      runId: string;
      nodeRunId: string;
      logicalOperation: string;
      correlationId: string;
      requestFingerprint: string;
    },
  ) {
    return parse(
      commandResultSchema,
      await request(`/v1/asset-edit-plans/${planId}/execute`, {
        method: "POST",
        headers: headers(),
        body: JSON.stringify(input),
      }),
    );
  },
  async reviewCandidate(
    candidateId: string,
    input:
      | z.input<typeof acceptCommandSchema>
      | {
          action: "reject" | "retake";
          expectedRevision: number;
          expectedBaseVersionId: string;
          scope: string[];
          references?: { referenceId: string; expectedRevision: number }[];
          logicalOperation?: string;
        },
  ) {
    const body =
      input.action === "accept" ? acceptCommandSchema.parse(input) : input;
    return parse(
      commandResultSchema,
      await request(`/v1/asset-edit-candidates/${candidateId}/review`, {
        method: "POST",
        headers: headers(),
        body: JSON.stringify(body),
      }),
    );
  },
  compareCandidate(candidateId: string) {
    return request(`/v1/asset-edit-candidates/${candidateId}/compare`);
  },
};

export async function hashUserInput(value: string) {
  const bytes = new TextEncoder().encode(value);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}
