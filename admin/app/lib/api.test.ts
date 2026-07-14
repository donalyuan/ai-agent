import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  createApiClient,
  createAiModel,
  deleteAiModel,
  createProject,
  generateScript,
  getApiBaseUrl,
  getScript,
  listProjects,
  listAiModels,
  setDefaultAiModel,
  updateAiModel,
  changeAiModelStatus,
  listScripts,
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

describe("api client", () => {
  const fetchMock = vi.fn();

  beforeEach(() => {
    fetchMock.mockReset();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("默认 fetcher 保持浏览器 fetch 调用上下文", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(function (this: typeof globalThis) {
        expect(this).toBe(globalThis);
        return Promise.resolve(jsonResponse({ projects: [project] }));
      }),
    );
    const client = createApiClient({ baseUrl: "http://api.test" });

    await listProjects(client);
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

  it("创建项目", async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse(project, { status: 201 }));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await createProject(client, {
      name: "科技博主",
      positioning: "科技知识账号",
      description: "面向程序员的知识短视频",
    });

    expect(fetchMock).toHaveBeenCalledWith("http://api.test/api/projects", {
      method: "POST",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
      },
      body: JSON.stringify({
        name: "科技博主",
        positioning: "科技知识账号",
        description: "面向程序员的知识短视频",
      }),
    });
    expect(result.name).toBe("科技博主");
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

  it("保留后端结构化错误", async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({ error: "项目名称不能为空" }, { status: 400 }),
    );
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    await expect(createProject(client, { name: "", positioning: "", description: "" }))
      .rejects.toMatchObject({
        status: 400,
        message: "项目名称不能为空",
        details: { error: "项目名称不能为空" },
      });
  });

  it("按筛选条件管理 AI 模型完整生命周期", async () => {
    const model = {
      model_id: "44444444-4444-4444-8444-444444444444",
      display_name: "GPT Text",
      model_type: "text",
      version: 2,
    };
    fetchMock.mockImplementation(() => Promise.resolve(jsonResponse(model)));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });
    const payload = {
      display_name: "GPT Text",
      model_type: "text" as const,
      provider_name: "OpenAI",
      api_protocol: "openai_responses" as const,
      protocol_version: "v1",
      auth_scheme: "bearer" as const,
      request_base_url: "https://api.example/v1",
      upstream_model: "gpt-test",
      api_key: "secret",
      api_secret: null,
      timeout_seconds: 120,
      reasoning_effort: "high",
      max_output_tokens: 3000,
      settings: {},
      sort_order: 0,
      remark: "",
      is_default: false,
    };

    await listAiModels(client, {
      type: "text",
      status: "enabled",
      provider: "OpenAI",
      protocol: "openai_responses",
      q: "GPT",
    });
    await createAiModel(client, payload);
    await updateAiModel(client, model.model_id, { ...payload, version: 2, api_key: "" });
    await setDefaultAiModel(client, model.model_id, { version: 3 });
    await changeAiModelStatus(client, model.model_id, {
      version: 4,
      status: "disabled",
      replacement_model_id: null,
      allow_no_default: true,
    });
    await deleteAiModel(client, model.model_id, {
      version: 5,
      replacement_model_id: null,
      allow_no_default: true,
    });

    expect(fetchMock.mock.calls.map(([url]) => url)).toEqual([
      "http://api.test/api/admin/models?type=text&status=enabled&provider=OpenAI&protocol=openai_responses&q=GPT",
      "http://api.test/api/admin/models",
      `http://api.test/api/admin/models/${model.model_id}`,
      `http://api.test/api/admin/models/${model.model_id}/default`,
      `http://api.test/api/admin/models/${model.model_id}/status`,
      `http://api.test/api/admin/models/${model.model_id}`,
    ]);
    expect(fetchMock.mock.calls[2][1]).toMatchObject({
      method: "PUT",
      body: JSON.stringify({ ...payload, version: 2, api_key: "" }),
    });
    expect(fetchMock.mock.calls[3][1]).toMatchObject({
      method: "POST",
      body: JSON.stringify({ version: 3 }),
    });
    expect(fetchMock.mock.calls[5][1]).toMatchObject({ method: "DELETE" });
  });
});
