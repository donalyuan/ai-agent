import { afterEach, describe, expect, it, vi } from "vitest";
import { timelineApi } from "./api";

afterEach(() => vi.unstubAllGlobals());

describe("timelineApi", () => {
  it("reads a scoped current Cut without mutation", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({
        id: "cut-1",
        projectId: "project-1",
        episodeId: "episode-1",
        schemaVersion: "1.0.0",
        revision: 1,
        fps: 30,
        clips: [],
        soundCues: [],
        captions: [],
        ducking: null,
        timelineFingerprint: "fp",
      }),
    });
    vi.stubGlobal("fetch", fetchMock);
    await timelineApi.current("project-1", "episode-1");
    expect(fetchMock).toHaveBeenCalledWith(
      expect.stringContaining(
        "/v1/projects/project-1/episodes/episode-1/timeline",
      ),
      expect.objectContaining({ headers: expect.any(Headers) }),
    );
  });

  it("preserves authoritative 409 diagnostics instead of retrying", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: false,
      status: 409,
      json: async () => ({
        detail: {
          type: "revision_conflict",
          message: "authoritative revision 2",
        },
      }),
    });
    vi.stubGlobal("fetch", fetchMock);
    await expect(
      timelineApi.command("project-1", "episode-1", 1, "DeleteClip", {
        clipId: "clip-1",
      }),
    ).rejects.toMatchObject({ status: 409, code: "revision_conflict" });
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});
