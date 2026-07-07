import { expect, test, type Page } from "@playwright/test";

const projectId = "11111111-1111-4111-8111-111111111111";
const scriptId = "22222222-2222-4222-8222-222222222222";
const previousTopicBatchId = "77777777-7777-4777-8777-777777777777";
const supplementTopicBatchId = "99999999-9999-4999-8999-999999999901";

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
  topic_id: null,
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
const topicConversationId = "12121212-1212-4212-8212-121212121212";

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

const topicConversation = {
  conversation_id: topicConversationId,
  project_id: projectId,
  agent_type: "topic",
  subject_type: null,
  subject_id: null,
  title: "选题 Agent 对话",
  status: "active",
  metadata: {},
  created_at: "2026-07-06T10:05:00Z",
  updated_at: "2026-07-06T10:05:00Z",
};

const supplementUserMessage = {
  message_id: "13131313-1313-4313-8313-131313131313",
  conversation_id: topicConversationId,
  role: "user",
  content: "补充遗漏的 AI 工作流复盘角度",
  metadata: {},
  created_at: "2026-07-06T10:05:00Z",
};

const supplementAssistantMessage = {
  message_id: "14141414-1414-4414-8414-141414141414",
  conversation_id: topicConversationId,
  role: "assistant",
  content: "已生成 2 个候选选题。",
  metadata: {
    intent: "generate_topics",
    batch_id: supplementTopicBatchId,
    supplement_of_batch_id: previousTopicBatchId,
    created_topic_ids: ["15151515-1515-4515-8515-151515151515"],
    topic_count: 2,
    status: "idea",
  },
  created_at: "2026-07-06T10:05:05Z",
};

const supplementAgentRun = {
  run_id: "16161616-1616-4616-8616-161616161616",
  conversation_id: topicConversationId,
  project_id: projectId,
  agent_type: "topic",
  status: "succeeded",
  input: { content: supplementUserMessage.content },
  output: { batch_id: supplementTopicBatchId },
  error_message: null,
  started_at: "2026-07-06T10:05:00Z",
  ended_at: "2026-07-06T10:05:05Z",
};

