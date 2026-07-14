import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  createAgentConversation,
  confirmAssetGenerationTask,
  createApiClient,
  createAssetGenerationTasks,
  createContentTopic,
  createSceneAssetGenerationTask,
  createTopicGroupReview,
  deleteContentTopic,
  dismissAssetGenerationTask,
  generateStrategyProfileDraft,
  generateScript,
  getAssetGenerationPlan,
  getApiBaseUrl,
  getLatestTopicQualityEvaluation,
  getLatestTopicGroupReview,
  getMaterial,
  getScript,
  getScriptAgentTurnMetadata,
  listAgentMessages,
  listAssetCandidates,
  listAssetGenerationTasks,
  listContentTopics,
  listMaterials,
  listModelOptions,
  listTopicGenerationBatches,
  listTopicGroups,
  listProjects,
  listScripts,
  listWorkspaceMenus,
  prepareScriptFromTopic,
  sendAgentMessage,
  createMaterial,
  rejectAssetCandidate,
  selectAssetCandidate,
  updateContentTopic,
  updateContentTopicStatus,
  updateMaterial,
  updateMaterialStatus,
  updateProjectStrategyProfile,
  updateScriptStatus,
  uploadMaterial,
} from "./api";
import type { MaterialPayload } from "./api";

const textModelId = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const imageModelId = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";

const strategyProfile = {
  target_audience: "内容运营负责人",
  content_pillars: ["AI 工具", "内容生产"],
  tone_style: "直接清晰",
  forbidden_topics: ["夸大收益"],
  reference_accounts: ["参考账号A"],
  topic_preferences: "优先教程和案例",
};

const project = {
  project_id: "11111111-1111-4111-8111-111111111111",
  name: "科技博主",
  positioning: "科技知识账号",
  description: "面向程序员的知识短视频",
  strategy_profile: strategyProfile,
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
  ],
  updated_at: "2026-07-02T00:05:00Z",
};

const contentTopic = {
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
  created_at: "2026-07-02T00:20:00Z",
  updated_at: "2026-07-02T00:20:00Z",
};

const contentTopicPayload = {
  title: contentTopic.title,
  angle: contentTopic.angle,
  target_audience: contentTopic.target_audience,
  hook_points: contentTopic.hook_points,
  content_type: contentTopic.content_type,
  score: contentTopic.score,
  score_reason: contentTopic.score_reason,
  tags: contentTopic.tags,
};

const material = {
  material_id: "abababab-abab-4aba-8aba-abababababab",
  project_id: project.project_id,
  material_type: "subtitle",
  file_url: "https://cdn.example.com/subtitles/demo.vtt",
  thumbnail_url: "https://cdn.example.com/covers/demo.jpg",
  file_name: "demo.vtt",
  tags: ["字幕", "中英双语"],
  metadata: {
    language: "zh-CN",
    subtitle_format: "vtt",
  },
  usage_count: 0,
  status: "active",
  created_at: "2026-07-09T00:00:00Z",
  updated_at: "2026-07-09T00:00:00Z",
} as const;

const materialPayload: MaterialPayload = {
  material_type: "subtitle",
  file_url: "https://cdn.example.com/subtitles/demo.vtt",
  thumbnail_url: "https://cdn.example.com/covers/demo.jpg",
  file_name: "demo.vtt",
  tags: ["字幕", "中英双语"],
  metadata: {
    language: "zh-CN",
    subtitle_format: "vtt",
  },
};

const assetGenerationPlan = {
  script_id: scriptSummary.script_id,
  scene_count: 2,
  image_candidate_count: 6,
  max_image_candidate_count: 48,
  model_id: imageModelId,
  provider: "gpt-image-2",
  reference_material_count: 1,
  video_task_count: 2,
  can_create: true,
  warnings: [],
};

const imageAssetGenerationTask = {
  task_id: "12121212-1212-4212-8212-121212121212",
  project_id: project.project_id,
  script_id: scriptSummary.script_id,
  scene_id: null,
  model_id: imageModelId,
  model_snapshot: null,
  provider: "gpt-image-2",
  task_type: "image_candidates",
  status: "pending",
  candidate_count: 6,
  reference_material_ids: [material.material_id],
  params: { image_candidates_per_scene: 3 },
  result: {},
  error_message: null,
  retry_count: 0,
  created_at: "2026-07-09T00:30:00Z",
  updated_at: "2026-07-09T00:30:00Z",
};

const videoDraftAssetGenerationTask = {
  ...imageAssetGenerationTask,
  task_id: "13131313-1313-4313-8313-131313131313",
  scene_id: scriptDetail.scenes[0].scene_id,
  task_type: "video_draft",
  status: "draft",
  candidate_count: 0,
  params: { requires_manual_confirmation: true },
};

