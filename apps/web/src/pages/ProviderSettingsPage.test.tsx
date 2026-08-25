import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App, queryClient } from "../App";

const catalog = {
  providers: [
    {
      id: "provider-1",
      name: "Mock Provider",
      adapterKey: "mock",
      enabled: true,
      revision: 2,
      approval: "approved",
    },
  ],
  profiles: [
    {
      id: "profile-1",
      providerId: "provider-1",
      name: "Local offline",
      adapterIdentity: "local_workspace",
      enabled: true,
      revision: 3,
      credentialStatus: "unconfigured",
      quotaSnapshots: { "image.generate": { status: "unknown" } },
    },
  ],
  models: [],
  skills: [
    {
      id: "skill-1",
      name: "novel-writing",
      version: "1.0.0",
      approval: "approved",
      enabled: true,
      provenance: "fixture",
      sourceType: "local",
      revision: 1,
      capabilities: ["text"],
    },
  ],
};

function response(payload: unknown) {
  return Promise.resolve({ ok: true, status: 200, json: async () => payload });
}

afterEach(() => {
  cleanup();
  queryClient.clear();
  vi.unstubAllGlobals();
  window.history.pushState({}, "", "/projects");
});

describe("ProviderSettingsPage", () => {
  it("读取与筛选 catalog 时不触发 Provider mutation", async () => {
    window.history.pushState({}, "", "/projects/project-1/settings");
    const fetchMock = vi.fn().mockImplementation((input: RequestInfo | URL) => {
      const path = String(input);
      if (path.endsWith("/health/ready")) return response({ status: "ready" });
      if (path.endsWith("/v1/catalog")) return response(catalog);
      return response({});
    });
    vi.stubGlobal("fetch", fetchMock);

    render(<App />);

    expect(
      (await screen.findAllByText("Mock Provider")).length,
    ).toBeGreaterThan(0);
    fireEvent.change(screen.getByLabelText("筛选 Provider / adapter"), {
      target: { value: "mock" },
    });
    expect(screen.getAllByText("Mock Provider").length).toBeGreaterThan(0);
    await waitFor(() => expect(fetchMock).toHaveBeenCalled());
    expect(
      fetchMock.mock.calls.filter(
        ([, init]) => init?.method && init.method !== "GET",
      ),
    ).toEqual([]);
  });

  it("requires an explicit remote model list before starting sync", async () => {
    window.history.pushState({}, "", "/projects/project-1/settings");
    const fetchMock = vi.fn().mockImplementation((input: RequestInfo | URL) => {
      const path = String(input);
      if (path.endsWith("/health/ready")) return response({ status: "ready" });
      if (path.endsWith("/v1/catalog")) return response(catalog);
      return response({});
    });
    vi.stubGlobal("fetch", fetchMock);
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "model sync" }));
    await waitFor(() => expect(screen.getByText(/候选模型 key/)).toBeVisible());
    expect(
      fetchMock.mock.calls.some(([input]) =>
        String(input).includes("/model-syncs"),
      ),
    ).toBe(false);
  });

  it("uses owner lifecycle endpoints with revision CAS", async () => {
    window.history.pushState({}, "", "/projects/project-1/settings");
    const fetchMock = vi.fn().mockImplementation((input: RequestInfo | URL) => {
      const path = String(input);
      if (path.endsWith("/health/ready")) return response({ status: "ready" });
      if (path.endsWith("/v1/catalog")) return response(catalog);
      return response({});
    });
    vi.stubGlobal("fetch", fetchMock);
    render(<App />);

    const [providerToggle] = await screen.findAllByRole("button", {
      name: "停用",
    });
    fireEvent.click(providerToggle);
    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some(
          ([input, init]) =>
            String(input).includes(
              "/v1/catalog/providers/provider-1/disable",
            ) &&
            init?.method === "POST" &&
            new Headers(init.headers).get("If-Match") === "2",
        ),
      ).toBe(true),
    );
  });

  it("marks sync candidates as explicit input without discovery", async () => {
    window.history.pushState({}, "", "/projects/project-1/settings");
    const fetchMock = vi.fn().mockImplementation((input: RequestInfo | URL) => {
      const path = String(input);
      if (path.endsWith("/health/ready")) return response({ status: "ready" });
      if (path.endsWith("/v1/catalog")) return response(catalog);
      if (path.includes("/model-syncs"))
        return response({
          id: "candidate-1",
          revision: 1,
          added: ["new-model"],
          removed: [],
          source: "explicit_input",
          discovery: "not_performed",
        });
      return response({});
    });
    vi.stubGlobal("fetch", fetchMock);
    render(<App />);
    fireEvent.change(await screen.findByLabelText("Remote model candidates"), {
      target: { value: "new-model" },
    });
    fireEvent.click(screen.getByRole("button", { name: "model sync" }));
    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some(
          ([input, init]) =>
            String(input).includes(
              "/v1/catalog/profiles/profile-1/model-syncs",
            ) &&
            init?.method === "POST" &&
            String(init.body).includes('"source":"explicit_input"'),
        ),
      ).toBe(true),
    );
    expect(screen.getByText(/discovery: not_performed/)).toBeVisible();
  });
});
