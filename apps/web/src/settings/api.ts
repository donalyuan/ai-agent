import { OwnerApiError } from "../workbench/api";
import { traceHeaders } from "../workbench/trace-context";

const request = async (path: string, init?: RequestInit) => {
  const headers = new Headers(traceHeaders());
  headers.set("Accept", "application/json");
  new Headers(init?.headers).forEach((value, key) => headers.set(key, value));
  const response = await fetch(`/api${path}`, { ...init, headers });
  const payload = await response.json().catch(() => null);
  if (!response.ok) {
    const detail = payload as {
      detail?: { type?: string; message?: string };
    } | null;
    throw new OwnerApiError(
      response.status,
      detail?.detail?.type ?? `settings_http_${response.status}`,
      detail?.detail?.message ??
        `Settings owner 请求失败（${response.status}）`,
    );
  }
  const normalize = (value: unknown): unknown => {
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
  };
  return normalize(payload);
};

export const settingsQueryKeys = {
  catalog: ["catalog", "owner"] as const,
  storage: (projectId: string) =>
    ["projects", projectId, "storage-profiles"] as const,
  storageProfile: (profileId: string) =>
    ["storage-profiles", profileId] as const,
};

export const settingsApi = {
  catalog: () => request("/v1/catalog"),
  updateProvider: (
    id: string,
    expectedRevision: number,
    changes: Record<string, unknown>,
  ) =>
    request(`/v1/catalog/providers/${id}`, {
      method: "PATCH",
      headers: {
        "Content-Type": "application/json",
        "If-Match": String(expectedRevision),
      },
      body: JSON.stringify({
        expectedRevision,
        changes,
        schemaVersion: "1.0.0",
      }),
    }),
  updateProfile: (
    id: string,
    expectedRevision: number,
    changes: Record<string, unknown>,
  ) =>
    request(`/v1/catalog/profiles/${id}`, {
      method: "PATCH",
      headers: {
        "Content-Type": "application/json",
        "If-Match": String(expectedRevision),
      },
      body: JSON.stringify({
        expectedRevision,
        changes,
        schemaVersion: "1.0.0",
      }),
    }),
  credentialStatus: (id: string) =>
    request(`/v1/catalog/profiles/${id}/credential`),
  replaceCredential: (
    id: string,
    expectedRevision: number,
    credentialId: string,
    value: string,
  ) =>
    request(`/v1/catalog/profiles/${id}/credential`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ expectedRevision, credentialId, value }),
    }),
  syncModels: (id: string, remoteModels: string[]) =>
    request(`/v1/catalog/profiles/${id}/model-syncs`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ remoteModels }),
    }),
  decideSync: (
    id: string,
    expectedRevision: number,
    decision: "accept" | "reject",
  ) =>
    request(`/v1/catalog/model-syncs/${id}/decision`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ expectedRevision, decision }),
    }),
  probe: (id: string, operation: string) =>
    request(`/v1/catalog/profiles/${id}/probe`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ operation }),
    }),
  storage: async (profileId: string) =>
    request(`/v1/storage-profiles/${profileId}`),
  createStorage: (projectId: string, body: Record<string, unknown>) =>
    request(`/v1/projects/${projectId}/storage-profiles`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    }),
  updateStorage: (
    profileId: string,
    expectedRevision: number,
    body: Record<string, unknown>,
  ) =>
    request(`/v1/storage-profiles/${profileId}`, {
      method: "PATCH",
      headers: {
        "Content-Type": "application/json",
        "If-Match": String(expectedRevision),
      },
      body: JSON.stringify({ ...body, expectedRevision }),
    }),
  enableStorage: (profileId: string, expectedRevision: number) =>
    request(`/v1/storage-profiles/${profileId}/enable`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ expectedRevision }),
    }),
  disableStorage: (profileId: string, expectedRevision: number) =>
    request(`/v1/storage-profiles/${profileId}/disable`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ expectedRevision }),
    }),
  testStorage: (
    profileId: string,
    expectedRevision: number,
    probeCorrelationId: string,
  ) =>
    request(`/v1/storage-profiles/${profileId}/connection-test`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ expectedRevision, probeCorrelationId }),
    }),
};
