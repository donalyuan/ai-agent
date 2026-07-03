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
  ProjectListResponse,
  ScriptDetail,
  ScriptListResponse,
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

const secondScriptSummary = {
  script_id: "99999999-9999-4999-8999-999999999999",
  title: "第二版脚本：AI 剪辑流程",
  status: "draft" as const,
  scene_count: 1,
  parent_id: null,
  created_at: "2026-07-02T00:10:00Z",
};

const secondScriptDetail: ScriptDetail = {
  ...secondScriptSummary,
  project_id: project.project_id,
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
  ],
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
  title: "ChatGPT 工作流新脚本",
  status: "draft" as const,
  scene_count: 3,
  parent_id: null,
  created_at: "2026-07-02T00:12:00Z",
};

const generatedScriptDetail: ScriptDetail = {
  ...generatedScriptSummary,
  project_id: project.project_id,
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

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((promiseResolve, promiseReject) => {
    resolve = promiseResolve;
    reject = promiseReject;
  });
  return { promise, resolve, reject };
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
    vi.mocked(api.getScript).mockResolvedValue(scriptDetail);
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

    expect(within(menu).getByRole("button", { name: /脚本创作/ })).toHaveClass("active");
    expect(within(menu).getByRole("button", { name: /内容策略/ })).toBeDisabled();
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

    const panel = await screen.findByRole("region", { name: "脚本 Agent 对话" });

    expect(await within(panel).findByText("当前项目：科技博主 / 脚本：程序员必看：ChatGPT工作流")).toBeInTheDocument();
    expect(within(panel).queryByText(/绑定：/)).not.toBeInTheDocument();
    expect(within(panel).getByPlaceholderText("描述要修改的分镜方向...")).toBeEnabled();
    expect(within(panel).getByRole("button", { name: "发送" })).toBeEnabled();
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

  it("使用已确认的蓝色主色板", () => {
    const styles = readFileSync(resolve(__dirname, "styles.css"), "utf8");

    expect(styles).toContain("--color-agent-rail: #182030");
    expect(styles).toContain("--color-primary: #2860e8");
    expect(styles).toContain("--color-primary-soft: #e8f0ff");
    expect(styles).not.toContain("#2f855a");
    expect(styles).not.toContain("#1f3b2d");
  });
});