const sceneAssetCandidate = {
  candidate_id: "14141414-1414-4414-8414-141414141414",
  project_id: project.project_id,
  script_id: scriptSummary.script_id,
  scene_id: scriptDetail.scenes[0].scene_id,
  material_id: material.material_id,
  candidate_type: "image",
  source: "ai_generated",
  status: "candidate",
  rank: 1,
  generation_task_id: imageAssetGenerationTask.task_id,
  metadata: { source_scene_id: scriptDetail.scenes[0].scene_id },
  file_url: "http://api.test/assets/generated/images/task/scene-1.png",
  thumbnail_url: "http://api.test/assets/generated/images/task/scene-1.png",
  file_name: "scene-1.png",
  created_at: "2026-07-09T00:31:00Z",
  updated_at: "2026-07-09T00:31:00Z",
};

const topicGenerationBatch = {
  batch_id: "77777777-7777-4777-8777-777777777777",
  project_id: project.project_id,
  supplement_of_batch_id: null,
  prompt: "本周 AI 工具方向，生成 5 个选题",
  requested_count: 5,
  topic_count: 5,
  status: "succeeded",
  error_message: null,
  created_at: "2026-07-02T00:22:00Z",
  updated_at: "2026-07-02T00:22:20Z",
};

const topicReviewSnapshot = {
  snapshot_id: "99999999-9999-4999-8999-999999999999",
  project_id: project.project_id,
  root_batch_id: topicGenerationBatch.batch_id,
  source_run_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
  status: "succeeded",
  review_summary: "优先推进 1 个选题，保留 1 个备选，淘汰 1 个泛化选题。",
  result: {
    topic_reviews: [
      {
        topic_id: contentTopic.topic_id,
        priority: "priority",
        reason: "与账号定位强相关，角度清晰。",
        risk_flags: [],
        similar_topic_ids: [],
      },
    ],
  },
  error_message: null,
  metadata: {},
  created_at: "2026-07-02T00:30:00Z",
  updated_at: "2026-07-02T00:30:10Z",
};

const topicQualityEvaluation = {
  evaluation_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
  project_id: project.project_id,
  batch_id: topicGenerationBatch.batch_id,
  source_run_id: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
  status: "succeeded",
  pass_count: 2,
  reject_count: 1,
  rewrite_triggered: true,
  result: {
    summary: "重写后 3 条中 2 条通过，1 条淘汰。",
    items: [
      {
        candidate_key: "candidate-1",
        title: "AI 工具如何改变选题会",
        decision: "pass",
        quality_score: 88,
        flags: [],
        reason: "贴合账号定位。",
      },
      {
        candidate_key: "candidate-2",
        title: "人工智能是什么",
        decision: "reject",
        quality_score: 52,
        flags: ["too_generic", "score_untrusted"],
        reason: "标题过于泛化。",
      },
    ],
  },
  error_message: null,
  created_at: "2026-07-02T00:22:12Z",
  updated_at: "2026-07-02T00:22:18Z",
};

const topicGroupSummary = {
  root_batch_id: topicGenerationBatch.batch_id,
  project_id: project.project_id,
  prompt: topicGenerationBatch.prompt,
  created_at: topicGenerationBatch.created_at,
  topic_count: 5,
  supplement_batch_count: 1,
  latest_review_snapshot_id: topicReviewSnapshot.snapshot_id,
  review_freshness: "fresh",
  script_priority: {
    status: "ready_for_script",
    score: 86,
    reason: "存在 3 个无明显风险的优先推荐选题，脚本化路径清晰。",
    metrics: {
      priority_count: 4,
      backup_count: 3,
      reject_count: 1,
      duplicate_count: 1,
      hard_to_script_count: 0,
      off_positioning_count: 0,
      compliance_risk_count: 0,
      ready_candidate_count: 3,
      high_score_topic_count: 4,
    },
    recommended_topic_ids: [contentTopic.topic_id],
  },
};

