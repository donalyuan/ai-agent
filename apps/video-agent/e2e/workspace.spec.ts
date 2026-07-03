import { expect, test } from "@playwright/test";

const projectId = "11111111-1111-4111-8111-111111111111";
const scriptId = "22222222-2222-4222-8222-222222222222";

const project = {
  project_id: projectId,
  name: "科技博主",
  positioning: "科技知识账号",
  description: "面向程序员的知识短视频",
  status: "active",
  created_at: "2026-07-02T00:00:00Z",
  updated_at: "2026-07-02T00:00:00Z",
};

const scriptSummary = {
  script_id: scriptId,
  title: "程序员必看：ChatGPT工作流",
  status: "draft",
  scene_count: 6,
  parent_id: null,
  created_at: "2026-07-02T00:05:00Z",
};

const scriptDetail = {
  ...scriptSummary,
  project_id: projectId,
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

const workspaceMenus = [
  menuNode("content-strategy", "内容策略", false, "planned", 10),
  {
    ...menuNode("script-creation", "脚本创作", true, "active", 20),
    children: [
      {
        ...menuNode("script-generator", "脚本生成", true, "active", 10),
        agent_key: "script-generation-agent",
        menu_type: "page",
        module_key: "script.generator",
      },
    ],
  },
  menuNode("material-management", "素材管理", false, "planned", 30),
  menuNode("production", "作品生产", false, "planned", 40),
  menuNode("publishing", "发布运营", false, "planned", 50),
  menuNode("analytics", "数据分析", false, "planned", 60),
  menuNode("workflow-tasks", "工作流任务", false, "planned", 70),
];

function menuNode(menuKey: string, label: string, isEnabled: boolean, status: string, sortOrder: number) {
  return {
    menu_id: `00000000-0000-4000-8000-${String(sortOrder).padStart(12, "0")}`,
    menu_key: menuKey,
    label,
    description: `${label}说明`,
    route_path: `/${menuKey}`,
    icon: "circle",
    menu_type: "section",
    module_key: menuKey,
    agent_key: null,
    sort_order: sortOrder,
    is_enabled: isEnabled,
    status,
    metadata: { phase: status === "active" ? 1 : 2 },
    children: [],
  };
}

test.beforeEach(async ({ page }) => {
  await page.route(/\/health$/, async (route) => {
    await route.fulfill({ contentType: "application/json", json: { ok: true } });
  });
  await page.route(/\/api\/video-workspace\/menus$/, async (route) => {
    await route.fulfill({ contentType: "application/json", json: { menus: workspaceMenus } });
  });
  await page.route(/\/api\/projects$/, async (route) => {
    await route.fulfill({ contentType: "application/json", json: { projects: [project] } });
  });
  await page.route(new RegExp(`/api/projects/${projectId}/scripts(\\?.*)?$`), async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: { scripts: [scriptSummary], total: 1, limit: 20, offset: 0 },
    });
  });
  await page.route(new RegExp(`/api/scripts/${scriptId}$`), async (route) => {
    await route.fulfill({ contentType: "application/json", json: scriptDetail });
  });
});

test("video-agent 桌面工作台使用业务菜单并保留脚本创作闭环", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByText("VEDIO-AGENT").first()).toBeVisible();
  await expect(page.getByRole("heading", { name: "视频工作台" })).toBeVisible();

  const workspaceMenu = page.getByRole("navigation", { name: "视频工作台菜单" });
  for (const label of ["内容策略", "脚本创作", "素材管理", "作品生产", "发布运营", "数据分析", "工作流任务"]) {
    await expect(workspaceMenu.getByText(label)).toBeVisible();
  }
  for (const label of ["选题智能体", "脚本智能体", "素材智能体", "视频智能体", "发布智能体", "优化智能体"]) {
    await expect(workspaceMenu.getByText(label)).toHaveCount(0);
  }
  await expect(workspaceMenu.getByRole("button", { name: /脚本创作/ })).toHaveClass(/active/);
  await expect(workspaceMenu.getByRole("button", { name: /内容策略/ })).toBeDisabled();

  await expect(page.getByRole("heading", { name: "项目上下文" })).toHaveCount(0);
  await expect(page.getByLabel("项目名称")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "创建项目" })).toHaveCount(0);

  await expect(page.getByLabel("分镜数").locator("option")).toHaveText([
    "3 镜",
    "4 镜",
    "5 镜",
    "6 镜",
    "7 镜",
    "8 镜",
    "9 镜",
    "10 镜",
    "11 镜",
    "12 镜",
  ]);

  await expect(page.getByText("时间轴对照视图")).toBeVisible();
  await expect(page.getByRole("heading", { name: "程序员必看：ChatGPT工作流" })).toBeVisible();
  await expect(page.getByText("第 1 镜")).toBeVisible();
  await expect(page.getByRole("heading", { name: "旁白" }).first()).toBeVisible();
  await expect(page.getByRole("heading", { name: "画面指令" }).first()).toBeVisible();
  await expect(page.getByText("传统程序员每天要写大量重复代码。")).toBeVisible();
  await expect(page.getByText("程序员盯着屏幕，快速切换多个代码文件。"));
});
