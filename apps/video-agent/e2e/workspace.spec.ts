import { expect, test, type Page } from "@playwright/test";

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

const generatedScriptId = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";

const generatedScriptSummary = {
  script_id: generatedScriptId,
  title: "ChatGPT 工作流新脚本",
  status: "draft",
  scene_count: 3,
  parent_id: null,
  created_at: "2026-07-02T00:12:00Z",
};

const generatedScriptDetail = {
  ...generatedScriptSummary,
  project_id: projectId,
  hook: "三个镜头看懂 AI 工作流。",
  scenes: [
    {
      scene_id: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
      sequence: 1,
      narration: "AI 工作流从清晰描述任务开始。",
      visual_description: "屏幕展示用户输入脚本需求。",
      emotion: "清晰",
      duration_sec: 8,
    },
    {
      scene_id: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
      sequence: 2,
      narration: "再把重复步骤交给 AI 起草。",
      visual_description: "AI 输出脚本大纲和镜头建议。",
      emotion: "高效",
      duration_sec: 9,
    },
    {
      scene_id: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
      sequence: 3,
      narration: "最后人工校准表达和事实细节。",
      visual_description: "创作者在时间轴上快速审阅并标注。",
      emotion: "笃定",
      duration_sec: 10,
    },
  ],
  updated_at: "2026-07-02T00:12:00Z",
};

const refreshedScriptDetail = {
  ...scriptDetail,
  scenes: scriptDetail.scenes.map((scene) =>
    scene.sequence === 2
      ? {
          ...scene,
          visual_description: "屏幕切到红色告警和密集 TODO，冲突更强。",
          emotion: "紧张",
        }
      : scene,
  ),
  updated_at: "2026-07-02T00:08:00Z",
};

const conversationId = "55555555-5555-4555-8555-555555555555";
const unboundConversationId = "99999999-9999-4999-8999-999999999999";

const conversation = {
  conversation_id: conversationId,
  project_id: projectId,
  agent_type: "script",
  subject_type: "script",
  subject_id: scriptId,
  title: "脚本 Agent 对话",
  status: "active",
  metadata: {},
  created_at: "2026-07-02T00:06:00Z",
  updated_at: "2026-07-02T00:06:00Z",
};

const unboundConversation = {
  conversation_id: unboundConversationId,
  project_id: projectId,
  agent_type: "script",
  subject_type: null,
  subject_id: null,
  title: "脚本 Agent 对话",
  status: "active",
  metadata: {},
  created_at: "2026-07-02T00:11:00Z",
  updated_at: "2026-07-02T00:11:00Z",
};

const userMessage = {
  message_id: "66666666-6666-4666-8666-666666666666",
  conversation_id: conversationId,
  role: "user",
  content: "把第 2 镜改得更有冲突感",
  metadata: {},
  created_at: "2026-07-02T00:07:00Z",
};

const assistantMessage = {
  message_id: "77777777-7777-4777-8777-777777777777",
  conversation_id: conversationId,
  role: "assistant",
  content: "已更新第 2 镜，时间轴已刷新。",
  metadata: { script_id: scriptId, scene_sequence: 2 },
  created_at: "2026-07-02T00:07:05Z",
};

const generatedUserMessage = {
  message_id: "10101010-1010-4010-8010-101010101010",
  conversation_id: unboundConversationId,
  role: "user",
  content: "生成一个关于 ChatGPT 工作流的 3 镜知识科普脚本",
  metadata: {},
  created_at: "2026-07-02T00:12:00Z",
};

const generatedAssistantMessage = {
  message_id: "20202020-2020-4020-8020-202020202020",
  conversation_id: unboundConversationId,
  role: "assistant",
  content: "已创建 3 镜脚本，时间轴已打开。",
  metadata: {
    intent: "generate_script",
    script_id: generatedScriptId,
    script_created: true,
    needs_input: false,
    missing_fields: [],
  },
  created_at: "2026-07-02T00:12:05Z",
};

