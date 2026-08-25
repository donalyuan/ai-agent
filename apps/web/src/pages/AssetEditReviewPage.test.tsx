import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App, queryClient } from "../App";
import { useAssetEditReviewStore } from "../asset-edit-review/store";

const hash = "a".repeat(64);

const candidate = {
  id: "candidate-1",
  schemaVersion: "1.0.0",
  revision: 1,
  status: "pending_review",
  projectId: "project-1",
  episodeId: "episode-1",
  targetId: "shot-1",
  assetVersion: {
    assetVersionId: "version-2",
    revision: 0,
    contentHash: "b".repeat(64),
    kind: "video",
    projectId: "project-1",
    mimeType: "video/mp4",
  },
  provenance: {
    providerStatus: "succeeded",
    derivativeStatus: "ready",
    derivativeFingerprint: "c".repeat(64),
    takeId: "take-1",
    acceptedCurrent: false,
    expectedTargetRevision: 3,
    adapterIdentity: "local_workspace",
  },
};

function ownerSession(continuityStatus = "accepted_current") {
  return {
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
        kind: "video",
        projectId: "project-1",
        mimeType: "video/mp4",
      },
      references: [],
    },
    continuity: {
      status: continuityStatus,
      snapshot: {
        id: "snapshot-1",
        revision: 1,
        contentHash: hash,
        targetId: "shot-1",
      },
      chain: [{ targetId: "shot-1", level: "shot", revision: 1 }],
      tasks:
        continuityStatus === "accepted_current"
          ? []
          : [
              {
                id: "task-1",
                targetId: "shot-1",
                status: "pending",
                revision: 1,
              },
            ],
    },
    conversation: {
      id: "session-1",
      schemaVersion: "1.0.0",
      projectId: "project-1",
      episodeId: "episode-1",
      revision: 3,
      messages: [
        {
          id: "message-1",
          sessionId: "session-1",
          sequence: 1,
          role: "user",
          contentHash: hash,
          status: "complete",
          correlationId: "corr-user",
        },
        {
          id: "message-2",
          sessionId: "session-1",
          sequence: 2,
          role: "agent",
          contentHash: "d".repeat(64),
          status: "complete",
          correlationId: "corr-agent",
        },
      ],
      turns: [
        {
          id: "turn-1",
          sessionId: "session-1",
          sequence: 1,
          userMessageId: "message-1",
          agentMessageId: "message-2",
          status: "complete",
          revision: 2,
        },
      ],
    },
    plans: [
      {
        id: "plan-1",
        schemaVersion: "1.0.0",
        revision: 1,
        projectId: "project-1",
        episodeId: "episode-1",
        targetId: "shot-1",
        turnId: "turn-1",
        status: "pending_review",
        instruction: "强化动作节奏",
        base: {
          assetVersionId: "version-1",
          revision: 0,
          contentHash: hash,
          kind: "video",
          projectId: "project-1",
          mimeType: "video/mp4",
        },
        references: [],
        cost: {
          status: "unknown",
          source: "owner_unavailable",
          currency: null,
          estimated: null,
        },
        impact: {
          id: null,
          status:
            continuityStatus === "accepted_current"
              ? "clear"
              : "continuity_stale",
          reasons: [],
          staleTargets: [],
        },
        continuity: {
          status: continuityStatus,
          snapshot: {
            id: "snapshot-1",
            revision: 1,
            contentHash: hash,
            targetId: "shot-1",
          },
          chain: [],
          tasks:
            continuityStatus === "accepted_current"
              ? []
              : [
                  {
                    id: "task-1",
                    targetId: "shot-1",
                    status: "pending",
                    revision: 1,
                  },
                ],
        },
        candidates: [candidate],
      },
    ],
  };
}

function response(payload: unknown, ok = true, status = 200) {
  return Promise.resolve({ ok, status, json: async () => payload });
}

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  queryClient.clear();
  localStorage.clear();
  useAssetEditReviewStore.setState({ slices: {}, diagnostics: {} });
  window.history.pushState({}, "", "/projects");
});

