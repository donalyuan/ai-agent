import { beforeEach, describe, expect, it } from "vitest";
import { useAssetEditReviewStore, validateReviewSlice } from "./store";

const hash = "a".repeat(64);
const primary = {
  assetVersionId: "version-1",
  revision: 0,
  contentHash: hash,
  kind: "image" as const,
  projectId: "project-1",
  mimeType: "image/png",
};

beforeEach(() => {
  localStorage.clear();
  useAssetEditReviewStore.setState({ slices: {}, diagnostics: {} });
});

describe("asset edit review presentation store", () => {
  it("isolates active session and selection by project plus episode", () => {
    const store = useAssetEditReviewStore.getState();
    store.patchSlice("project-1", "episode-a", {
      activeSessionId: "session-a",
      sessionRevision: 2,
      targetId: "shot-a",
      primary,
    });
    store.patchSlice("project-1", "episode-b", {
      activeSessionId: "session-b",
      sessionRevision: 1,
      targetId: "shot-b",
      primary: { ...primary, assetVersionId: "version-b" },
    });
    expect(store.getSlice("project-1", "episode-a").activeSessionId).toBe(
      "session-a",
    );
    expect(store.getSlice("project-1", "episode-b").activeSessionId).toBe(
      "session-b",
    );
  });

  it("atomically clears stale or foreign restored owner references", () => {
    const result = validateReviewSlice(
      {
        activeSessionId: "session-a",
        sessionRevision: 2,
        targetId: "shot-a",
        primary,
        references: [{ ...primary, assetVersionId: "foreign" }],
      },
      {
        projectId: "project-1",
        episodeId: "episode-a",
        sessionId: "session-a",
        sessionRevision: 3,
        targetId: "shot-a",
        primary,
        references: [],
      },
    );
    expect(result.slice.activeSessionId).toBeNull();
    expect(result.slice.primary).toBeNull();
    expect(result.slice.references).toEqual([]);
    expect(result.diagnostics).toContain("active_session_revision_stale");
  });

  it("switching target or version clears primary and refs without fallback", () => {
    const store = useAssetEditReviewStore.getState();
    store.patchSlice("project-1", "episode-a", {
      activeSessionId: "session-a",
      sessionRevision: 1,
      targetId: "shot-a",
      primary,
      references: [{ ...primary, assetVersionId: "version-2" }],
    });
    store.switchScope("project-1", "episode-a", "shot-b");
    const slice = useAssetEditReviewStore
      .getState()
      .getSlice("project-1", "episode-a");
    expect(slice.activeSessionId).toBeNull();
    expect(slice.primary).toBeNull();
    expect(slice.references).toEqual([]);
  });
});