const workspaceMenus = [
  {
    ...menuNode("content-strategy", "内容策略", true, "active", 10),
    children: [
      {
        ...menuNode("topic-history", "历史生成", true, "active", 10),
        agent_key: "topic-generation-agent",
        menu_type: "page",
        module_key: "strategy.topic-history",
      },
      {
        ...menuNode("topic-generator", "当前选题池", true, "active", 20),
        agent_key: "topic-generation-agent",
        menu_type: "page",
        module_key: "strategy.topics",
      },
    ],
  },
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

const contentStrategyWorkspaceMenus = [
  {
    ...menuNode("content-strategy", "内容策略", true, "active", 10),
    children: [
      {
        ...menuNode("topic-history", "历史生成", true, "active", 10),
        agent_key: "topic-generation-agent",
        menu_type: "page",
        module_key: "strategy.topic-history",
      },
      {
        ...menuNode("topic-generator", "当前选题池", true, "active", 20),
        agent_key: "topic-generation-agent",
        menu_type: "page",
        module_key: "strategy.topics",
      },
    ],
  },
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

const ideaTopic = {
  topic_id: "44444444-4444-4444-8444-444444444444",
  project_id: projectId,
  batch_id: null,
  title: "AI 工具如何重塑内容团队",
  angle: "从选题、脚本到分发拆解团队协作变化",
  target_audience: "内容团队负责人",
  hook_points: ["三步判断团队是否需要 AI 工作流", "从单点提效走向流程提效"],
  content_type: "knowledge",
  score: 86,
  score_reason: "和账号定位强相关，能自然转入案例。",
  tags: ["AI", "内容团队"],
  source: "manual",
  status: "idea",
  metadata: {},
  deleted_at: null,
  created_at: "2026-07-02T00:20:00Z",
  updated_at: "2026-07-02T00:20:00Z",
};

const approvedTopic = {
  ...ideaTopic,
  topic_id: "55555555-5555-4555-8555-555555555555",
  title: "普通人如何搭建 AI 内容流水线",
  source: "agent",
  status: "approved",
  score: 82,
};

const archivedTopic = {
  ...ideaTopic,
  topic_id: "66666666-6666-4666-8666-666666666666",
  title: "已经过时的工具清单",
  status: "archived",
  score: 40,
};

const scriptedTopic = {
  ...ideaTopic,
  topic_id: "657dcd2b-ebbd-47fd-ac1d-15663bae6cfa",
  title: "程序员如何用 AI 搭一条短视频生产流水线：从脚本到发布",
  source: "agent",
  status: "scripted",
  score: 94,
};

const latestTopicBatch = {
  batch_id: "88888888-8888-4888-8888-888888888888",
  project_id: projectId,
  supplement_of_batch_id: null,
  prompt: "最新一批 AI 工具选题",
  requested_count: 5,
  topic_count: 5,
  status: "succeeded",
  error_message: null,
  created_at: "2026-07-06T10:00:00Z",
  updated_at: "2026-07-06T10:00:10Z",
};

const previousTopicBatch = {
  batch_id: previousTopicBatchId,
  project_id: projectId,
  supplement_of_batch_id: null,
  prompt: "上一批 AI 内容流水线选题",
  requested_count: 5,
  topic_count: 5,
  status: "succeeded",
  error_message: null,
  created_at: "2026-07-02T00:22:00Z",
  updated_at: "2026-07-02T00:22:20Z",
};

const failedTopicBatch = {
  batch_id: "66666666-6666-4666-8666-666666666666",
  project_id: projectId,
  supplement_of_batch_id: null,
  prompt: "失败的 AI 选题生成",
  requested_count: 5,
  topic_count: 0,
  status: "failed",
  error_message: "invalid topic JSON",
  created_at: "2026-07-02T00:20:00Z",
  updated_at: "2026-07-02T00:20:20Z",
};

const supplementTopicBatch = {
  batch_id: supplementTopicBatchId,
  project_id: projectId,
  supplement_of_batch_id: previousTopicBatchId,
  prompt: "补充上一批 AI 内容流水线选题",
  requested_count: 2,
  topic_count: 2,
  status: "succeeded",
  error_message: null,
  created_at: "2026-07-06T10:05:00Z",
  updated_at: "2026-07-06T10:05:20Z",
};

const overflowTopics = Array.from({ length: 12 }, (_, index) => ({
  ...ideaTopic,
  topic_id: `77777777-7777-4777-8777-${String(index + 1).padStart(12, "0")}`,
  title: `AI 内容流水线扩展选题 ${index + 1}`,
  score: 70 - index,
  created_at: `2026-07-02T00:${String(10 + index).padStart(2, "0")}:00Z`,
  updated_at: `2026-07-02T00:${String(10 + index).padStart(2, "0")}:00Z`,
}));

const preparedTopic = {
  topic: approvedTopic,
  topic_snapshot: {
    topic_id: approvedTopic.topic_id,
    title: approvedTopic.title,
    angle: approvedTopic.angle,
    target_audience: approvedTopic.target_audience,
    hook_points: approvedTopic.hook_points,
    content_type: approvedTopic.content_type,
    score: approvedTopic.score,
    score_reason: approvedTopic.score_reason,
    tags: approvedTopic.tags,
    source: approvedTopic.source,
    status: approvedTopic.status,
    created_at: approvedTopic.created_at,
  },
  script_request: {
    project_id: projectId,
    topic_id: approvedTopic.topic_id,
    topic: approvedTopic.title,
    style: "knowledge",
    scene_count: 6,
  },
};

const topicScriptId = "abababab-bbbb-4ccc-8ddd-eeeeeeeeeeee";

const topicScriptSummary = {
  script_id: topicScriptId,
  title: "AI 内容流水线脚本",
  status: "draft",
  scene_count: 1,
  parent_id: null,
  created_at: "2026-07-02T00:30:00Z",
};

const topicScriptDetail = {
  ...topicScriptSummary,
  project_id: projectId,
  topic_id: approvedTopic.topic_id,
  topic_snapshot: preparedTopic.topic_snapshot,
  hook: "为什么同样用 AI，有的人只是更忙？",
  scenes: [
    {
      scene_id: "cdcdcdcd-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
      sequence: 1,
      narration: "先用一个真实团队的改造前后做对比。",
      visual_description: "内容团队会议白板从混乱任务变成清晰流程。",
      emotion: "清晰",
      duration_sec: 8,
    },
  ],
  updated_at: "2026-07-02T00:30:00Z",
};

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

async function mockContentStrategyWorkflow(page: Page) {
  let generatedFromTopic = false;

  await page.unroute(/\/api\/video-workspace\/menus$/);
  await page.route(/\/api\/video-workspace\/menus$/, async (route) => {
    await route.fulfill({ contentType: "application/json", json: { menus: contentStrategyWorkspaceMenus } });
  });
  await page.route(new RegExp(`/api/projects/${projectId}/topic-generation-batches$`), async (route) => {
    await route.fulfill({ contentType: "application/json", json: { batches: [] } });
  });
  await page.route(new RegExp(`/api/projects/${projectId}/topics(\\?.*)?$`), async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: {
        topics: [
          ideaTopic,
          generatedFromTopic ? { ...approvedTopic, status: "scripted" } : approvedTopic,
          archivedTopic,
          ...overflowTopics,
        ],
        stats: generatedFromTopic
          ? { total: 15, idea: 13, approved: 0, scripted: 1, archived: 1 }
          : { total: 15, idea: 13, approved: 1, scripted: 0, archived: 1 },
      },
    });
  });
  await page.route(new RegExp(`/api/projects/${projectId}/scripts(\\?.*)?$`), async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: generatedFromTopic
        ? { scripts: [topicScriptSummary], total: 1, limit: 20, offset: 0 }
        : { scripts: [], total: 0, limit: 20, offset: 0 },
    });
  });
  await page.route(new RegExp(`/api/scripts/${topicScriptId}$`), async (route) => {
    await route.fulfill({ contentType: "application/json", json: topicScriptDetail });
  });
  await page.route(new RegExp(`/api/topics/${approvedTopic.topic_id}/prepare-script$`), async (route) => {
    expect(route.request().method()).toBe("POST");
    expect(route.request().postDataJSON()).toEqual({ style: "knowledge", scene_count: 6 });
    await route.fulfill({ contentType: "application/json", json: preparedTopic });
  });
  await page.route(/\/api\/scripts\/generate$/, async (route) => {
    expect(route.request().method()).toBe("POST");
    expect(route.request().postDataJSON()).toEqual({
      project_id: projectId,
      topic_id: approvedTopic.topic_id,
      style: "knowledge",
      scene_count: 6,
    });
    generatedFromTopic = true;
    await route.fulfill({ contentType: "application/json", json: topicScriptDetail });
  });
}

