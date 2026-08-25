import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App, queryClient } from "../App";
import { emptyAssetFilters } from "../asset-center/contracts";
import { useAssetCenterStore } from "../asset-center/store";

const hash = "a".repeat(64);
const asset = {
  id: "asset-1",
  projectId: "project-1",
  revision: 3,
  name: "主角对白.wav",
  kind: "audio",
  status: "approved",
  sourceType: "user_upload",
  catalogRole: "dialogue",
  tags: ["lead"],
  authorizationStatus: "verified",
  copyrightOwner: null,
  licenseLabel: "Owned",
  licenseReference: null,
  updatedAt: "2026-08-23T01:02:03Z",
  versionCount: 1,
  processingStatus: "ready",
  latestVersion: {
    id: "version-1",
    revision: 0,
    contentHash: hash,
    checksum: hash,
    mimeType: "audio/wav",
    sizeBytes: 2048,
    durationMs: 3000,
  },
};

function response(payload: unknown, ok = true, status = 200) {
  return Promise.resolve({ ok, status, json: async () => payload });
}

afterEach(() => {
  vi.unstubAllGlobals();
  act(() => {
    queryClient.clear();
    useAssetCenterStore.setState({
      projectId: "",
      selectedAssetId: null,
      filters: { ...emptyAssetFilters },
      playing: false,
    });
  });
  window.history.pushState({}, "", "/projects");
});