describe("AssetEdit Review page", () => {
  it("creates a session only after exact AssetVersion owner revalidation", async () => {
    window.history.pushState(
      {},
      "",
      `/projects/project-1/review?episodeId=episode-1&shotId=shot-1&assetVersionId=version-1&assetVersionRevision=0&assetVersionHash=${hash}&continuitySnapshotId=snapshot-1&continuitySnapshotRevision=1&continuitySnapshotHash=${hash}`,
    );
    const fetchMock = vi
      .fn()
      .mockImplementation((input: RequestInfo | URL, init?: RequestInit) => {
        const path = String(input);
        if (path.endsWith("/health/ready"))
          return response({ status: "ready" });
        if (path.includes("text-review-batches")) return response([]);
        if (path.endsWith("/asset-versions/version-1"))
          return response({
            id: "version-1",
            schemaVersion: "1.0.0",
            revision: 0,
            projectId: "project-1",
            contentHash: hash,
            mimeType: "image/png",
          });
        if (path.includes("asset-edit-sessions?") && !init?.method)
          return response({ schemaVersion: "1.0.0", items: [] });
        if (path.endsWith("/asset-edit-sessions") && init?.method === "POST")
          return response(
            { id: "session-new", revision: 1, status: "active" },
            true,
            201,
          );
        if (path.endsWith("/asset-edit-sessions/session-new"))
          return response(ownerSession());
        return response([]);
      });
    vi.stubGlobal("fetch", fetchMock);
    render(<App />);
    expect(await screen.findByText("完整图片版本")).toBeVisible();
    expect(fetchMock.mock.calls.filter(([, init]) => init?.method)).toEqual([]);
    fireEvent.click(screen.getByRole("button", { name: "创建审核会话" }));
    await waitFor(() => {
      const create = fetchMock.mock.calls.find(
        ([input, init]) =>
          String(input).endsWith("/asset-edit-sessions") &&
          init?.method === "POST",
      );
      expect(JSON.parse(String(create?.[1]?.body))).toMatchObject({
        episodeId: "episode-1",
        targetId: "shot-1",
        primary: {
          id: "version-1",
          revision: 0,
          contentHash: hash,
          kind: "image",
          projectId: "project-1",
          mimeType: "image/png",
        },
        continuity: {
          id: "snapshot-1",
          revision: 1,
          contentHash: hash,
          targetId: "shot-1",
        },
      });
    });
  });

  it("loads owner session with no mutation and exposes complete version facts", async () => {
    window.history.pushState(
      {},
      "",
      "/projects/project-1/review?episodeId=episode-1&sessionId=session-1",
    );
    const fetchMock = vi.fn().mockImplementation((input: RequestInfo | URL) => {
      const path = String(input);
      if (path.endsWith("/health/ready")) return response({ status: "ready" });
      if (path.includes("text-review-batches")) return response([]);
      if (path.endsWith("/asset-edit-sessions/session-1"))
        return response(ownerSession());
      if (path.includes("asset-edit-sessions?"))
        return response({
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
      return response([]);
    });
    vi.stubGlobal("fetch", fetchMock);
    render(<App />);
    expect(await screen.findByText("强化动作节奏")).toBeVisible();
    expect(screen.getByText(/version-1 · rev 0/)).toBeVisible();
    expect(screen.getAllByText(/sha256 aaaaaaaaaaaa/).length).toBeGreaterThan(
      0,
    );
    expect(screen.getAllByText("Mock + Local offline").length).toBeGreaterThan(
      0,
    );
    expect(fetchMock.mock.calls.filter(([, init]) => init?.method)).toEqual([]);
  });

  it("requires explicit execute confirmation and sends exact accept once", async () => {
    window.history.pushState(
      {},
      "",
      "/projects/project-1/review?episodeId=episode-1&sessionId=session-1&runId=run-1&nodeRunId=node-1",
    );
    const fetchMock = vi.fn().mockImplementation((input: RequestInfo | URL) => {
      const path = String(input);
      if (path.endsWith("/health/ready")) return response({ status: "ready" });
      if (path.includes("text-review-batches")) return response([]);
      if (path.endsWith("/asset-edit-sessions/session-1"))
        return response(ownerSession());
      if (path.includes("asset-edit-sessions?"))
        return response({
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
      if (path.endsWith("/asset-edit-plans/plan-1/execute"))
        return response({ id: "execution-1", revision: 1, status: "queued" });
      if (path.endsWith("/asset-edit-candidates/candidate-1/review"))
        return response({ id: "candidate-1", revision: 2, status: "accepted" });
      return response({});
    });
    vi.stubGlobal("fetch", fetchMock);
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "执行计划" }));
    expect(
      screen.getByRole("dialog", { name: "确认执行编辑计划" }),
    ).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "确认执行" }));
    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some(
          ([input, init]) =>
            String(input).endsWith("/asset-edit-plans/plan-1/execute") &&
            init?.method === "POST",
        ),
      ).toBe(true),
    );

    fireEvent.click(screen.getByRole("button", { name: "接受候选" }));
    fireEvent.click(screen.getByRole("button", { name: "确认接受" }));
    await waitFor(() => {
      const calls = fetchMock.mock.calls.filter(([input]) =>
        String(input).endsWith("/asset-edit-candidates/candidate-1/review"),
      );
      expect(calls).toHaveLength(1);
      expect(JSON.parse(String(calls[0][1]?.body))).toMatchObject({
        action: "accept",
        expectedRevision: 1,
        expectedBaseVersionId: "version-1",
        scope: ["shot-1"],
        references: [{ referenceId: "shot-1", expectedRevision: 3 }],
      });
    });
  });

  it("blocks all costly actions on continuity stale and rejects partial edit links", async () => {
    window.history.pushState(
      {},
      "",
      "/projects/project-1/review?episodeId=episode-1&sessionId=session-1&mask=x",
    );
    const fetchMock = vi.fn().mockImplementation((input: RequestInfo | URL) => {
      const path = String(input);
      if (path.endsWith("/health/ready")) return response({ status: "ready" });
      if (path.includes("text-review-batches")) return response([]);
      return response(ownerSession("continuity_stale"));
    });
    vi.stubGlobal("fetch", fetchMock);
    render(<App />);
    expect(await screen.findByText("unsupported_feature")).toBeVisible();
    expect(
      screen.queryByRole("button", { name: "执行计划" }),
    ).not.toBeInTheDocument();
    expect(fetchMock.mock.calls.filter(([, init]) => init?.method)).toEqual([]);
  });

  it("keeps accept disabled unless the plan impact is clear", async () => {
    window.history.pushState(
      {},
      "",
      "/projects/project-1/review?episodeId=episode-1&sessionId=session-1",
    );
    const stale = JSON.parse(JSON.stringify(ownerSession())) as {
      plans: Array<{
        impact: {
          id: string | null;
          status: string;
          reasons: string[];
          staleTargets: string[];
        };
      }>;
    };
    stale.plans[0].impact = {
      id: "impact-1",
      status: "stale",
      reasons: ["target changed"],
      staleTargets: ["shot-1"],
    };
    const fetchMock = vi.fn().mockImplementation((input: RequestInfo | URL) => {
      const path = String(input);
      if (path.endsWith("/health/ready")) return response({ status: "ready" });
      if (path.includes("text-review-batches")) return response([]);
      if (path.endsWith("/asset-edit-sessions/session-1"))
        return response(stale);
      if (path.includes("asset-edit-sessions?"))
        return response({
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
      return response({});
    });
    vi.stubGlobal("fetch", fetchMock);
    render(<App />);
    expect(await screen.findByText("target changed")).toBeVisible();
    expect(screen.getByRole("button", { name: "接受候选" })).toBeDisabled();
    expect(fetchMock.mock.calls.filter(([, init]) => init?.method)).toEqual([]);
  });

  it("refetches the authoritative session after a candidate 409 without retrying", async () => {
    window.history.pushState(
      {},
      "",
      "/projects/project-1/review?episodeId=episode-1&sessionId=session-1",
    );
    let sessionReads = 0;
    const fetchMock = vi
      .fn()
      .mockImplementation((input: RequestInfo | URL, init?: RequestInit) => {
        const path = String(input);
        if (path.endsWith("/health/ready"))
          return response({ status: "ready" });
        if (path.includes("text-review-batches")) return response([]);
        if (path.endsWith("/asset-edit-sessions/session-1")) {
          sessionReads += 1;
          return response(ownerSession());
        }
        if (path.includes("asset-edit-sessions?"))
          return response({
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
        if (
          path.endsWith("/asset-edit-candidates/candidate-1/review") &&
          init?.method === "POST"
        )
          return response(
            {
              detail: { type: "revision_conflict", message: "stale candidate" },
            },
            false,
            409,
          );
        return response({});
      });
    vi.stubGlobal("fetch", fetchMock);
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "接受候选" }));
    fireEvent.click(screen.getByRole("button", { name: "确认接受" }));
    await waitFor(() => expect(sessionReads).toBeGreaterThan(1));
    expect(
      fetchMock.mock.calls.filter(
        ([input, init]) =>
          String(input).endsWith("/asset-edit-candidates/candidate-1/review") &&
          init?.method === "POST",
      ),
    ).toHaveLength(1);
    expect(await screen.findByText("stale candidate")).toBeVisible();
  });

  it("rejects a foreign Episode session before reading it and never falls back", async () => {
    window.history.pushState(
      {},
      "",
      "/projects/project-1/review?episodeId=episode-2&sessionId=session-1",
    );
    const fetchMock = vi.fn().mockImplementation((input: RequestInfo | URL) => {
      const path = String(input);
      if (path.endsWith("/health/ready")) return response({ status: "ready" });
      if (path.includes("text-review-batches")) return response([]);
      if (path.includes("asset-edit-sessions?"))
        return response({ schemaVersion: "1.0.0", items: [] });
      return response({}, false, 500);
    });
    vi.stubGlobal("fetch", fetchMock);
    render(<App />);
    expect(
      await screen.findByText("active_session_scope_invalid"),
    ).toBeVisible();
    expect(
      fetchMock.mock.calls.some(([input]) =>
        String(input).endsWith("/asset-edit-sessions/session-1"),
      ),
    ).toBe(false);
    expect(screen.queryByText("会话轮次")).not.toBeInTheDocument();
    expect(fetchMock.mock.calls.filter(([, init]) => init?.method)).toEqual([]);
  });
});
