import { render, screen, within } from "@testing-library/react";
import { createElement } from "react";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Home from "./page";
import * as api from "./lib/api";
import type { ProjectListResponse, ScriptDetail, ScriptListResponse } from "./lib/api";

vi.mock("./lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./lib/api")>();
  return {
    ...actual,
    checkHealth: vi.fn(),
    createApiClient: vi.fn(() => ({ baseUrl: "http://api.test", fetcher: vi.fn() })),
    listProjects: vi.fn(),
    listScripts: vi.fn(),
    getScript: vi.fn(),
  };
});

const project = {
  project_id: "11111111-1111-4111-8111-111111111111",
  name: "科技博主",
  positioning: "科技知识账号",
  description: "面向程序员的知识短视频",
  status: "active",
  created_at: "2026-07-02T00:00:00Z",
  updated_at: "2026-07-02T00:00:00Z",
};

const scriptSummary = {
  script_id: "22222222-2222-4222-8222-222222222222",
  title: "程序员必看：ChatGPT工作流",
  status: "draft" as const,
  scene_count: 2,
  parent_id: null,
  created_at: "2026-07-02T00:05:00Z",
};

const scriptDetail: ScriptDetail = {
  ...scriptSummary,
  project_id: project.project_id,
  hook: "还在手写重复代码？",
  scenes: [
    {
      scene_id: "33333333-3333-4333-8333-333333333333",
      sequence: 1,
      narration: "传统程序员每天要写大量重复代码。",
      visual_description: "程序员盯着屏幕，快速切换多个代码文件。",
      emotion: "焦虑",
      duration_sec: 8,
    },
    {
      scene_id: "44444444-4444-4444-8444-444444444444",
      sequence: 2,
      narration: "AI 能快速生成初稿。",
      visual_description: "屏幕上弹出代码建议。",
      emotion: "惊喜",
      duration_sec: 9,
    },
  ],
  updated_at: "2026-07-02T00:05:00Z",
};

function mockProjects(response: ProjectListResponse) {
  vi.mocked(api.checkHealth).mockResolvedValue(true);
  vi.mocked(api.listProjects).mockResolvedValue(response);
}

function mockScripts(response: ScriptListResponse) {
  vi.mocked(api.listScripts).mockResolvedValue(response);
}

describe("智能体工作台页面", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockProjects({ projects: [] });
    mockScripts({ scripts: [], total: 0, limit: 20, offset: 0 });
    vi.mocked(api.getScript).mockResolvedValue(scriptDetail);
  });

  it("展示 AI-AGENT 品牌、中文标题和六个智能体菜单", async () => {
    render(createElement(Home));

    expect((await screen.findAllByText("AI-AGENT")).length).toBeGreaterThanOrEqual(1);
    expect(screen.getByRole("heading", { name: "智能体工作台" })).toBeInTheDocument();

    const menu = screen.getByLabelText("智能体菜单");
    for (const label of [
      "选题智能体",
      "脚本智能体",
      "素材智能体",
      "视频智能体",
      "发布智能体",
      "优化智能体",
    ]) {
      expect(within(menu).getByText(label)).toBeInTheDocument();
    }
  });

  it("不在脚本工作台展示项目管理入口", async () => {
    render(createElement(Home));

    expect(await screen.findByRole("heading", { name: "生成脚本" })).toBeInTheDocument();
    expect(screen.getByLabelText("当前项目")).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "项目上下文" })).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "还没有项目" })).not.toBeInTheDocument();
    expect(screen.queryByLabelText("项目名称")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "创建项目" })).not.toBeInTheDocument();
    expect(screen.queryByText(/创建/)).not.toBeInTheDocument();
  });

  it("分镜数支持选择 3 到 12 镜", async () => {
    mockProjects({ projects: [project] });
    render(createElement(Home));

    const sceneCountSelect = await screen.findByLabelText("分镜数");
    const options = within(sceneCountSelect).getAllByRole("option").map((option) => option.textContent);

    expect(options).toEqual(["3 镜", "4 镜", "5 镜", "6 镜", "7 镜", "8 镜", "9 镜", "10 镜", "11 镜", "12 镜"]);
  });

  it("有脚本时展示时间轴对照视图", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [scriptSummary], total: 1, limit: 20, offset: 0 });
    render(createElement(Home));

    expect((await screen.findAllByText("程序员必看：ChatGPT工作流")).length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("时间轴对照视图")).toBeInTheDocument();
    expect(screen.getByText("第 1 镜")).toBeInTheDocument();
    expect(screen.getAllByText("旁白").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("画面指令").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("传统程序员每天要写大量重复代码。")).toBeInTheDocument();
    expect(screen.getByText("程序员盯着屏幕，快速切换多个代码文件。")).toBeInTheDocument();
  });

  it("使用原型确认的蓝色主色板", () => {
    const styles = readFileSync(resolve(__dirname, "styles.css"), "utf8");

    expect(styles).toContain("--color-agent-rail: #182030");
    expect(styles).toContain("--color-primary: #2860e8");
    expect(styles).toContain("--color-primary-soft: #e8f0ff");
    expect(styles).not.toContain("#2f855a");
    expect(styles).not.toContain("#1f3b2d");
  });
});