describe("Project Asset Center", () => {
  it("loads and filters a catalog without issuing a write request", async () => {
    window.history.pushState({}, "", "/projects/project-1/assets");
    const fetchMock = vi.fn().mockImplementation((input: RequestInfo | URL) => {
      const path = String(input);
      if (path.endsWith("/health/ready")) return response({ status: "ready" });
      if (path.includes("/v1/assets/asset-1/versions"))
        return response(
          Array.from({ length: 120 }, (_, index) => ({
            ...asset.latestVersion,
            id: `version-${index + 1}`,
            versionNumber: index + 1,
          })),
        );
      if (path.includes("/asset-versions/version-1/media"))
        return response({
          status: "ready",
          derivatives: [
            { kind: "waveform", status: "ready", grantAvailable: true },
          ],
        });
      if (path.includes("/v1/projects/project-1/assets?"))
        return response({
          schemaVersion: "1.0.0",
          items: [asset],
          nextCursor: null,
        });
      return response({});
    });
    vi.stubGlobal("fetch", fetchMock);

    render(<App />);
    expect((await screen.findAllByText("主角对白.wav")).length).toBe(2);
    expect(await screen.findByText("waveform · ready")).toBeVisible();
    expect(
      (
        await screen.findByRole("list", { name: "AssetVersion 版本历史" })
      ).querySelectorAll('[role="listitem"]').length,
    ).toBeLessThan(120);
    expect(screen.getByRole("table")).toBeVisible();
    fireEvent.change(screen.getByLabelText("类型"), {
      target: { value: "audio" },
    });

    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some(([input]) =>
          String(input).includes("kind=audio"),
        ),
      ).toBe(true),
    );
    for (const [label, value, queryParam] of [
      ["角色", "dialogue", "catalogRole=dialogue"],
      ["来源", "user_upload", "sourceType=user_upload"],
      ["授权", "verified", "authorizationStatus=verified"],
      ["处理", "ready", "processingStatus=ready"],
    ] as const) {
      fireEvent.change(screen.getByLabelText(label), { target: { value } });
      await waitFor(() =>
        expect(
          fetchMock.mock.calls.some(([input]) =>
            String(input).includes(queryParam),
          ),
        ).toBe(true),
      );
    }
    fireEvent.change(screen.getByPlaceholderText("例如 lead"), {
      target: { value: "lead" },
    });
    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some(([input]) =>
          String(input).includes("tag=lead"),
        ),
      ).toBe(true),
    );
    expect(fetchMock.mock.calls.filter(([, init]) => init?.method).length).toBe(
      0,
    );
  });

  it("shows partial usage instead of treating unavailable owners as empty", async () => {
    window.history.pushState({}, "", "/projects/project-1/assets");
    const fetchMock = vi.fn().mockImplementation((input: RequestInfo | URL) => {
      const path = String(input);
      if (path.endsWith("/health/ready")) return response({ status: "ready" });
      if (path.includes("/usage"))
        return response({
          status: "partial",
          references: [],
          unavailableOwners: ["timeline"],
        });
      if (path.includes("/versions")) return response([]);
      if (path.includes("/media"))
        return response({ status: "ready", derivatives: [] });
      return response({
        schemaVersion: "1.0.0",
        items: [asset],
        nextCursor: null,
      });
    });
    vi.stubGlobal("fetch", fetchMock);

    render(<App />);
    const usageTab = await screen.findByRole("tab", { name: "使用位置" });
    fireEvent.mouseDown(usageTab, { button: 0 });
    fireEvent.click(usageTab);
    expect(await screen.findByText("partial · 0 个引用")).toBeVisible();
    expect(screen.getByText("owner unavailable: timeline")).toBeVisible();
  });

  it("plays ready audio only through a short-lived opaque media grant", async () => {
    window.history.pushState({}, "", "/projects/project-1/assets");
    const fetchMock = vi
      .fn()
      .mockImplementation((input: RequestInfo | URL, init?: RequestInit) => {
        const path = String(input);
        if (path.endsWith("/health/ready"))
          return response({ status: "ready" });
        if (path.includes("/v1/assets/asset-1/versions"))
          return response([{ ...asset.latestVersion, versionNumber: 1 }]);
        if (path.includes("/asset-versions/version-1/media/wave-1/grant"))
          return response({
            action: "read",
            expiresAt: "2026-08-23T01:04:03Z",
            accessPath: "/v1/asset-media-grants/opaque-token",
          });
        if (path.includes("/asset-versions/version-1/media"))
          return response({
            status: "ready",
            derivatives: [
              {
                id: "wave-1",
                kind: "waveform",
                status: "ready",
                grantAvailable: true,
              },
            ],
          });
        if (path.includes("/v1/projects/project-1/assets?"))
          return response({
            schemaVersion: "1.0.0",
            items: [asset],
            nextCursor: null,
          });
        return response(
          {},
          init?.method !== "POST",
          init?.method === "POST" ? 500 : 200,
        );
      });
    vi.stubGlobal("fetch", fetchMock);

    render(<App />);
    const play = await screen.findByTitle("试听");
    await waitFor(() => expect(play).toBeEnabled());
    fireEvent.click(play);

    await waitFor(() =>
      expect(document.querySelector("audio")).toHaveAttribute(
        "src",
        "/api/v1/asset-media-grants/opaque-token",
      ),
    );
    const grantCall = fetchMock.mock.calls.find(([input]) =>
      String(input).includes("/media/wave-1/grant"),
    );
    expect(grantCall?.[1]?.method).toBe("POST");
    expect(JSON.parse(String(grantCall?.[1]?.body))).toEqual({
      ttlSeconds: 120,
      schemaVersion: "1.0.0",
    });
  });

  it("uploads raw parts through one reservation and completes one version", async () => {
    window.history.pushState({}, "", "/projects/project-1/assets");
    let admitted = false;
    let contentReads = 0;
    Object.defineProperty(Blob.prototype, "arrayBuffer", {
      configurable: true,
      value: async () => {
        expect(admitted).toBe(true);
        contentReads += 1;
        return new Uint8Array([1, 2, 3]).buffer;
      },
    });
    const fetchMock = vi
      .fn()
      .mockImplementation((input: RequestInfo | URL, init?: RequestInit) => {
        const path = String(input);
        if (path.endsWith("/health/ready"))
          return response({ status: "ready" });
        if (path.endsWith("/asset-upload-profiles"))
          return response([
            {
              storageProfileId: "local-test-offline",
              revision: 1,
              name: "Local test/offline",
              adapterKey: "local_workspace",
              enabled: true,
            },
          ]);
        if (path.endsWith("/asset-upload-admissions")) {
          expect(contentReads).toBe(0);
          admitted = true;
          return response({
            storageProfileId: "local-test-offline",
            storageProfileRevision: 1,
            storageProfileSnapshotHash: "b".repeat(64),
            minPartSizeBytes: 1,
            maxPartSizeBytes: 64 * 1024 * 1024,
            maxPartCount: 10_000,
            maxObjectSizeBytes: 8 * 1024 ** 4,
          });
        }
        if (path.includes("/assets?") && !init?.method)
          return response({
            schemaVersion: "1.0.0",
            items: [],
            nextCursor: null,
          });
        if (path.endsWith("/assets") && init?.method === "POST")
          return response({ id: "asset-new", revision: 1 }, true, 201);
        if (path.endsWith("/reservations") && init?.method === "POST")
          return response(
            {
              id: "reservation-1",
              revision: 1,
              fingerprint: hash,
              status: "reserved",
            },
            true,
            201,
          );
        if (path.endsWith("/asset-reservations/reservation-1"))
          return response({
            id: "reservation-1",
            revision: 1,
            fingerprint: hash,
            status: "reserved",
          });
        if (path.endsWith("/uploads/resume"))
          return response({ sessionId: "session-1" });
        if (path.includes("/parts/1"))
          return response({
            schemaVersion: "1.0.0",
            reservationId: "reservation-1",
            sessionId: "session-1",
            partNumber: 1,
            checksum: hash,
            eTag: hash,
            sizeBytes: 3,
          });
        if (path.endsWith("/uploads/complete"))
          return response({ id: "version-new" }, true, 201);
        return response({});
      });
    vi.stubGlobal("fetch", fetchMock);

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "上传素材" }));
    expect(
      await screen.findByRole("option", { name: /Local test\/offline/ }),
    ).toBeVisible();
    const file = new File([new Uint8Array([1, 2, 3])], "sample.wav", {
      type: "audio/wav",
    });
    const fileInput = screen.getByLabelText(/选择图片、视频、音频或文档/);
    await waitFor(() => expect(fileInput).toBeEnabled());
    fireEvent.change(fileInput, {
      target: { files: [file] },
    });

    expect(await screen.findByText("已登记 AssetVersion")).toBeVisible();
    expect(contentReads).toBe(2);
    const admissionCall = fetchMock.mock.calls.find(([input]) =>
      String(input).endsWith("/asset-upload-admissions"),
    );
    const reservationCall = fetchMock.mock.calls.find(
      ([input, init]) =>
        String(input).endsWith("/reservations") && init?.method === "POST",
    );
    expect(admissionCall).toBeDefined();
    expect(JSON.parse(String(reservationCall?.[1]?.body))).toMatchObject({
      storageProfileId: "local-test-offline",
      storageProfileRevision: 1,
      storageProfileSnapshotHash: "b".repeat(64),
    });
    const partCall = fetchMock.mock.calls.find(([input]) =>
      String(input).includes("/parts/1"),
    );
    expect(partCall?.[1]?.body).toBeInstanceOf(Uint8Array);
    const completeCall = fetchMock.mock.calls.find(([input]) =>
      String(input).endsWith("/uploads/complete"),
    );
    expect(JSON.parse(String(completeCall?.[1]?.body))).toEqual({
      sessionId: "session-1",
      parts: [
        {
          partNumber: 1,
          checksum: hash,
          eTag: hash,
          sizeBytes: 3,
        },
      ],
      correlationId: "asset-center-ui",
      schemaVersion: "1.0.0",
    });
  });

  it("准入拒绝时不读取文件内容且不创建 Asset 或 reservation", async () => {
    window.history.pushState({}, "", "/projects/project-1/assets");
    const contentRead = vi.fn();
    Object.defineProperty(Blob.prototype, "arrayBuffer", {
      configurable: true,
      value: contentRead,
    });
    const fetchMock = vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const path = String(input);
      if (path.endsWith("/health/ready")) return response({ status: "ready" });
      if (path.endsWith("/asset-upload-profiles"))
        return response([
          {
            storageProfileId: "local-test-offline",
            revision: 1,
            name: "Local test/offline",
            adapterKey: "local_workspace",
            enabled: true,
          },
        ]);
      if (path.endsWith("/asset-upload-admissions"))
        return response(
          {
            detail: {
              type: "storage_object_size_unsupported",
              message: "storage_object_size_unsupported",
            },
          },
          false,
          422,
        );
      if (path.includes("/assets?") && !init?.method)
        return response({
          schemaVersion: "1.0.0",
          items: [],
          nextCursor: null,
        });
      return response({});
    });
    vi.stubGlobal("fetch", fetchMock);

    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "上传素材" }));
    await screen.findByRole("option", { name: /Local test\/offline/ });
    const input = screen.getByLabelText(/选择图片、视频、音频或文档/);
    await waitFor(() => expect(input).toBeEnabled());
    fireEvent.change(input, {
      target: {
        files: [
          new File([new Uint8Array([1])], "too-large.wav", {
            type: "audio/wav",
          }),
        ],
      },
    });

    expect(
      (await screen.findAllByText("storage_object_size_unsupported")).length,
    ).toBeGreaterThan(0);
    expect(contentRead).not.toHaveBeenCalled();
    expect(
      fetchMock.mock.calls.some(
        ([input, init]) =>
          String(input).endsWith("/assets") && init?.method === "POST",
      ),
    ).toBe(false);
    expect(
      fetchMock.mock.calls.some(([input]) =>
        String(input).endsWith("/reservations"),
      ),
    ).toBe(false);
  });
});
