import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createElement } from "react";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Home from "./page";
import * as api from "./lib/api";
import type {
  AgentConversation,
  AgentMessage,
  AgentRun,
  ContentTopic,
  ContentTopicListResponse,
  ProjectListResponse,
  ScriptDetail,
  ScriptListResponse,
  TopicGenerationBatchListResponse,
  WorkspaceMenuListResponse,
} from "./lib/api";

vi.mock("./lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./lib/api")>();
  return {
    ...actual,
    checkHealth: vi.fn(),
    createApiClient: vi.fn(() => ({ baseUrl: "http://api.test", fetcher: vi.fn() })),
    listWorkspaceMenus: vi.fn(),
    listProjects: vi.fn(),
    listScripts: vi.fn(),
    getScript: vi.fn(),
    generateScript: vi.fn(),
    listContentTopics: vi.fn(),
    listTopicGenerationBatches: vi.fn(),
    createContentTopic: vi.fn(),
    deleteContentTopic: vi.fn(),
    updateContentTopic: vi.fn(),
    updateContentTopicStatus: vi.fn(),
    prepareScriptFromTopic: vi.fn(),
    createAgentConversation: vi.fn(),
    listAgentMessages: vi.fn(),
    sendAgentMessage: vi.fn(),
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
  topic_id: null,
  source_topic_title: null,
  title: "程序员必看：ChatGPT工作流",
  status: "draft" as const,
  scene_count: 2,
  parent_id: null,
  created_at: "2026-07-02T00:05:00Z",
};

const scriptDetail: ScriptDetail = {
  ...scriptSummary,
  project_id: project.project_id,
  topic_id: null,
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

const secondScriptSummary = {
  script_id: "99999999-9999-4999-8999-999999999999",
  topic_id: null,
  source_topic_title: null,
  title: "第二版脚本：AI 剪辑流程",
  status: "draft" as const,
  scene_count: 1,
  parent_id: null,
  created_at: "2026-07-02T00:10:00Z",
};

const secondScriptDetail: ScriptDetail = {
  ...secondScriptSummary,
  project_id: project.project_id,
  topic_id: null,
  hook: "剪辑还在手动拖时间线？",
  scenes: [
    {
      scene_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
      sequence: 1,
      narration: "AI 可以先完成粗剪和节奏整理。",
      visual_description: "剪辑软件时间线自动排列素材。",
      emotion: "高效",
      duration_sec: 10,
    },
  ],
  updated_at: "2026-07-02T00:10:00Z",
};

const workspaceMenus: WorkspaceMenuListResponse = {
  menus: [
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
  ],
};

const contentStrategyWorkspaceMenus: WorkspaceMenuListResponse = {
  menus: [
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
  ],
};

const ideaTopic: ContentTopic = {
  topic_id: "44444444-4444-4444-8444-444444444444",
  project_id: project.project_id,
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

const approvedTopic: ContentTopic = {
  ...ideaTopic,
  topic_id: "55555555-5555-4555-8555-555555555555",
  title: "普通人如何搭建 AI 内容流水线",
  status: "approved",
  source: "agent",
  batch_id: "77777777-7777-4777-8777-777777777777",
  score: 82,
};

const scriptedTopic: ContentTopic = {
  ...ideaTopic,
  topic_id: "657dcd2b-ebbd-47fd-ac1d-15663bae6cfa",
  title: "程序员如何用 AI 搭一条短视频生产流水线：从脚本到发布",
  status: "scripted",
  source: "agent",
  score: 94,
};

const scriptedTopicScriptSummary = {
  script_id: "0f784f83-e41d-4af0-9000-afdccf5a4679",
  title: "程序员的AI短视频流水线",
  status: "draft" as const,
  scene_count: 6,
  parent_id: null,
  topic_id: scriptedTopic.topic_id,
  source_topic_title: scriptedTopic.title,
  created_at: "2026-07-06T10:25:45.998632Z",
};

const archivedTopic: ContentTopic = {
  ...ideaTopic,
  topic_id: "66666666-6666-4666-8666-666666666666",
  title: "已经过时的工具清单",
  status: "archived",
  score: 40,
};

const latestTopicBatch = {
  batch_id: "88888888-8888-4888-8888-888888888888",
  project_id: project.project_id,
  prompt: "最新一批 AI 工具选题",
  requested_count: 5,
  topic_count: 5,
  status: "succeeded" as const,
  error_message: null,
  created_at: "2026-07-06T10:00:00Z",
  updated_at: "2026-07-06T10:00:10Z",
};

const previousTopicBatch = {
  batch_id: "77777777-7777-4777-8777-777777777777",
  project_id: project.project_id,
  prompt: "上一批 AI 内容流水线选题",
  requested_count: 5,
  topic_count: 5,
  status: "succeeded" as const,
  error_message: null,
  created_at: "2026-07-02T00:22:00Z",
  updated_at: "2026-07-02T00:22:20Z",
};

const failedTopicBatch = {
  batch_id: "66666666-6666-4666-8666-666666666666",
  project_id: project.project_id,
  prompt: "失败的 AI 选题生成",
  requested_count: 5,
  topic_count: 0,
  status: "failed" as const,
  error_message: "invalid topic JSON",
  created_at: "2026-07-02T00:20:00Z",
  updated_at: "2026-07-02T00:20:20Z",
};

const topicBatchListResponse: TopicGenerationBatchListResponse = {
  batches: [latestTopicBatch, failedTopicBatch, previousTopicBatch],
};

const topicListResponse: ContentTopicListResponse = {
  topics: [ideaTopic, approvedTopic, archivedTopic],
  stats: { total: 3, idea: 1, approved: 1, scripted: 0, archived: 1 },
};

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
    project_id: project.project_id,
    topic_id: approvedTopic.topic_id,
    topic: approvedTopic.title,
    style: "knowledge" as const,
    scene_count: 6,
  },
};

const topicGeneratedScript: ScriptDetail = {
  script_id: "abababab-bbbb-4ccc-8ddd-eeeeeeeeeeee",
  project_id: project.project_id,
  topic_id: approvedTopic.topic_id,
  topic_snapshot: preparedTopic.topic_snapshot,
  title: "AI 内容流水线脚本",
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
  status: "draft",
  parent_id: null,
  created_at: "2026-07-02T00:30:00Z",
  updated_at: "2026-07-02T00:30:00Z",
};

const conversation: AgentConversation = {
  conversation_id: "55555555-5555-4555-8555-555555555555",
  project_id: project.project_id,
  agent_type: "script",
  subject_type: "script",
  subject_id: scriptSummary.script_id,
  title: "脚本 Agent 对话",
  status: "active",
  metadata: {},
  created_at: "2026-07-02T00:06:00Z",
  updated_at: "2026-07-02T00:06:00Z",
};

