import { render, screen } from "@testing-library/react";
import { createElement } from "react";
import { describe, expect, it } from "vitest";
import Home from "./page";

describe("Novex 管理后台首页", () => {
  it("展示平台管理后台边界和核心管理能力", () => {
    const { container } = render(createElement(Home));

    expect(screen.getAllByText("NOVEX ADMIN").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByRole("heading", { name: "平台管理后台" })).toBeInTheDocument();
    expect(screen.getByText("管理用户、权限、模型、工具、任务和运行状态。视频内容生产工作台已迁移到 apps/video-agent。"))
      .toBeInTheDocument();

    for (const label of ["用户与权限", "模型与路由", "工具与 MCP", "任务与日志", "成本与限额", "环境健康"]) {
      expect(screen.getByRole("heading", { name: label })).toBeInTheDocument();
    }
    expect(container.querySelector(".adminOverviewPage")).toBeInTheDocument();
  });

  it("不再展示视频生产工作台流程", () => {
    render(createElement(Home));

    expect(screen.queryByText("视频工作台")).not.toBeInTheDocument();
    expect(screen.queryByText("脚本智能体")).not.toBeInTheDocument();
    expect(screen.queryByText("素材智能体")).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "生成脚本" })).not.toBeInTheDocument();
    expect(screen.queryByText("时间轴对照视图")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("分镜数")).not.toBeInTheDocument();
  });

  it("侧栏模型菜单导航到真实模型管理路由", () => {
    render(createElement(Home));

    expect(screen.getByRole("link", { name: "模型与路由" })).toHaveAttribute("href", "/models");
    expect(screen.getByRole("navigation", { name: "管理后台导航菜单" })).toBeInTheDocument();
  });
});
