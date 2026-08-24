import { OwnerApiError } from "../workbench/api";
import { traceHeaders } from "../workbench/trace-context";
import { catalogSchema, type CatalogPage, type FilterState } from "./contracts";

export const ASSET_SCHEMA_VERSION = "1.0.0";

function normalize(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(normalize);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, child]) => [
        key.replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase()),
        normalize(child),
      ]),
    );
  }
  return value;
}

export async function assetRequest(
  path: string,
  init?: RequestInit,
): Promise<unknown> {
  const headers = new Headers(traceHeaders());
  headers.set("Accept", "application/json");
  new Headers(init?.headers).forEach((value, key) => headers.set(key, value));
  const response = await fetch(`/api${path}`, { ...init, headers });
  const body = await response.json().catch(() => null);
  if (!response.ok) {
    const detail = body as {
      detail?: { message?: string; type?: string };
    } | null;
    throw new OwnerApiError(
      response.status,
      detail?.detail?.type ?? `asset_http_${response.status}`,
      detail?.detail?.message ?? `资产 owner 请求失败（${response.status}）`,
    );
  }
  return normalize(body);
}

export const assetCenterQueryKeys = {
  catalog: (projectId: string, cursor: string | null, filters: FilterState) =>
    ["projects", projectId, "asset-center", cursor, filters] as const,
  versions: (assetId: string) => ["assets", assetId, "versions"] as const,
  media: (versionId: string) => ["asset-versions", versionId, "media"] as const,
  usage: (versionId: string) => ["asset-versions", versionId, "usage"] as const,
};

export const assetCenterApi = {
  uploadProfiles(projectId: string) {
    return assetRequest(
      `/v1/projects/${projectId}/asset-upload-profiles`,
    ) as Promise<
      Array<{
        storageProfileId: string;
        revision: number;
        name: string;
        adapterKey: string;
        enabled: boolean;
      }>
    >;
  },
  admitUpload(
    projectId: string,
    body: {
      storageProfileId: string;
      storageProfileRevision: number;
      declaredMimeType: string;
      declaredSizeBytes: number;
      partSizeBytes: number;
    },
  ) {
    return assetRequest(`/v1/projects/${projectId}/asset-upload-admissions`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ ...body, schemaVersion: ASSET_SCHEMA_VERSION }),
    }) as Promise<{
      storageProfileId: string;
      storageProfileRevision: number;
      storageProfileSnapshotHash: string;
      minPartSizeBytes: number;
      maxPartSizeBytes: number;
      maxPartCount: number;
      maxObjectSizeBytes: number;
      warning?: string | null;
    }>;
  },
  async catalog(
    projectId: string,
    cursor: string | null,
    filters: FilterState,
    limit = 30,
  ): Promise<CatalogPage> {
    const params = new URLSearchParams({ limit: String(limit) });
    if (cursor) params.set("cursor", cursor);
    if (filters.kind) params.set("kind", filters.kind);
    if (filters.role) params.set("catalogRole", filters.role);
    if (filters.source) params.set("sourceType", filters.source);
    if (filters.authorization)
      params.set("authorizationStatus", filters.authorization);
    if (filters.processing) params.set("processingStatus", filters.processing);
    if (filters.tag.trim()) params.set("tag", filters.tag.trim());
    const parsed = catalogSchema.safeParse(
      await assetRequest(
        `/v1/projects/${projectId}/assets?${params.toString()}`,
      ),
    );
    if (!parsed.success) {
      throw new OwnerApiError(
        502,
        "asset_contract_invalid",
        parsed.error.issues.map((item) => item.message).join("；"),
      );
    }
    return parsed.data;
  },
  versions(assetId: string) {
    return assetRequest(`/v1/assets/${assetId}/versions`);
  },
  media(projectId: string, versionId: string) {
    return assetRequest(
      `/v1/projects/${projectId}/asset-versions/${versionId}/media`,
    );
  },
  usage(projectId: string, versionId: string) {
    return assetRequest(
      `/v1/projects/${projectId}/asset-versions/${versionId}/usage`,
    );
  },
  reservation(projectId: string, reservationId: string) {
    return assetRequest(
      `/v1/projects/${projectId}/asset-reservations/${reservationId}`,
    );
  },
  patchMetadata(
    assetId: string,
    expectedRevision: number,
    metadata: Record<string, unknown>,
  ) {
    return assetRequest(`/v1/assets/${assetId}`, {
      method: "PATCH",
      headers: {
        "Content-Type": "application/json",
        "If-Match": String(expectedRevision),
      },
      body: JSON.stringify({
        ...metadata,
        expectedRevision,
        schemaVersion: ASSET_SCHEMA_VERSION,
      }),
    });
  },
  mutateReservation(
    projectId: string,
    reservationId: string,
    action: "cancel" | "reconcile",
    expectedRevision: number,
    sessionId?: string,
  ) {
    return assetRequest(
      `/v1/projects/${projectId}/asset-reservations/${reservationId}/${action}`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          expectedRevision,
          sessionId,
          correlationId: "asset-center-ui",
          schemaVersion: ASSET_SCHEMA_VERSION,
        }),
      },
    );
  },
  mediaGrant(projectId: string, versionId: string, derivativeId: string) {
    return assetRequest(
      `/v1/projects/${projectId}/asset-versions/${versionId}/media/${derivativeId}/grant`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          ttlSeconds: 120,
          schemaVersion: ASSET_SCHEMA_VERSION,
        }),
      },
    );
  },
  timelineSelection(projectId: string, versionId: string, episodeId: string) {
    return assetRequest(
      `/v1/projects/${projectId}/asset-versions/${versionId}/timeline-selection`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          episodeId,
          schemaVersion: ASSET_SCHEMA_VERSION,
        }),
      },
    );
  },
};