const userMessage: AgentMessage = {
  message_id: "66666666-6666-4666-8666-666666666666",
  conversation_id: conversation.conversation_id,
  role: "user",
  content: "把第 2 镜改得更有冲突感",
  metadata: {},
  created_at: "2026-07-02T00:07:00Z",
};

const assistantMessage: AgentMessage = {
  message_id: "77777777-7777-4777-8777-777777777777",
  conversation_id: conversation.conversation_id,
  role: "assistant",
  content: "已更新第 2 镜，时间轴已刷新。",
  metadata: { scene_sequence: 2 },
  created_at: "2026-07-02T00:07:05Z",
};

const generatedScriptSummary = {
  script_id: "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee",
  topic_id: null,
  source_topic_title: null,
  title: "ChatGPT 工作流新脚本",
  status: "draft" as const,
  scene_count: 3,
  parent_id: null,
  created_at: "2026-07-02T00:12:00Z",
};

const generatedScriptDetail: ScriptDetail = {
  ...generatedScriptSummary,
  project_id: project.project_id,
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
  ],
  updated_at: "2026-07-02T00:12:00Z",
};

const generatedAssistantMessage: AgentMessage = {
  ...assistantMessage,
  content: "已创建 3 镜脚本，时间轴已打开。",
  metadata: {
    intent: "generate_script",
    script_id: generatedScriptSummary.script_id,
    script_created: true,
    needs_input: false,
    missing_fields: [],
  },
};

const missingInputAssistantMessage: AgentMessage = {
  ...assistantMessage,
  content: "请补充选题、风格和分镜数。",
  metadata: {
    intent: "generate_script",
    script_id: null,
    script_created: false,
    needs_input: true,
    missing_fields: ["topic", "style", "scene_count"],
  },
};

const unboundConversation: AgentConversation = {
  ...conversation,
  subject_type: null,
  subject_id: null,
  title: "脚本 Agent 对话",
};

const topicConversation: AgentConversation = {
  conversation_id: "12121212-1212-4212-8212-121212121212",
  project_id: project.project_id,
  agent_type: "topic",
  subject_type: null,
  subject_id: null,
  title: "选题 Agent 对话",
  status: "active",
  metadata: {},
  created_at: "2026-07-02T00:22:00Z",
  updated_at: "2026-07-02T00:22:00Z",
};

const topicUserMessage: AgentMessage = {
  message_id: "23232323-2323-4232-8232-232323232323",
  conversation_id: topicConversation.conversation_id,
  role: "user",
  content: "本周 AI 工具方向，生成 3 个选题",
  metadata: {},
  created_at: "2026-07-02T00:22:10Z",
};

const topicAssistantMessage: AgentMessage = {
  message_id: "34343434-3434-4343-8343-343434343434",
  conversation_id: topicConversation.conversation_id,
  role: "assistant",
  content: "已生成 3 个选题候选，已写入选题池。",
  metadata: {
    intent: "generate_topics",
    batch_id: "45454545-4545-4545-8545-454545454545",
    created_topic_ids: [ideaTopic.topic_id, approvedTopic.topic_id],
    topic_count: 3,
  },
  created_at: "2026-07-02T00:22:20Z",
};

const topicAgentRun: AgentRun = {
  run_id: "56565656-5656-4656-8656-565656565656",
  conversation_id: topicConversation.conversation_id,
  project_id: project.project_id,
  agent_type: "topic",
  status: "completed",
  input: { content: topicUserMessage.content },
  output: { reply: topicAssistantMessage.content },
  error: null,
  started_at: "2026-07-02T00:22:10Z",
  finished_at: "2026-07-02T00:22:20Z",
};

const agentRun: AgentRun = {
  run_id: "88888888-8888-4888-8888-888888888888",
  conversation_id: conversation.conversation_id,
  project_id: project.project_id,
  agent_type: "script",
  status: "completed",
  input: { content: userMessage.content },
  output: { reply: assistantMessage.content },
  error: null,
  started_at: "2026-07-02T00:07:00Z",
  finished_at: "2026-07-02T00:07:05Z",
};

function menuNode(
  menuKey: string,
  label: string,
  isEnabled: boolean,
  status: "active" | "planned" | "disabled",
  sortOrder: number,
) {
  return {
    menu_id: `00000000-0000-4000-8000-${String(sortOrder).padStart(12, "0")}`,
    menu_key: menuKey,
    label,
    description: `${label}说明`,
    route_path: `/${menuKey}`,
    icon: "circle",
    menu_type: "section" as const,
    module_key: menuKey,
    agent_key: null,
    sort_order: sortOrder,
    is_enabled: isEnabled,
    status,
    metadata: { phase: status === "active" ? 1 : 2 },
    children: [],
  };
}

function mockProjects(response: ProjectListResponse) {
  vi.mocked(api.checkHealth).mockResolvedValue(true);
  vi.mocked(api.listProjects).mockResolvedValue(response);
}

function mockScripts(response: ScriptListResponse) {
  vi.mocked(api.listScripts).mockResolvedValue(response);
}

function mockTopics(response: ContentTopicListResponse = topicListResponse) {
  vi.mocked(api.listContentTopics).mockResolvedValue(response);
}

function mockTopicBatches(response: TopicGenerationBatchListResponse = { batches: [] }) {
  vi.mocked(api.listTopicGenerationBatches).mockResolvedValue(response);
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((promiseResolve, promiseReject) => {
    resolve = promiseResolve;
    reject = promiseReject;
  });
  return { promise, resolve, reject };
}

async function openScriptCreationWorkspace() {
  fireEvent.click(await screen.findByRole("button", { name: /脚本创作/ }));
}

