import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  createAgentConversation,
  createApiClient,
  generateScript,
  getApiBaseUrl,
  getScript,
  listAgentMessages,
  listProjects,
  listScripts,
  listWorkspaceMenus,
  sendAgentMessage,
  updateScriptStatus,
} from "./api";

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
  status: "draft",
  scene_count: 2,
  parent_id: null,
  created_at: "2026-07-02T00:05:00Z",
};

const scriptDetail = {
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
  ],
  updated_at: "2026-07-02T00:05:00Z",
};

const conversation = {
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

const userMessage = {
  message_id: "66666666-6666-4666-8666-666666666666",
  conversation_id: conversation.conversation_id,
  role: "user",
  content: "把第 2 镜改得更有冲突感",
  metadata: {},
  created_at: "2026-07-02T00:07:00Z",
};

const assistantMessage = {
  message_id: "77777777-7777-4777-8777-777777777777",
  conversation_id: conversation.conversation_id,
  role: "assistant",
  content: "已更新第 2 镜，时间轴已刷新。",
  metadata: { scene_sequence: 2 },
  created_at: "2026-07-02T00:07:05Z",
};

const agentRun = {
  run_id: "88888888-8888-4888-8888-888888888888",
  conversation_id: conversation.conversation_id,
  project_id: project.project_id,
  agent_type: "script",
  status: "completed",
  input: { message: userMessage.content },
  output: { reply: assistantMessage.content },
  error: null,
  started_at: "2026-07-02T00:07:00Z",
  finished_at: "2026-07-02T00:07:05Z",
};

const workspaceMenus = [
  {
    menu_id: "10000000-0000-4000-8000-000000000001",
    menu_key: "content-strategy",
    label: "内容策略",
    description: "选题池、热点趋势、账号定位、受众画像和策略建议。",
    route_path: "/strategy",
    icon: "lightbulb",
    menu_type: "section",
    module_key: "strategy",
    agent_key: null,
    sort_order: 10,
    is_enabled: false,
    status: "planned",
    metadata: { phase: 2 },
    children: [],
  },
  {
    menu_id: "10000000-0000-4000-8000-000000000002",
    menu_key: "script-creation",
    label: "脚本创作",
    description: "脚本生成、分镜确认、时间轴详情和状态流转。",
    route_path: "/scripts",
    icon: "file-pen-line",
    menu_type: "section",
    module_key: "script",
    agent_key: null,
    sort_order: 20,
    is_enabled: true,
    status: "active",
    metadata: { phase: 1 },
    children: [
      {
        menu_id: "20000000-0000-4000-8000-000000000002",
        menu_key: "script-generator",
        label: "脚本生成",
        description: "当前可用的脚本生成闭环。",
        route_path: "/scripts/generator",
        icon: "file-text",
        menu_type: "page",
        module_key: "script.generator",
        agent_key: "script-generation-agent",
        sort_order: 10,
        is_enabled: true,
        status: "active",
        metadata: { phase: 1 },
        children: [],
      },
    ],
  },
];

function jsonResponse(body: unknown, init: ResponseInit = {}) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json" },
    ...init,
  });
}

describe("getApiBaseUrl", () => {
  const originalValue = process.env.NEXT_PUBLIC_API_BASE_URL;

  afterEach(() => {
    if (originalValue === undefined) {
      delete process.env.NEXT_PUBLIC_API_BASE_URL;
    } else {
      process.env.NEXT_PUBLIC_API_BASE_URL = originalValue;
    }
  });

  it("默认使用本地 API 端口", () => {
    delete process.env.NEXT_PUBLIC_API_BASE_URL;

    expect(getApiBaseUrl()).toBe("http://localhost:18180");
  });

  it("使用环境变量覆盖，并移除结尾斜杠", () => {
    process.env.NEXT_PUBLIC_API_BASE_URL = "http://api.example.test/";

    expect(getApiBaseUrl()).toBe("http://api.example.test");
  });
});