const preparedTopicResponse = {
  topic: { ...contentTopic, status: "approved" },
  topic_snapshot: {
    topic_id: contentTopic.topic_id,
    title: contentTopic.title,
    angle: contentTopic.angle,
    target_audience: contentTopic.target_audience,
    hook_points: contentTopic.hook_points,
    content_type: contentTopic.content_type,
    score: contentTopic.score,
    score_reason: contentTopic.score_reason,
    tags: contentTopic.tags,
    source: contentTopic.source,
    status: "approved",
    created_at: contentTopic.created_at,
  },
  script_request: {
    project_id: project.project_id,
    topic_id: contentTopic.topic_id,
    topic: contentTopic.title,
    style: "knowledge",
    scene_count: 6,
  },
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

const generatedScriptAssistantMessage = {
  ...assistantMessage,
  content: "已创建 6 镜脚本，列表已刷新。",
  metadata: {
    intent: "generate_script",
    script_id: scriptSummary.script_id,
    script_created: true,
    needs_input: false,
    missing_fields: [],
  },
};

const missingInputAssistantMessage = {
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
    is_enabled: true,
    status: "active",
    metadata: { phase: 2 },
    children: [
      {
        menu_id: "20000000-0000-4000-8000-000000000009",
        menu_key: "account-strategy",
        label: "账号策略",
        description: "维护当前内容账号策略资料。",
        route_path: "/strategy/account",
        icon: "badge",
        menu_type: "page",
        module_key: "strategy.account",
        agent_key: "topic-generation-agent",
        sort_order: 10,
        is_enabled: true,
        status: "active",
        metadata: { phase: 2 },
        children: [],
      },
      {
        menu_id: "20000000-0000-4000-8000-000000000008",
        menu_key: "topic-history",
        label: "历史生成",
        description: "历史生成批次。",
        route_path: "/strategy/topic-history",
        icon: "history",
        menu_type: "page",
        module_key: "strategy.topic-history",
        agent_key: "topic-generation-agent",
        sort_order: 20,
        is_enabled: true,
        status: "active",
        metadata: { phase: 2 },
        children: [],
      },
      {
        menu_id: "20000000-0000-4000-8000-000000000001",
        menu_key: "topic-generator",
        label: "当前选题池",
        description: "基于账号定位、热点趋势和数据回流生成候选选题。",
        route_path: "/strategy/topics",
        icon: "sparkles",
        menu_type: "page",
        module_key: "strategy.topics",
        agent_key: "topic-generation-agent",
        sort_order: 30,
        is_enabled: true,
        status: "active",
        metadata: { phase: 2 },
        children: [],
      },
    ],
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
    vi.unstubAllGlobals();
  });

  it("默认使用本地 API 端口", () => {
    delete process.env.NEXT_PUBLIC_API_BASE_URL;

    expect(getApiBaseUrl()).toBe("http://localhost:18180");
  });

  it("未配置环境变量时使用当前页面内网 hostname 派生 API 端口", () => {
    delete process.env.NEXT_PUBLIC_API_BASE_URL;
    vi.stubGlobal("location", new URL("http://10.1.31.7:18183/"));

    expect(getApiBaseUrl()).toBe("http://10.1.31.7:18180");
  });

  it("未配置环境变量时使用当前页面回环 hostname 派生 API 端口", () => {
    delete process.env.NEXT_PUBLIC_API_BASE_URL;
    vi.stubGlobal("location", new URL("http://127.0.0.1:18183/"));

    expect(getApiBaseUrl()).toBe("http://127.0.0.1:18180");
  });

  it("使用环境变量覆盖，并移除结尾斜杠", () => {
    process.env.NEXT_PUBLIC_API_BASE_URL = "http://api.example.test/";
    vi.stubGlobal("location", new URL("http://10.1.31.7:18183/"));

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
    expect(result.menus[0].children[0].menu_key).toBe("account-strategy");
    expect(result.menus[0].children[1].menu_key).toBe("topic-history");
    expect(result.menus[0].children[2].label).toBe("当前选题池");
    expect(result.menus[1].children[0].agent_key).toBe("script-generation-agent");
  });

  it("按类型读取工作台模型选项", async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({
      models: [{
        model_id: "99999999-9999-4999-8999-999999999999",
        display_name: "GPT Text",
        model_type: "text",
        provider_name: "OpenAI",
        api_protocol: "openai_responses",
        upstream_model: "gpt-test",
        is_default: true,
      }],
    }));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await listModelOptions(client, "text");

    expect(fetchMock).toHaveBeenCalledWith("http://api.test/api/model-options?type=text", {
      headers: { accept: "application/json" },
    });
    expect(result.models[0].is_default).toBe(true);
  });

  it("更新账号策略资料并生成策略草稿", async () => {
    const draftResponse = {
      draft: {
        ...strategyProfile,
        target_audience: "AI 副业新手",
      },
      draft_summary: "草稿偏向 AI 工具教程和避坑。",
    };
    fetchMock
      .mockResolvedValueOnce(jsonResponse({ ...project, name: "AI 工具账号" }))
      .mockResolvedValueOnce(jsonResponse(draftResponse));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const updated = await updateProjectStrategyProfile(client, project.project_id, {
      name: "AI 工具账号",
      positioning: "AI 工具教程账号",
      description: "面向内容运营负责人",
      strategy_profile: strategyProfile,
    });
    const draft = await generateStrategyProfileDraft(client, project.project_id, {
      direction_notes: "偏 AI 副业新手，强调避坑。",
      model_id: textModelId,
    });

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      `http://api.test/api/projects/${project.project_id}/strategy-profile`,
      {
        method: "PUT",
        headers: {
          accept: "application/json",
          "content-type": "application/json",
        },
        body: JSON.stringify({
          name: "AI 工具账号",
          positioning: "AI 工具教程账号",
          description: "面向内容运营负责人",
          strategy_profile: strategyProfile,
        }),
      },
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      `http://api.test/api/projects/${project.project_id}/strategy-profile/draft`,
      {
        method: "POST",
        headers: {
          accept: "application/json",
          "content-type": "application/json",
        },
        body: JSON.stringify({
          direction_notes: "偏 AI 副业新手，强调避坑。",
          model_id: textModelId,
        }),
      },
    );
    expect(updated.name).toBe("AI 工具账号");
    expect(draft.draft.target_audience).toBe("AI 副业新手");
    expect(draft.draft_summary).toContain("避坑");
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

  it("请求选题列表时带状态和来源筛选", async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        topics: [contentTopic],
        stats: { total: 1, idea: 1, approved: 0, scripted: 0, archived: 0 },
      }),
    );
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await listContentTopics(client, project.project_id, {
      status: "idea",
      source: "manual",
    });

    expect(fetchMock).toHaveBeenCalledWith(
      `http://api.test/api/projects/${project.project_id}/topics?status=idea&source=manual`,
      { headers: { accept: "application/json" } },
    );
    expect(result.stats.idea).toBe(1);
    expect(result.topics[0].title).toBe(contentTopic.title);
  });

  it("请求素材列表时带类型、状态、关键词和标签筛选", async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({ materials: [material] }));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await listMaterials(client, project.project_id, {
      material_type: "subtitle",
      status: "archived",
      q: "demo",
      tag: "字幕",
    });

    expect(fetchMock).toHaveBeenCalledWith(
      `http://api.test/api/projects/${project.project_id}/materials?type=subtitle&status=archived&q=demo&tag=%E5%AD%97%E5%B9%95`,
      { headers: { accept: "application/json" } },
    );
    expect(result.materials[0].material_id).toBe(material.material_id);
  });

  it("创建、读取、编辑素材并更新状态", async () => {
    fetchMock
      .mockResolvedValueOnce(jsonResponse(material, { status: 201 }))
      .mockResolvedValueOnce(jsonResponse(material))
      .mockResolvedValueOnce(jsonResponse({ ...material, file_name: "demo-updated.vtt" }))
      .mockResolvedValueOnce(jsonResponse({ ...material, status: "archived" }));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const created = await createMaterial(client, project.project_id, materialPayload);
    const detail = await getMaterial(client, material.material_id);
    const updated = await updateMaterial(client, material.material_id, {
      ...materialPayload,
      file_name: "demo-updated.vtt",
    });
    const archived = await updateMaterialStatus(client, material.material_id, "archived");

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      `http://api.test/api/projects/${project.project_id}/materials`,
      {
        method: "POST",
        headers: {
          accept: "application/json",
          "content-type": "application/json",
        },
        body: JSON.stringify(materialPayload),
      },
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      `http://api.test/api/materials/${material.material_id}`,
      { headers: { accept: "application/json" } },
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      `http://api.test/api/materials/${material.material_id}`,
      {
        method: "PUT",
        headers: {
          accept: "application/json",
          "content-type": "application/json",
        },
        body: JSON.stringify({ ...materialPayload, file_name: "demo-updated.vtt" }),
      },
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      4,
      `http://api.test/api/materials/${material.material_id}/status`,
      {
        method: "PUT",
        headers: {
          accept: "application/json",
          "content-type": "application/json",
        },
        body: JSON.stringify({ status: "archived" }),
      },
    );
    expect(created.material_id).toBe(material.material_id);
    expect(detail.file_name).toBe("demo.vtt");
    expect(updated.file_name).toBe("demo-updated.vtt");
    expect(archived.status).toBe("archived");
  });

  it("使用 multipart 上传素材且不手工设置 content-type", async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        ...material,
        material_type: "image",
        file_url: "/assets/uploads/project/upload.png",
        thumbnail_url: null,
        file_name: "封面素材",
      }),
    );
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });
    const file = new File(["png"], "cover.png", { type: "image/png" });

    const uploaded = await uploadMaterial(client, project.project_id, {
      file,
      file_name: "封面素材",
      tags: ["封面", "办公"],
    });

    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe(`http://api.test/api/projects/${project.project_id}/materials/upload`);
    expect(init?.method).toBe("POST");
    expect(init?.headers).toEqual({ accept: "application/json" });
    expect(init?.body).toBeInstanceOf(FormData);
    const body = init?.body as FormData;
    expect(body.get("file")).toBe(file);
    expect(body.get("file_name")).toBe("封面素材");
    expect(body.get("tags")).toBe(JSON.stringify(["封面", "办公"]));
    expect(uploaded.file_url).toBe("http://api.test/assets/uploads/project/upload.png");
  });

  it("编辑自管素材时回传稳定相对地址且不改写外部地址", async () => {
    fetchMock
      .mockResolvedValueOnce(jsonResponse(material))
      .mockResolvedValueOnce(jsonResponse(material));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    await updateMaterial(client, material.material_id, {
      ...materialPayload,
      file_url: "http://api.test/assets/uploads/project/upload.png",
      thumbnail_url: "http://api.test/assets/uploads/project/thumb.png",
    });
    await updateMaterial(client, material.material_id, {
      ...materialPayload,
      file_url: "https://cdn.example.com/assets/external.png",
      thumbnail_url: null,
    });

    expect(JSON.parse(String(fetchMock.mock.calls[0][1]?.body))).toMatchObject({
      file_url: "/assets/uploads/project/upload.png",
      thumbnail_url: "/assets/uploads/project/thumb.png",
    });
    expect(JSON.parse(String(fetchMock.mock.calls[1][1]?.body))).toMatchObject({
      file_url: "https://cdn.example.com/assets/external.png",
      thumbnail_url: null,
    });
  });

  it("创建素材生成计划和批量任务", async () => {
    fetchMock
      .mockResolvedValueOnce(jsonResponse(assetGenerationPlan))
      .mockResolvedValueOnce(
        jsonResponse(
          {
            script_id: scriptSummary.script_id,
            tasks: [imageAssetGenerationTask, videoDraftAssetGenerationTask],
          },
          { status: 201 },
        ),
      )
      .mockResolvedValueOnce(
        jsonResponse({
          script_id: scriptSummary.script_id,
          tasks: [imageAssetGenerationTask, videoDraftAssetGenerationTask],
        }),
      );
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });
    const payload = {
      model_id: imageModelId,
      image_candidates_per_scene: 3,
      use_reference_materials: true,
    };

    const plan = await getAssetGenerationPlan(client, scriptSummary.script_id, payload);
    const tasks = await createAssetGenerationTasks(client, scriptSummary.script_id, payload);
    const listedTasks = await listAssetGenerationTasks(client, scriptSummary.script_id);

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      `http://api.test/api/scripts/${scriptSummary.script_id}/asset-generation-plan`,
      {
        method: "POST",
        headers: {
          accept: "application/json",
          "content-type": "application/json",
        },
        body: JSON.stringify(payload),
      },
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      `http://api.test/api/scripts/${scriptSummary.script_id}/asset-generation-tasks`,
      {
        headers: {
          accept: "application/json",
        },
      },
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      `http://api.test/api/scripts/${scriptSummary.script_id}/asset-generation-tasks`,
      {
        method: "POST",
        headers: {
          accept: "application/json",
          "content-type": "application/json",
        },
        body: JSON.stringify(payload),
      },
    );
    expect(plan.image_candidate_count).toBe(6);
    expect(plan.model_id).toBe(imageModelId);
    expect(tasks.tasks.map((task) => task.task_type)).toEqual(["image_candidates", "video_draft"]);
    expect(listedTasks.tasks.map((task) => task.status)).toEqual(["pending", "draft"]);
  });

  it("清理失败素材生成任务", async () => {
    const dismissedAt = "2026-07-10T08:30:00Z";
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        ...imageAssetGenerationTask,
        status: "failed",
        error_message: "Image generation is not enabled for this group",
        dismissed_at: dismissedAt,
      }),
    );
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const task = await dismissAssetGenerationTask(client, imageAssetGenerationTask.task_id);

    expect(fetchMock).toHaveBeenCalledWith(
      `http://api.test/api/asset-generation-tasks/${imageAssetGenerationTask.task_id}/dismiss`,
      {
        method: "POST",
        headers: { accept: "application/json" },
      },
    );
    expect(task.status).toBe("failed");
    expect(task.dismissed_at).toBe(dismissedAt);
  });

  it("读取并操作分镜素材候选", async () => {
    fetchMock
      .mockResolvedValueOnce(jsonResponse({ candidates: [sceneAssetCandidate] }))
      .mockResolvedValueOnce(jsonResponse({ ...sceneAssetCandidate, status: "selected" }))
      .mockResolvedValueOnce(jsonResponse({ ...sceneAssetCandidate, status: "rejected" }))
      .mockResolvedValueOnce(jsonResponse({ ...imageAssetGenerationTask, scene_id: scriptDetail.scenes[0].scene_id }, { status: 201 }));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });
    const payload = {
      model_id: imageModelId,
      image_candidates_per_scene: 2,
      use_reference_materials: false,
    };
    const idempotencyKey = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";

    const candidates = await listAssetCandidates(client, scriptSummary.script_id);
    const selected = await selectAssetCandidate(
      client,
      scriptDetail.scenes[0].scene_id,
      sceneAssetCandidate.candidate_id,
    );
    const rejected = await rejectAssetCandidate(
      client,
      scriptDetail.scenes[0].scene_id,
      sceneAssetCandidate.candidate_id,
    );
    const regenerateTask = await createSceneAssetGenerationTask(
      client,
      scriptDetail.scenes[0].scene_id,
      payload,
      idempotencyKey,
    );

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      `http://api.test/api/scripts/${scriptSummary.script_id}/asset-candidates`,
      { headers: { accept: "application/json" } },
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      `http://api.test/api/scenes/${scriptDetail.scenes[0].scene_id}/asset-candidates/${sceneAssetCandidate.candidate_id}/select`,
      { method: "PUT", headers: { accept: "application/json" } },
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      `http://api.test/api/scenes/${scriptDetail.scenes[0].scene_id}/asset-candidates/${sceneAssetCandidate.candidate_id}/reject`,
      { method: "PUT", headers: { accept: "application/json" } },
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      4,
      `http://api.test/api/scenes/${scriptDetail.scenes[0].scene_id}/asset-generation-tasks`,
      {
        method: "POST",
        headers: {
          accept: "application/json",
          "content-type": "application/json",
          "idempotency-key": idempotencyKey,
        },
        body: JSON.stringify(payload),
      },
    );
    expect(candidates.candidates[0].source).toBe("ai_generated");
    expect(selected.status).toBe("selected");
    expect(rejected.status).toBe("rejected");
    expect(regenerateTask.task_type).toBe("image_candidates");
  });

  it("将自管素材相对 URL 解析到 API 地址", async () => {
    const relativeCandidate = {
      ...sceneAssetCandidate,
      file_url: "/assets/generated/images/task-1/scene-1.png",
      thumbnail_url: "/assets/generated/images/task-1/scene-1-thumb.png",
    };
    const relativeMaterial = {
      ...material,
      material_type: "image" as const,
      file_url: "/assets/generated/images/task-1/scene-1.png",
      thumbnail_url: null,
    };
    fetchMock
      .mockResolvedValueOnce(jsonResponse({ candidates: [relativeCandidate] }))
      .mockResolvedValueOnce(jsonResponse({ materials: [relativeMaterial] }));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const candidates = await listAssetCandidates(client, scriptSummary.script_id);
    const materials = await listMaterials(client, project.project_id);

    expect(candidates.candidates[0].file_url).toBe(
      "http://api.test/assets/generated/images/task-1/scene-1.png",
    );
    expect(candidates.candidates[0].thumbnail_url).toBe(
      "http://api.test/assets/generated/images/task-1/scene-1-thumb.png",
    );
    expect(materials.materials[0].file_url).toBe(
      "http://api.test/assets/generated/images/task-1/scene-1.png",
    );
  });

  it("确认 AI 视频生成任务", async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        ...videoDraftAssetGenerationTask,
        task_type: "video_generation",
        status: "pending",
      }),
    );
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await confirmAssetGenerationTask(client, videoDraftAssetGenerationTask.task_id);

    expect(fetchMock).toHaveBeenCalledWith(
      `http://api.test/api/asset-generation-tasks/${videoDraftAssetGenerationTask.task_id}/confirm`,
      {
        method: "POST",
        headers: {
          accept: "application/json",
        },
      },
    );
    expect(result.task_type).toBe("video_generation");
    expect(result.status).toBe("pending");
  });

  it("请求选题生成批次列表", async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({ batches: [topicGenerationBatch] }));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await listTopicGenerationBatches(client, project.project_id);

    expect(fetchMock).toHaveBeenCalledWith(
      `http://api.test/api/projects/${project.project_id}/topic-generation-batches`,
      { headers: { accept: "application/json" } },
    );
    expect(result.batches[0].batch_id).toBe(topicGenerationBatch.batch_id);
    expect(result.batches[0].topic_count).toBe(5);
  });

  it("按脚本优先级请求主题组列表", async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse({ topic_groups: [topicGroupSummary] }));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await listTopicGroups(client, project.project_id, { sort: "script_priority" });

    expect(fetchMock).toHaveBeenCalledWith(
      `http://api.test/api/projects/${project.project_id}/topic-groups?sort=script_priority`,
      { headers: { accept: "application/json" } },
    );
    expect(result.topic_groups[0].root_batch_id).toBe(topicGenerationBatch.batch_id);
    expect(result.topic_groups[0].script_priority.status).toBe("ready_for_script");
  });

  it("创建主题组评审快照", async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse(topicReviewSnapshot, { status: 201 }));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await createTopicGroupReview(client, topicGenerationBatch.batch_id, {
      model_id: textModelId,
    });

    expect(fetchMock).toHaveBeenCalledWith(
      `http://api.test/api/topic-groups/${topicGenerationBatch.batch_id}/reviews`,
      {
        method: "POST",
        headers: {
          accept: "application/json",
          "content-type": "application/json",
        },
        body: JSON.stringify({ model_id: textModelId }),
      },
    );
    expect(result.snapshot_id).toBe(topicReviewSnapshot.snapshot_id);
    expect(result.result.topic_reviews[0].priority).toBe("priority");
  });

  it("读取主题组最新评审快照", async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse(topicReviewSnapshot));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await getLatestTopicGroupReview(client, topicGenerationBatch.batch_id);

    expect(fetchMock).toHaveBeenCalledWith(
      `http://api.test/api/topic-groups/${topicGenerationBatch.batch_id}/reviews/latest`,
      { headers: { accept: "application/json" } },
    );
    expect(result?.root_batch_id).toBe(topicGenerationBatch.batch_id);
    expect(result?.review_summary).toContain("优先推进");
  });

  it("读取选题批次最新质量评估报告", async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse(topicQualityEvaluation));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await getLatestTopicQualityEvaluation(
      client,
      topicGenerationBatch.batch_id,
      project.project_id,
    );

    expect(fetchMock).toHaveBeenCalledWith(
      `http://api.test/api/topic-generation-batches/${topicGenerationBatch.batch_id}/quality-evaluation?project_id=${project.project_id}`,
      { headers: { accept: "application/json" } },
    );
    expect(result?.pass_count).toBe(2);
    expect(result?.reject_count).toBe(1);
    expect(result?.rewrite_triggered).toBe(true);
    expect(result?.result.items[1].flags).toEqual(["too_generic", "score_untrusted"]);
  });

  it("创建、编辑并更新选题状态", async () => {
    fetchMock
      .mockResolvedValueOnce(jsonResponse(contentTopic, { status: 201 }))
      .mockResolvedValueOnce(jsonResponse({ ...contentTopic, angle: "新角度" }))
      .mockResolvedValueOnce(jsonResponse({ ...contentTopic, status: "approved" }));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const created = await createContentTopic(client, project.project_id, contentTopicPayload);
    const updated = await updateContentTopic(client, contentTopic.topic_id, {
      ...contentTopicPayload,
      angle: "新角度",
    });
    const approved = await updateContentTopicStatus(client, contentTopic.topic_id, "approved");

    expect(fetchMock).toHaveBeenNthCalledWith(1, `http://api.test/api/projects/${project.project_id}/topics`, {
      method: "POST",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
      },
      body: JSON.stringify(contentTopicPayload),
    });
    expect(fetchMock).toHaveBeenNthCalledWith(2, `http://api.test/api/topics/${contentTopic.topic_id}`, {
      method: "PUT",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
      },
      body: JSON.stringify({ ...contentTopicPayload, angle: "新角度" }),
    });
    expect(fetchMock).toHaveBeenNthCalledWith(3, `http://api.test/api/topics/${contentTopic.topic_id}/status`, {
      method: "PUT",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
      },
      body: JSON.stringify({ status: "approved" }),
    });
    expect(created.topic_id).toBe(contentTopic.topic_id);
    expect(updated.angle).toBe("新角度");
    expect(approved.status).toBe("approved");
  });

  it("删除选题时调用软删除接口", async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        topic_id: contentTopic.topic_id,
        deleted_at: "2026-07-07T10:00:00Z",
      }),
    );
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await deleteContentTopic(client, contentTopic.topic_id);

    expect(fetchMock).toHaveBeenCalledWith(`http://api.test/api/topics/${contentTopic.topic_id}`, {
      method: "DELETE",
      headers: {
        accept: "application/json",
      },
    });
    expect(result.topic_id).toBe(contentTopic.topic_id);
    expect(result.deleted_at).toBe("2026-07-07T10:00:00Z");
  });

  it("准备从已确认选题生成脚本", async () => {
    fetchMock.mockResolvedValueOnce(jsonResponse(preparedTopicResponse));
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await prepareScriptFromTopic(client, contentTopic.topic_id, {
      style: "knowledge",
      scene_count: 6,
    });

    expect(fetchMock).toHaveBeenCalledWith(
      `http://api.test/api/topics/${contentTopic.topic_id}/prepare-script`,
      {
        method: "POST",
        headers: {
          accept: "application/json",
          "content-type": "application/json",
        },
        body: JSON.stringify({ style: "knowledge", scene_count: 6 }),
      },
    );
    expect(result.topic_snapshot.title).toBe(contentTopic.title);
    expect(result.script_request.topic_id).toBe(contentTopic.topic_id);
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
      model_id: textModelId,
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
        model_id: textModelId,
        topic: "ChatGPT如何改变程序员工作流",
        style: "knowledge",
        scene_count: 5,
      }),
    });
    expect(result.title).toBe("程序员必看：ChatGPT工作流");
  });

  it("提交脚本生成请求时可只携带已确认选题 ID", async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        ...scriptDetail,
        topic_id: contentTopic.topic_id,
        topic_snapshot: preparedTopicResponse.topic_snapshot,
      }),
    );
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await generateScript(client, {
      project_id: project.project_id,
      model_id: textModelId,
      topic_id: contentTopic.topic_id,
      style: "knowledge",
      scene_count: 6,
    });

    expect(fetchMock).toHaveBeenCalledWith("http://api.test/api/scripts/generate", {
      method: "POST",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
      },
      body: JSON.stringify({
        project_id: project.project_id,
        model_id: textModelId,
        topic_id: contentTopic.topic_id,
        style: "knowledge",
        scene_count: 6,
      }),
    });
    expect(result.topic_id).toBe(contentTopic.topic_id);
    expect(result.topic_snapshot?.title).toBe(contentTopic.title);
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
      model_id: textModelId,
    });

    expect(fetchMock).toHaveBeenCalledWith(
      `http://api.test/api/agent/conversations/${conversation.conversation_id}/messages`,
      {
        method: "POST",
        headers: {
          accept: "application/json",
          "content-type": "application/json",
        },
        body: JSON.stringify({ content: userMessage.content, model_id: textModelId }),
      },
    );
    expect(result.assistant_message.content).toContain("已更新第 2 镜");
    expect(result.run.status).toBe("completed");
  });

  it("发送补充选题 Agent 消息时携带目标批次", async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({ user_message: userMessage, assistant_message: assistantMessage, run: agentRun }),
    );
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    await sendAgentMessage(client, conversation.conversation_id, {
      content: "补充 2 个 AI 内容流水线选题",
      model_id: textModelId,
      supplement_of_batch_id: "77777777-7777-4777-8777-777777777777",
    });

    expect(fetchMock).toHaveBeenCalledWith(
      `http://api.test/api/agent/conversations/${conversation.conversation_id}/messages`,
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          content: "补充 2 个 AI 内容流水线选题",
          model_id: textModelId,
          supplement_of_batch_id: "77777777-7777-4777-8777-777777777777",
        }),
      }),
    );
  });

  it("读取对话生成脚本返回的稳定 metadata", async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        user_message: userMessage,
        assistant_message: generatedScriptAssistantMessage,
        run: agentRun,
      }),
    );
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await sendAgentMessage(client, conversation.conversation_id, {
      content: "生成一个关于 ChatGPT 工作流的 6 镜知识科普脚本",
      model_id: textModelId,
    });

    expect(getScriptAgentTurnMetadata(result.assistant_message)).toEqual({
      intent: "generate_script",
      script_id: scriptSummary.script_id,
      script_created: true,
      needs_input: false,
      missing_fields: [],
    });
  });

  it("读取参数不足追问返回的稳定 metadata", async () => {
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        user_message: userMessage,
        assistant_message: missingInputAssistantMessage,
        run: agentRun,
      }),
    );
    const client = createApiClient({ baseUrl: "http://api.test", fetcher: fetchMock });

    const result = await sendAgentMessage(client, conversation.conversation_id, {
      content: "帮我生成脚本",
      model_id: textModelId,
    });

    expect(getScriptAgentTurnMetadata(result.assistant_message)).toEqual({
      intent: "generate_script",
      script_id: null,
      script_created: false,
      needs_input: true,
      missing_fields: ["topic", "style", "scene_count"],
    });
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