async function mockContentStrategyHistoryWorkflow(page: Page) {
  let supplementGenerated = false;

  await page.unroute(/\/api\/video-workspace\/menus$/);
  await page.route(/\/api\/video-workspace\/menus$/, async (route) => {
    await route.fulfill({ contentType: "application/json", json: { menus: contentStrategyWorkspaceMenus } });
  });
  await page.route(new RegExp(`/api/projects/${projectId}/topic-generation-batches$`), async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: {
        batches: supplementGenerated
          ? [supplementTopicBatch, latestTopicBatch, failedTopicBatch, previousTopicBatch]
          : [latestTopicBatch, failedTopicBatch, previousTopicBatch],
      },
    });
  });
  await page.route(new RegExp(`/api/projects/${projectId}/topics(\\?.*)?$`), async (route) => {
    const url = new URL(route.request().url());
    const batchId = url.searchParams.get("batch_id");
    if (batchId === supplementTopicBatch.batch_id) {
      await route.fulfill({
        contentType: "application/json",
        json: {
          topics: [
            {
              ...ideaTopic,
              batch_id: supplementTopicBatch.batch_id,
              title: "补充批次选题",
              source: "agent",
            },
          ],
          stats: { total: 4, idea: 2, approved: 1, scripted: 1, archived: 0 },
        },
      });
      return;
    }
    if (batchId === previousTopicBatch.batch_id) {
      await route.fulfill({
        contentType: "application/json",
        json: {
          topics: [{ ...approvedTopic, batch_id: previousTopicBatch.batch_id, title: "历史批次选题" }],
          stats: { total: 3, idea: 1, approved: 1, scripted: 1, archived: 0 },
        },
      });
      return;
    }
    await route.fulfill({
      contentType: "application/json",
      json: {
        topics: [
          { ...approvedTopic, batch_id: latestTopicBatch.batch_id },
          { ...scriptedTopic, batch_id: latestTopicBatch.batch_id },
        ],
        stats: { total: 3, idea: 1, approved: 1, scripted: 1, archived: 0 },
      },
    });
  });
  await page.route(new RegExp(`/api/projects/${projectId}/scripts(\\?.*)?$`), async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: { scripts: [], total: 0, limit: 20, offset: 0 },
    });
  });
  await page.route(/\/api\/agent\/conversations$/, async (route) => {
    expect(route.request().method()).toBe("POST");
    expect(route.request().postDataJSON()).toEqual({
      project_id: projectId,
      agent_type: "topic",
      title: "选题 Agent 对话",
    });
    await route.fulfill({ contentType: "application/json", json: topicConversation });
  });
  await page.route(new RegExp(`/api/agent/conversations/${topicConversationId}/messages$`), async (route) => {
    if (route.request().method() === "GET") {
      await route.fulfill({ contentType: "application/json", json: { messages: [] } });
      return;
    }

    expect(route.request().method()).toBe("POST");
    expect(route.request().postDataJSON()).toEqual({
      content: supplementUserMessage.content,
      supplement_of_batch_id: previousTopicBatch.batch_id,
    });
    supplementGenerated = true;
    await route.fulfill({
      contentType: "application/json",
      json: {
        user_message: supplementUserMessage,
        assistant_message: supplementAssistantMessage,
        run: supplementAgentRun,
      },
    });
  });
}