async function flushAsyncWork() {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

describe("video-agent 视频工作台页面", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(workspaceMenus);
    mockProjects({ projects: [] });
    mockScripts({ scripts: [], total: 0, limit: 20, offset: 0 });
    mockTopics({ topics: [], stats: { total: 0, idea: 0, approved: 0, scripted: 0, archived: 0 } });
    mockTopicBatches();
    vi.mocked(api.getScript).mockResolvedValue(scriptDetail);
    vi.mocked(api.generateScript).mockResolvedValue(topicGeneratedScript);
    vi.mocked(api.createContentTopic).mockResolvedValue(ideaTopic);
    vi.mocked(api.deleteContentTopic).mockResolvedValue({
      topic_id: ideaTopic.topic_id,
      deleted_at: "2026-07-07T10:00:00Z",
    });
    vi.mocked(api.updateContentTopic).mockResolvedValue(ideaTopic);
    vi.mocked(api.updateContentTopicStatus).mockResolvedValue({ ...ideaTopic, status: "approved" });
    vi.mocked(api.prepareScriptFromTopic).mockResolvedValue(preparedTopic);
    vi.mocked(api.createAgentConversation).mockResolvedValue(conversation);
    vi.mocked(api.listAgentMessages).mockResolvedValue({ messages: [] });
    vi.mocked(api.sendAgentMessage).mockResolvedValue({
      user_message: userMessage,
      assistant_message: assistantMessage,
      run: agentRun,
    });
  });

  it("展示 VEDIO-AGENT 品牌、中文标题和业务流程菜单", async () => {
    render(createElement(Home));

    expect((await screen.findAllByText("VEDIO-AGENT")).length).toBeGreaterThanOrEqual(1);
    expect(screen.getByRole("heading", { name: "视频工作台" })).toBeInTheDocument();

    const menu = screen.getByLabelText("视频工作台菜单");
    for (const label of [
      "内容策略",
      "脚本创作",
      "素材管理",
      "作品生产",
      "发布运营",
      "数据分析",
      "工作流任务",
    ]) {
      expect(within(menu).getByText(label)).toBeInTheDocument();
    }

    for (const label of ["选题智能体", "脚本智能体", "素材智能体", "视频智能体", "发布智能体", "优化智能体"]) {
      expect(within(menu).queryByText(label)).not.toBeInTheDocument();
    }

    expect(within(menu).getByRole("button", { name: /内容策略/ })).toHaveClass("active");
    expect(await screen.findByRole("heading", { name: "内容策略" })).toBeInTheDocument();
    expect(within(menu).getByRole("button", { name: /脚本创作/ })).toBeEnabled();
    expect(within(menu).getByRole("button", { name: /内容策略/ })).toBeEnabled();
    expect(within(menu).getByRole("button", { name: /素材管理/ })).toBeDisabled();
  });

  it("菜单加载失败时不回退旧智能体菜单", async () => {
    vi.mocked(api.listWorkspaceMenus).mockRejectedValue(new Error("菜单接口不可用"));
    render(createElement(Home));

    expect(await screen.findByText("菜单接口不可用")).toBeInTheDocument();
    const menu = screen.getByLabelText("视频工作台菜单");
    expect(within(menu).queryByText("脚本智能体")).not.toBeInTheDocument();
  });

  it("不在脚本生产工作台展示项目创建或项目管理入口", async () => {
    render(createElement(Home));
    await openScriptCreationWorkspace();

    expect(await screen.findByRole("heading", { name: "脚本 Agent 对话" })).toBeInTheDocument();
    expect(screen.getByLabelText("当前项目")).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "项目上下文" })).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "还没有项目" })).not.toBeInTheDocument();
    expect(screen.queryByLabelText("项目名称")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "创建项目" })).not.toBeInTheDocument();
    expect(screen.queryByText(/创建项目|项目管理/)).not.toBeInTheDocument();
  });

  it("右侧只展示单一脚本 Agent 对话入口", async () => {
    mockProjects({ projects: [project] });
    render(createElement(Home));
    await openScriptCreationWorkspace();

    const actionColumn = await screen.findByLabelText("脚本 Agent 操作");

    expect(within(actionColumn).getByRole("heading", { name: "脚本 Agent 对话" })).toBeInTheDocument();
    expect(within(actionColumn).queryByRole("heading", { name: "生成脚本" })).not.toBeInTheDocument();
    expect(within(actionColumn).queryByLabelText("选题")).not.toBeInTheDocument();
    expect(within(actionColumn).queryByLabelText("分镜数")).not.toBeInTheDocument();
    expect(within(actionColumn).getAllByRole("textbox")).toHaveLength(1);
  });

  it("有脚本时展示时间轴对照视图", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [scriptSummary], total: 1, limit: 20, offset: 0 });
    render(createElement(Home));
    await openScriptCreationWorkspace();

    expect((await screen.findAllByText("程序员必看：ChatGPT工作流")).length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("时间轴对照视图")).toBeInTheDocument();
    expect(screen.getByText("第 1 镜")).toBeInTheDocument();
    expect(screen.getAllByText("旁白").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("画面指令").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("传统程序员每天要写大量重复代码。")).toBeInTheDocument();
    expect(screen.getByText("程序员盯着屏幕，快速切换多个代码文件。"));
  });

  it("脚本详情旁显示脚本 Agent 对话面板", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [scriptSummary], total: 1, limit: 20, offset: 0 });
    render(createElement(Home));
    await openScriptCreationWorkspace();

    const panel = await screen.findByRole("region", { name: "脚本 Agent 对话" });

    expect(await within(panel).findByText("当前项目：科技博主 / 脚本：程序员必看：ChatGPT工作流")).toBeInTheDocument();
    expect(within(panel).queryByText(/绑定：/)).not.toBeInTheDocument();
    expect(within(panel).getByPlaceholderText("描述要修改的分镜方向...")).toBeEnabled();
    expect(within(panel).getByRole("button", { name: "发送" })).toBeEnabled();
  });

  it("脚本创作列表展示来源选题，便于找到内容策略生成的稿件", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [scriptedTopicScriptSummary], total: 1, limit: 20, offset: 0 });
    vi.mocked(api.getScript).mockResolvedValue({
      ...scriptDetail,
      script_id: scriptedTopicScriptSummary.script_id,
      title: scriptedTopicScriptSummary.title,
      topic_id: scriptedTopic.topic_id,
      topic_snapshot: {
        topic_id: scriptedTopic.topic_id,
        title: scriptedTopic.title,
        angle: scriptedTopic.angle,
        target_audience: scriptedTopic.target_audience,
        hook_points: scriptedTopic.hook_points,
        content_type: scriptedTopic.content_type,
        score: scriptedTopic.score,
        score_reason: scriptedTopic.score_reason,
        tags: scriptedTopic.tags,
        source: scriptedTopic.source,
        status: scriptedTopic.status,
        created_at: scriptedTopic.created_at,
      },
    });
    render(createElement(Home));
    await openScriptCreationWorkspace();

    const scriptItem = await screen.findByRole("button", { name: /程序员的AI短视频流水线/ });
    expect(within(scriptItem).getByText(`来源选题：${scriptedTopic.title}`)).toBeInTheDocument();
    expect(await screen.findByRole("heading", { name: scriptedTopicScriptSummary.title })).toBeInTheDocument();
  });

  it("有脚本时可新建脚本并切换到生成模式", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [scriptSummary], total: 1, limit: 20, offset: 0 });
    vi.mocked(api.createAgentConversation).mockResolvedValue(unboundConversation);
    vi.mocked(api.sendAgentMessage).mockResolvedValue({
      user_message: { ...userMessage, content: "生成一个关于 ChatGPT 工作流的 3 镜知识科普脚本" },
      assistant_message: generatedAssistantMessage,
      run: agentRun,
    });
    vi.mocked(api.getScript).mockResolvedValueOnce(scriptDetail).mockResolvedValue(generatedScriptDetail);
    render(createElement(Home));
    await openScriptCreationWorkspace();

    expect(await screen.findByRole("heading", { name: scriptSummary.title })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "新建脚本" }));

    const panel = await screen.findByRole("region", { name: "脚本 Agent 对话" });
    expect(within(panel).getByText("当前项目：科技博主 / 新脚本生成")).toBeInTheDocument();
    expect(within(panel).getByPlaceholderText("描述你想生成的脚本...")).toBeEnabled();
    expect(screen.getByRole("heading", { name: "选择脚本后查看分镜" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /程序员必看：ChatGPT工作流/ })).not.toHaveClass("selected");

    fireEvent.change(within(panel).getByPlaceholderText("描述你想生成的脚本..."), {
      target: { value: "生成一个关于 ChatGPT 工作流的 3 镜知识科普脚本" },
    });
    fireEvent.click(within(panel).getByRole("button", { name: "发送" }));

    await waitFor(() => {
      expect(api.createAgentConversation).toHaveBeenCalledWith(expect.anything(), {
        project_id: project.project_id,
        agent_type: "script",
        title: "脚本 Agent 对话",
      });
    });
    expect(api.sendAgentMessage).toHaveBeenCalledWith(expect.anything(), unboundConversation.conversation_id, {
      content: "生成一个关于 ChatGPT 工作流的 3 镜知识科普脚本",
    });
  });

  it("无脚本时脚本 Agent 对话可发送生成请求", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [], total: 0, limit: 20, offset: 0 });
    vi.mocked(api.createAgentConversation).mockResolvedValue(unboundConversation);
    vi.mocked(api.sendAgentMessage).mockResolvedValue({
      user_message: { ...userMessage, content: "生成一个关于 ChatGPT 工作流的 3 镜知识科普脚本" },
      assistant_message: generatedAssistantMessage,
      run: agentRun,
    });
    vi.mocked(api.getScript).mockResolvedValue(generatedScriptDetail);
    render(createElement(Home));
    await openScriptCreationWorkspace();

    const panel = await screen.findByRole("region", { name: "脚本 Agent 对话" });
    expect(within(panel).getByText("当前项目：科技博主 / 新脚本生成")).toBeInTheDocument();
    expect(within(panel).queryByText(/绑定：/)).not.toBeInTheDocument();
    fireEvent.change(within(panel).getByPlaceholderText("描述你想生成的脚本..."), {
      target: { value: "生成一个关于 ChatGPT 工作流的 3 镜知识科普脚本" },
    });
    fireEvent.click(within(panel).getByRole("button", { name: "发送" }));

    await waitFor(() => {
      expect(api.createAgentConversation).toHaveBeenCalledWith(expect.anything(), {
        project_id: project.project_id,
        agent_type: "script",
        title: "脚本 Agent 对话",
      });
    });
    expect(api.sendAgentMessage).toHaveBeenCalledWith(expect.anything(), unboundConversation.conversation_id, {
      content: "生成一个关于 ChatGPT 工作流的 3 镜知识科普脚本",
    });
  });

  it("对话生成成功后刷新脚本列表并打开新脚本详情", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [], total: 0, limit: 20, offset: 0 });
    vi.mocked(api.createAgentConversation).mockResolvedValue(unboundConversation);
    vi.mocked(api.sendAgentMessage).mockResolvedValue({
      user_message: { ...userMessage, content: "生成一个关于 ChatGPT 工作流的 3 镜知识科普脚本" },
      assistant_message: generatedAssistantMessage,
      run: agentRun,
    });
    vi.mocked(api.getScript).mockResolvedValue(generatedScriptDetail);
    render(createElement(Home));
    await openScriptCreationWorkspace();

    const panel = await screen.findByRole("region", { name: "脚本 Agent 对话" });
    fireEvent.change(within(panel).getByPlaceholderText("描述你想生成的脚本..."), {
      target: { value: "生成一个关于 ChatGPT 工作流的 3 镜知识科普脚本" },
    });
    fireEvent.click(within(panel).getByRole("button", { name: "发送" }));

    expect(await screen.findByRole("heading", { name: generatedScriptSummary.title })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /ChatGPT 工作流新脚本/ })).toBeInTheDocument();
    expect(api.getScript).toHaveBeenCalledWith(expect.anything(), generatedScriptSummary.script_id);
  });

  it("生成参数不足时显示 Agent 追问且不新增脚本", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [], total: 0, limit: 20, offset: 0 });
    vi.mocked(api.createAgentConversation).mockResolvedValue(unboundConversation);
    vi.mocked(api.sendAgentMessage).mockResolvedValue({
      user_message: { ...userMessage, content: "帮我生成脚本" },
      assistant_message: missingInputAssistantMessage,
      run: agentRun,
    });
    render(createElement(Home));
    await openScriptCreationWorkspace();

    const panel = await screen.findByRole("region", { name: "脚本 Agent 对话" });
    fireEvent.change(within(panel).getByPlaceholderText("描述你想生成的脚本..."), {
      target: { value: "帮我生成脚本" },
    });
    fireEvent.click(within(panel).getByRole("button", { name: "发送" }));

    expect(await within(panel).findByText("请补充选题、风格和分镜数。")).toBeInTheDocument();
    expect(api.getScript).not.toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: /ChatGPT 工作流新脚本/ })).not.toBeInTheDocument();
  });

  it("首次发送消息会创建脚本会话并调用发送接口", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [scriptSummary], total: 1, limit: 20, offset: 0 });
    render(createElement(Home));
    await openScriptCreationWorkspace();

    const panel = await screen.findByRole("region", { name: "脚本 Agent 对话" });
    fireEvent.change(within(panel).getByPlaceholderText("描述要修改的分镜方向..."), {
      target: { value: userMessage.content },
    });
    fireEvent.click(within(panel).getByRole("button", { name: "发送" }));

    await waitFor(() => {
      expect(api.createAgentConversation).toHaveBeenCalledWith(expect.anything(), {
        project_id: project.project_id,
        agent_type: "script",
        subject_type: "script",
        subject_id: scriptSummary.script_id,
        title: "脚本 Agent 对话",
      });
    });
    expect(api.sendAgentMessage).toHaveBeenCalledWith(expect.anything(), conversation.conversation_id, {
      content: userMessage.content,
    });
    expect(await within(panel).findByText(assistantMessage.content)).toBeInTheDocument();
  });

  it("发送成功后刷新脚本详情", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [scriptSummary], total: 1, limit: 20, offset: 0 });
    const refreshedScript = {
      ...scriptDetail,
      scenes: scriptDetail.scenes.map((scene) =>
        scene.sequence === 2
          ? { ...scene, visual_description: "屏幕切到红色告警和密集 TODO，冲突更强。" }
          : scene,
      ),
      updated_at: "2026-07-02T00:08:00Z",
    };
    vi.mocked(api.getScript).mockResolvedValueOnce(scriptDetail).mockResolvedValueOnce(refreshedScript);
    render(createElement(Home));
    await openScriptCreationWorkspace();

    const panel = await screen.findByRole("region", { name: "脚本 Agent 对话" });
    fireEvent.change(within(panel).getByPlaceholderText("描述要修改的分镜方向..."), {
      target: { value: userMessage.content },
    });
    fireEvent.click(within(panel).getByRole("button", { name: "发送" }));

    expect(await screen.findByText("屏幕切到红色告警和密集 TODO，冲突更强。")).toBeInTheDocument();
    expect(api.getScript).toHaveBeenCalledTimes(2);
  });

  it("发送失败时错误只显示在对话面板内", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [scriptSummary], total: 1, limit: 20, offset: 0 });
    vi.mocked(api.sendAgentMessage).mockRejectedValue(new Error("LLM 输出无法解析"));
    render(createElement(Home));
    await openScriptCreationWorkspace();

    const panel = await screen.findByRole("region", { name: "脚本 Agent 对话" });
    fireEvent.change(within(panel).getByPlaceholderText("描述要修改的分镜方向..."), {
      target: { value: userMessage.content },
    });
    fireEvent.click(within(panel).getByRole("button", { name: "发送" }));

    expect(await within(panel).findByText("LLM 输出无法解析")).toBeInTheDocument();
    expect(screen.getByText("传统程序员每天要写大量重复代码。")).toBeInTheDocument();
  });

  it("切换脚本时不会把上一脚本未完成对话写入当前面板", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [scriptSummary, secondScriptSummary], total: 2, limit: 20, offset: 0 });
    const pendingTurn = deferred<{
      user_message: AgentMessage;
      assistant_message: AgentMessage;
      run: AgentRun;
    }>();
    const pendingSecondScript = deferred<ScriptDetail>();
    vi.mocked(api.getScript)
      .mockResolvedValueOnce(scriptDetail)
      .mockReturnValueOnce(pendingSecondScript.promise)
      .mockResolvedValue(scriptDetail);
    vi.mocked(api.sendAgentMessage).mockReturnValue(pendingTurn.promise);
    render(createElement(Home));
    await openScriptCreationWorkspace();

    const panel = await screen.findByRole("region", { name: "脚本 Agent 对话" });
    fireEvent.change(within(panel).getByPlaceholderText("描述要修改的分镜方向..."), {
      target: { value: userMessage.content },
    });
    fireEvent.click(within(panel).getByRole("button", { name: "发送" }));

    await waitFor(() => expect(api.sendAgentMessage).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole("button", { name: /第二版脚本/ }));

    await waitFor(() => expect(api.getScript).toHaveBeenCalledWith(expect.anything(), secondScriptSummary.script_id));

    await act(async () => {
      pendingTurn.resolve({ user_message: userMessage, assistant_message: assistantMessage, run: agentRun });
      await flushAsyncWork();
    });

    expect(screen.queryByText(assistantMessage.content)).not.toBeInTheDocument();
    expect(api.getScript).toHaveBeenCalledTimes(2);

    await act(async () => {
      pendingSecondScript.resolve(secondScriptDetail);
      await flushAsyncWork();
    });

    expect(await screen.findByRole("heading", { name: secondScriptSummary.title })).toBeInTheDocument();
  });

  it("使用内容策略原型的浅色工作台色板和三列布局", () => {
    const styles = readFileSync(resolve(__dirname, "styles.css"), "utf8");

    expect(styles).toContain("--color-page: #f4f7fb");
    expect(styles).toContain("--color-surface-muted: #f8fafc");
    expect(styles).toContain("--color-primary: #2860e8");
    expect(styles).toContain("--color-primary-soft: #eaf1ff");
    expect(styles).toContain("--color-primary-border: #b8cdfd");
    expect(styles).toContain("--color-success-soft: #e8f6ee");
    expect(styles).toContain("--color-success-border: #9fd3b4");
    expect(styles).toContain("--color-success-text: #1e6b3a");
    expect(styles).toContain("--color-agent-rail: #ffffff");
    expect(styles).toContain("--color-agent-rail-muted: #5d6878");
    expect(styles).toContain(".metricCard.neutral");
    expect(styles).toContain("--metric-bg: var(--color-surface-muted)");
    expect(styles).toContain("--metric-border: var(--color-border)");
    expect(styles).toContain(".metricCard.success");
    expect(styles).toContain("--metric-bg: var(--color-success-soft)");
    expect(styles).toContain("--metric-border: var(--color-success-border)");
    expect(styles).toContain("--metric-value: var(--color-success-text)");
    expect(styles).toContain(".metricCard.primary");
    expect(styles).toContain("--metric-bg: var(--color-primary-soft)");
    expect(styles).toContain("--metric-border: var(--color-primary-border)");
    expect(styles).toContain("--metric-value: var(--color-primary)");
    expect(styles).toContain("grid-template-rows: auto minmax(0, 1fr)");
    expect(styles).toContain("grid-template-columns: 360px minmax(360px, 1fr) 360px");
    expect(styles).toContain(".agentSubMenu");
    expect(styles).toContain(".agentSubItem");
    expect(styles).toContain(".topicBatchHistory");
    expect(styles).toContain("flex-direction: column");
    expect(styles).toContain("flex: 1");
    expect(styles).toMatch(/\.topicList\s*{[^}]*align-content:\s*start/s);
    expect(styles).toContain("max-height: calc(100vh - 232px)");
    expect(styles).toContain("overflow-y: auto");
    expect(styles).toContain("display: contents");
    expect(styles).not.toContain("#2f855a");
    expect(styles).not.toContain("#1f3b2d");
  });

  it("页面入口只负责编排并导入独立功能页面", () => {
    const pageSource = readFileSync(resolve(__dirname, "page.tsx"), "utf8");

    expect(pageSource).toContain('from "./pages/content-strategy/ContentStrategyPage"');
    expect(pageSource).toContain('from "./pages/script-creation/ScriptCreationPage"');
    expect(pageSource).not.toContain("function ContentStrategyWorkspace");
    expect(pageSource).not.toContain("function ScriptDetailView");
    expect(pageSource).not.toContain("function ScriptAgentConversationPanel");
  });

  it("内容策略页展示选题统计、选题池和详情侧栏，不在顶部展示账号描述", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockTopics(topicListResponse);
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));

    expect(await screen.findByRole("heading", { name: "内容策略" })).toBeInTheDocument();
    const summaryPanel = document.querySelector(".strategySummaryPanel");
    expect(summaryPanel).not.toBeNull();
    expect(within(summaryPanel as HTMLElement).queryByText(project.name)).not.toBeInTheDocument();
    expect(within(summaryPanel as HTMLElement).queryByText(project.positioning)).not.toBeInTheDocument();
    expect(within(summaryPanel as HTMLElement).queryByText(project.description)).not.toBeInTheDocument();
    const stats = screen.getByLabelText("选题统计");
    expect(within(stats).getByText("全部选题")).toBeInTheDocument();
    expect(within(stats).queryByText("待确认")).not.toBeInTheDocument();
    expect(within(stats).getByText("已确认")).toBeInTheDocument();
    expect(within(stats).getByText("已成稿")).toBeInTheDocument();
    expect(within(stats).getByText("全部选题").closest(".metricCard")).toHaveClass("neutral");
    expect(within(stats).getByText("已确认").closest(".metricCard")).toHaveClass("success");
    expect(within(stats).getByText("已成稿").closest(".metricCard")).toHaveClass("primary");

    const topicPool = screen.getByRole("region", { name: "选题池" });
    expect(within(topicPool).getByRole("button", { name: /AI 工具如何重塑内容团队/ })).toBeInTheDocument();
    expect(within(topicPool).getByRole("button", { name: /普通人如何搭建 AI 内容流水线/ })).toBeInTheDocument();
    expect(within(topicPool).getAllByText(/来源：人工/)[0]).toBeInTheDocument();
    expect(within(topicPool).getAllByText(/类型：知识科普/)[0]).toBeInTheDocument();
    const statusFilters = within(topicPool).getByLabelText("选题状态筛选");
    expect(within(statusFilters).getAllByRole("button")).toHaveLength(5);
    for (const label of ["全部", "待评估", "已确认", "已成稿", "已归档"]) {
      expect(within(statusFilters).getByRole("button", { name: label })).toBeInTheDocument();
    }

    const detail = screen.getByRole("region", { name: "选题详情" });
    expect(within(detail).getByRole("heading", { name: ideaTopic.title })).toBeInTheDocument();
    expect(within(detail).getByText("来源：人工")).toBeInTheDocument();
    expect(within(detail).getByText("类型：知识科普")).toBeInTheDocument();
    expect(within(detail).queryByText("knowledge")).not.toBeInTheDocument();
    expect(within(detail).getByText(ideaTopic.angle)).toBeInTheDocument();
    expect(within(detail).getByRole("button", { name: "确认选题" })).toBeEnabled();
    expect(api.listContentTopics).toHaveBeenCalledWith(expect.anything(), project.project_id, {
      status: "all",
      source: "all",
    });
  });

  it("内容策略二级菜单将历史生成展示在当前选题池上方", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockTopics(topicListResponse);
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));

    const workspaceMenu = screen.getByRole("navigation", { name: "视频工作台菜单" });
    const menuButtons = within(workspaceMenu).getAllByRole("button");
    const historyButton = within(workspaceMenu).getByRole("button", { name: "历史生成" });
    const currentPoolButton = within(workspaceMenu).getByRole("button", { name: "当前选题池" });
    expect(menuButtons.indexOf(historyButton)).toBeLessThan(menuButtons.indexOf(currentPoolButton));
    expect(historyButton).toHaveClass("agentSubItem");
    expect(currentPoolButton).toHaveClass("active");
    expect(screen.queryByLabelText("内容策略视图菜单")).not.toBeInTheDocument();
    expect(screen.getByRole("region", { name: "选题池" })).toBeInTheDocument();
  });

  it("内容策略页按生成批次展示历史选题，避免多个批次混在一起", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockTopicBatches(topicBatchListResponse);
    const latestBatchTopic = {
      ...ideaTopic,
      topic_id: "99999999-9999-4999-8999-999999999991",
      batch_id: latestTopicBatch.batch_id,
      title: "最新批次选题",
    };
    const previousBatchTopic = {
      ...approvedTopic,
      topic_id: "99999999-9999-4999-8999-999999999992",
      batch_id: previousTopicBatch.batch_id,
      title: "历史批次选题",
    };
    vi.mocked(api.listContentTopics).mockImplementation(async (_client, _projectId, filters = {}) => {
      if (filters.batch_id === latestTopicBatch.batch_id) {
        return {
          topics: [latestBatchTopic],
          stats: { total: 10, idea: 8, approved: 1, scripted: 1, archived: 0 },
        };
      }
      if (filters.batch_id === previousTopicBatch.batch_id) {
        return {
          topics: [previousBatchTopic],
          stats: { total: 10, idea: 8, approved: 1, scripted: 1, archived: 0 },
        };
      }
      return {
        topics: [latestBatchTopic, previousBatchTopic],
        stats: { total: 10, idea: 8, approved: 1, scripted: 1, archived: 0 },
      };
    });

    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "历史生成" }));
    const history = await screen.findByRole("region", { name: "历史生成列表页" });
    expect(within(history).getByRole("button", { name: /最新一批 AI 工具选题/ })).toHaveClass("selected");
    expect(within(history).getByRole("button", { name: /上一批 AI 内容流水线选题/ })).toBeInTheDocument();
    expect(within(history).queryByRole("button", { name: /失败的 AI 选题生成/ })).not.toBeInTheDocument();
    expect(within(history).queryByText(/生成失败/)).not.toBeInTheDocument();
    await waitFor(() => {
      expect(api.listContentTopics).toHaveBeenLastCalledWith(expect.anything(), project.project_id, {
        status: "all",
        source: "all",
        batch_id: latestTopicBatch.batch_id,
      });
    });
    expect(await screen.findByRole("article", { name: "历史选题：最新批次选题" })).toBeInTheDocument();
    expect(screen.queryByRole("article", { name: "历史选题：历史批次选题" })).not.toBeInTheDocument();

    fireEvent.click(within(history).getByRole("button", { name: /上一批 AI 内容流水线选题/ }));
    await waitFor(() => {
      expect(api.listContentTopics).toHaveBeenLastCalledWith(expect.anything(), project.project_id, {
        status: "all",
        source: "all",
        batch_id: previousTopicBatch.batch_id,
      });
    });
    expect(await screen.findByRole("article", { name: "历史选题：历史批次选题" })).toBeInTheDocument();
    expect(screen.queryByRole("article", { name: "历史选题：最新批次选题" })).not.toBeInTheDocument();

    fireEvent.click(within(screen.getByRole("navigation", { name: "视频工作台菜单" })).getByRole("button", {
      name: "当前选题池",
    }));
    await waitFor(() => {
      expect(api.listContentTopics).toHaveBeenLastCalledWith(expect.anything(), project.project_id, {
        status: "all",
        source: "all",
        batch_id: previousTopicBatch.batch_id,
      });
    });
    const topicPool = await screen.findByRole("region", { name: "选题池" });
    expect(within(topicPool).getByRole("button", { name: /历史批次选题/ })).toBeInTheDocument();
    expect(within(topicPool).queryByRole("button", { name: /最新批次选题/ })).not.toBeInTheDocument();
  });

  it("历史生成页只允许未生成脚本选题移除，已成稿选题不可删除", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockTopicBatches(topicBatchListResponse);
    mockTopics({
      topics: [
        { ...approvedTopic, batch_id: latestTopicBatch.batch_id },
        { ...scriptedTopic, batch_id: latestTopicBatch.batch_id },
      ],
      stats: { total: 2, idea: 0, approved: 1, scripted: 1, archived: 0 },
    });
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "历史生成" }));

    const removableRow = await screen.findByRole("article", { name: `历史选题：${approvedTopic.title}` });
    const lockedRow = await screen.findByRole("article", { name: `历史选题：${scriptedTopic.title}` });
    expect(within(removableRow).getByRole("button", { name: "移除" })).toBeEnabled();
    expect(within(lockedRow).getByText("已生成脚本，不可删除")).toBeInTheDocument();
    expect(within(lockedRow).queryByRole("button", { name: "移除" })).not.toBeInTheDocument();
  });

  it("历史生成页移除未生成脚本选题后刷新选题列表和批次列表", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    vi.mocked(api.listTopicGenerationBatches)
      .mockResolvedValueOnce(topicBatchListResponse)
      .mockResolvedValueOnce({ batches: [{ ...latestTopicBatch, topic_count: 4 }, previousTopicBatch] });
    vi.mocked(api.listContentTopics)
      .mockResolvedValueOnce({
        topics: [{ ...approvedTopic, batch_id: latestTopicBatch.batch_id }],
        stats: { total: 1, idea: 0, approved: 1, scripted: 0, archived: 0 },
      })
      .mockResolvedValueOnce({
        topics: [{ ...approvedTopic, batch_id: latestTopicBatch.batch_id }],
        stats: { total: 1, idea: 0, approved: 1, scripted: 0, archived: 0 },
      })
      .mockResolvedValueOnce({
        topics: [],
        stats: { total: 0, idea: 0, approved: 0, scripted: 0, archived: 0 },
      });
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    vi.mocked(api.deleteContentTopic).mockResolvedValue({
      topic_id: approvedTopic.topic_id,
      deleted_at: "2026-07-07T10:00:00Z",
    });

    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "历史生成" }));
    const removableRow = await screen.findByRole("article", { name: `历史选题：${approvedTopic.title}` });
    fireEvent.click(within(removableRow).getByRole("button", { name: "移除" }));

    await waitFor(() => {
      expect(api.deleteContentTopic).toHaveBeenCalledWith(expect.anything(), approvedTopic.topic_id);
    });
    expect(confirmSpy).toHaveBeenCalledWith(`确认从管理视图移除「${approvedTopic.title}」吗？`);
    await waitFor(() => {
      expect(api.listTopicGenerationBatches).toHaveBeenCalledTimes(2);
      expect(api.listContentTopics).toHaveBeenLastCalledWith(expect.anything(), project.project_id, {
        status: "all",
        source: "all",
        batch_id: latestTopicBatch.batch_id,
      });
    });
    expect(screen.queryByRole("article", { name: `历史选题：${approvedTopic.title}` })).not.toBeInTheDocument();
    confirmSpy.mockRestore();
  });

  it("内容策略页通过选题 Agent 生成候选后刷新选题池", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    const latestBatchId = topicAssistantMessage.metadata.batch_id as string;
    const latestBatchTopics: ContentTopicListResponse = {
      topics: [
        { ...ideaTopic, batch_id: latestBatchId },
        { ...approvedTopic, batch_id: latestBatchId },
      ],
      stats: { total: 15, idea: 14, approved: 1, scripted: 0, archived: 0 },
    };
    vi.mocked(api.listContentTopics)
      .mockResolvedValueOnce(topicListResponse)
      .mockResolvedValueOnce(latestBatchTopics);
    vi.mocked(api.createAgentConversation).mockResolvedValue(topicConversation);
    vi.mocked(api.sendAgentMessage).mockResolvedValue({
      user_message: topicUserMessage,
      assistant_message: topicAssistantMessage,
      run: topicAgentRun,
    });
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    const agentPanel = await screen.findByRole("region", { name: "选题 Agent" });
    fireEvent.change(within(agentPanel).getByLabelText("生成要求"), {
      target: { value: topicUserMessage.content },
    });
    fireEvent.click(within(agentPanel).getByRole("button", { name: "生成选题" }));

    await waitFor(() => {
      expect(api.createAgentConversation).toHaveBeenCalledWith(expect.anything(), {
        project_id: project.project_id,
        agent_type: "topic",
        title: "选题 Agent 对话",
      });
    });
    expect(api.sendAgentMessage).toHaveBeenCalledWith(expect.anything(), topicConversation.conversation_id, {
      content: topicUserMessage.content,
    });
    expect(await within(agentPanel).findByText(topicAssistantMessage.content)).toBeInTheDocument();
    await waitFor(() => {
      expect(api.listContentTopics).toHaveBeenLastCalledWith(expect.anything(), project.project_id, {
        status: "all",
        source: "all",
        batch_id: latestBatchId,
      });
    });
    expect(screen.getByRole("button", { name: "查看全部选题" })).toBeInTheDocument();
  });

  it("内容策略页本地新增选题后仍按评分高低展示选题池", async () => {
    const highScoreTopic = {
      ...approvedTopic,
      title: "高分选题",
      score: 95,
    };
    const mediumScoreTopic = {
      ...ideaTopic,
      title: "中分选题",
      score: 86,
    };
    const lowScoreTopic = {
      ...ideaTopic,
      topic_id: "99999999-9999-4999-8999-999999999999",
      title: "低分新增选题",
      score: null,
      created_at: "2026-07-02T00:30:00Z",
      updated_at: "2026-07-02T00:30:00Z",
    };
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockTopics({
      topics: [highScoreTopic, mediumScoreTopic],
      stats: { total: 2, idea: 1, approved: 1, scripted: 0, archived: 0 },
    });
    vi.mocked(api.createContentTopic).mockResolvedValue(lowScoreTopic);
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "新增选题" }));
    fireEvent.change(screen.getByLabelText("选题标题"), { target: { value: lowScoreTopic.title } });
    fireEvent.change(screen.getByLabelText("选题角度"), { target: { value: lowScoreTopic.angle } });
    fireEvent.change(screen.getByLabelText("目标受众"), { target: { value: lowScoreTopic.target_audience } });
    fireEvent.change(screen.getByLabelText("核心看点"), { target: { value: lowScoreTopic.hook_points.join("\n") } });
    fireEvent.change(screen.getByLabelText("内容类型"), { target: { value: lowScoreTopic.content_type } });
    fireEvent.change(screen.getByLabelText("评分理由"), { target: { value: lowScoreTopic.score_reason } });
    fireEvent.change(screen.getByLabelText("标签"), { target: { value: lowScoreTopic.tags.join(",") } });
    fireEvent.click(screen.getByRole("button", { name: "保存选题" }));

    await waitFor(() => {
      expect(api.createContentTopic).toHaveBeenCalled();
    });
    const topicTitles = Array.from(document.querySelectorAll(".topicList .topicTitle")).map(
      (element) => element.textContent,
    );
    expect(topicTitles).toEqual(["高分选题", "中分选题", "低分新增选题"]);
  });

  it("内容策略页支持手动新增、确认和归档选题", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockTopics(topicListResponse);
    vi.mocked(api.createContentTopic).mockResolvedValue({ ...ideaTopic, title: "AI 产品周报选题" });
    vi.mocked(api.updateContentTopic).mockResolvedValue({ ...ideaTopic, angle: "编辑后的角度" });
    vi.mocked(api.updateContentTopicStatus)
      .mockResolvedValueOnce({ ...ideaTopic, status: "approved" })
      .mockResolvedValueOnce({ ...ideaTopic, status: "archived" });
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "编辑选题" }));
    fireEvent.change(screen.getByLabelText("选题角度"), { target: { value: "编辑后的角度" } });
    fireEvent.click(screen.getByRole("button", { name: "保存选题" }));

    await waitFor(() => {
      expect(api.updateContentTopic).toHaveBeenCalledWith(expect.anything(), ideaTopic.topic_id, {
        title: ideaTopic.title,
        angle: "编辑后的角度",
        target_audience: ideaTopic.target_audience,
        hook_points: ideaTopic.hook_points,
        content_type: ideaTopic.content_type,
        score: ideaTopic.score,
        score_reason: ideaTopic.score_reason,
        tags: ideaTopic.tags,
      });
    });

    fireEvent.click(await screen.findByRole("button", { name: "新增选题" }));
    fireEvent.change(screen.getByLabelText("选题标题"), { target: { value: "AI 产品周报选题" } });
    fireEvent.change(screen.getByLabelText("选题角度"), { target: { value: ideaTopic.angle } });
    fireEvent.change(screen.getByLabelText("目标受众"), { target: { value: ideaTopic.target_audience } });
    fireEvent.change(screen.getByLabelText("核心看点"), { target: { value: ideaTopic.hook_points.join("\n") } });
    fireEvent.change(screen.getByLabelText("内容类型"), { target: { value: ideaTopic.content_type } });
    fireEvent.change(screen.getByLabelText("评分理由"), { target: { value: ideaTopic.score_reason } });
    fireEvent.change(screen.getByLabelText("标签"), { target: { value: ideaTopic.tags.join(",") } });
    fireEvent.click(screen.getByRole("button", { name: "保存选题" }));

    await waitFor(() => {
      expect(api.createContentTopic).toHaveBeenCalledWith(expect.anything(), project.project_id, {
        title: "AI 产品周报选题",
        angle: ideaTopic.angle,
        target_audience: ideaTopic.target_audience,
        hook_points: ideaTopic.hook_points,
        content_type: ideaTopic.content_type,
        score: null,
        score_reason: ideaTopic.score_reason,
        tags: ideaTopic.tags,
      });
    });

    fireEvent.click(screen.getByRole("button", { name: "确认选题" }));
    await waitFor(() => {
      expect(api.updateContentTopicStatus).toHaveBeenCalledWith(expect.anything(), ideaTopic.topic_id, "approved");
    });

    fireEvent.click(screen.getByRole("button", { name: "归档选题" }));
    await waitFor(() => {
      expect(api.updateContentTopicStatus).toHaveBeenCalledWith(expect.anything(), ideaTopic.topic_id, "archived");
    });
  });

  it("已确认选题通过弹窗确认参数后生成脚本并展示来源快照", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockTopics(topicListResponse);
    mockScripts({ scripts: [], total: 0, limit: 20, offset: 0 });
    vi.mocked(api.generateScript).mockResolvedValue(topicGeneratedScript);
    vi.mocked(api.getScript).mockResolvedValue(topicGeneratedScript);
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: /普通人如何搭建 AI 内容流水线/ }));
    fireEvent.click(screen.getByRole("button", { name: "生成脚本" }));

    const dialog = await screen.findByRole("dialog", { name: "脚本生成确认" });
    expect(api.prepareScriptFromTopic).toHaveBeenCalledWith(expect.anything(), approvedTopic.topic_id, {
      style: "knowledge",
      scene_count: 6,
    });
    expect(within(dialog).getByText(approvedTopic.title)).toBeInTheDocument();
    expect(within(dialog).getByLabelText("脚本风格")).toHaveValue("knowledge");
    expect(within(dialog).getByLabelText("分镜数")).toHaveValue(6);

    fireEvent.click(within(dialog).getByRole("button", { name: "确认生成" }));

    await waitFor(() => {
      expect(api.generateScript).toHaveBeenCalledWith(expect.anything(), {
        project_id: project.project_id,
        topic_id: approvedTopic.topic_id,
        style: "knowledge",
        scene_count: 6,
      });
    });
    expect(api.listContentTopics).toHaveBeenCalledTimes(2);
    expect(api.listScripts).toHaveBeenCalled();
    expect(await screen.findByRole("heading", { name: topicGeneratedScript.title })).toBeInTheDocument();
    expect(screen.getByText("来源选题")).toBeInTheDocument();
    expect(screen.getByText(approvedTopic.title)).toBeInTheDocument();
    expect(screen.getByText(approvedTopic.angle)).toBeInTheDocument();
  });
});
