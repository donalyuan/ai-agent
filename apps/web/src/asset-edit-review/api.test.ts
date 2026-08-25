import { describe, expect, it, vi } from "vitest";
import { assetEditReviewApi, assetEditReviewQueryKeys } from "./api";

const hash = "a".repeat(64);

function ok(payload: unknown, status = 200) {
  return Promise.resolve({ ok: true, status, json: async () => payload });
}

describe("assetEditReviewApi", () => {
  it("loads the episode index and session through GET only", async () => {
    const fetchMock = vi.fn().mockImplementation((input: RequestInfo | URL) => {
      const path = String(input);
      if (path.includes("session-1"))
        return ok({
          id: "session-1",
          schemaVersion: "1.0.0",
          revision: 1,
          status: "active",
          projectId: "project-1",
          episodeId: "episode-1",
          targetId: "shot-1",
          selection: {
            projectId: "project-1",
            episodeId: "episode-1",
            targetId: "shot-1",
            primary: {
              assetVersionId: "version-1",
              revision: 0,
              contentHash: hash,
              kind: "image",
              projectId: "project-1",
              mimeType: "image/png",
            },
            references: [],
          },
          continuity: {
            status: "accepted_current",
            snapshot: {
              id: "snapshot-1",
              revision: 1,
              contentHash: hash,
              targetId: "shot-1",
            },
            chain: [],
            tasks: [],
          },
          conversation: {
            id: "",
            schemaVersion: "1.0.0",
            revision: 0,
            messages: [],
            turns: [],
          },
          plans: [],
        });
      return ok({
        schemaVersion: "1.0.0",
        items: [
          {
            id: "session-1",
            revision: 1,
            projectId: "project-1",
            episodeId: "episode-1",
            targetId: "shot-1",
            status: "active",
          },
        ],
      });
    });
    vi.stubGlobal("fetch", fetchMock);
    const index = await assetEditReviewApi.listSessions(
      "project-1",
      "episode-1",
    );
    await assetEditReviewApi.getSession("project-1", index.items[0].id);
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(fetchMock.mock.calls.every(([, init]) => !init?.method)).toBe(true);
    expect(
      fetchMock.mock.calls.every(
        ([, init]) =>
          new Headers(init?.headers).get("X-Project-Scope") === "project-1",
      ),
    ).toBe(true);
    expect(assetEditReviewQueryKeys.session("project-1", "session-1")).toEqual([
      "projects",
      "project-1",
      "asset-edit",
      "sessions",
      "session-1",
    ]);
  });

  it("submits exact accept refs once and never retries a 409", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: false,
      status: 409,
      json: async () => ({
        detail: { type: "base_version_conflict", message: "stale" },
      }),
    });
    vi.stubGlobal("fetch", fetchMock);
    await expect(
      assetEditReviewApi.reviewCandidate("project-1", "candidate-1", {
        action: "accept",
        expectedRevision: 1,
        expectedBaseVersionId: "version-1",
        scope: ["shot-1"],
        candidateFacts: {
          candidateId: "candidate-1",
          projectId: "project-1",
          episodeId: "episode-1",
          targetId: "shot-1",
          assetVersionId: "version-2",
          assetVersionRevision: 0,
          assetVersionHash: hash,
          expectedTargetRevision: 2,
        },
        references: [{ referenceId: "shot-1", expectedRevision: 2 }],
      }),
    ).rejects.toMatchObject({ status: 409, code: "base_version_conflict" });
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(
      new Headers(fetchMock.mock.calls[0]?.[1]?.headers).get("X-Project-Scope"),
    ).toBe("project-1");
  });
});
