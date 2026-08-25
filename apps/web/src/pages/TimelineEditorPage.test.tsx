import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App, queryClient } from "../App";

const hash = "a".repeat(64);
const replacementHash = "b".repeat(64);
const replacementFingerprint = "c".repeat(64);
const current = {
  id: "cut-1",
  projectId: "project-1",
  episodeId: "episode-1",
  schemaVersion: "1.0.0",
  revision: 7,
  fps: 30,
  clips: [
    {
      id: "clip-1",
      assetVersionId: "asset-1",
      assetVersionRevision: 0,
      assetVersionHash: hash,
      derivativeFingerprint: hash,
      timelineStart: 0,
      durationFrames: 30,
      inFrame: 0,
      outFrame: 30,
    },
  ],
  soundCues: [],
  captions: [],
  ducking: null,
  timelineFingerprint: hash,
};

function response(payload: unknown, ok = true, status = 200) {
  return Promise.resolve({ ok, status, json: async () => payload });
}

afterEach(() => {
  vi.unstubAllGlobals();
  queryClient.clear();
  window.history.pushState({}, "", "/projects");
});

describe("TimelineEditorPage", () => {
  it("loads without implicit writes and sends the current revision on explicit commands", async () => {
    window.history.pushState(
      {},
      "",
      "/projects/project-1/episodes/episode-1/timeline",
    );
    const fetchMock = vi
      .fn()
      .mockImplementation((input: RequestInfo | URL, init?: RequestInit) => {
        const path = String(input);
        if (path.endsWith("/health/ready"))
          return response({ status: "ready" });
        if (path.endsWith("/timeline/commands") && init?.method === "POST")
          return response(current);
        if (path.endsWith("/timeline/versions")) return response([]);
        if (path.endsWith("/timeline")) return response(current);
        return response({});
      });
    vi.stubGlobal("fetch", fetchMock);

    render(<App />);
    await screen.findByTestId("timeline-panel-group");
    expect(
      fetchMock.mock.calls.filter(([, init]) => init?.method === "POST"),
    ).toHaveLength(0);

    fireEvent.change(await screen.findByLabelText("手工字幕文本"), {
      target: { value: "hello" },
    });
    fireEvent.click(
      await screen.findByRole("button", { name: "添加手工字幕" }),
    );
    await waitFor(() => {
      const call = fetchMock.mock.calls.find(
        ([input, init]) =>
          String(input).endsWith("/timeline/commands") &&
          init?.method === "POST",
      );
      expect(call).toBeDefined();
      const body = JSON.parse(String(call?.[1]?.body));
      expect(body.expectedRevision).toBe(7);
      expect(body.command).toBe("UpsertManualCaption");
      expect(body.payload.caption.text).toBe("hello");
    });
  });

  it("uses the handoff availableFrames exactly for AddClip", async () => {
    const handoff = {
      projectId: "project-1",
      episodeId: "episode-1",
      assetVersionId: "asset-2",
      assetVersionRevision: 3,
      assetVersionHash: replacementHash,
      derivativeFingerprint: replacementFingerprint,
      acceptedCurrent: true,
      derivativeStatus: "ready",
      availableFrames: 125,
      kind: "video",
      shotId: "shot-1",
    };
    window.history.pushState(
      {},
      "",
      `/projects/project-1/episodes/episode-1/timeline?handoff=${encodeURIComponent(JSON.stringify(handoff))}`,
    );
    const fetchMock = vi
      .fn()
      .mockImplementation((input: RequestInfo | URL, init?: RequestInit) => {
        const path = String(input);
        if (path.endsWith("/health/ready"))
          return response({ status: "ready" });
        if (path.endsWith("/timeline/commands") && init?.method === "POST")
          return response(current);
        if (path.endsWith("/timeline/versions")) return response([]);
        if (path.endsWith("/timeline")) return response(current);
        return response({});
      });
    vi.stubGlobal("fetch", fetchMock);

    render(<App />);
    fireEvent.click(
      await screen.findByRole("button", { name: "添加 Video / Image Clip" }),
    );

    await waitFor(() => {
      const call = fetchMock.mock.calls.find(
        ([input, init]) =>
          String(input).endsWith("/timeline/commands") &&
          init?.method === "POST",
      );
      expect(call).toBeDefined();
      const body = JSON.parse(String(call?.[1]?.body));
      expect(body.command).toBe("AddClip");
      expect(body.payload.clip).toMatchObject({
        assetVersionId: "asset-2",
        assetVersionRevision: 3,
        assetVersionHash: replacementHash,
        derivativeFingerprint: replacementFingerprint,
        outFrame: 125,
      });
    });
  });

  it("sends exact old and new sources for ReplaceClipSource", async () => {
    const handoff = {
      projectId: "project-1",
      episodeId: "episode-1",
      assetVersionId: "asset-2",
      assetVersionRevision: 3,
      assetVersionHash: replacementHash,
      derivativeFingerprint: replacementFingerprint,
      acceptedCurrent: true,
      derivativeStatus: "ready",
      availableFrames: 90,
      kind: "video",
      shotId: "shot-1",
    };
    window.history.pushState(
      {},
      "",
      `/projects/project-1/episodes/episode-1/timeline?handoff=${encodeURIComponent(JSON.stringify(handoff))}`,
    );
    const fetchMock = vi
      .fn()
      .mockImplementation((input: RequestInfo | URL, init?: RequestInit) => {
        const path = String(input);
        if (path.endsWith("/health/ready"))
          return response({ status: "ready" });
        if (path.endsWith("/timeline/commands") && init?.method === "POST")
          return response(current);
        if (path.endsWith("/timeline/versions")) return response([]);
        if (path.endsWith("/timeline")) return response(current);
        return response({});
      });
    vi.stubGlobal("fetch", fetchMock);

    render(<App />);
    fireEvent.change(await screen.findByLabelText("Replace Clip ID"), {
      target: { value: "clip-1" },
    });
    fireEvent.click(
      await screen.findByRole("button", { name: "确认替换 Clip source" }),
    );

    await waitFor(() => {
      const call = fetchMock.mock.calls.find(
        ([input, init]) =>
          String(input).endsWith("/timeline/commands") &&
          init?.method === "POST",
      );
      expect(call).toBeDefined();
      const body = JSON.parse(String(call?.[1]?.body));
      expect(body.command).toBe("ReplaceClipSource");
      expect(body.payload.oldSource).toEqual({
        assetVersionId: "asset-1",
        assetVersionRevision: 0,
        assetVersionHash: hash,
        derivativeFingerprint: hash,
      });
      expect(body.payload.newSource).toEqual({
        projectId: "project-1",
        episodeId: "episode-1",
        shotId: "shot-1",
        assetVersionId: "asset-2",
        assetVersionRevision: 3,
        assetVersionHash: replacementHash,
        derivativeFingerprint: replacementFingerprint,
        acceptedCurrent: true,
        derivativeStatus: "ready",
        authorizationStatus: "authorized",
        licenseStatus: "approved",
        availableFrames: 90,
      });
    });
  });

  it("blocks replacement and shows a diagnostic when the handoff is too short", async () => {
    const handoff = {
      projectId: "project-1",
      episodeId: "episode-1",
      assetVersionId: "asset-2",
      assetVersionRevision: 3,
      assetVersionHash: replacementHash,
      derivativeFingerprint: replacementFingerprint,
      acceptedCurrent: true,
      derivativeStatus: "ready",
      availableFrames: 29,
      kind: "video",
      shotId: "shot-1",
    };
    window.history.pushState(
      {},
      "",
      `/projects/project-1/episodes/episode-1/timeline?handoff=${encodeURIComponent(JSON.stringify(handoff))}`,
    );
    const fetchMock = vi.fn().mockImplementation((input: RequestInfo | URL) => {
      const path = String(input);
      if (path.endsWith("/health/ready")) return response({ status: "ready" });
      if (path.endsWith("/timeline/versions")) return response([]);
      if (path.endsWith("/timeline")) return response(current);
      return response({});
    });
    vi.stubGlobal("fetch", fetchMock);

    render(<App />);
    fireEvent.change(await screen.findByLabelText("Replace Clip ID"), {
      target: { value: "clip-1" },
    });

    expect(
      await screen.findByTestId("timeline-replace-frame-diagnostic"),
    ).toHaveTextContent("availableFrames=29");
    expect(
      screen.getByRole("button", { name: "确认替换 Clip source" }),
    ).toBeDisabled();
    expect(
      fetchMock.mock.calls.filter(([, init]) => init?.method === "POST"),
    ).toHaveLength(0);
  });

  it("preflights publication before a separate confirmation command", async () => {
    window.history.pushState(
      {},
      "",
      "/projects/project-1/episodes/episode-1/timeline",
    );
    const fetchMock = vi
      .fn()
      .mockImplementation((input: RequestInfo | URL, init?: RequestInit) => {
        const path = String(input);
        if (path.endsWith("/health/ready"))
          return response({ status: "ready" });
        if (path.endsWith("/versions/preflight"))
          return response({
            cutId: "cut-1",
            expectedRevision: 7,
            timelineFingerprint: hash,
            name: "cut-v1",
          });
        if (path.endsWith("/timeline/versions") && init?.method === "POST")
          return response({
            id: "version-1",
            projectId: "project-1",
            episodeId: "episode-1",
            schemaVersion: "1.0.0",
            revision: 1,
            sourceCutRevision: 7,
            name: "cut-v1",
            snapshot: {},
          });
        if (path.endsWith("/timeline/versions")) return response([]);
        if (path.endsWith("/timeline")) return response(current);
        return response({});
      });
    vi.stubGlobal("fetch", fetchMock);
    render(<App />);
    fireEvent.change(await screen.findByPlaceholderText("例如 cut-v1"), {
      target: { value: "cut-v1" },
    });
    fireEvent.click(screen.getByRole("button", { name: "检查并发布" }));
    await screen.findByRole("dialog", { name: "确认发布 TimelineVersion" });
    expect(
      fetchMock.mock.calls.some(
        ([input, init]) =>
          String(input).endsWith("/timeline/versions") &&
          init?.method === "POST",
      ),
    ).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: /确认发布 cut-v1/ }));
    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some(
          ([input, init]) =>
            String(input).endsWith("/timeline/versions") &&
            init?.method === "POST",
        ),
      ).toBe(true),
    );
  });
});
