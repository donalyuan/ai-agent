import { z } from "zod";
import { OwnerApiError } from "../workbench/api";
import { traceHeaders } from "../workbench/trace-context";
import { timelineSchema, timelineVersionSchema } from "./contracts";

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
      detail?.detail?.type ?? `timeline_http_${response.status}`,
      detail?.detail?.message ??
        `Timeline owner 请求失败（${response.status}）`,
    );
  }
  return payload;
};

const parse = <T>(schema: z.ZodType<T>, value: unknown): T => {
  const result = schema.safeParse(value);
  if (!result.success)
    throw new OwnerApiError(
      502,
      "timeline_contract_invalid",
      result.error.message,
    );
  return result.data;
};

const scope = (projectId: string) => ({ "X-Project-Scope": projectId });

export const timelineQueryKeys = {
  current: (projectId: string, episodeId: string) =>
    [
      "projects",
      projectId,
      "episodes",
      episodeId,
      "timeline",
      "current",
    ] as const,
  versions: (projectId: string, episodeId: string) =>
    [
      "projects",
      projectId,
      "episodes",
      episodeId,
      "timeline",
      "versions",
    ] as const,
  version: (projectId: string, episodeId: string, versionId: string) =>
    [
      "projects",
      projectId,
      "episodes",
      episodeId,
      "timeline",
      "versions",
      versionId,
    ] as const,
};

export const timelineApi = {
  current: async (projectId: string, episodeId: string) =>
    parse(
      timelineSchema,
      await request(
        `/v1/projects/${projectId}/episodes/${episodeId}/timeline`,
        { headers: scope(projectId) },
      ),
    ),
  versions: async (projectId: string, episodeId: string) =>
    z
      .array(timelineVersionSchema)
      .parse(
        await request(
          `/v1/projects/${projectId}/episodes/${episodeId}/timeline/versions`,
          { headers: scope(projectId) },
        ),
      ),
  version: async (projectId: string, episodeId: string, versionId: string) =>
    parse(
      timelineVersionSchema,
      await request(
        `/v1/projects/${projectId}/episodes/${episodeId}/timeline/versions/${versionId}`,
        { headers: scope(projectId) },
      ),
    ),
  command: async (
    projectId: string,
    episodeId: string,
    expectedRevision: number,
    command: string,
    payload: Record<string, unknown>,
  ) =>
    parse(
      timelineSchema,
      await request(
        `/v1/projects/${projectId}/episodes/${episodeId}/timeline/commands`,
        {
          method: "POST",
          headers: { ...scope(projectId), "Content-Type": "application/json" },
          body: JSON.stringify({
            expectedRevision,
            command,
            payload,
            schemaVersion: "1.0.0",
          }),
        },
      ),
    ),
  publish: async (
    projectId: string,
    episodeId: string,
    expectedRevision: number,
    name: string,
  ) =>
    parse(
      timelineVersionSchema,
      await request(
        `/v1/projects/${projectId}/episodes/${episodeId}/timeline/versions`,
        {
          method: "POST",
          headers: { ...scope(projectId), "Content-Type": "application/json" },
          body: JSON.stringify({
            expectedRevision,
            name,
            schemaVersion: "1.0.0",
          }),
        },
      ),
    ),
  probeRenderer: async (projectId: string) =>
    request(`/v1/projects/${projectId}/renderer/probe`, {
      headers: scope(projectId),
    }),
};
