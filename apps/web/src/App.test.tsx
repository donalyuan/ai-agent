import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("阶段 0 工作台壳层", () => {
  it("呈现桌面导航、阶段轨与 API 就绪状态", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ status: "ready" }),
      }),
    );

    render(<App />);

    expect(
      screen.getByRole("navigation", { name: "工作台导航" }),
    ).toBeVisible();
    expect(screen.getByText("阶段 0 / 工程基线")).toBeVisible();
    expect(await screen.findByText("API 已就绪")).toBeVisible();
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
});