const agentRun = {
  run_id: "88888888-8888-4888-8888-888888888888",
  conversation_id: conversationId,
  project_id: projectId,
  agent_type: "script",
  status: "completed",
  input: { content: userMessage.content },
  output: { reply: assistantMessage.content },
  error: null,
  started_at: "2026-07-02T00:07:00Z",
  finished_at: "2026-07-02T00:07:05Z",
};

const generatedAgentRun = {
  run_id: "30303030-3030-4030-8030-303030303030",
  conversation_id: unboundConversationId,
  project_id: projectId,
  agent_type: "script",
  status: "succeeded",
  input: { content: generatedUserMessage.content },
  output: { reply: generatedAssistantMessage.content, script_id: generatedScriptId },
  error_message: null,
  started_at: "2026-07-02T00:12:00Z",
  ended_at: "2026-07-02T00:12:05Z",
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
});

async function mockExistingScriptWorkflow(page: Page) {
  let scriptRefreshed = false;

  await page.route(new RegExp(`/api/projects/${projectId}/scripts(\\?.*)?$`), async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: { scripts: [scriptSummary], total: 1, limit: 20, offset: 0 },
    });
  });
  await page.route(new RegExp(`/api/scripts/${scriptId}$`), async (route) => {
    await route.fulfill({ contentType: "application/json", json: scriptRefreshed ? refreshedScriptDetail : scriptDetail });
  });
  await page.route(/\/api\/agent\/conversations$/, async (route) => {
    expect(route.request().method()).toBe("POST");
    expect(route.request().postDataJSON()).toMatchObject({
      project_id: projectId,
      agent_type: "script",
      subject_type: "script",
      subject_id: scriptId,
      title: "脚本 Agent 对话",
    });
    await route.fulfill({ contentType: "application/json", json: conversation });
  });
  await page.route(new RegExp(`/api/agent/conversations/${conversationId}/messages$`), async (route) => {
    if (route.request().method() === "GET") {
      await route.fulfill({ contentType: "application/json", json: { messages: [] } });
      return;
    }

    expect(route.request().method()).toBe("POST");
    expect(route.request().postDataJSON()).toEqual({ content: userMessage.content });
    scriptRefreshed = true;
    await route.fulfill({
      contentType: "application/json",
      json: { user_message: userMessage, assistant_message: assistantMessage, run: agentRun },
    });
  });
}

async function mockEmptyScriptGeneration(page: Page) {
  let scriptsRequestedAfterGeneration = false;

  await page.route(new RegExp(`/api/projects/${projectId}/scripts(\\?.*)?$`), async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: scriptsRequestedAfterGeneration
        ? { scripts: [generatedScriptSummary], total: 1, limit: 20, offset: 0 }
        : { scripts: [], total: 0, limit: 20, offset: 0 },
    });
  });
  await page.route(new RegExp(`/api/scripts/${generatedScriptId}$`), async (route) => {
    await route.fulfill({ contentType: "application/json", json: generatedScriptDetail });
  });
  await page.route(/\/api\/agent\/conversations$/, async (route) => {
    expect(route.request().method()).toBe("POST");
    expect(route.request().postDataJSON()).toEqual({
      project_id: projectId,
      agent_type: "script",
      title: "脚本 Agent 对话",
    });
    await route.fulfill({ contentType: "application/json", json: unboundConversation });
  });
  await page.route(new RegExp(`/api/agent/conversations/${unboundConversationId}/messages$`), async (route) => {
    if (route.request().method() === "GET") {
      await route.fulfill({ contentType: "application/json", json: { messages: [] } });
      return;
    }

    expect(route.request().method()).toBe("POST");
    expect(route.request().postDataJSON()).toEqual({ content: generatedUserMessage.content });
    scriptsRequestedAfterGeneration = true;
    await route.fulfill({
      contentType: "application/json",
      json: {
        user_message: generatedUserMessage,
        assistant_message: generatedAssistantMessage,
        run: generatedAgentRun,
      },
    });
  });
}