describe("video-agent api client", () => {
  const fetchMock = vi.fn();

  beforeEach(() => {
    fetchMock.mockReset();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("请求项目列表", async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({ projects: [project] }));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await listProjects(client);

    expect(fetchMock).toHaveBeenCalledWith("http://api.test/api/projects", {
      headers: { accept: "application/json" },
    });
    expect(result.projects[0].project_id).toBe(project.project_id);
  });

  it("请求视频工作台菜单树", async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({ menus: workspaceMenus }));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await listWorkspaceMenus(client);

    expect(fetchMock).toHaveBeenCalledWith("http://api.test/api/video-workspace/menus", {
      headers: { accept: "application/json" },
    });
    expect(result.menus[0].label).toBe("内容策略");
    expect(result.menus[1].children[0].agent_key).toBe("script-generation-agent");
  });

  it("请求脚本列表时带状态筛选", async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({ scripts: [scriptSummary], total: 1, limit: 20, offset: 0 }),
    );
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await listScripts(client, project.project_id, { status: "draft" });

    expect(fetchMock).toHaveBeenCalledWith(
      `http://api.test/api/projects/${project.project_id}/scripts?status=draft`,
      { headers: { accept: "application/json" } },
    );
    expect(result.scripts[0].script_id).toBe(scriptSummary.script_id);
  });

  it("读取脚本详情", async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse(scriptDetail));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await getScript(client, scriptSummary.script_id);

    expect(fetchMock).toHaveBeenCalledWith(
      `http://api.test/api/scripts/${scriptSummary.script_id}`,
      { headers: { accept: "application/json" } },
    );
    expect(result.scenes[0].sequence).toBe(1);
  });

  it("提交脚本生成请求", async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse(scriptDetail));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await generateScript(client, {
      project_id: project.project_id,
      topic: "ChatGPT如何改变程序员工作流",
      style: "knowledge",
      scene_count: 5,
    });

    expect(fetchMock).toHaveBeenCalledWith("http://api.test/api/scripts/generate", {
      method: "POST",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
      },
      body: JSON.stringify({
        project_id: project.project_id,
        topic: "ChatGPT如何改变程序员工作流",
        style: "knowledge",
        scene_count: 5,
      }),
    });
    expect(result.title).toBe("程序员必看：ChatGPT工作流");
  });

  it("更新脚本状态", async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        script_id: scriptSummary.script_id,
        status: "approved",
        updated_at: "2026-07-02T00:10:00Z",
      }),
    );
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await updateScriptStatus(client, scriptSummary.script_id, "approved");

    expect(fetchMock).toHaveBeenCalledWith(
      `http://api.test/api/scripts/${scriptSummary.script_id}/status`,
      {
        method: "PUT",
        headers: {
          accept: "application/json",
          "content-type": "application/json",
        },
        body: JSON.stringify({ status: "approved" }),
      },
    );
    expect(result.status).toBe("approved");
  });

  it("创建脚本 Agent 会话", async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse(conversation));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await createAgentConversation(client, {
      project_id: project.project_id,
      agent_type: "script",
      subject_type: "script",
      subject_id: scriptSummary.script_id,
      title: "脚本 Agent 对话",
    });

    expect(fetchMock).toHaveBeenCalledWith("http://api.test/api/agent/conversations", {
      method: "POST",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
      },
      body: JSON.stringify({
        project_id: project.project_id,
        agent_type: "script",
        subject_type: "script",
        subject_id: scriptSummary.script_id,
        title: "脚本 Agent 对话",
      }),
    });
    expect(result.conversation_id).toBe(conversation.conversation_id);
  });

  it("读取 Agent 会话消息", async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({ messages: [userMessage, assistantMessage] }));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await listAgentMessages(client, conversation.conversation_id);

    expect(fetchMock).toHaveBeenCalledWith(
      `http://api.test/api/agent/conversations/${conversation.conversation_id}/messages`,
      { headers: { accept: "application/json" } },
    );
    expect(result.messages).toHaveLength(2);
    expect(result.messages[1].role).toBe("assistant");
  });

  it("发送 Agent 消息", async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({ user_message: userMessage, assistant_message: assistantMessage, run: agentRun }),
    );
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await sendAgentMessage(client, conversation.conversation_id, {
      content: userMessage.content,
    });

    expect(fetchMock).toHaveBeenCalledWith(
      `http://api.test/api/agent/conversations/${conversation.conversation_id}/messages`,
      {
        method: "POST",
        headers: {
          accept: "application/json",
          "content-type": "application/json",
        },
        body: JSON.stringify({ content: userMessage.content }),
      },
    );
    expect(result.assistant_message.content).toContain("已更新第 2 镜");
    expect(result.run.status).toBe("completed");
  });

  it("对话 API 失败时沿用 ApiError", async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({ error: "会话不存在" }, { status: 404 }));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    await expect(listAgentMessages(client, conversation.conversation_id)).rejects.toMatchObject({
      name: "ApiError",
      status: 404,
      message: "会话不存在",
    });
  });
});
