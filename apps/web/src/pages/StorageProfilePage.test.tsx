import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App, queryClient } from "../App";

afterEach(() => {
  vi.unstubAllGlobals();
  queryClient.clear();
  window.history.pushState({}, "", "/projects");
});

describe("StorageProfilePage", () => {
  it("编辑普通字段时保留 owner 返回的 binding 与 credential reference", async () => {
    window.history.pushState(
      {},
      "",
      "/projects/project-1/settings/storage-profiles/profile-1",
    );
    const profile = {
      storageProfileId: "profile-1",
      revision: 4,
      projectId: "project-1",
      name: "原名称",
      endpoint: "https://tos.example.invalid",
      bucket: "private-bucket",
      region: "cn-test",
      adapterKey: "tos",
      privateBucket: true,
      bucketBindingId: "binding-1",
      credentialRef: "credential-1",
      credentialStatus: "configured",
      enabled: true,
      projectScope: ["project-1"],
      connectTimeoutMs: 10_000,
      readTimeoutMs: 30_000,
      writeTimeoutMs: 60_000,
      presignMaxTtlSeconds: 300,
    };
    const fetchMock = vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const path = String(input);
      if (path.endsWith("/health/ready")) {
        return Promise.resolve({
          ok: true,
          json: async () => ({ status: "ready" }),
        });
      }
      if (
        path.endsWith("/v1/storage-profiles/profile-1") &&
        init?.method === "PATCH"
      ) {
        return Promise.resolve({
          ok: true,
          json: async () => ({ ...profile, name: "新名称", revision: 5 }),
        });
      }
      if (path.endsWith("/v1/storage-profiles/profile-1")) {
        return Promise.resolve({ ok: true, json: async () => profile });
      }
      return Promise.resolve({
        ok: false,
        status: 404,
        json: async () => ({}),
      });
    });
    vi.stubGlobal("fetch", fetchMock);

    render(<App />);
    fireEvent.change(await screen.findByDisplayValue("原名称"), {
      target: { value: "新名称" },
    });
    fireEvent.click(screen.getByRole("button", { name: /保存/ }));

    await waitFor(() => {
      const call = fetchMock.mock.calls.find(
        ([input, init]) =>
          String(input).endsWith("/v1/storage-profiles/profile-1") &&
          init?.method === "PATCH",
      );
      expect(call).toBeDefined();
      const body = JSON.parse(String(call?.[1]?.body));
      expect(body.bucketBindingId).toBe("binding-1");
      expect(body.credentialRef).toBe("credential-1");
      expect(body.name).toBe("新名称");
    });
  });
});