test("video-agent 桌面工作台使用业务菜单并保留脚本创作闭环", async ({ page }) => {
  await mockExistingScriptWorkflow(page);
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

  const actionColumn = page.locator('aside[aria-label="脚本 Agent 操作"]');
  await expect(actionColumn.getByRole("heading", { name: "脚本 Agent 对话" })).toBeVisible();
  await expect(actionColumn.getByRole("heading", { name: "生成脚本" })).toHaveCount(0);
  const agentInput = actionColumn.locator("textarea");
  await expect(agentInput).toHaveCount(1);
  await expect(agentInput).toBeInViewport();
  await expect(page.getByLabel("分镜数")).toHaveCount(0);

  await expect(page.getByText("时间轴对照视图")).toBeVisible();
  await expect(page.getByRole("heading", { name: "程序员必看：ChatGPT工作流" })).toBeVisible();
  await expect(page.getByText("第 1 镜")).toBeVisible();
  await expect(page.getByRole("heading", { name: "旁白" }).first()).toBeVisible();
  await expect(page.getByRole("heading", { name: "画面指令" }).first()).toBeVisible();
  await expect(page.getByText("传统程序员每天要写大量重复代码。")).toBeVisible();
  await expect(page.getByText("程序员盯着屏幕，快速切换多个代码文件。"));

  await page.getByRole("button", { name: "新建脚本" }).click();
  await expect(page.getByRole("heading", { name: "选择脚本后查看分镜" })).toBeVisible();
  await expect(actionColumn.getByText("当前项目：科技博主 / 新脚本生成")).toBeVisible();
  await expect(actionColumn.getByPlaceholder("描述你想生成的脚本...")).toBeVisible();
  await expect(page.getByRole("button", { name: /程序员必看：ChatGPT工作流/ })).not.toHaveClass(/selected/);

  await page.getByRole("button", { name: /程序员必看：ChatGPT工作流/ }).click();

  const agentPanel = page.getByRole("region", { name: "脚本 Agent 对话" });
  await expect(agentPanel.getByText("当前项目：科技博主 / 脚本：程序员必看：ChatGPT工作流")).toBeVisible();
  await expect(agentPanel.getByText(/绑定：/)).toHaveCount(0);
  await agentPanel.getByPlaceholder("描述要修改的分镜方向...").fill("把第 2 镜改得更有冲突感");
  await agentPanel.getByRole("button", { name: "发送" }).click();

  await expect(agentPanel.getByText("已更新第 2 镜，时间轴已刷新。")).toBeVisible();
  await expect(page.getByText("屏幕切到红色告警和密集 TODO，冲突更强。")).toBeVisible();
});

test("空脚本列表时通过脚本 Agent 对话生成脚本并打开时间轴详情", async ({ page }) => {
  await mockEmptyScriptGeneration(page);
  await page.goto("/");

  await expect(page.getByText("在右侧脚本 Agent 对话中描述需求后生成第一版结构化脚本。")).toBeVisible();

  const actionColumn = page.locator('aside[aria-label="脚本 Agent 操作"]');
  const agentInput = actionColumn.locator("textarea");
  await expect(agentInput).toHaveCount(1);
  await expect(agentInput).toBeInViewport();
  await expect(actionColumn.getByRole("heading", { name: "生成脚本" })).toHaveCount(0);

  const agentPanel = page.getByRole("region", { name: "脚本 Agent 对话" });
  await expect(agentPanel.getByText("当前项目：科技博主 / 新脚本生成")).toBeVisible();
  await expect(agentPanel.getByText(/绑定：/)).toHaveCount(0);
  await agentPanel.getByPlaceholder("描述你想生成的脚本...").fill(generatedUserMessage.content);
  await agentPanel.getByRole("button", { name: "发送" }).click();

  await expect(agentPanel.getByText("已创建 3 镜脚本，时间轴已打开。")).toBeVisible();
  await expect(page.getByRole("button", { name: /ChatGPT 工作流新脚本/ })).toBeVisible();
  await expect(page.getByRole("heading", { name: "ChatGPT 工作流新脚本" })).toBeVisible();
  await expect(page.getByText("时间轴对照视图")).toBeVisible();
  await expect(page.getByText("AI 工作流从清晰描述任务开始。")).toBeVisible();
  await expect(page.getByText("屏幕展示用户输入脚本需求。")).toBeVisible();
});