async function mockEmptyContentStrategyWorkflow(page: Page) {
  await page.unroute(/\/api\/video-workspace\/menus$/);
  await page.route(/\/api\/video-workspace\/menus$/, async (route) => {
    await route.fulfill({ contentType: "application/json", json: { menus: contentStrategyWorkspaceMenus } });
  });
  await page.route(new RegExp(`/api/projects/${projectId}/topic-generation-batches$`), async (route) => {
    await route.fulfill({ contentType: "application/json", json: { batches: [] } });
  });
  await page.route(new RegExp(`/api/projects/${projectId}/topics(\\?.*)?$`), async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: { topics: [], stats: { total: 0, idea: 0, approved: 0, scripted: 0, archived: 0 } },
    });
  });
  await page.route(new RegExp(`/api/projects/${projectId}/scripts(\\?.*)?$`), async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: { scripts: [], total: 0, limit: 20, offset: 0 },
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
  await expect(workspaceMenu.getByRole("button", { name: /内容策略/ })).toHaveClass(/active/);
  await expect(page.getByRole("heading", { name: "内容策略" })).toBeVisible();
  await workspaceMenu.getByRole("button", { name: /脚本创作/ }).click();
  await expect(workspaceMenu.getByRole("button", { name: /脚本创作/ })).toHaveClass(/active/);
  await expect(workspaceMenu.getByRole("button", { name: /内容策略/ })).toBeEnabled();
  await expect(workspaceMenu.getByRole("button", { name: /素材管理/ })).toBeDisabled();

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
  await page.getByRole("navigation", { name: "视频工作台菜单" }).getByRole("button", { name: /脚本创作/ }).click();

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

test("内容策略页从已确认选题确认参数并生成脚本", async ({ page }) => {
  await mockContentStrategyWorkflow(page);
  await page.goto("/");

  const workspaceMenu = page.getByRole("navigation", { name: "视频工作台菜单" });
  await expect(workspaceMenu.getByRole("button", { name: /内容策略/ })).toHaveClass(/active/);

  await expect(page.getByRole("heading", { name: "内容策略" })).toBeVisible();
  await expect(page.getByText("全部选题")).toBeVisible();
  const summaryLayout = await page.locator('[aria-label="选题统计"]').evaluate((stats) =>
    Array.from(stats.children).map((card) => {
      const rect = card.getBoundingClientRect();
      return { text: card.textContent, width: Math.round(rect.width), height: Math.round(rect.height) };
    }),
  );
  expect(summaryLayout).toHaveLength(3);
  expect(summaryLayout.map((card) => card.text)).toEqual(["15全部选题", "1已确认", "0已成稿"]);
  for (const card of summaryLayout) {
    expect(card.width).toBeGreaterThanOrEqual(120);
    expect(card.width).toBeLessThanOrEqual(150);
    expect(card.height).toBeLessThanOrEqual(76);
  }
  const poolLayout = await page.locator('[aria-label="选题池"]').evaluate((pool) => {
    const poolRect = pool.getBoundingClientRect();
    const detail = document.querySelector(".topicDetailColumn")?.getBoundingClientRect();
    const filters = pool.querySelector(".topicFilters")?.getBoundingClientRect();
    return {
      filtersTop: filters ? Math.round(filters.top - poolRect.top) : null,
      poolWidth: Math.round(poolRect.width),
      detailWidth: detail ? Math.round(detail.width) : null,
    };
  });
  expect(poolLayout.filtersTop).not.toBeNull();
  expect(poolLayout.filtersTop!).toBeLessThanOrEqual(96);
  expect(poolLayout.detailWidth).not.toBeNull();
  expect(poolLayout.detailWidth!).toBeGreaterThanOrEqual(340);
  expect(poolLayout.poolWidth).toBeGreaterThanOrEqual(360);
  const topicListScroll = await page.locator(".topicList").evaluate((list) => ({
    clientHeight: Math.round(list.clientHeight),
    scrollHeight: Math.round(list.scrollHeight),
    overflowY: window.getComputedStyle(list).overflowY,
    firstMeta: list.querySelector(".topicMeta")?.textContent,
  }));
  expect(topicListScroll.overflowY).toBe("auto");
  expect(topicListScroll.scrollHeight).toBeGreaterThan(topicListScroll.clientHeight);
  expect(topicListScroll.firstMeta).toBe("来源：人工 · 类型：知识科普");
  await expect(page.getByRole("combobox", { name: "选题状态筛选" })).toHaveCount(0);
  const statusFilters = page.locator('[aria-label="选题状态筛选"]');
  await expect(statusFilters.getByRole("button")).toHaveCount(5);
  await expect(statusFilters.getByRole("button", { name: "全部" })).toHaveClass(/selected/);
  await expect(statusFilters.getByRole("button", { name: "待评估" })).toBeVisible();
  await expect(statusFilters.getByRole("button", { name: "已确认" })).toBeVisible();
  await expect(statusFilters.getByRole("button", { name: "已成稿" })).toBeVisible();
  await expect(statusFilters.getByRole("button", { name: "已归档" })).toBeVisible();
  await expect(page.getByRole("button", { name: /普通人如何搭建 AI 内容流水线/ })).toBeVisible();
  await page.getByRole("button", { name: /普通人如何搭建 AI 内容流水线/ }).click();
  await expect(page.getByRole("region", { name: "选题详情" }).getByRole("button", { name: "生成脚本" })).toBeVisible();
  await page.getByRole("region", { name: "选题详情" }).getByRole("button", { name: "生成脚本" }).click();

  const dialog = page.getByRole("dialog", { name: "脚本生成确认" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText(approvedTopic.title)).toBeVisible();
  await expect(dialog.getByLabel("分镜数")).toHaveValue("6");
  await dialog.getByRole("button", { name: "确认生成" }).click();

  await expect(page.getByRole("heading", { name: "AI 内容流水线脚本" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "来源选题" })).toBeVisible();
  const sourceTopicPanel = page.locator(".sourceTopicPanel");
  await expect(sourceTopicPanel.getByText(approvedTopic.title, { exact: true })).toBeVisible();
  await expect(sourceTopicPanel.getByText(approvedTopic.angle)).toBeVisible();
});

test("内容策略历史生成列表页展示批次并限制已成稿选题删除", async ({ page }) => {
  await mockContentStrategyHistoryWorkflow(page);
  await page.goto("/");

  const workspaceMenu = page.getByRole("navigation", { name: "视频工作台菜单" });
  const contentStrategySubMenu = workspaceMenu.getByLabel("内容策略二级菜单");
  await expect(contentStrategySubMenu.getByRole("button")).toHaveText(["历史生成", "当前选题池"]);
  await expect(contentStrategySubMenu.getByRole("button", { name: "当前选题池" })).toHaveClass(/active/);
  const contentStrategyRows = await page.locator(".contentStrategyWorkspace").evaluate((element) =>
    getComputedStyle(element).gridTemplateRows.split(" ").length,
  );
  expect(contentStrategyRows).toBe(2);
  await expect(page.getByRole("navigation", { name: "内容策略视图菜单" })).toHaveCount(0);
  await contentStrategySubMenu.getByRole("button", { name: "历史生成" }).click();

  const historyPage = page.getByRole("region", { name: "历史生成列表页" });
  await expect(historyPage).toBeVisible();
  await expect(historyPage.getByRole("complementary", { name: "历史生成批次" })).toBeVisible();
  await expect(historyPage.getByRole("region", { name: "当前主题选题" })).toBeVisible();
  await expect(historyPage.getByRole("complementary", { name: "补充操作" })).toBeVisible();
  const historyColumns = await historyPage.evaluate((element) =>
    getComputedStyle(element).gridTemplateColumns.split(" ").filter(Boolean).length,
  );
  expect(historyColumns).toBe(3);
  await expect(contentStrategySubMenu.getByRole("button", { name: "历史生成" })).toHaveClass(/active/);
  await expect(historyPage.getByRole("button", { name: /最新一批 AI 工具选题/ })).toHaveClass(/selected/);
  await expect(historyPage.getByRole("button", { name: /上一批 AI 内容流水线选题/ })).toBeVisible();
  await expect(historyPage.getByRole("button", { name: /失败的 AI 选题生成/ })).toHaveCount(0);

  const removableRow = historyPage.getByRole("article", { name: `历史选题：${approvedTopic.title}` });
  await expect(removableRow.getByRole("button", { name: "移除" })).toBeVisible();
  const lockedRow = historyPage.getByRole("article", { name: `历史选题：${scriptedTopic.title}` });
  await expect(lockedRow.getByText("已生成脚本，不可删除")).toBeVisible();
  await expect(lockedRow.getByRole("button", { name: "移除" })).toHaveCount(0);

  await historyPage.getByRole("button", { name: /上一批 AI 内容流水线选题/ }).click();
  await expect(historyPage.getByRole("article", { name: "历史选题：历史批次选题" })).toBeVisible();
  const supplementPanel = page.getByRole("region", { name: "补充选题" });
  await supplementPanel.getByLabel("补充要求").fill(supplementUserMessage.content);
  await supplementPanel.getByRole("button", { name: "补充生成" }).click();
  await expect(historyPage.getByRole("button", { name: /补充上一批 AI 内容流水线选题/ }).first()).toHaveClass(/selected/);
  const topicGroupPanel = historyPage.getByRole("region", { name: "当前主题选题" });
  await expect(topicGroupPanel.getByRole("article", { name: "历史选题：历史批次选题" })).toBeVisible();
  await expect(topicGroupPanel.getByRole("article", { name: "历史选题：补充批次选题" })).toBeVisible();
  await expect(topicGroupPanel.getByText("原始生成", { exact: true })).toBeVisible();
  await expect(topicGroupPanel.getByText("补充生成", { exact: true })).toBeVisible();
  await expect(page.getByRole("region", { name: "关联补充批次" })).toBeVisible();

  await contentStrategySubMenu.getByRole("button", { name: "当前选题池" }).click();
  const topicPool = page.getByRole("region", { name: "选题池" });
  await expect(topicPool.getByRole("button", { name: /补充批次选题/ })).toBeVisible();
  await expect(topicPool.getByRole("button", { name: /最新批次选题/ })).toHaveCount(0);
});

test("内容策略空选题池布局贴近原型顶部", async ({ page }) => {
  await mockEmptyContentStrategyWorkflow(page);
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "内容策略" })).toBeVisible();
  await expect(page.getByText("还没有选题")).toBeVisible();
  const topicAgent = page.getByRole("region", { name: "选题 Agent" });
  const agentBox = await topicAgent.boundingBox();
  const agentMessagesBox = await topicAgent.getByLabel("选题 Agent 消息").boundingBox();
  const agentInputBox = await topicAgent.getByLabel("生成要求").boundingBox();
  const agentButtonBox = await topicAgent.getByRole("button", { name: "生成选题" }).boundingBox();
  expect(agentBox).not.toBeNull();
  expect(agentMessagesBox).not.toBeNull();
  expect(agentInputBox).not.toBeNull();
  expect(agentButtonBox).not.toBeNull();
  expect(Math.round(agentBox!.height)).toBeLessThanOrEqual(480);
  expect(Math.round(agentInputBox!.y - agentBox!.y)).toBeLessThanOrEqual(128);
  expect(Math.round(agentButtonBox!.width)).toBeLessThanOrEqual(160);
  expect(Math.round(agentButtonBox!.y + agentButtonBox!.height - agentBox!.y)).toBeLessThanOrEqual(260);
  expect(Math.round(agentMessagesBox!.y - agentBox!.y)).toBeLessThanOrEqual(310);

  const topicPool = page.getByRole("region", { name: "选题池" });
  const poolBox = await topicPool.boundingBox();
  const filtersBox = await topicPool.getByLabel("选题状态筛选").boundingBox();
  const emptyTitleBox = await topicPool.getByText("还没有选题").boundingBox();
  const emptyHintBox = await topicPool
    .getByText("可以手动新增，或用选题 Agent 生成候选后再确认。")
    .boundingBox();
  expect(poolBox).not.toBeNull();
  expect(filtersBox).not.toBeNull();
  expect(emptyTitleBox).not.toBeNull();
  expect(emptyHintBox).not.toBeNull();
  const filtersTop = Math.round(filtersBox!.y - poolBox!.y);
  const emptyTop = Math.round(emptyTitleBox!.y - poolBox!.y);
  const emptyHeight = Math.round(emptyHintBox!.y + emptyHintBox!.height - emptyTitleBox!.y);
  expect(filtersTop).toBeLessThanOrEqual(96);
  expect(emptyTop).toBeLessThanOrEqual(180);
  expect(emptyHeight).toBeLessThanOrEqual(150);
});
