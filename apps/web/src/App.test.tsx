import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App, queryClient } from "./App";

afterEach(() => {
  vi.unstubAllGlobals();
  queryClient.clear();
  window.history.pushState({}, "", "/projects");
});

describe("阶段一工作台壳层", () => {
  it("项目索引保留全局菜单，按内容高度呈现且不继承全屏裁切", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ status: "ready" }),
      }),
    );

    render(<App />);

    expect(screen.getByText("项目索引")).toBeVisible();
    expect(screen.queryByText("阶段一")).not.toBeInTheDocument();
    expect(await screen.findByText("API 已就绪")).toBeVisible();
    expect(
      screen.getByRole("navigation", { name: "工作台导航" }),
    ).toBeVisible();
    expect(screen.getByTestId("project-shell")).not.toHaveClass("lg:h-dvh");
    expect(screen.getByRole("main")).not.toHaveClass("lg:flex-1");
    expect(screen.getByTestId("project-index-grid")).toHaveClass("items-start");
  });

  it("在 API 不可用时呈现可诊断状态", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockRejectedValue(new TypeError("Network error")),
    );

    render(<App />);

    expect(await screen.findByText("API 不可用")).toBeVisible();
    expect(screen.getByText("无法连接 /api/v1/health/ready")).toBeVisible();
  });

  it("空 Episode presentation slice 在 React 19 下保持稳定", async () => {
    window.history.pushState(
      {},
      "",
      "/projects/project-1/workbench?episodeId=episode-1",
    );
    vi.stubGlobal(
      "fetch",
      vi.fn().mockImplementation((input: RequestInfo | URL) => {
        const path = String(input);
        if (path.endsWith("/health/ready"))
          return Promise.resolve({
            ok: true,
            json: async () => ({ status: "ready" }),
          });
        if (path.endsWith("/episodes"))
          return Promise.resolve({
            ok: true,
            json: async () => [
              {
                id: "episode-1",
                schemaVersion: "1.0.0",
                revision: 1,
                status: "draft",
                projectId: "project-1",
                number: 1,
                title: "E1",
              },
            ],
          });
        if (path.includes("/storyboard"))
          return Promise.resolve({ ok: true, json: async () => [] });
        return Promise.resolve({
          ok: false,
          status: 404,
          json: async () => ({}),
        });
      }),
    );
    render(<App />);
    expect(
      await screen.findByRole("option", { name: "01 / E1" }),
    ).toBeInTheDocument();
    expect(screen.getByText("当前剧集还没有分镜")).toBeVisible();
  });

  it("项目入口编辑使用 owner revision 的 If-Match，冲突不静默覆盖", async () => {
    const project = {
      id: "project-1",
      schemaVersion: "1.0.0",
      revision: 4,
      status: "active",
      name: "旧项目",
    };
    const fetchMock = vi
      .fn()
      .mockImplementation((input: RequestInfo | URL, init?: RequestInit) => {
        const path = String(input);
        if (path.endsWith("/health/ready"))
          return Promise.resolve({
            ok: true,
            json: async () => ({ status: "ready" }),
          });
        if (path.endsWith("/v1/projects") && init?.method === "PATCH")
          return Promise.resolve({
            ok: true,
            json: async () => ({ ...project, name: "新项目", revision: 5 }),
          });
        if (path.endsWith("/v1/projects"))
          return Promise.resolve({ ok: true, json: async () => [project] });
        return Promise.resolve({ ok: true, json: async () => ({}) });
      });
    vi.stubGlobal("fetch", fetchMock);

    render(<App />);
    expect(await screen.findByText("旧项目")).toBeVisible();
    fireEvent.click(screen.getByTitle("编辑项目名称"));
    fireEvent.change(screen.getByDisplayValue("旧项目"), {
      target: { value: "新项目" },
    });
    fireEvent.click(screen.getByRole("button", { name: /保存名称/ }));
    await waitFor(() => {
      const patchCall = fetchMock.mock.calls.find(
        ([input, init]) =>
          input === "/api/v1/projects/project-1" && init?.method === "PATCH",
      );
      expect(patchCall).toBeDefined();
      const headers = new Headers(patchCall?.[1]?.headers);
      expect(headers.get("If-Match")).toBe("4");
      expect(headers.get("traceparent")).toMatch(
        /^00-[0-9a-f]{32}-[0-9a-f]{16}-01$/,
      );
    });
  });

  it("Review 只在 pending_review 时提交一次 owner batch decision", async () => {
    window.history.pushState({}, "", "/projects/project-1/review");
    const batch = {
      id: "batch-1",
      projectId: "project-1",
      runId: "run-1",
      revision: 2,
      status: "pending_review",
      schemaVersion: "1.0.0",
      candidates: [
        {
          id: "candidate-1",
          kind: "story_spec",
          payloadHash: "a".repeat(64),
          status: "pending_review",
          revision: 1,
        },
      ],
    };
    const fetchMock = vi
      .fn()
      .mockImplementation((input: RequestInfo | URL, init?: RequestInit) => {
        const path = String(input);
        if (path.endsWith("/health/ready"))
          return Promise.resolve({
            ok: true,
            json: async () => ({ status: "ready" }),
          });
        if (path.includes("text-review-batches") && init?.method === "POST")
          return Promise.resolve({
            ok: true,
            json: async () => ({ batch: { ...batch, status: "accepted" } }),
          });
        if (path.includes("text-review-batches"))
          return Promise.resolve({ ok: true, json: async () => [batch] });
        return Promise.resolve({ ok: true, json: async () => ({}) });
      });
    vi.stubGlobal("fetch", fetchMock);
    render(<App />);
    expect(await screen.findByText("batch-1")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Accept/ }));
    fireEvent.click(screen.getByRole("button", { name: "确认提交" }));
    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/v1/text-review-batches/batch-1/decision",
        expect.objectContaining({
          method: "POST",
          body: JSON.stringify({ expectedRevision: 2, action: "accept" }),
        }),
      ),
    );
  });

  it("Review 在 owner 409 后权威重读 batch，且不自动重试命令", async () => {
    window.history.pushState({}, "", "/projects/project-1/review");
    const batch = {
      id: "batch-409",
      projectId: "project-1",
      runId: "run-1",
      revision: 2,
      status: "pending_review",
      schemaVersion: "1.0.0",
      candidates: [
        {
          id: "candidate-1",
          kind: "story_spec",
          payloadHash: "a".repeat(64),
          status: "pending_review",
          revision: 1,
        },
      ],
    };
    let reads = 0;
    const fetchMock = vi
      .fn()
      .mockImplementation((input: RequestInfo | URL, init?: RequestInit) => {
        const path = String(input);
        if (path.endsWith("/health/ready"))
          return Promise.resolve({
            ok: true,
            json: async () => ({ status: "ready" }),
          });
        if (path.includes("text-review-batches") && init?.method === "POST")
          return Promise.resolve({
            ok: false,
            status: 409,
            json: async () => ({
              detail: {
                type: "batch_revision_conflict",
                message: "stale batch",
              },
            }),
          });
        if (path.includes("text-review-batches")) {
          reads += 1;
          return Promise.resolve({ ok: true, json: async () => [batch] });
        }
        return Promise.resolve({ ok: true, json: async () => ({}) });
      });
    vi.stubGlobal("fetch", fetchMock);
    render(<App />);
    expect(await screen.findByText("batch-409")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: /Accept/ }));
    fireEvent.click(screen.getByRole("button", { name: "确认提交" }));
    expect(await screen.findByText("stale batch")).toBeVisible();
    await waitFor(() => expect(reads).toBeGreaterThan(1));
    expect(
      screen.queryByText(/已选择 accept；提交仍需 owner revision/),
    ).not.toBeInTheDocument();
    expect(
      fetchMock.mock.calls.filter(
        ([input, init]) =>
          String(input).includes("text-review-batches") &&
          init?.method === "POST",
      ),
    ).toHaveLength(1);
  });

  it("Workbench 将项目与剧集上下文放在右侧滚动工作区顶部", async () => {
    window.history.pushState(
      {},
      "",
      "/projects/project-1/workbench?episodeId=episode-1",
    );
    vi.stubGlobal(
      "fetch",
      vi.fn().mockImplementation((input: RequestInfo | URL) => {
        const path = String(input);
        if (path.endsWith("/health/ready"))
          return Promise.resolve({
            ok: true,
            json: async () => ({ status: "ready" }),
          });
        if (path.endsWith("/episodes"))
          return Promise.resolve({ ok: true, json: async () => [] });
        return Promise.resolve({
          ok: false,
          status: 404,
          json: async () => ({}),
        });
      }),
    );

    render(<App />);

    expect(await screen.findByTestId("workbench-scroll-region")).toBeVisible();
    expect(screen.getByTestId("workbench-canvas")).not.toHaveClass(
      "max-w-screen-2xl",
    );
    expect(screen.getByText("当前剧集")).toBeVisible();
    expect(screen.getByTestId("workbench-context")).not.toHaveClass("border-t");
    expect(screen.getByText("当前页面不会自动写入数据")).toBeVisible();
    expect(
      screen.queryByText("PROJECTS OWNER / CREATIVEBRIEF"),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("Workflow source")).not.toBeInTheDocument();
    expect(screen.queryByText("AssetBible")).not.toBeInTheDocument();
  });
});
