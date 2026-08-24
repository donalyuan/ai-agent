import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App, queryClient } from "../App";

const hash = "a".repeat(64);

function artifact(
  id: string,
  type: "mp4" | "srt" | "light_manifest",
  status = "verified",
) {
  return {
    id,
    artifactType: type,
    status,
    sizeBytes: status === "verified" ? 12 : null,
    checksum: status === "verified" ? hash : null,
    mimeType: status === "verified" ? "video/mp4" : null,
    hold: false,
    licenseStatus: "approved",
    expiresAt: "2999-12-31T23:59:59Z",
  };
}

function job(episodeId: string, status: "failed" | "succeeded") {
  return {
    id: `job-${episodeId}`,
    projectId: "project-1",
    episodeId,
    timelineVersionId: `version-${episodeId}`,
    batchId: "batch-1",
    revision: 4,
    status,
    packagingPhase: null,
    logicalOperation: `export:${episodeId}`,
    renderPlanHash: status === "succeeded" ? hash : null,
    rendererDiagnostic: status === "failed" ? "render failed" : null,
    diagnostics: [],
    artifacts: [
      artifact(
        `mp4-${episodeId}`,
        "mp4",
        status === "succeeded" ? "verified" : "pending",
      ),
      artifact(
        `srt-${episodeId}`,
        "srt",
        status === "succeeded" ? "verified" : "pending",
      ),
      artifact(
        `manifest-${episodeId}`,
        "light_manifest",
        status === "succeeded" ? "verified" : "pending",
      ),
    ],
  };
}

function batch(
  jobs = [job("episode-1", "failed"), job("episode-2", "succeeded")],
) {
  return {
    id: "batch-1",
    schemaVersion: "1.0.0",
    revision: 1,
    projectId: "project-1",
    exportProfile: "light",
    settings: { fps: 30 },
    status: "partially_failed",
    jobs,
    members: jobs.map((item) => ({
      episodeId: item.episodeId,
      timelineVersionId: item.timelineVersionId,
      timelineVersionRevision: 1,
      outputBaseName: item.episodeId,
      exportJobId: item.id,
      status: item.status,
    })),
  };
}

function response(payload: unknown, ok = true, status = 200) {
  return Promise.resolve({ ok, status, json: async () => payload });
}

afterEach(() => {
  vi.unstubAllGlobals();
  queryClient.clear();
  window.history.pushState({}, "", "/projects");
});

describe("Exports page", () => {
  it("submits one deduplicated batch containing multiple explicit members", async () => {
    window.history.pushState({}, "", "/projects/project-1/exports");
    const fetchMock = vi
      .fn()
      .mockImplementation((input: RequestInfo | URL, init?: RequestInit) => {
        const path = String(input);
        if (path.endsWith("/health/ready"))
          return response({ status: "ready" });
        if (path.endsWith("/export-batches") && init?.method === "POST")
          return response(batch(), true, 201);
        if (path.endsWith("/export-batches/batch-1")) return response(batch());
        return response({});
      });
    vi.stubGlobal("fetch", fetchMock);
    render(<App />);

    const fill = (episode: string, version: string, name: string) => {
      fireEvent.change(screen.getByLabelText("Episode ID"), {
        target: { value: episode },
      });
      fireEvent.change(screen.getByLabelText("Published Version ID"), {
        target: { value: version },
      });
      fireEvent.change(screen.getByLabelText("Output base name"), {
        target: { value: name },
      });
      fireEvent.click(screen.getByRole("button", { name: "添加成员" }));
    };
    fill("episode-1", "version-1", "episode-01");
    fill("episode-2", "version-2", "episode-02");
    fireEvent.change(screen.getByLabelText("StorageProfile ID"), {
      target: { value: "local-test-offline" },
    });
    fireEvent.click(screen.getByRole("button", { name: "提交 2 集导出" }));

    await waitFor(() => {
      const createCall = fetchMock.mock.calls.find(
        ([input, init]) =>
          String(input).endsWith("/export-batches") && init?.method === "POST",
      );
      const payload = JSON.parse(String(createCall?.[1]?.body));
      expect(payload.selections).toHaveLength(2);
      expect(
        payload.selections.map((item: { episodeId: string }) => item.episodeId),
      ).toEqual(["episode-1", "episode-2"]);
      expect(payload.storageProfileId).toBe("local-test-offline");
    });
  });

  it("retries only an explicit non-empty selection of latest failed members", async () => {
    window.history.pushState({}, "", "/projects/project-1/exports");
    const current = batch();
    const fetchMock = vi
      .fn()
      .mockImplementation((input: RequestInfo | URL, init?: RequestInit) => {
        const path = String(input);
        if (path.endsWith("/health/ready"))
          return response({ status: "ready" });
        if (
          path.endsWith("/export-batches/batch-1/retries") &&
          init?.method === "POST"
        )
          return response([] as unknown[]);
        if (path.endsWith("/export-batches/batch-1")) return response(current);
        return response({});
      });
    vi.stubGlobal("fetch", fetchMock);
    render(<App />);

    fireEvent.change(screen.getByLabelText("ExportBatch ID"), {
      target: { value: "batch-1" },
    });
    fireEvent.click(screen.getByRole("button", { name: "读取 batch" }));
    const retryButton = await screen.findByRole("button", {
      name: "重试所选失败集",
    });
    expect(retryButton).toBeDisabled();
    fireEvent.click(
      screen.getByRole("checkbox", { name: "重试 Episode episode-1" }),
    );
    expect(retryButton).toBeEnabled();
    fireEvent.click(retryButton);

    await waitFor(() => {
      const retryCall = fetchMock.mock.calls.find(([input]) =>
        String(input).endsWith("/export-batches/batch-1/retries"),
      );
      expect(JSON.parse(String(retryCall?.[1]?.body)).episodeIds).toEqual([
        "episode-1",
      ]);
    });
    expect(
      screen.queryByRole("checkbox", { name: "重试 Episode episode-2" }),
    ).toBeNull();
  });

  it("opens an opaque grant only for a verified artifact on a succeeded job", async () => {
    window.history.pushState({}, "", "/projects/project-1/exports");
    const succeeded = job("episode-2", "succeeded");
    succeeded.artifacts[1] = artifact("srt-episode-2", "srt", "held");
    const current = batch([succeeded]);
    const open = vi.spyOn(window, "open").mockImplementation(() => null);
    const fetchMock = vi
      .fn()
      .mockImplementation((input: RequestInfo | URL, init?: RequestInit) => {
        const path = String(input);
        if (path.endsWith("/health/ready"))
          return response({ status: "ready" });
        if (path.endsWith("/download-grants") && init?.method === "POST")
          return response({
            schemaVersion: "1.0.0",
            artifactId: "mp4-episode-2",
            expiresAt: 9999999999,
            action: "read",
            accessPath: "/v1/asset-media-grants/opaque123",
          });
        if (path.endsWith("/export-batches/batch-1")) return response(current);
        return response({});
      });
    vi.stubGlobal("fetch", fetchMock);
    render(<App />);

    fireEvent.change(screen.getByLabelText("ExportBatch ID"), {
      target: { value: "batch-1" },
    });
    fireEvent.click(screen.getByRole("button", { name: "读取 batch" }));
    const mp4 = await screen.findByRole("button", { name: "下载 MP4" });
    fireEvent.click(mp4);
    await waitFor(() =>
      expect(open).toHaveBeenCalledWith(
        "/api/v1/asset-media-grants/opaque123",
        "_blank",
        "noopener,noreferrer",
      ),
    );
    expect(screen.getByRole("button", { name: "下载 SRT" })).toBeDisabled();
  });
});
