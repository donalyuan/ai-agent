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
  AssetGenerationPlanResponse,
  AssetGenerationTask,
  ContentTopic,
  ContentTopicListResponse,
  Material,
  ModelOption,
  ProjectListResponse,
  SceneAssetCandidate,
  SceneVisualManifest,
  ScriptDetail,
  ScriptListResponse,
  SoundTask,
  TopicQualityEvaluation,
  TopicGenerationBatchListResponse,
  TopicGroupListResponse,
  TopicReviewSnapshot,
  WorkVersion,
  WorkspaceMenuListResponse,
} from "./lib/api";

vi.mock("react-konva", async () => {
  const React = await import("react");
  const MockNode = ({
    children,
    className,
    text,
  }: {
    children?: React.ReactNode;
    className?: string;
    text?: string;
  }) => React.createElement("div", className ? { className } : null, children ?? text ?? null);
  return {
    Stage: MockNode,
    Layer: MockNode,
    Group: MockNode,
    Rect: MockNode,
    Text: MockNode,
    Image: MockNode,
    Circle: MockNode,
    Line: MockNode,
  };
});

vi.mock("react-konva/es/ReactKonvaCore", async () => {
  const React = await import("react");
  const MockNode = ({
    children,
    className,
    text,
  }: {
    children?: React.ReactNode;
    className?: string;
    text?: string;
  }) => React.createElement("div", className ? { className } : null, children ?? text ?? null);
  return {
    Stage: MockNode,
    Layer: MockNode,
    Group: MockNode,
    Rect: MockNode,
    Text: MockNode,
    Image: MockNode,
  };
});

vi.mock("./lib/api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./lib/api")>();
  return {
    ...actual,
    checkHealth: vi.fn(),
    createApiClient: vi.fn(() => ({ baseUrl: "http://api.test", fetcher: vi.fn() })),
    listWorkspaceMenus: vi.fn(),
    listProjects: vi.fn(),
    listModelOptions: vi.fn(),
    listScripts: vi.fn(),
    getScript: vi.fn(),
    generateScript: vi.fn(),
    getAssetGenerationPlan: vi.fn(),
    createAssetGenerationTasks: vi.fn(),
    listAssetCandidates: vi.fn(),
    listAssetGenerationTasks: vi.fn(),
    selectAssetCandidate: vi.fn(),
    rejectAssetCandidate: vi.fn(),
    createSceneAssetGenerationTask: vi.fn(),
    getSceneVisualManifest: vi.fn(),
    validateSceneVisualManifest: vi.fn(),
    dismissAssetGenerationTask: vi.fn(),
    listContentTopics: vi.fn(),
    listMaterials: vi.fn(),
    getVoiceCatalog: vi.fn(),
    requestWorkspaceVoiceCatalogCheck: vi.fn(),
    requestAudioInspection: vi.fn(),
    getAudioInspection: vi.fn(),
    preflightSoundTask: vi.fn(),
    createSoundTask: vi.fn(),
    listSoundTasks: vi.fn(),
    getSoundTask: vi.fn(),
    retrySoundTask: vi.fn(),
    cancelSoundTask: vi.fn(),
    listTopicGenerationBatches: vi.fn(),
    listTopicGroups: vi.fn(),
    createTopicGroupReview: vi.fn(),
    generateStrategyProfileDraft: vi.fn(),
    getLatestTopicGroupReview: vi.fn(),
    getLatestTopicQualityEvaluation: vi.fn(),
    createContentTopic: vi.fn(),
    createMaterial: vi.fn(),
    uploadMaterial: vi.fn(),
    deleteContentTopic: vi.fn(),
    getMaterial: vi.fn(),
    updateMaterial: vi.fn(),
    updateMaterialStatus: vi.fn(),
    updateContentTopic: vi.fn(),
    updateContentTopicStatus: vi.fn(),
    updateProjectStrategyProfile: vi.fn(),
    prepareScriptFromTopic: vi.fn(),
    createAgentConversation: vi.fn(),
    listAgentMessages: vi.fn(),
    sendAgentMessage: vi.fn(),
    listWorks: vi.fn(),
    getWork: vi.fn(),
    deriveWorkVersion: vi.fn(),
    regenerateWorkVersion: vi.fn(),
    analyzeWorkVersionDiff: vi.fn(),
    confirmWorkVersionDiff: vi.fn(),
    getWorkVersionDownloads: vi.fn(),
    archiveWork: vi.fn(),
    restoreWork: vi.fn(),
    deleteWork: vi.fn(),
    createPublicationHandoff: vi.fn(),
    createPublicationPlan: vi.fn(),
    listPublications: vi.fn(),
    getPublication: vi.fn(),
    savePublicationTarget: vi.fn(),
    generatePublicationPackage: vi.fn(),
    getPublicationDownloads: vi.fn(),
    auditPublicationCopy: vi.fn(),
    auditPublicationDownload: vi.fn(),
    handoffPublicationTarget: vi.fn(),
    markPublicationNeedsAttention: vi.fn(),
    cancelPublicationTarget: vi.fn(),
    confirmPublicationPublished: vi.fn(),
    correctPublicationResult: vi.fn(),
  };
});

const strategyProfile = {
  target_audience: "内容运营负责人",
  content_pillars: ["AI 工具", "内容生产"],
  tone_style: "直接清晰",
  forbidden_topics: ["夸大收益"],
  reference_accounts: ["参考账号A"],
  topic_preferences: "优先教程和案例",
};

const textModel: ModelOption = {
  model_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
  display_name: "默认文本模型",
  model_type: "text",
  provider_name: "OpenAI",
  api_protocol: "openai_responses",
  upstream_model: "gpt-test",
  is_default: true,
};

const imageModel: ModelOption = {
  model_id: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
  display_name: "默认图片模型",
  model_type: "image",
  provider_name: "OpenAI",
  api_protocol: "openai_images",
  upstream_model: "gpt-image-test",
  is_default: true,
};

const secondaryTextModel: ModelOption = {
  ...textModel,
  model_id: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
  display_name: "备用文本模型",
  upstream_model: "gpt-secondary",
  is_default: false,
};

const emptyStrategyProfile = {
  target_audience: "",
  content_pillars: [],
  tone_style: "",
  forbidden_topics: [],
  reference_accounts: [],
  topic_preferences: "",
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
  topic_id: null,
  source_topic_title: null,
  title: "程序员必看：ChatGPT工作流",
  status: "draft" as const,
  scene_count: 2,
  parent_id: null,
  created_at: "2026-07-02T00:05:00Z",
  updated_at: "2026-07-02T00:05:00Z",
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
  updated_at: "2026-07-02T00:10:00Z",
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
          ...menuNode("account-strategy", "账号策略", true, "active", 10),
          agent_key: "topic-generation-agent",
          menu_type: "page",
          module_key: "strategy.account",
        },
        {
          ...menuNode("topic-history", "历史生成", true, "active", 20),
          agent_key: "topic-generation-agent",
          menu_type: "page",
          module_key: "strategy.topic-history",
        },
        {
          ...menuNode("topic-generator", "当前选题池", true, "active", 30),
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
    {
      ...menuNode("material-management", "素材管理", true, "active", 30),
      children: [
        {
          ...menuNode("material-library", "素材库", true, "active", 10),
          menu_type: "page",
          module_key: "materials.library",
        },
        {
          ...menuNode("asset-generation", "画面生成", true, "active", 20),
          agent_key: "material-generation-agent",
          menu_type: "page",
          module_key: "materials.asset-generation",
        },
        soundSubtitleMenuNode(),
      ],
    },
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
          ...menuNode("account-strategy", "账号策略", true, "active", 10),
          agent_key: "topic-generation-agent",
          menu_type: "page",
          module_key: "strategy.account",
        },
        {
          ...menuNode("topic-history", "历史生成", true, "active", 20),
          agent_key: "topic-generation-agent",
          menu_type: "page",
          module_key: "strategy.topic-history",
        },
        {
          ...menuNode("topic-generator", "当前选题池", true, "active", 30),
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
    {
      ...menuNode("material-management", "素材管理", true, "active", 30),
      children: [
        {
          ...menuNode("material-library", "素材库", true, "active", 10),
          menu_type: "page",
          module_key: "materials.library",
        },
        {
          ...menuNode("asset-generation", "画面生成", true, "active", 20),
          agent_key: "material-generation-agent",
          menu_type: "page",
          module_key: "materials.asset-generation",
        },
        soundSubtitleMenuNode(),
      ],
    },
    menuNode("production", "作品生产", false, "planned", 40),
    menuNode("publishing", "发布运营", false, "planned", 50),
    menuNode("analytics", "数据分析", false, "planned", 60),
    menuNode("workflow-tasks", "工作流任务", false, "planned", 70),
  ],
};

const materialWorkspaceMenus: WorkspaceMenuListResponse = {
  menus: [
    ...contentStrategyWorkspaceMenus.menus.slice(0, 2),
    {
      ...menuNode("material-management", "素材管理", true, "active", 30),
      children: [
        {
          ...menuNode("material-library", "素材库", true, "active", 10),
          menu_type: "page",
          module_key: "materials.library",
        },
        {
          ...menuNode("asset-generation", "画面生成", true, "active", 20),
          agent_key: "material-generation-agent",
          menu_type: "page",
          module_key: "materials.asset-generation",
        },
        soundSubtitleMenuNode(),
      ],
    },
    menuNode("production", "作品生产", false, "planned", 40),
    menuNode("publishing", "发布运营", false, "planned", 50),
    menuNode("analytics", "数据分析", false, "planned", 60),
    menuNode("workflow-tasks", "工作流任务", false, "planned", 70),
  ],
};

function soundSubtitleMenuNode() {
  return {
    ...menuNode("sound-subtitle-generation", "声音与字幕生成", true, "active", 30),
    agent_key: "sound-generation-agent",
    menu_type: "page" as const,
    module_key: "materials.sound-subtitle-generation",
  };
}

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
  updated_at: "2026-07-06T10:25:45.998632Z",
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
  supplement_of_batch_id: null,
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
  supplement_of_batch_id: null,
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
  supplement_of_batch_id: null,
  prompt: "失败的 AI 选题生成",
  requested_count: 5,
  topic_count: 0,
  status: "failed" as const,
  error_message: "invalid topic JSON",
  created_at: "2026-07-02T00:20:00Z",
  updated_at: "2026-07-02T00:20:20Z",
};

const supplementTopicBatch = {
  batch_id: "99999999-9999-4999-8999-999999999901",
  project_id: project.project_id,
  supplement_of_batch_id: previousTopicBatch.batch_id,
  prompt: "补充上一批 AI 内容流水线选题",
  requested_count: 2,
  topic_count: 2,
  status: "succeeded" as const,
  error_message: null,
  created_at: "2026-07-06T10:05:00Z",
  updated_at: "2026-07-06T10:05:20Z",
};

const topicBatchListResponse: TopicGenerationBatchListResponse = {
  batches: [latestTopicBatch, failedTopicBatch, previousTopicBatch],
};

const topicReviewSnapshot: TopicReviewSnapshot = {
  snapshot_id: "13131313-1313-4313-8313-131313131313",
  project_id: project.project_id,
  root_batch_id: previousTopicBatch.batch_id,
  source_run_id: "14141414-1414-4414-8414-141414141414",
  status: "succeeded",
  review_summary: "优先推荐 1 个，可备选 1 个，建议淘汰 1 个疑似重复选题。",
  result: {
    topic_reviews: [
      {
        topic_id: approvedTopic.topic_id,
        priority: "priority",
        reason: "账号定位匹配度高，能直接进入脚本创作。",
        risk_flags: [],
        similar_topic_ids: [],
      },
      {
        topic_id: ideaTopic.topic_id,
        priority: "backup",
        reason: "角度有潜力，但需要补充更具体的案例。",
        risk_flags: ["hard_to_script"],
        similar_topic_ids: [],
      },
      {
        topic_id: scriptedTopic.topic_id,
        priority: "reject",
        reason: "和优先选题表达过近，容易重复消耗同一受众。",
        risk_flags: ["duplicate", "too_generic"],
        similar_topic_ids: [approvedTopic.topic_id],
      },
    ],
  },
  error_message: null,
  metadata: {},
  created_at: "2026-07-07T09:00:00Z",
  updated_at: "2026-07-07T09:00:15Z",
};

const topicQualityEvaluation: TopicQualityEvaluation = {
  evaluation_id: "15151515-1515-4515-8515-151515151515",
  project_id: project.project_id,
  batch_id: latestTopicBatch.batch_id,
  source_run_id: "16161616-1616-4616-8616-161616161616",
  status: "succeeded",
  pass_count: 2,
  reject_count: 1,
  rewrite_triggered: true,
  result: {
    summary: "重写后 3 条中 2 条通过，1 条淘汰。",
    items: [
      {
        candidate_key: "candidate-1",
        title: approvedTopic.title,
        decision: "pass",
        quality_score: 88,
        flags: ["hard_to_script"],
        reason: "贴合账号定位，但脚本化案例需要补强。",
      },
      {
        candidate_key: "candidate-2",
        title: "人工智能是什么",
        decision: "reject",
        quality_score: 52,
        flags: ["too_generic", "score_untrusted"],
        reason: "标题过于泛化，原始评分可信度不足。",
      },
    ],
  },
  error_message: null,
  created_at: "2026-07-06T10:00:04Z",
  updated_at: "2026-07-06T10:00:09Z",
};

const readyTopicGroup = {
  root_batch_id: previousTopicBatch.batch_id,
  project_id: project.project_id,
  prompt: previousTopicBatch.prompt,
  created_at: previousTopicBatch.created_at,
  topic_count: 7,
  supplement_batch_count: 1,
  latest_review_snapshot_id: topicReviewSnapshot.snapshot_id,
  review_freshness: "fresh" as const,
  script_priority: {
    status: "ready_for_script" as const,
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
    recommended_topic_ids: [approvedTopic.topic_id],
  },
};

const missingReviewTopicGroup = {
  root_batch_id: latestTopicBatch.batch_id,
  project_id: project.project_id,
  prompt: latestTopicBatch.prompt,
  created_at: latestTopicBatch.created_at,
  topic_count: 5,
  supplement_batch_count: 0,
  latest_review_snapshot_id: null,
  review_freshness: "missing" as const,
  script_priority: {
    status: "needs_review" as const,
    score: null,
    reason: "缺少成功主题组评审快照，请先评审当前主题组。",
    metrics: {
      priority_count: 0,
      backup_count: 0,
      reject_count: 0,
      duplicate_count: 0,
      hard_to_script_count: 0,
      off_positioning_count: 0,
      compliance_risk_count: 0,
      ready_candidate_count: 0,
      high_score_topic_count: 0,
    },
    recommended_topic_ids: [],
  },
};

const staleReviewTopicGroup = {
  ...missingReviewTopicGroup,
  root_batch_id: previousTopicBatch.batch_id,
  prompt: previousTopicBatch.prompt,
  latest_review_snapshot_id: topicReviewSnapshot.snapshot_id,
  review_freshness: "stale" as const,
  script_priority: {
    ...missingReviewTopicGroup.script_priority,
    reason: "评审已过期，请重新评审当前主题组。",
  },
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

const subtitleMaterial: Material = {
  material_id: "abababab-abab-4aba-8aba-abababababab",
  project_id: project.project_id,
  material_type: "subtitle",
  file_url: "https://cdn.example.com/subtitles/demo.vtt",
  thumbnail_url: null,
  file_name: "demo.vtt",
  tags: ["字幕", "中英双语"],
  metadata: {
    language: "zh-CN",
    subtitle_format: "vtt",
    source_note: "人工整理",
  },
  source: null,
  audio_usage: null,
  work_id: null,
  work_version_id: null,
  generation: null,
  usage_count: 0,
  status: "active",
  created_at: "2026-07-09T00:00:00Z",
  updated_at: "2026-07-09T00:00:00Z",
};

const archivedSubtitleMaterial: Material = {
  ...subtitleMaterial,
  status: "archived",
  updated_at: "2026-07-09T00:10:00Z",
};

const uploadedImageMaterial: Material = {
  ...subtitleMaterial,
  material_id: "cdcdcdcd-cdcd-4cdc-8cdc-cdcdcdcdcdcd",
  material_type: "image",
  file_url: "http://api.test/assets/uploads/project/cover.png",
  file_name: "办公桌面近景",
  tags: ["办公", "场景"],
  metadata: {
    source: "user_upload",
    storage_provider: "local",
    mime_type: "image/png",
    format: "png",
    file_size_bytes: 2_515_456,
    width: 1920,
    height: 1080,
  },
  source: "user_upload",
};

const generatedTtsMaterial: Material = {
  ...subtitleMaterial,
  material_id: "edededed-eded-4ded-8ded-edededededed",
  material_type: "audio",
  file_url: "http://api.test/assets/generated/tts.wav",
  file_name: "Debug不内耗-V3-旁白.wav",
  tags: ["旁白", "TTS", "作品 V3"],
  metadata: { format: "wav", duration_sec: 31.4 },
  source: "work_generation",
  audio_usage: "tts",
  work_id: "31313131-3131-4131-8131-313131313131",
  work_version_id: "32323232-3232-4232-8232-323232323232",
  generation: {
    work_id: "31313131-3131-4131-8131-313131313131",
    work_version_id: "32323232-3232-4232-8232-323232323232",
    generation_run_id: "33333333-3333-4333-8333-333333333333",
    generation_step_id: "34343434-3434-4434-8434-343434343434",
    model_snapshot: { display_name: "豆包语音 2.0" },
    voice_snapshot: { speaker_name: "灿灿", language: "zh-CN", emotion: "温暖", speed: 1.05 },
    prompt_snapshot: { text_summary: "三段旁白，共 128 字" },
    request_trace_id: "req_7P2K8",
    duration_sec: 31.4,
  },
};

const uploadedAudioMaterial: Material = {
  ...generatedTtsMaterial,
  material_id: "fefefefe-fefe-4efe-8efe-fefefefefefe",
  file_url: "http://api.test/assets/uploads/project/city-morning.wav",
  file_name: "城市清晨环境声",
  tags: ["城市", "清晨", "环境声"],
  metadata: { format: "wav", duration_sec: 42, file_size_bytes: 19_503_514 },
  source: "user_upload",
  audio_usage: "ambient",
  work_id: null,
  work_version_id: null,
  generation: null,
};

const assetGenerationPlan: AssetGenerationPlanResponse = {
  script_id: scriptDetail.script_id,
  scene_count: scriptDetail.scenes.length,
  image_candidate_count: scriptDetail.scenes.length * 3,
  max_image_candidate_count: 48,
  model_id: imageModel.model_id,
  provider: "gpt-image-2",
  reference_material_count: 1,
  can_create: true,
  warnings: [],
};

const imageGenerationTask: AssetGenerationTask = {
  task_id: "17171717-1717-4717-8717-171717171717",
  project_id: project.project_id,
  script_id: scriptDetail.script_id,
  scene_id: null,
  model_id: imageModel.model_id,
  model_snapshot: null,
  provider: "gpt-image-2",
  task_type: "image_candidates",
  status: "pending",
  candidate_count: assetGenerationPlan.image_candidate_count,
  reference_material_ids: [subtitleMaterial.material_id],
  params: { image_candidates_per_scene: 3 },
  result: {},
  error_message: null,
  retry_count: 0,
  dismissed_at: null,
  read_only: false,
  created_at: "2026-07-09T00:30:00Z",
  updated_at: "2026-07-09T00:30:00Z",
};

const failedImageGenerationTask: AssetGenerationTask = {
  ...imageGenerationTask,
  status: "failed",
  result: { generated_count: 0, failed_count: 6 },
  error_message: "Image generation is not enabled for this group",
};

const videoDraftTask: AssetGenerationTask = {
  ...imageGenerationTask,
  task_id: "18181818-1818-4818-8818-181818181818",
  scene_id: scriptDetail.scenes[0].scene_id,
  task_type: "video_draft",
  status: "draft",
  candidate_count: 0,
  params: { requires_manual_confirmation: true },
  read_only: true,
};

const confirmedVideoTask: AssetGenerationTask = {
  ...videoDraftTask,
  task_type: "video_generation",
  status: "pending",
  result: { confirmed: true },
};

const selectedExistingCandidate: SceneAssetCandidate = {
  candidate_id: "19191919-1919-4919-8919-191919191919",
  project_id: project.project_id,
  script_id: scriptDetail.script_id,
  scene_id: scriptDetail.scenes[0].scene_id,
  material_id: subtitleMaterial.material_id,
  candidate_type: "image",
  source: "existing_material",
  status: "selected",
  rank: 0,
  generation_task_id: null,
  metadata: { role: "reference" },
  file_url: subtitleMaterial.file_url,
  thumbnail_url: subtitleMaterial.thumbnail_url,
  file_name: subtitleMaterial.file_name,
  created_at: "2026-07-09T00:31:00Z",
  updated_at: "2026-07-09T00:31:00Z",
};

const aiImageCandidate: SceneAssetCandidate = {
  ...selectedExistingCandidate,
  candidate_id: "20202020-2020-4020-8020-202020202020",
  material_id: "21212121-2121-4121-8121-212121212121",
  source: "ai_generated",
  status: "candidate",
  rank: 1,
  generation_task_id: imageGenerationTask.task_id,
  file_url: "http://api.test/assets/generated/images/task/scene-1.png",
  thumbnail_url: "http://api.test/assets/generated/images/task/scene-1.png",
  file_name: "scene-1.png",
};

const failedAiImageCandidate: SceneAssetCandidate = {
  ...aiImageCandidate,
  candidate_id: "22222222-2222-4222-8222-222222222223",
  material_id: null,
  status: "failed",
  rank: 2,
  file_url: null,
  thumbnail_url: null,
  file_name: null,
  metadata: { error_message: "人物漂移" },
};

const videoTaskCandidate: SceneAssetCandidate = {
  ...selectedExistingCandidate,
  candidate_id: "23232323-2323-4232-8232-232323232323",
  material_id: null,
  candidate_type: "video",
  source: "video_task",
  status: "candidate",
  rank: 10000,
  generation_task_id: videoDraftTask.task_id,
  file_url: null,
  thumbnail_url: null,
  file_name: null,
  metadata: { requires_manual_confirmation: true },
};

const completeSceneVisualManifest: SceneVisualManifest = {
  script_id: scriptDetail.script_id,
  script_title: scriptDetail.title,
  script_updated_at: scriptDetail.updated_at,
  input_version: "a".repeat(64),
  scenes: scriptDetail.scenes.map((scene, index) => ({
    scene_id: scene.scene_id,
    sequence: scene.sequence,
    narration: scene.narration,
    visual_description: scene.visual_description,
    emotion: scene.emotion,
    duration_sec: scene.duration_sec,
    candidate_id: index === 0 ? selectedExistingCandidate.candidate_id : `candidate-${index + 1}`,
    material_id: index === 0 ? selectedExistingCandidate.material_id as string : `material-${index + 1}`,
    file_url: `http://api.test/assets/generated/images/scene-${index + 1}.png`,
    thumbnail_url: null,
    source_snapshot: { candidate_source: "ai_generated" },
  })),
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
  updated_at: "2026-07-02T00:12:00Z",
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
    route_path: workspaceRoutePath(menuKey),
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

function workspaceRoutePath(menuKey: string) {
  return {
    "content-strategy": "/strategy",
    "account-strategy": "/strategy/account",
    "topic-history": "/strategy/topic-history",
    "topic-generator": "/strategy/topics",
    "script-creation": "/scripts",
    "script-generator": "/scripts/generator",
    "material-management": "/materials",
    "material-library": "/materials/library",
    "asset-generation": "/materials/generation",
    "sound-subtitle-generation": "/materials/sound-subtitle-generation",
    production: "/production",
    "work-generation": "/production/generation",
    "work-generation-task": "/production/tasks",
    "work-library": "/production/library",
    "publish-scheduler": "/publishing/workbench",
    publishing: "/publishing",
    analytics: "/analytics",
    "workflow-tasks": "/workflow-tasks",
  }[menuKey] ?? `/${menuKey}`;
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

function mockMaterials(materials: Material[] = []) {
  vi.mocked(api.listMaterials).mockResolvedValue({ materials });
}

function mockTopicBatches(response: TopicGenerationBatchListResponse = { batches: [] }) {
  vi.mocked(api.listTopicGenerationBatches).mockResolvedValue(response);
}

function mockTopicGroups(response: TopicGroupListResponse = { topic_groups: [] }) {
  vi.mocked(api.listTopicGroups).mockResolvedValue(response);
}

function mockTopicQualityEvaluation(response: TopicQualityEvaluation | null = null) {
  vi.mocked(api.getLatestTopicQualityEvaluation).mockResolvedValue(response);
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
    window.history.replaceState({}, "", "/");
    window.sessionStorage.clear();
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(workspaceMenus);
    vi.mocked(api.listModelOptions).mockImplementation(async (_client, modelType) => ({
      models: modelType === "text" ? [textModel] : modelType === "image" ? [imageModel] : [],
    }));
    mockProjects({ projects: [] });
    mockScripts({ scripts: [], total: 0, limit: 20, offset: 0 });
    mockTopics({ topics: [], stats: { total: 0, idea: 0, approved: 0, scripted: 0, archived: 0 } });
    mockTopicBatches();
    mockTopicGroups();
    vi.mocked(api.getScript).mockResolvedValue(scriptDetail);
    vi.mocked(api.generateScript).mockResolvedValue(topicGeneratedScript);
    vi.mocked(api.getAssetGenerationPlan).mockResolvedValue(assetGenerationPlan);
    vi.mocked(api.createAssetGenerationTasks).mockResolvedValue({
      script_id: scriptDetail.script_id,
      tasks: [imageGenerationTask],
    });
    vi.mocked(api.listAssetGenerationTasks).mockResolvedValue({
      script_id: scriptDetail.script_id,
      tasks: [imageGenerationTask, videoDraftTask],
    });
    vi.mocked(api.listAssetCandidates).mockResolvedValue({
      candidates: [selectedExistingCandidate, aiImageCandidate, failedAiImageCandidate, videoTaskCandidate],
    });
    vi.mocked(api.selectAssetCandidate).mockResolvedValue({
      ...aiImageCandidate,
      status: "selected",
    });
    vi.mocked(api.rejectAssetCandidate).mockResolvedValue({
      ...aiImageCandidate,
      status: "rejected",
    });
    vi.mocked(api.createSceneAssetGenerationTask).mockResolvedValue({
      ...imageGenerationTask,
      scene_id: scriptDetail.scenes[0].scene_id,
      candidate_count: 3,
    });
    vi.mocked(api.getSceneVisualManifest).mockRejectedValue(
      new api.ApiError(409, "主画面清单不完整", {
        code: "scene_visual_manifest_incomplete",
        blockers: [
          {
            scene_id: scriptDetail.scenes[1].scene_id,
            sequence: scriptDetail.scenes[1].sequence,
            reason: "selected_image_missing",
          },
        ],
      }),
    );
    vi.mocked(api.validateSceneVisualManifest).mockResolvedValue(completeSceneVisualManifest);
    vi.mocked(api.dismissAssetGenerationTask).mockResolvedValue({
      ...failedImageGenerationTask,
      dismissed_at: "2026-07-10T08:30:00Z",
    });
    vi.mocked(api.createContentTopic).mockResolvedValue(ideaTopic);
    vi.mocked(api.createMaterial).mockResolvedValue(subtitleMaterial);
    vi.mocked(api.deleteContentTopic).mockResolvedValue({
      topic_id: ideaTopic.topic_id,
      deleted_at: "2026-07-07T10:00:00Z",
    });
    vi.mocked(api.updateContentTopic).mockResolvedValue(ideaTopic);
    vi.mocked(api.getMaterial).mockResolvedValue(subtitleMaterial);
    vi.mocked(api.updateMaterial).mockResolvedValue(subtitleMaterial);
    vi.mocked(api.updateMaterialStatus).mockResolvedValue(archivedSubtitleMaterial);
    vi.mocked(api.updateContentTopicStatus).mockResolvedValue({ ...ideaTopic, status: "approved" });
    vi.mocked(api.prepareScriptFromTopic).mockResolvedValue(preparedTopic);
    vi.mocked(api.createAgentConversation).mockResolvedValue(conversation);
    vi.mocked(api.listAgentMessages).mockResolvedValue({ messages: [] });
    vi.mocked(api.sendAgentMessage).mockResolvedValue({
      user_message: userMessage,
      assistant_message: assistantMessage,
      run: agentRun,
    });
    vi.mocked(api.createTopicGroupReview).mockResolvedValue(topicReviewSnapshot);
    vi.mocked(api.getLatestTopicGroupReview).mockResolvedValue(null);
    mockTopicQualityEvaluation();
    vi.mocked(api.generateStrategyProfileDraft).mockResolvedValue({
      draft: {
        ...strategyProfile,
        target_audience: "AI 副业新手",
        topic_preferences: "优先教程、避坑和真实案例",
      },
      draft_summary: "草稿偏向 AI 工具教程、避坑和真实案例。",
    });
    vi.mocked(api.updateProjectStrategyProfile).mockResolvedValue(project);
    vi.mocked(api.listSoundTasks).mockResolvedValue({ tasks: [] });
    vi.mocked(api.listWorks).mockResolvedValue({ items: [], archived: false });
    mockMaterials();
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

    for (const label of ["选题 Agent", "脚本 Agent", "素材智能体", "视频智能体", "发布智能体", "优化智能体"]) {
      expect(within(menu).queryByText(label)).not.toBeInTheDocument();
    }

    expect(within(menu).getByRole("button", { name: /内容策略/ })).toHaveClass("active");
    expect(await screen.findByRole("heading", { name: "内容策略" })).toBeInTheDocument();
    expect(within(menu).getByRole("button", { name: /脚本创作/ })).toBeEnabled();
    expect(within(menu).getByRole("button", { name: /内容策略/ })).toBeEnabled();
    expect(within(menu).getByRole("button", { name: /素材管理/ })).toBeEnabled();
  });

  it("根据数据库 route_path 直达并恢复声音与字幕页面", async () => {
    window.history.replaceState({}, "", "/materials/sound-subtitle-generation");
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(materialWorkspaceMenus);
    mockProjects({ projects: [project] });

    render(createElement(Home));

    expect(await screen.findByRole("heading", { name: "声音与字幕生成" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /素材管理/ })).toHaveClass("active");
    expect(screen.getByRole("button", { name: "声音与字幕生成" })).toHaveClass("active");
    expect(window.location.pathname).toBe("/materials/sound-subtitle-generation");
  });

  it("根据数据库 route_path 直达作品库并使用当前账号查询", async () => {
    mockProjects({ projects: [project] });
    const menus: WorkspaceMenuListResponse = {
      menus: workspaceMenus.menus.map((menu) => menu.menu_key === "production" ? {
        ...menu,
        is_enabled: true,
        status: "active",
        children: [
          { ...menuNode("work-generation", "作品生成", true, "active", 10), menu_type: "page", module_key: "production.work-generation" },
          { ...menuNode("work-generation-task", "生成任务", true, "active", 20), menu_type: "page", module_key: "production.work-generation-task" },
          { ...menuNode("work-library", "作品库", true, "active", 30), menu_type: "page", module_key: "production.work-library" },
        ],
      } : menu),
    };
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(menus);
    window.history.replaceState({}, "", "/production/library");

    render(<Home />);

    expect(await screen.findByRole("heading", { name: "作品库" })).toBeInTheDocument();
    await waitFor(() => expect(api.listWorks).toHaveBeenCalledWith(expect.anything(), project.project_id, { archived: false, query: undefined }));
    expect(screen.getByRole("button", { name: "作品库" })).toHaveClass("active");
  });

  it("根据数据库 route_path 和 plan 参数直达发布工作台明确计划", async () => {
    mockProjects({ projects: [project] });
    const target = {
      id: "target-1",
      publication_plan_id: "plan-1",
      platform: "douyin" as const,
      status: "draft" as const,
      title: "抖音标题",
      body: "抖音正文",
      tags: [],
      cover_artifact_id: null,
      planned_at: null,
      draft_revision: 1,
      handed_off_at: null,
      published_at: null,
      published_url: null,
      result_snapshot: {},
      overdue: false,
      created_at: "2026-07-23T00:00:00Z",
      updated_at: "2026-07-23T00:00:00Z",
    };
    const plan = {
      id: "plan-1",
      handoff_id: "handoff-1",
      work_id: "work-1",
      work_version_id: "version-2",
      final_video_artifact_id: "video-1",
      subtitle_artifact_id: null,
      status: "draft" as const,
      targets: [target],
      created_at: "2026-07-23T00:00:00Z",
      updated_at: "2026-07-23T00:00:00Z",
    };
    const menus: WorkspaceMenuListResponse = {
      menus: workspaceMenus.menus.map((menu) => menu.menu_key === "publishing" ? {
        ...menu,
        is_enabled: true,
        status: "active",
        children: [{ ...menuNode("publish-scheduler", "发布工作台", true, "active", 10), menu_type: "page", module_key: "publishing.workbench" }],
      } : menu),
    };
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(menus);
    vi.mocked(api.listPublications).mockResolvedValue({ items: [{ ...plan, work_title: "夏日防晒指南" }] });
    vi.mocked(api.getPublication).mockResolvedValue(plan);
    vi.mocked(api.getWork).mockResolvedValue({
      id: "work-1", project_id: project.project_id, script_id: "script-1", title: "夏日防晒指南", status: "succeeded", archived: false, current_version_id: "version-2", versions: [], artifacts: [], timelines: [], generation_audit: [], created_at: "2026-07-23T00:00:00Z", updated_at: "2026-07-23T00:00:00Z",
    });
    window.history.replaceState({}, "", "/publishing/workbench?plan=plan-1");

    render(<Home />);

    expect(await screen.findByRole("heading", { name: "人工发布运营" })).toBeInTheDocument();
    expect(await screen.findByRole("heading", { name: "夏日防晒指南" })).toBeInTheDocument();
    expect(api.getPublication).toHaveBeenCalledWith(expect.anything(), "plan-1");
    expect(screen.getByRole("button", { name: /发布运营/ })).toHaveClass("active");
    expect(screen.getByRole("button", { name: "发布工作台" })).toHaveClass("active");
    expect(window.location.search).toBe("?plan=plan-1");
  });

  it("作品详情按 v3 分组展示业务摘要、Agent 对话并折叠技术快照与历史版本", async () => {
    mockProjects({ projects: [project] });
    const workId = "a1111111-1111-4111-8111-111111111111";
    const currentDraftId = "a2222222-2222-4222-8222-222222222222";
    const completedId = "a3333333-3333-4333-8333-333333333333";
    const modelId = "a4444444-4444-4444-8444-444444444444";
    const baseVersion = {
      work_id: workId,
      source_manifest_version: "manifest-v1",
      input_snapshot: { scenes: [{ sequence: 1, narration: "先识别问题" }] },
      model_snapshot: { video_model_id: modelId },
      parameter_snapshot: { aspect_ratio: "9:16", resolution: "1080p" },
      prompt_snapshot: { full_prompt: "用 Debug 方法拆解烦心事" },
      timeline_snapshot: { duration_seconds: 30, audio_mode: "independent_tts", burn_subtitles: true },
      created_at: "2026-07-20T00:00:00Z",
      updated_at: "2026-07-22T00:00:00Z",
      completed_at: null,
    };
    const versions: WorkVersion[] = [
      { ...baseVersion, id: currentDraftId, version_no: 11, status: "draft" as const, source_version_id: completedId, derivation_kind: "edit" },
      { ...baseVersion, id: "a5555555-5555-4555-8555-555555555555", version_no: 10, status: "failed" as const, source_version_id: null, derivation_kind: "initial" },
      { ...baseVersion, id: "a6666666-6666-4666-8666-666666666666", version_no: 9, status: "failed" as const, source_version_id: null, derivation_kind: "initial" },
      { ...baseVersion, id: completedId, version_no: 5, status: "completed" as const, source_version_id: null, derivation_kind: "initial", completed_at: "2026-07-20T00:05:00Z" },
      { ...baseVersion, id: "a7777777-7777-4777-8777-777777777777", version_no: 4, status: "draft" as const, source_version_id: null, derivation_kind: "initial" },
    ];
    vi.mocked(api.listWorks).mockResolvedValue({
      items: [{
        id: workId,
        project_id: project.project_id,
        script_id: scriptSummary.script_id,
        title: "别硬扛，用Debug解决烦心事",
        status: "draft",
        archived: false,
        current_version_id: currentDraftId,
        current_completed_version_id: completedId,
        current_completed_version_no: 5,
        aspect_ratio: "9:16",
        duration_seconds: 30,
        cover_artifact_id: null,
        cover_storage_path: null,
        created_at: "2026-07-20T00:00:00Z",
        updated_at: "2026-07-22T00:00:00Z",
      }],
      archived: false,
    });
    vi.mocked(api.getWork).mockResolvedValue({
      id: workId,
      project_id: project.project_id,
      script_id: scriptSummary.script_id,
      title: "别硬扛，用Debug解决烦心事",
      status: "draft",
      archived: false,
      current_version_id: currentDraftId,
      versions,
      artifacts: [],
      timelines: [],
      generation_audit: [
        { id: "run-failed-1", work_version_id: versions[1].id, status: "failed", current_stage: "video_segment", progress_percent: 40, error_category: "provider", error_summary: "上游视频生成失败", attempt_count: 2, created_at: "2026-07-21T00:00:00Z", updated_at: "2026-07-21T00:01:00Z" },
      ],
      model_catalog: { [modelId]: { display_name: "Seedance 2.0", model_type: "video" } },
      created_at: "2026-07-20T00:00:00Z",
      updated_at: "2026-07-22T00:00:00Z",
    });
    window.history.replaceState({}, "", "/production/library");
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue({
      menus: workspaceMenus.menus.map((menu) => menu.menu_key === "production" ? {
        ...menu,
        is_enabled: true,
        status: "active",
        children: [
          { ...menuNode("work-generation", "作品生成", true, "active", 10), menu_type: "page", module_key: "production.work-generation" },
          { ...menuNode("work-generation-task", "生成任务", true, "active", 20), menu_type: "page", module_key: "production.work-generation-task" },
          { ...menuNode("work-library", "作品库", true, "active", 30), menu_type: "page", module_key: "production.work-library" },
        ],
      } : menu),
    });

    render(<Home />);
    fireEvent.click(await screen.findByRole("button", { name: /别硬扛，用Debug解决烦心事.*查看详情/ }));

    const summary = await screen.findByRole("region", { name: "制作摘要" });
    expect(within(summary).getByText("Seedance 2.0")).toBeInTheDocument();
    expect(within(summary).getByText(/9:16 \/ 1080p/)).toBeInTheDocument();
    expect(within(summary).getByText("来自 V5")).toBeInTheDocument();
    expect(screen.getByText("暂无运行产物")).toBeInTheDocument();
    expect(document.querySelector(".workLibraryTimelineRuler")).toBeNull();

    const versionsPanel = screen.getByRole("region", { name: "版本记录" });
    expect(within(versionsPanel).getByRole("heading", { name: "当前草稿 · 1" })).toBeInTheDocument();
    expect(within(versionsPanel).getByRole("heading", { name: "可用成片 · 1" })).toBeInTheDocument();
    expect(within(versionsPanel).getByRole("button", { name: /V11.*草稿/ })).toBeInTheDocument();
    expect(within(versionsPanel).getByRole("button", { name: /V5.*已完成/ })).toBeInTheDocument();
    const historyToggle = within(versionsPanel).getByRole("button", { name: /失败与早期记录.*失败 2.*未运行草稿 1/ });
    expect(historyToggle).toHaveAttribute("aria-expanded", "false");
    expect(within(versionsPanel).queryByRole("button", { name: /V10.*失败/ })).not.toBeInTheDocument();
    expect(within(versionsPanel).getByRole("region", { name: "作品 Agent 对话" })).toBeInTheDocument();
    expect(screen.queryByLabelText("全局提示词")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "保存草稿修改" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "分析版本差异" })).not.toBeInTheDocument();

    expect(screen.queryByText(new RegExp(modelId))).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "展开技术快照" }));
    expect(screen.getByText(new RegExp(modelId))).toBeInTheDocument();
    fireEvent.click(historyToggle);
    expect(within(versionsPanel).getByRole("button", { name: /V10.*失败/ })).toBeInTheDocument();
    expect(within(versionsPanel).getByRole("button", { name: /V4.*草稿/ })).toBeInTheDocument();
  });

  it("失败声音任务展示完整脱敏诊断且保留多任务卡片内容", async () => {
    window.history.replaceState({}, "", "/materials/sound-subtitle-generation");
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(materialWorkspaceMenus);
    mockProjects({ projects: [project] });
    const failedTask: SoundTask = {
      task_id: "498ed8cd-601d-4664-b401-454f688c5ba7",
      project_id: project.project_id,
      parent_task_id: null,
      task_type: "tts",
      status: "failed",
      model_id: "aaaaaaaa-1111-4111-8111-aaaaaaaaaaaa",
      audio_inspection_id: null,
      source_audio_material_id: null,
      source_script_id: null,
      source_script_snapshot: null,
      output_audio_material_id: null,
      output_subtitle_material_id: null,
      text_content: "项目旁白完整配音",
      voice_type: "zh_female_cancan_mars_bigtts",
      language: "zh-cn",
      emotion: null,
      parameters: { audio_format: "mp3", sample_rate: 24000 },
      generate_subtitle: false,
      subtitle_segments: [],
      model_snapshot: {
        display_name: "豆包语音合成 2.0",
        upstream_model: "doubao-seed-tts-2.0",
        api_protocol: "volcengine_tts_v3",
      },
      voice_snapshot: { name: "灿灿" },
      resource_usage: { character_count: 8, task_count: 1 },
      timeline: null,
      result: null,
      request_id: "f1f273d6-82da-4101-a284-6c4b54b89910",
      upstream_log_id: "20260717150632A1B2C3D4E5F60789",
      attempt_count: 1,
      max_attempts: 2,
      error_code: "tts_http_error",
      error_summary: "语音供应商返回 HTTP 403",
      error_details: {
        http_status: 403,
        provider_error_code: "45000020",
        provider_error_message: "Permission denied",
      },
      staging_status: "none",
      cleanup_attempt_count: 0,
      cleanup_error_summary: null,
      started_at: "2026-07-17T07:06:28Z",
      completed_at: "2026-07-17T07:06:29Z",
      created_at: "2026-07-17T07:06:28Z",
      updated_at: "2026-07-17T07:06:29Z",
    };
    vi.mocked(api.listSoundTasks).mockResolvedValue({
      tasks: Array.from({ length: 8 }, (_, index) => ({
        ...failedTask,
        task_id: `498ed8cd-601d-4664-b401-454f688c5ba${index}`,
        text_content: `第 ${index + 1} 条项目旁白完整配音`,
      })),
    });

    render(createElement(Home));

    const detail = await screen.findByLabelText("当前失败任务详情");
    expect(within(detail).getByText("HTTP 403")).toBeInTheDocument();
    expect(within(detail).getByText(/Permission denied/)).toBeInTheDocument();
    expect(within(detail).getByText(/45000020/)).toBeInTheDocument();
    expect(within(detail).getByText(/f1f273d6-82da-4101-a284-6c4b54b89910/)).toBeInTheDocument();
    expect(within(detail).getByText(/20260717150632A1B2C3D4E5F60789/)).toBeInTheDocument();
    const taskPanel = screen.getByRole("complementary", { name: "配音任务列表" });
    expect(within(taskPanel).getAllByRole("article")).toHaveLength(8);
    expect(within(taskPanel).getByText("第 8 条项目旁白完整配音")).toBeInTheDocument();
  });

  it("菜单点击更新 URL 并响应浏览器前进后退事件", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(materialWorkspaceMenus);
    mockProjects({ projects: [project] });

    render(createElement(Home));
    fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));
    expect(window.location.pathname).toBe("/materials");

    fireEvent.click(screen.getByRole("button", { name: "声音与字幕生成" }));
    expect(window.location.pathname).toBe("/materials/sound-subtitle-generation");

    act(() => {
      window.history.replaceState({}, "", "/strategy/account");
      window.dispatchEvent(new PopStateEvent("popstate"));
    });
    expect(await screen.findByRole("heading", { name: "账号策略" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /内容策略/ })).toHaveClass("active");
    expect(screen.getByRole("button", { name: "账号策略" })).toHaveClass("active");
  });

  it("未知路径使用 replace 回退首个可用菜单", async () => {
    window.history.replaceState({}, "", "/unknown-workspace-route");
    const replaceState = vi.spyOn(window.history, "replaceState");

    render(createElement(Home));

    expect(await screen.findByRole("heading", { name: "内容策略" })).toBeInTheDocument();
    await waitFor(() => expect(window.location.pathname).toBe("/strategy"));
    expect(replaceState).toHaveBeenCalledWith(expect.anything(), "", "/strategy");
    replaceState.mockRestore();
  });

  it("从工作台菜单打开素材库画布空状态", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(materialWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockMaterials([]);

    render(createElement(Home));
    fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));

    expect(await screen.findByRole("heading", { name: "素材库" })).toBeInTheDocument();
    expect(screen.getByText("还没有素材")).toBeInTheDocument();
    expect(screen.getByLabelText("素材画布")).toBeInTheDocument();
    expect(screen.getByLabelText("素材资产浮层")).toBeInTheDocument();
    expect(screen.queryByLabelText("素材详情浮层")).not.toBeInTheDocument();
    expect(api.listMaterials).toHaveBeenCalledWith(expect.anything(), project.project_id, {
      material_type: "all",
      status: "active",
      q: "",
      tag: "",
      audio_usage: "all",
      source: "all",
      work_id: "",
      work_version_id: "",
    });
  });

  it("从素材管理打开声音与字幕双标签工作区", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(materialWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockMaterials([]);

    render(createElement(Home));
    fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));
    fireEvent.click(screen.getByRole("button", { name: "声音与字幕生成" }));

    expect(await screen.findByRole("heading", { name: "声音与字幕生成" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "TTS配音" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "字幕" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "声音与字幕生成" })).toHaveClass("active");
  });

  it("素材库按声音用途、生成来源、作品和版本组合筛选", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(materialWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockMaterials([generatedTtsMaterial]);

    render(createElement(Home));
    fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));
    await screen.findByRole("heading", { name: "素材库" });

    fireEvent.change(screen.getByLabelText("声音用途筛选"), { target: { value: "tts" } });
    fireEvent.change(screen.getByLabelText("生成来源筛选"), { target: { value: "work_generation" } });
    fireEvent.change(screen.getByLabelText("来源作品筛选"), {
      target: { value: generatedTtsMaterial.work_id },
    });
    fireEvent.change(screen.getByLabelText("来源版本筛选"), {
      target: { value: generatedTtsMaterial.work_version_id },
    });

    await waitFor(() => {
      expect(api.listMaterials).toHaveBeenLastCalledWith(expect.anything(), project.project_id, {
        material_type: "all",
        status: "active",
        q: "",
        tag: "",
        audio_usage: "tts",
        source: "work_generation",
        work_id: generatedTtsMaterial.work_id,
        work_version_id: generatedTtsMaterial.work_version_id,
      });
    });
  });

  it("素材库空状态点击上传后进入文件选择状态", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(materialWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockMaterials([]);

    render(createElement(Home));
    fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));
    fireEvent.click(await screen.findByRole("button", { name: "上传素材" }));

    expect(screen.getByRole("heading", { name: "上传素材" })).toBeInTheDocument();
    expect(screen.getByLabelText("素材文件")).toBeInTheDocument();
    expect(screen.queryByLabelText("素材 URL")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("缩略图 URL")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("来源备注")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("授权备注")).not.toBeInTheDocument();
  });

  it("素材库默认隐藏详情，选择字幕素材后打开并可关闭", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(materialWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockMaterials([subtitleMaterial]);

    render(createElement(Home));
    fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));

    expect(await screen.findByRole("heading", { name: "素材库" })).toBeInTheDocument();
    const materialButton = screen.getByRole("button", { name: /demo.vtt/ });
    expect(materialButton).toBeInTheDocument();
    expect(screen.getAllByText("字幕").length).toBeGreaterThanOrEqual(1);
    expect(screen.queryByLabelText("素材详情浮层")).not.toBeInTheDocument();

    fireEvent.click(materialButton);

    expect(screen.getByLabelText("素材详情浮层")).toBeInTheDocument();
    expect(screen.getByLabelText("素材名称")).toHaveValue(subtitleMaterial.file_name);
    expect(screen.getByLabelText("标签")).toHaveValue("字幕, 中英双语");
    expect(screen.getByText("字幕 · VTT")).toBeInTheDocument();
    expect(screen.queryByLabelText("素材 URL")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("缩略图 URL")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("来源备注")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("授权备注")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("时长")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("格式")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("宽度")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("高度")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "查看demo.vtt大图" })).not.toBeInTheDocument();
    expect(screen.getByLabelText("画布工具栏")).toBeInTheDocument();
    expect(screen.queryByText("Assets")).not.toBeInTheDocument();
    expect(screen.queryByText("Detail")).not.toBeInTheDocument();
    expect(screen.queryByText("语义检索")).not.toBeInTheDocument();
    expect(screen.queryByText("分镜候选")).not.toBeInTheDocument();
    expect(screen.queryByText("素材清单确认")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "关闭素材详情" }));
    expect(screen.queryByLabelText("素材详情浮层")).not.toBeInTheDocument();
  });

  it("素材库选择文件后自动填充名称并上传素材", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(materialWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockMaterials([]);

    vi.mocked(api.uploadMaterial).mockResolvedValue(uploadedImageMaterial);

    render(createElement(Home));
    fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));
    fireEvent.click(await screen.findByRole("button", { name: "上传素材" }));
    const file = new File(["png"], "办公桌面近景.png", { type: "image/png" });
    fireEvent.change(screen.getByLabelText("素材文件"), { target: { files: [file] } });

    expect(screen.getByLabelText("素材名称")).toHaveValue("办公桌面近景");
    fireEvent.change(screen.getByLabelText("标签（选填）"), { target: { value: "办公, 场景" } });
    fireEvent.click(screen.getByRole("button", { name: "上传并保存" }));

    await waitFor(() => {
      expect(api.uploadMaterial).toHaveBeenCalledWith(expect.anything(), project.project_id, {
        file,
        file_name: "办公桌面近景",
        tags: ["办公", "场景"],
      });
    });
    expect(
      await within(screen.getByLabelText("素材资产浮层")).findByRole("button", {
        name: /办公桌面近景/,
      }),
    ).toBeInTheDocument();
  });

  it("上传音频时可选择声音用途并提交标准值", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(materialWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockMaterials([]);
    vi.mocked(api.uploadMaterial).mockResolvedValue(uploadedAudioMaterial);

    render(createElement(Home));
    fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));
    fireEvent.click(await screen.findByRole("button", { name: "上传素材" }));
    const file = new File(["wav"], "城市清晨环境声.wav", { type: "audio/wav" });
    fireEvent.change(screen.getByLabelText("素材文件"), { target: { files: [file] } });
    fireEvent.change(screen.getByLabelText("声音用途（选填）"), { target: { value: "ambient" } });
    fireEvent.change(screen.getByLabelText("标签（选填）"), {
      target: { value: "城市, 清晨, 环境声" },
    });
    fireEvent.click(screen.getByRole("button", { name: "上传并保存" }));

    await waitFor(() => {
      expect(api.uploadMaterial).toHaveBeenCalledWith(expect.anything(), project.project_id, {
        file,
        file_name: "城市清晨环境声",
        tags: ["城市", "清晨", "环境声"],
        audio_usage: "ambient",
      });
    });
  });

  it("作品生成素材详情展示只读审计快照且不出现未落地生成入口", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(materialWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockMaterials([generatedTtsMaterial]);

    render(createElement(Home));
    fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));
    fireEvent.click(await screen.findByRole("button", { name: /Debug不内耗-V3-旁白/ }));

    const detail = screen.getByLabelText("素材详情浮层");
    expect(within(detail).getByRole("heading", { name: "生成来源" })).toBeInTheDocument();
    expect(within(detail).getByText("豆包语音 2.0")).toBeInTheDocument();
    expect(within(detail).getByText("灿灿")).toBeInTheDocument();
    expect(within(detail).getByText("req_7P2K8")).toBeInTheDocument();
    expect(within(detail).getByText("凭据未记录")).toBeInTheDocument();
    expect(screen.queryByText("AI 音乐")).not.toBeInTheDocument();
    expect(screen.queryByText("环境音生成")).not.toBeInTheDocument();
    expect(screen.queryByText("动作音效生成")).not.toBeInTheDocument();
  });

  it("视频素材详情使用原文件播放并显示缩略图海报", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(materialWorkspaceMenus);
    mockProjects({ projects: [project] });
    const generatedVideo: Material = {
      ...generatedTtsMaterial,
      material_id: "41414141-4141-4141-8141-414141414141",
      material_type: "video",
      file_name: "Debug解决烦心事 成片.mp4",
      file_url: "http://api.test/assets/generated/final.mp4",
      thumbnail_url: "http://api.test/assets/generated/final.jpg",
      audio_usage: null,
    };
    mockMaterials([generatedVideo]);

    render(createElement(Home));
    fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));
    fireEvent.click(await screen.findByRole("button", { name: /Debug解决烦心事 成片/ }));

    const player = screen.getByLabelText("Debug解决烦心事 成片.mp4 视频播放器");
    expect(player).toHaveAttribute("src", generatedVideo.file_url);
    expect(player).toHaveAttribute("poster", generatedVideo.thumbnail_url);
    expect(player).toHaveAttribute("controls");
    expect(screen.getByRole("button", { name: "播放视频" })).toBeInTheDocument();
  });

  it("图片详情可打开、缩放并关闭大图预览", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(materialWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockMaterials([uploadedImageMaterial]);

    render(createElement(Home));
    fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));
    fireEvent.click(await screen.findByRole("button", { name: /办公桌面近景/ }));
    const previewTrigger = screen.getByRole("button", { name: "查看办公桌面近景大图" });
    fireEvent.click(previewTrigger);

    expect(screen.getByRole("dialog", { name: "图片大图预览" })).toBeInTheDocument();
    expect(screen.getByText("100%")).toBeInTheDocument();
    const zoomIn = screen.getByRole("button", { name: "放大图片" });
    for (let index = 0; index < 4; index += 1) {
      fireEvent.click(zoomIn);
    }
    expect(screen.getByText("200%")).toBeInTheDocument();
    expect(zoomIn).toBeDisabled();
    const zoomOut = screen.getByRole("button", { name: "缩小图片" });
    for (let index = 0; index < 6; index += 1) {
      fireEvent.click(zoomOut);
    }
    expect(screen.getByText("50%")).toBeInTheDocument();
    expect(zoomOut).toBeDisabled();

    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "图片大图预览" })).not.toBeInTheDocument();
    await waitFor(() => expect(previewTrigger).toHaveFocus());

    fireEvent.click(previewTrigger);
    fireEvent.click(screen.getByRole("button", { name: "关闭大图预览" }));
    expect(screen.queryByRole("dialog", { name: "图片大图预览" })).not.toBeInTheDocument();
    await waitFor(() => expect(previewTrigger).toHaveFocus());

    fireEvent.click(previewTrigger);
    const dialog = screen.getByRole("dialog", { name: "图片大图预览" });
    fireEvent.mouseDown(dialog.parentElement as HTMLElement);
    expect(screen.queryByRole("dialog", { name: "图片大图预览" })).not.toBeInTheDocument();
    await waitFor(() => expect(previewTrigger).toHaveFocus());
  });

  it("素材库可归档、查看归档并恢复素材", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(materialWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockMaterials([subtitleMaterial]);
    vi.mocked(api.updateMaterialStatus)
      .mockResolvedValueOnce(archivedSubtitleMaterial)
      .mockResolvedValueOnce(subtitleMaterial);

    render(createElement(Home));
    fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));
    fireEvent.click(await screen.findByRole("button", { name: /demo.vtt/ }));
    fireEvent.click(screen.getByRole("button", { name: "归档素材" }));

    await waitFor(() => {
      expect(api.updateMaterialStatus).toHaveBeenCalledWith(
        expect.anything(),
        subtitleMaterial.material_id,
        "archived",
      );
    });
    expect(screen.getByText("还没有素材")).toBeInTheDocument();

    vi.mocked(api.listMaterials).mockResolvedValueOnce({ materials: [archivedSubtitleMaterial] });
    fireEvent.click(screen.getByRole("button", { name: "已归档" }));
    expect(await screen.findByRole("button", { name: /demo.vtt/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /demo.vtt/ }));
    fireEvent.click(screen.getByRole("button", { name: "恢复素材" }));

    await waitFor(() => {
      expect(api.updateMaterialStatus).toHaveBeenLastCalledWith(
        expect.anything(),
        subtitleMaterial.material_id,
        "active",
      );
    });
  });

  it("菜单加载失败时不回退旧智能体菜单", async () => {
    vi.mocked(api.listWorkspaceMenus).mockRejectedValue(new Error("菜单接口不可用"));
    render(createElement(Home));

    expect(await screen.findByText("菜单接口不可用")).toBeInTheDocument();
    const menu = screen.getByLabelText("视频工作台菜单");
    expect(within(menu).queryByText("脚本 Agent")).not.toBeInTheDocument();
  });

  it("不在脚本生产工作台展示项目创建或项目管理入口", async () => {
    render(createElement(Home));
    await openScriptCreationWorkspace();

    expect(await screen.findByRole("heading", { name: "脚本 Agent 对话" })).toBeInTheDocument();
    expect(screen.getByLabelText("当前账号")).toBeInTheDocument();
    expect(screen.queryByLabelText("当前项目")).not.toBeInTheDocument();
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

  it("脚本生成页不展示素材候选入口", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [scriptSummary], total: 1, limit: 20, offset: 0 });
    render(createElement(Home));
    await openScriptCreationWorkspace();

    expect(await screen.findByRole("heading", { name: scriptSummary.title })).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "画面生成图片候选" })).not.toBeInTheDocument();
    expect(api.getAssetGenerationPlan).not.toHaveBeenCalled();
    expect(api.listAssetCandidates).not.toHaveBeenCalled();
    expect(api.listAssetGenerationTasks).not.toHaveBeenCalled();
  });

  it("画面生成页的已选 AI 主画面保留在候选区并可打开大图", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [scriptSummary], total: 1, limit: 20, offset: 0 });
    const selectedAiImageCandidate: SceneAssetCandidate = {
      ...aiImageCandidate,
      candidate_id: "21212121-2121-4121-8121-212121212122",
      status: "selected",
      rank: 0,
      file_name: "selected-scene-1.png",
      file_url: "http://api.test/assets/generated/images/task/selected-scene-1.png",
      thumbnail_url: "http://api.test/assets/generated/images/task/selected-scene-1-thumb.png",
    };
    vi.mocked(api.listAssetCandidates).mockResolvedValue({
      candidates: [
        selectedAiImageCandidate,
        aiImageCandidate,
        {
          ...failedAiImageCandidate,
          file_name: "failed-with-stale-url.png",
          file_url: "http://api.test/assets/generated/images/task/failed-with-stale-url.png",
        },
        videoTaskCandidate,
      ],
    });
    render(createElement(Home));
    fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));
    const materialSubMenu = screen.getByLabelText("素材管理二级菜单");
    expect(within(materialSubMenu).getAllByRole("button").map((button) => button.textContent))
      .toEqual(["素材库", "画面生成", "声音与字幕生成"]);
    fireEvent.click(await screen.findByRole("button", { name: "画面生成" }));

    const panel = await screen.findByRole("region", { name: "画面生成图片候选" });
    const previewTriggers = within(panel).getAllByRole("button", { name: /^查看.*大图$/ });
    expect(previewTriggers).toHaveLength(2);
    expect(previewTriggers[0]).toHaveAccessibleName("查看selected-scene-1.png大图");
    expect(previewTriggers[1]).toHaveAccessibleName("查看scene-1.png大图");
    expect(within(panel).queryByRole("heading", { name: "当前主素材" })).not.toBeInTheDocument();
    const currentCandidateSummary = panel.querySelector(".assetCurrentCandidateSummary");
    expect(currentCandidateSummary).toHaveTextContent("当前主素材");
    expect(currentCandidateSummary).toHaveTextContent("selected-scene-1.png");
    const aiCandidateSection = within(panel).getByRole("region", { name: "AI 图片候选" });
    const selectedCandidateNames = within(aiCandidateSection).getAllByText("selected-scene-1.png");
    expect(selectedCandidateNames).toHaveLength(1);
    expect(selectedCandidateNames[0].closest("article")).toHaveClass("selected");
    expect(
      within(panel).queryByRole("button", { name: "查看failed-with-stale-url.png大图" }),
    ).not.toBeInTheDocument();

    fireEvent.click(previewTriggers[0]);
    const dialog = screen.getByRole("dialog", { name: "图片大图预览" });
    expect(within(dialog).getByText("selected-scene-1.png")).toBeInTheDocument();
    expect(within(dialog).getByText("AI 生成图片候选")).toBeInTheDocument();
    expect(within(dialog).getByRole("img", { name: "selected-scene-1.png" })).toHaveAttribute(
      "src",
      selectedAiImageCandidate.file_url,
    );
    expect(api.selectAssetCandidate).not.toHaveBeenCalled();
    expect(api.rejectAssetCandidate).not.toHaveBeenCalled();

    const zoomIn = within(dialog).getByRole("button", { name: "放大图片" });
    for (let index = 0; index < 4; index += 1) {
      fireEvent.click(zoomIn);
    }
    expect(within(dialog).getByText("200%")).toBeInTheDocument();
    expect(zoomIn).toBeDisabled();

    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "图片大图预览" })).not.toBeInTheDocument();
    await waitFor(() => expect(previewTriggers[0]).toHaveFocus());

    fireEvent.click(previewTriggers[0]);
    fireEvent.click(screen.getByRole("button", { name: "关闭大图预览" }));
    expect(screen.queryByRole("dialog", { name: "图片大图预览" })).not.toBeInTheDocument();
    await waitFor(() => expect(previewTriggers[0]).toHaveFocus());

    fireEvent.click(previewTriggers[0]);
    const reopenedDialog = screen.getByRole("dialog", { name: "图片大图预览" });
    fireEvent.mouseDown(reopenedDialog.parentElement as HTMLElement);
    expect(screen.queryByRole("dialog", { name: "图片大图预览" })).not.toBeInTheDocument();
    await waitFor(() => expect(previewTriggers[0]).toHaveFocus());
  });

  it("画面生成页展示图片候选三栏并触发生成、选择、排除和重生", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [scriptSummary], total: 1, limit: 20, offset: 0 });
    render(createElement(Home));
    fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));
    fireEvent.click(await screen.findByRole("button", { name: "画面生成" }));

    expect(await screen.findByRole("heading", { name: "画面生成" })).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "脚本列表" })).not.toBeInTheDocument();
    expect(screen.getByLabelText("当前脚本")).toHaveValue(scriptSummary.script_id);
    const panel = await screen.findByRole("region", { name: "画面生成图片候选" });
    expect(
      within(panel).getByRole("region", { name: "AI 图片候选" })
        .querySelector(".assetCandidateCards"),
    ).toHaveClass("triple");
    expect(within(panel).queryByRole("button", { name: "生成图片候选" })).not.toBeInTheDocument();
    const generateCandidatesButton = screen.getByRole("button", { name: "生成图片候选" });

    expect(within(panel).getByText("分镜列表")).toBeInTheDocument();
    expect(within(panel).getByText("候选素材")).toBeInTheDocument();
    expect(within(panel).getByText("生成设置与任务")).toBeInTheDocument();
    expect(within(panel).getByRole("combobox", { name: "图片模型" })).toHaveValue(imageModel.model_id);
    expect(within(panel).getByText("2 分镜 × 3 = 6 张图片候选")).toBeInTheDocument();
    expect(within(panel).getByText("单次最多 48 张")).toBeInTheDocument();
    expect(within(panel).getByText("当前主素材")).toBeInTheDocument();
    expect(within(panel).queryByRole("heading", { name: "当前主素材" })).not.toBeInTheDocument();
    expect(within(panel).getByText("AI 图片候选")).toBeInTheDocument();
    expect(within(panel).getByText("历史逐分镜视频任务")).toBeInTheDocument();
    expect(within(panel).getByText("只读审计")).toBeInTheDocument();
    expect(within(panel).queryByRole("button", { name: "确认生成视频" })).not.toBeInTheDocument();
    const workEntry = within(panel).getByRole("button", { name: "进入作品生成" });
    expect(workEntry).toBeDisabled();
    expect(within(panel).getByText("还缺 1 个主画面")).toBeInTheDocument();
    const sceneRail = within(panel).getByRole("region", { name: "分镜列表" });
    const candidateBrowser = within(panel).getByRole("region", { name: "候选素材" });
    expect(within(candidateBrowser).getByText("旁白")).toBeInTheDocument();
    expect(within(candidateBrowser).getByText("画面")).toBeInTheDocument();
    expect(within(candidateBrowser).getByText(scriptDetail.scenes[0].narration)).toBeInTheDocument();
    expect(
      within(candidateBrowser).getByText(scriptDetail.scenes[0].visual_description),
    ).toBeInTheDocument();
    expect(within(sceneRail).queryByText(scriptDetail.scenes[0].narration)).not.toBeInTheDocument();
    expect(
      within(sceneRail).queryByText(scriptDetail.scenes[0].visual_description),
    ).not.toBeInTheDocument();

    fireEvent.click(generateCandidatesButton);
    await waitFor(() => {
      expect(api.createAssetGenerationTasks).toHaveBeenCalledWith(expect.anything(), scriptSummary.script_id, {
        model_id: imageModel.model_id,
        image_candidates_per_scene: 3,
        use_reference_materials: true,
      });
    });

    fireEvent.click(within(panel).getByRole("button", { name: "选择为主素材" }));
    await waitFor(() => {
      expect(api.selectAssetCandidate).toHaveBeenCalledWith(
        expect.anything(),
        scriptDetail.scenes[0].scene_id,
        aiImageCandidate.candidate_id,
      );
    });

    fireEvent.click(within(panel).getByRole("button", { name: "排除候选" }));
    await waitFor(() => {
      expect(api.rejectAssetCandidate).toHaveBeenCalledWith(
        expect.anything(),
        scriptDetail.scenes[0].scene_id,
        aiImageCandidate.candidate_id,
      );
    });

    fireEvent.click(within(panel).getByRole("button", { name: "单镜头重生" }));
    await waitFor(() => {
      expect(api.createSceneAssetGenerationTask).toHaveBeenCalledWith(
        expect.anything(),
        scriptDetail.scenes[0].scene_id,
        {
          model_id: imageModel.model_id,
          image_candidates_per_scene: 3,
          use_reference_materials: true,
        },
        expect.stringMatching(
          /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i,
        ),
      );
    });

    expect(api.getSceneVisualManifest).toHaveBeenCalledWith(
      expect.anything(),
      scriptSummary.script_id,
    );
  });

  it("主画面齐备且作品生成菜单启用时校验版本并传递 Manifest", async () => {
    const workGenerationMenus: WorkspaceMenuListResponse = {
      menus: workspaceMenus.menus.map((menu) =>
        menu.menu_key === "production"
          ? {
              ...menu,
              is_enabled: true,
              status: "active",
              children: [
                {
                  ...menuNode("work-generation", "作品生成", true, "active", 10),
                  menu_type: "page",
                  module_key: "production.work-generation",
                },
              ],
            }
          : menu,
      ),
    };
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(workGenerationMenus);
    vi.mocked(api.getSceneVisualManifest).mockResolvedValue(completeSceneVisualManifest);
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [scriptSummary], total: 1, limit: 20, offset: 0 });

    render(createElement(Home));
    fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));
    fireEvent.click(await screen.findByRole("button", { name: "画面生成" }));
    const panel = await screen.findByRole("region", { name: "画面生成图片候选" });
    const entry = within(panel).getByRole("button", { name: "进入作品生成" });

    await waitFor(() => expect(entry).toBeEnabled());
    fireEvent.click(entry);

    await waitFor(() => {
      expect(api.validateSceneVisualManifest).toHaveBeenCalledWith(
        expect.anything(),
        scriptSummary.script_id,
        completeSceneVisualManifest.input_version,
      );
    });
    expect(JSON.parse(window.sessionStorage.getItem("scene-visual-manifest-handoff") || "{}"))
      .toEqual({
        script_id: scriptSummary.script_id,
        input_version: completeSceneVisualManifest.input_version,
      });
    expect(api.createAssetGenerationTasks).not.toHaveBeenCalled();
  });

  it("镜头内容为空时在候选区保留旁白和画面双栏", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [scriptSummary], total: 1, limit: 20, offset: 0 });
    vi.mocked(api.getScript).mockResolvedValue({
      ...scriptDetail,
      scenes: [
        {
          ...scriptDetail.scenes[0],
          narration: "",
          visual_description: "",
        },
      ],
    });

    render(createElement(Home));
    fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));
    fireEvent.click(await screen.findByRole("button", { name: "画面生成" }));
    const panel = await screen.findByRole("region", { name: "画面生成图片候选" });
    const candidateBrowser = within(panel).getByRole("region", { name: "候选素材" });

    expect(within(candidateBrowser).getByText("未填写旁白")).toBeInTheDocument();
    expect(within(candidateBrowser).getByText("未填写画面")).toBeInTheDocument();
  });

  it("失败任务二次确认清理并同步刷新任务和失败候选", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [scriptSummary], total: 1, limit: 20, offset: 0 });
    vi.mocked(api.listAssetGenerationTasks)
      .mockResolvedValueOnce({
        script_id: scriptDetail.script_id,
        tasks: [failedImageGenerationTask],
      })
      .mockResolvedValue({ script_id: scriptDetail.script_id, tasks: [] });
    vi.mocked(api.listAssetCandidates)
      .mockResolvedValueOnce({ candidates: [failedAiImageCandidate] })
      .mockResolvedValue({ candidates: [] });
    let resolveDismiss!: (task: AssetGenerationTask) => void;
    vi.mocked(api.dismissAssetGenerationTask).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveDismiss = resolve;
        }),
    );

    render(createElement(Home));
    fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));
    fireEvent.click(await screen.findByRole("button", { name: "画面生成" }));
    const panel = await screen.findByRole("region", { name: "画面生成图片候选" });

    fireEvent.click(await within(panel).findByRole("button", { name: "清理失败任务" }));
    const dialog = screen.getByRole("dialog", { name: "清理失败任务？" });
    expect(within(dialog).getByText(/任务及其失败候选将从画面生成页面隐藏/)).toBeInTheDocument();
    expect(within(dialog).getByText(/不会重新调用供应商，也不会产生额外费用/)).toBeInTheDocument();
    expect(within(dialog).getByText(/数据库继续保留任务状态、错误、候选数量和费用审计/)).toBeInTheDocument();
    const confirmButton = within(dialog).getByRole("button", { name: "确认清理" });

    act(() => {
      confirmButton.click();
      confirmButton.click();
    });
    expect(api.dismissAssetGenerationTask).toHaveBeenCalledTimes(1);
    expect(api.dismissAssetGenerationTask).toHaveBeenCalledWith(
      expect.anything(),
      failedImageGenerationTask.task_id,
    );

    await act(async () => {
      resolveDismiss({
        ...failedImageGenerationTask,
        dismissed_at: "2026-07-10T08:30:00Z",
      });
    });
    await waitFor(() => {
      expect(api.listAssetGenerationTasks).toHaveBeenCalledTimes(2);
      expect(api.listAssetCandidates).toHaveBeenCalledTimes(2);
    });
    expect(screen.queryByRole("dialog", { name: "清理失败任务？" })).not.toBeInTheDocument();
    expect(within(panel).queryByText(failedImageGenerationTask.error_message || "")).not.toBeInTheDocument();
  });

  it("单镜头重生同步连点只提交一次可计费请求", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [scriptSummary], total: 1, limit: 20, offset: 0 });
    let resolveRegeneration!: (task: AssetGenerationTask) => void;
    vi.mocked(api.createSceneAssetGenerationTask).mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveRegeneration = resolve;
        }),
    );

    render(createElement(Home));
    fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));
    fireEvent.click(await screen.findByRole("button", { name: "画面生成" }));
    const panel = await screen.findByRole("region", { name: "画面生成图片候选" });
    const regenerateButton = within(panel).getByRole("button", { name: "单镜头重生" });

    act(() => {
      regenerateButton.click();
      regenerateButton.click();
    });

    expect(api.createSceneAssetGenerationTask).toHaveBeenCalledTimes(1);
    expect(api.createSceneAssetGenerationTask).toHaveBeenCalledWith(
      expect.anything(),
      scriptDetail.scenes[0].scene_id,
      {
        model_id: imageModel.model_id,
        image_candidates_per_scene: 3,
        use_reference_materials: true,
      },
      expect.stringMatching(
        /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i,
      ),
    );

    await act(async () => {
      resolveRegeneration({
        ...imageGenerationTask,
        scene_id: scriptDetail.scenes[0].scene_id,
        candidate_count: 3,
      });
    });
  });

  it("单镜头重生请求失败后人工重试复用原幂等键", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [scriptSummary], total: 1, limit: 20, offset: 0 });
    vi.mocked(api.createSceneAssetGenerationTask)
      .mockRejectedValueOnce(new Error("响应丢失"))
      .mockResolvedValueOnce({
        ...imageGenerationTask,
        scene_id: scriptDetail.scenes[0].scene_id,
        candidate_count: 3,
      });

    render(createElement(Home));
    fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));
    fireEvent.click(await screen.findByRole("button", { name: "画面生成" }));
    const panel = await screen.findByRole("region", { name: "画面生成图片候选" });
    const regenerateButton = within(panel).getByRole("button", { name: "单镜头重生" });

    fireEvent.click(regenerateButton);
    expect(await within(panel).findByText("响应丢失")).toBeInTheDocument();
    fireEvent.click(regenerateButton);

    await waitFor(() => {
      expect(api.createSceneAssetGenerationTask).toHaveBeenCalledTimes(2);
    });
    const firstKey = vi.mocked(api.createSceneAssetGenerationTask).mock.calls[0][3];
    const retryKey = vi.mocked(api.createSceneAssetGenerationTask).mock.calls[1][3];
    expect(retryKey).toBe(firstKey);
  });

  it("重新打开脚本详情时恢复真实画面生成任务状态", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [scriptSummary], total: 1, limit: 20, offset: 0 });
    vi.mocked(api.listAssetGenerationTasks).mockResolvedValue({
      script_id: scriptDetail.script_id,
      tasks: [imageGenerationTask, confirmedVideoTask],
    });

    render(createElement(Home));
    fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));
    fireEvent.click(await screen.findByRole("button", { name: "画面生成" }));

    const panel = await screen.findByRole("region", { name: "画面生成图片候选" });

    await waitFor(() => {
      expect(api.listAssetGenerationTasks).toHaveBeenCalledWith(expect.anything(), scriptSummary.script_id);
    });
    const imageTaskRegion = within(panel).getByRole("region", { name: "AI 图片生成任务" });
    expect(within(imageTaskRegion).getByText("整批图片候选")).toBeInTheDocument();
    expect(within(imageTaskRegion).getByText("6 张")).toBeInTheDocument();
    expect(within(imageTaskRegion).getByText("排队中")).toBeInTheDocument();
    expect(within(panel).queryByRole("button", { name: "确认生成视频" })).not.toBeInTheDocument();
  });

  it("图片任务在途时轮询并在完成后展示新候选", async () => {
    mockProjects({ projects: [project] });
    mockScripts({ scripts: [scriptSummary], total: 1, limit: 20, offset: 0 });
    const completedImageTask: AssetGenerationTask = {
      ...imageGenerationTask,
      status: "completed",
      result: { generated_count: 6, failed_count: 0, partial: false },
      updated_at: "2026-07-09T00:35:00Z",
    };
    vi.mocked(api.listAssetGenerationTasks)
      .mockResolvedValueOnce({
        script_id: scriptDetail.script_id,
        tasks: [imageGenerationTask, videoDraftTask],
      })
      .mockResolvedValue({
        script_id: scriptDetail.script_id,
        tasks: [completedImageTask, videoDraftTask],
      });
    vi.mocked(api.listAssetCandidates)
      .mockResolvedValueOnce({ candidates: [] })
      .mockResolvedValue({ candidates: [aiImageCandidate] });

    let poll: (() => Promise<void>) | null = null;
    const setIntervalSpy = vi.spyOn(window, "setInterval").mockImplementation((handler, timeout) => {
      if (timeout === 3000) {
        poll = handler as () => Promise<void>;
      }
      return 1 as unknown as ReturnType<typeof setInterval>;
    });
    const clearIntervalSpy = vi.spyOn(window, "clearInterval").mockImplementation(() => undefined);

    try {
      render(createElement(Home));
      fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));
      fireEvent.click(await screen.findByRole("button", { name: "画面生成" }));
      const panel = await screen.findByRole("region", { name: "画面生成图片候选" });

      await waitFor(() => {
        expect(setIntervalSpy).toHaveBeenCalled();
      });
      expect(poll).not.toBeNull();
      await act(async () => {
        await poll?.();
      });
      expect(api.listAssetGenerationTasks).toHaveBeenCalledTimes(2);
      expect(api.listAssetCandidates).toHaveBeenCalledTimes(2);

      const imageTaskRegion = within(panel).getByRole("region", { name: "AI 图片生成任务" });
      expect(await within(imageTaskRegion).findByText("已完成")).toBeInTheDocument();
      expect(within(imageTaskRegion).getByText("已生成 6 张")).toBeInTheDocument();
      expect(within(panel).getByAltText(aiImageCandidate.file_name || "素材候选预览")).toBeInTheDocument();
      await waitFor(() => {
        expect(clearIntervalSpy).toHaveBeenCalled();
      });
    } finally {
      setIntervalSpy.mockRestore();
      clearIntervalSpy.mockRestore();
    }
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
      model_id: textModel.model_id,
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
      model_id: textModel.model_id,
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
      model_id: textModel.model_id,
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
    expect(styles).toContain(".accountStrategyHero");
    expect(styles).toContain(".accountStrategyCard");
    expect(styles).toContain(".accountStrategyBodyGrid");
    expect(styles).toContain(".strategyBasicsPanel");
    expect(styles).toContain(".strategyStructuredPanel");
    expect(styles).toContain(".strategyContextPanel");
    expect(styles).toContain(".accountDraftPanel");
    expect(styles).toMatch(/\.accountDraftButton\s*{[^}]*width:\s*132px/s);
    expect(styles).not.toMatch(/\.accountStrategyPage\s*{[^}]*max-width:\s*1168px/s);
    expect(styles).toMatch(/\.accountStrategyBodyGrid\s*{[^}]*grid-template-columns:\s*minmax\(480px,\s*1fr\)\s+minmax\(480px,\s*1fr\)/s);
    expect(styles).toMatch(/\.accountStrategyBodyGrid\s*{[^}]*grid-template-rows:\s*210px\s+120px\s+126px/s);
    expect(styles).toMatch(
      /\.accountStrategyBodyGrid\s*{[^}]*grid-template-areas:\s*"basics structured"\s*"context structured"\s*"draft structured"/s,
    );
    expect(styles).toMatch(/\.strategyStructuredPanel\s*{[^}]*align-self:\s*stretch/s);
    expect(styles).not.toMatch(/\.strategyStructuredPanel\s*{[^}]*margin-top:/s);
    expect(styles).not.toMatch(/\.strategyStructuredPanel\s*{[^}]*height:\s*360px/s);
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

  it("内容策略二级菜单展示账号策略、历史生成和当前选题池", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockTopics(topicListResponse);
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));

    const workspaceMenu = screen.getByRole("navigation", { name: "视频工作台菜单" });
    const menuButtons = within(workspaceMenu).getAllByRole("button");
    const accountButton = within(workspaceMenu).getByRole("button", { name: "账号策略" });
    const historyButton = within(workspaceMenu).getByRole("button", { name: "历史生成" });
    const currentPoolButton = within(workspaceMenu).getByRole("button", { name: "当前选题池" });
    expect(menuButtons.indexOf(accountButton)).toBeLessThan(menuButtons.indexOf(historyButton));
    expect(menuButtons.indexOf(historyButton)).toBeLessThan(menuButtons.indexOf(currentPoolButton));
    expect(accountButton).toHaveClass("agentSubItem");
    expect(historyButton).toHaveClass("agentSubItem");
    expect(currentPoolButton).toHaveClass("active");
    expect(screen.queryByLabelText("内容策略视图菜单")).not.toBeInTheDocument();
    expect(screen.getByRole("region", { name: "选题池" })).toBeInTheDocument();
  });

  it("账号策略独立页面按原型展示标题区、资料卡、AI 草稿和编辑控件", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockTopics(topicListResponse);
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "账号策略" }));
    const accountPage = await screen.findByRole("region", { name: "账号策略资料" });

    expect(document.querySelector(".accountStrategyHero")).not.toBeNull();
    expect(document.querySelector(".accountStrategyCard")).not.toBeNull();
    expect(within(accountPage).getByText("内容策略 / 账号策略")).toBeInTheDocument();
    expect(within(accountPage).getByRole("heading", { name: "账号策略" })).toBeInTheDocument();
    expect(within(accountPage).getByText("content-strategy")).toBeInTheDocument();
    expect(within(accountPage).getAllByText("账号策略")[0]).toBeInTheDocument();
    expect(within(accountPage).getByRole("button", { name: "返回当前选题池" })).toBeInTheDocument();
    expect(within(accountPage).queryByRole("button", { name: "AI 生成草稿" })).not.toBeInTheDocument();
    expect(within(accountPage).getByText("账号策略资料")).toBeInTheDocument();
    expect(within(accountPage).getByText(/第一版只维护内容账号策略资料/)).toBeInTheDocument();
    expect(within(accountPage).getByText(/AI 草稿只预填下方表单/)).toBeInTheDocument();

    const basics = within(accountPage).getByRole("region", { name: "基础资料" });
    expect(within(basics).getByText(/账号名称：科技博主/)).toBeInTheDocument();
    expect(within(basics).getByText(/定位摘要：科技知识账号/)).toBeInTheDocument();
    expect(within(basics).getByText(/描述：面向程序员的知识短视频/)).toBeInTheDocument();
    const structured = within(accountPage).getByRole("region", { name: "结构化策略" });
    expect(within(structured).getByText(/目标受众：内容运营负责人/)).toBeInTheDocument();
    expect(within(structured).getByText(/内容支柱：AI 工具 \/ 内容生产/)).toBeInTheDocument();
    expect(structured.querySelector(".strategyValueTags")).not.toBeInTheDocument();
    expect(within(accountPage).getByRole("region", { name: "保存后应用到选题链路" })).toBeInTheDocument();
    const draftPanel = within(accountPage).getByRole("region", { name: "AI 生成策略草稿" });
    expect(within(draftPanel).getByRole("button", { name: "生成草稿" })).toHaveClass("accountDraftButton");
    expect(within(draftPanel).getByText(/手动触发/)).toBeInTheDocument();
    expect(within(accountPage).getByLabelText("账号名称")).toHaveValue("科技博主");
    const targetAudienceField = within(accountPage).getByLabelText("目标受众");
    expect(targetAudienceField.tagName).toBe("TEXTAREA");
    expect(targetAudienceField).toHaveValue("内容运营负责人");
    expect(within(accountPage).getByRole("button", { name: "取消" })).toBeInTheDocument();
    expect(within(accountPage).getByRole("button", { name: "保存并应用" })).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "选题池" })).not.toBeInTheDocument();

    fireEvent.click(within(accountPage).getByRole("button", { name: "返回当前选题池" }));
    expect(await screen.findByRole("region", { name: "选题池" })).toBeInTheDocument();
  });

  it("账号策略页未修改时禁用取消，手工修改后取消恢复正式资料", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "账号策略" }));
    const accountPage = await screen.findByRole("region", { name: "账号策略资料" });
    const cancelButton = within(accountPage).getByRole("button", { name: "取消" });

    expect(cancelButton).toBeDisabled();

    fireEvent.change(within(accountPage).getByLabelText("目标受众"), {
      target: { value: "临时受众" },
    });

    expect(cancelButton).toBeEnabled();
    fireEvent.click(cancelButton);

    expect(within(accountPage).getByLabelText("目标受众")).toHaveValue("内容运营负责人");
    expect(cancelButton).toBeDisabled();
    expect(api.updateProjectStrategyProfile).not.toHaveBeenCalled();
  });

  it("账号策略页取消 AI 草稿时清空草稿内容并恢复正式资料", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "账号策略" }));
    const accountPage = await screen.findByRole("region", { name: "账号策略资料" });
    const cancelButton = within(accountPage).getByRole("button", { name: "取消" });

    fireEvent.change(within(accountPage).getByLabelText("AI 草稿补充方向"), {
      target: { value: "面向 AI 副业新手，强调避坑。" },
    });
    expect(cancelButton).toBeEnabled();
    fireEvent.click(within(accountPage).getByRole("button", { name: "生成草稿" }));

    expect(await within(accountPage).findByDisplayValue("AI 副业新手")).toBeInTheDocument();
    expect(within(accountPage).getByText(/草稿偏向 AI 工具教程、避坑和真实案例/)).toBeInTheDocument();

    fireEvent.click(cancelButton);

    expect(within(accountPage).getByLabelText("目标受众")).toHaveValue("内容运营负责人");
    expect(within(accountPage).getByLabelText("AI 草稿补充方向")).toHaveValue("");
    expect(within(accountPage).queryByText(/草稿偏向 AI 工具教程、避坑和真实案例/)).not.toBeInTheDocument();
    expect(within(accountPage).getByText(/草稿摘要：保存前不会修改正式账号资料/)).toBeInTheDocument();
    expect(cancelButton).toBeDisabled();
    expect(api.updateProjectStrategyProfile).not.toHaveBeenCalled();
  });

  it("账号策略页 AI 草稿只预填表单，保存前不更新当前账号资料", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "账号策略" }));
    const accountPage = await screen.findByRole("region", { name: "账号策略资料" });

    fireEvent.change(within(accountPage).getByLabelText("AI 草稿补充方向"), {
      target: { value: "面向 AI 副业新手，强调避坑。" },
    });
    fireEvent.click(within(accountPage).getByRole("button", { name: "生成草稿" }));

    expect(await within(accountPage).findByDisplayValue("AI 副业新手")).toBeInTheDocument();
    expect(within(accountPage).getByText(/草稿偏向 AI 工具教程、避坑和真实案例/)).toBeInTheDocument();
    expect(api.generateStrategyProfileDraft).toHaveBeenCalledWith(expect.anything(), project.project_id, {
      direction_notes: "面向 AI 副业新手，强调避坑。",
      model_id: textModel.model_id,
    });
    expect(api.updateProjectStrategyProfile).not.toHaveBeenCalled();
    expect(screen.getByLabelText("当前账号")).toHaveDisplayValue("科技博主");
  });

  it("账号策略草稿允许用户切换文本模型并提交所选模型 ID", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    vi.mocked(api.listModelOptions).mockImplementation(async (_client, modelType) => ({
      models: modelType === "text" ? [textModel, secondaryTextModel] : [imageModel],
    }));
    mockProjects({ projects: [project] });
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "账号策略" }));
    const accountPage = await screen.findByRole("region", { name: "账号策略资料" });
    fireEvent.change(within(accountPage).getByRole("combobox", { name: "推理模型" }), {
      target: { value: secondaryTextModel.model_id },
    });
    fireEvent.change(within(accountPage).getByLabelText("AI 草稿补充方向"), {
      target: { value: "更偏向真实案例" },
    });
    fireEvent.click(within(accountPage).getByRole("button", { name: "生成草稿" }));

    await waitFor(() => {
      expect(api.generateStrategyProfileDraft).toHaveBeenCalledWith(expect.anything(), project.project_id, {
        direction_notes: "更偏向真实案例",
        model_id: secondaryTextModel.model_id,
      });
    });
  });

  it("模型调用时被停用会刷新选项、保留输入并要求重新选择", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    let modelDisabled = false;
    vi.mocked(api.listModelOptions).mockImplementation(async (_client, modelType) => ({
      models:
        modelType === "text"
          ? modelDisabled
            ? [secondaryTextModel]
            : [textModel, secondaryTextModel]
          : [imageModel],
    }));
    vi.mocked(api.generateStrategyProfileDraft).mockImplementation(async () => {
      modelDisabled = true;
      throw new api.ApiError(409, "模型已停用或删除", {
        error: { code: "model_disabled", message: "模型已停用或删除" },
      });
    });
    mockProjects({ projects: [project] });
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "账号策略" }));
    const accountPage = await screen.findByRole("region", { name: "账号策略资料" });
    const notes = within(accountPage).getByLabelText("AI 草稿补充方向");
    fireEvent.change(notes, { target: { value: "保留这段未提交输入" } });
    fireEvent.click(within(accountPage).getByRole("button", { name: "生成草稿" }));

    expect(await within(accountPage).findByText("原选择已停用或删除，请刷新后重新选择")).toBeInTheDocument();
    expect(notes).toHaveValue("保留这段未提交输入");
    expect(within(accountPage).getByRole("button", { name: "生成草稿" })).toBeDisabled();
    expect(vi.mocked(api.listModelOptions).mock.calls.filter((call) => call[1] === "text")).toHaveLength(2);
  });

  it("账号策略页保存成功后同步账号列表和页面回显", async () => {
    const updatedProject = {
      ...project,
      name: "AI 工具账号",
      strategy_profile: {
        ...strategyProfile,
        target_audience: "AI 副业新手",
      },
    };
    vi.mocked(api.updateProjectStrategyProfile).mockResolvedValue(updatedProject);
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "账号策略" }));
    const accountPage = await screen.findByRole("region", { name: "账号策略资料" });

    fireEvent.change(within(accountPage).getByLabelText("账号名称"), {
      target: { value: "AI 工具账号" },
    });
    fireEvent.change(within(accountPage).getByLabelText("目标受众"), {
      target: { value: "AI 副业新手" },
    });
    fireEvent.click(within(accountPage).getByRole("button", { name: "保存并应用" }));

    await waitFor(() => {
      expect(api.updateProjectStrategyProfile).toHaveBeenCalledWith(expect.anything(), project.project_id, {
        name: "AI 工具账号",
        positioning: "科技知识账号",
        description: "面向程序员的知识短视频",
        strategy_profile: {
          ...strategyProfile,
          target_audience: "AI 副业新手",
        },
      });
    });
    expect(screen.getByLabelText("当前账号")).toHaveDisplayValue("AI 工具账号");
    const structured = within(accountPage).getByRole("region", { name: "结构化策略" });
    await waitFor(() => {
      expect(structured.querySelector(".strategyTextPreview")).toHaveTextContent("目标受众：AI 副业新手");
    });
    expect(within(accountPage).getByLabelText("目标受众")).toHaveValue("AI 副业新手");
  });

  it("账号策略页保存失败时展示错误且不覆盖旧资料", async () => {
    vi.mocked(api.updateProjectStrategyProfile).mockRejectedValue(new Error("策略保存失败"));
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "账号策略" }));
    const accountPage = await screen.findByRole("region", { name: "账号策略资料" });
    const structured = within(accountPage).getByRole("region", { name: "结构化策略" });
    const savedPreview = structured.querySelector(".strategyTextPreview") as HTMLElement;

    fireEvent.change(within(accountPage).getByLabelText("目标受众"), {
      target: { value: "错误保存受众" },
    });
    fireEvent.click(within(accountPage).getByRole("button", { name: "保存并应用" }));

    expect(await within(accountPage).findByRole("alert")).toHaveTextContent("策略保存失败");
    expect(within(savedPreview).getByText(/内容运营负责人/)).toBeInTheDocument();
    expect(within(savedPreview).queryByText(/错误保存受众/)).not.toBeInTheDocument();
    expect(screen.getByLabelText("当前账号")).toHaveDisplayValue("科技博主");
  });

  it("当前选题池不展示账号策略区块，账号策略只在独立二级页面维护", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "当前选题池" }));
    const topicPool = await screen.findByRole("region", { name: "选题池" });

    expect(within(topicPool).queryByRole("heading", { name: "账号策略" })).not.toBeInTheDocument();
    expect(within(topicPool).queryByText("策略资料状态")).not.toBeInTheDocument();
    expect(within(topicPool).queryByText("账号策略摘要")).not.toBeInTheDocument();
    expect(within(topicPool).queryByText("内容运营负责人")).not.toBeInTheDocument();
    expect(within(topicPool).queryByText("表达风格")).not.toBeInTheDocument();
    expect(within(topicPool).queryByText("选题偏好")).not.toBeInTheDocument();
    expect(within(topicPool).queryByRole("button", { name: "编辑账号策略" })).not.toBeInTheDocument();
    expect(within(topicPool).queryByLabelText("账号名称")).not.toBeInTheDocument();
    expect(within(topicPool).queryByLabelText("AI 草稿补充方向")).not.toBeInTheDocument();
  });

  it("历史生成页使用批次、选题和补充操作三列布局", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockTopicBatches(topicBatchListResponse);
    mockTopics(topicListResponse);
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "历史生成" }));
    const history = await screen.findByRole("region", { name: "历史生成列表页" });

    const batchColumn = within(history).getByRole("complementary", { name: "历史生成批次" });
    const topicColumn = within(history).getByRole("region", { name: "当前主题选题" });
    const supplementColumn = within(history).getByRole("complementary", { name: "补充操作" });
    expect(batchColumn).toHaveClass("topicHistoryBatchPanel");
    expect(topicColumn).toHaveClass("topicHistoryTopicPanel");
    expect(supplementColumn).toHaveClass("topicHistorySupplementPanel");
    expect(within(supplementColumn).queryByText("Supplement")).not.toBeInTheDocument();
    expect(within(supplementColumn).queryByText("Review")).not.toBeInTheDocument();
    expect(within(supplementColumn).queryByText("Prompt")).not.toBeInTheDocument();
    expect(supplementColumn.querySelectorAll(".topicHistorySupplementSectionHeader").length).toBeGreaterThanOrEqual(2);
    expect(within(topicColumn).getByRole("article", { name: `历史选题：${ideaTopic.title}` })).toBeInTheDocument();
    expect(within(supplementColumn).getByRole("region", { name: "补充选题" })).toBeInTheDocument();
  });

  it("历史生成页默认按脚本产出优先级展示主题组", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockTopicBatches({ batches: [latestTopicBatch, supplementTopicBatch, previousTopicBatch] });
    mockTopicGroups({ topic_groups: [readyTopicGroup, missingReviewTopicGroup] });
    mockTopics(topicListResponse);
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "历史生成" }));
    const history = await screen.findByRole("region", { name: "历史生成列表页" });
    const batchColumn = within(history).getByRole("complementary", { name: "历史生成批次" });
    const groupButtons = within(batchColumn).getAllByRole("button", {
      name: /AI/,
    });

    expect(within(batchColumn).getByRole("button", { name: "脚本优先" })).toHaveAttribute("aria-pressed", "true");
    expect(groupButtons[0]).toHaveTextContent("建议立刻出脚本");
    expect(groupButtons[0]).toHaveTextContent(previousTopicBatch.prompt);
    expect(groupButtons[0]).toHaveTextContent("86 分");
    expect(groupButtons[0]).toHaveTextContent("1 个候选");
    expect(groupButtons[1]).toHaveTextContent("待评审");
    expect(api.listTopicGroups).toHaveBeenCalledWith(expect.anything(), project.project_id, {
      sort: "script_priority",
    });
  });

  it("历史生成页切换按时间排序后展示时间依据并刷新主题组顺序", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockTopicBatches({ batches: [latestTopicBatch, supplementTopicBatch, previousTopicBatch] });
    vi.mocked(api.listTopicGroups).mockImplementation(async (_client, _projectId, options = {}) => ({
      topic_groups:
        options.sort === "created_at"
          ? [missingReviewTopicGroup, readyTopicGroup]
          : [readyTopicGroup, missingReviewTopicGroup],
    }));
    mockTopics(topicListResponse);
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "历史生成" }));
    const history = await screen.findByRole("region", { name: "历史生成列表页" });
    const batchColumn = within(history).getByRole("complementary", { name: "历史生成批次" });

    expect(within(batchColumn).getAllByRole("button", { name: /AI/ })[0]).toHaveTextContent(
      previousTopicBatch.prompt,
    );

    fireEvent.click(within(batchColumn).getByRole("button", { name: "按时间" }));

    await waitFor(() => {
      expect(api.listTopicGroups).toHaveBeenCalledWith(expect.anything(), project.project_id, {
        sort: "created_at",
      });
      expect(within(batchColumn).getByRole("button", { name: "按时间" })).toHaveAttribute(
        "aria-pressed",
        "true",
      );
      const sortedGroupButtons = within(batchColumn).getAllByRole("button", { name: /AI/ });
      expect(sortedGroupButtons[0]).toHaveTextContent(latestTopicBatch.prompt);
      expect(sortedGroupButtons[0]).toHaveTextContent("07-06");
    });
  });

  it("历史生成页标记缺失或过期评审主题组为需评审", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockTopicBatches({ batches: [latestTopicBatch, previousTopicBatch] });
    mockTopicGroups({ topic_groups: [staleReviewTopicGroup, missingReviewTopicGroup] });
    mockTopics(topicListResponse);
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "历史生成" }));
    const history = await screen.findByRole("region", { name: "历史生成列表页" });

    expect(within(history).getByRole("button", { name: /需重新评审/ })).toHaveTextContent("评审已过期");
    expect(within(history).getByRole("button", { name: /待评审/ })).toHaveTextContent("缺少评审");
    expect(within(history).getByRole("button", { name: "评审当前主题组" })).toBeEnabled();
  });

  it("历史生成页按同一主题展示原始批次和补充批次选题", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockTopicBatches({ batches: [latestTopicBatch, supplementTopicBatch, previousTopicBatch] });
    vi.mocked(api.listContentTopics).mockImplementation(async (_client, _projectId, filters = {}) => {
      if (filters.batch_id === previousTopicBatch.batch_id) {
        return {
          topics: [
            {
              ...approvedTopic,
              batch_id: previousTopicBatch.batch_id,
              title: "原始批次主题选题",
            },
          ],
          stats: { total: 1, idea: 0, approved: 1, scripted: 0, archived: 0 },
        };
      }
      if (filters.batch_id === supplementTopicBatch.batch_id) {
        return {
          topics: [
            {
              ...ideaTopic,
              batch_id: supplementTopicBatch.batch_id,
              title: "补充批次主题选题",
              source: "agent",
            },
          ],
          stats: { total: 1, idea: 1, approved: 0, scripted: 0, archived: 0 },
        };
      }
      return {
        topics: [{ ...ideaTopic, batch_id: latestTopicBatch.batch_id, title: "最新批次选题" }],
        stats: { total: 1, idea: 1, approved: 0, scripted: 0, archived: 0 },
      };
    });

    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "历史生成" }));
    const history = await screen.findByRole("region", { name: "历史生成列表页" });
    const batchColumn = within(history).getByRole("complementary", { name: "历史生成批次" });
    expect(within(batchColumn).queryByRole("button", { name: /补充上一批 AI 内容流水线选题/ })).not.toBeInTheDocument();
    fireEvent.click(within(history).getByRole("button", { name: /^上一批 AI 内容流水线选题/ }));
    const supplementColumn = within(history).getByRole("complementary", { name: "补充操作" });
    expect(within(supplementColumn).queryByText("Related")).not.toBeInTheDocument();

    const topicColumn = await screen.findByRole("region", { name: "当前主题选题" });
    expect(await within(topicColumn).findByRole("article", { name: "历史选题：原始批次主题选题" })).toBeInTheDocument();
    expect(await within(topicColumn).findByRole("article", { name: "历史选题：补充批次主题选题" })).toBeInTheDocument();
    expect(within(topicColumn).getByText("原始生成")).toBeInTheDocument();
    expect(within(topicColumn).getByText("补充生成")).toBeInTheDocument();
    await waitFor(() => {
      expect(api.listContentTopics).toHaveBeenCalledWith(expect.anything(), project.project_id, {
        status: "all",
        source: "all",
        batch_id: previousTopicBatch.batch_id,
      });
      expect(api.listContentTopics).toHaveBeenCalledWith(expect.anything(), project.project_id, {
        status: "all",
        source: "all",
        batch_id: supplementTopicBatch.batch_id,
      });
    });
  });

  it("历史生成页手动评审主题组后展示评审分层", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockTopicBatches({ batches: [latestTopicBatch, supplementTopicBatch, previousTopicBatch] });
    vi.mocked(api.listContentTopics).mockImplementation(async (_client, _projectId, filters = {}) => {
      if (filters.batch_id === previousTopicBatch.batch_id) {
        return {
          topics: [{ ...approvedTopic, batch_id: previousTopicBatch.batch_id }],
          stats: { total: 1, idea: 0, approved: 1, scripted: 0, archived: 0 },
        };
      }
      if (filters.batch_id === supplementTopicBatch.batch_id) {
        return {
          topics: [
            { ...ideaTopic, batch_id: supplementTopicBatch.batch_id, source: "agent" },
            { ...scriptedTopic, batch_id: supplementTopicBatch.batch_id, source: "agent" },
          ],
          stats: { total: 2, idea: 1, approved: 0, scripted: 1, archived: 0 },
        };
      }
      return {
        topics: [{ ...ideaTopic, batch_id: latestTopicBatch.batch_id, title: "最新批次选题" }],
        stats: { total: 1, idea: 1, approved: 0, scripted: 0, archived: 0 },
      };
    });
    let reviewCreated = false;
    vi.mocked(api.createTopicGroupReview).mockImplementation(async (_client, rootBatchId) => {
      reviewCreated = rootBatchId === previousTopicBatch.batch_id;
      return topicReviewSnapshot;
    });
    vi.mocked(api.getLatestTopicGroupReview).mockImplementation(async (_client, rootBatchId) =>
      rootBatchId === previousTopicBatch.batch_id && reviewCreated ? topicReviewSnapshot : null,
    );

    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "历史生成" }));
    const history = await screen.findByRole("region", { name: "历史生成列表页" });
    fireEvent.click(within(history).getByRole("button", { name: /^上一批 AI 内容流水线选题/ }));

    const topicColumn = await screen.findByRole("region", { name: "当前主题选题" });
    expect(within(topicColumn).queryByRole("region", { name: "优先推荐" })).not.toBeInTheDocument();

    fireEvent.click(within(history).getByRole("button", { name: "评审当前主题组" }));

    await waitFor(() => {
      expect(api.createTopicGroupReview).toHaveBeenCalledWith(expect.anything(), previousTopicBatch.batch_id, {
        model_id: textModel.model_id,
      });
      expect(api.getLatestTopicGroupReview).toHaveBeenCalledWith(expect.anything(), previousTopicBatch.batch_id);
    });
    expect(await within(topicColumn).findByRole("region", { name: "优先推荐" })).toBeInTheDocument();
    expect(within(topicColumn).getByRole("region", { name: "可备选" })).toBeInTheDocument();
    expect(within(topicColumn).getByRole("region", { name: "建议淘汰" })).toBeInTheDocument();
    expect(within(topicColumn).getByRole("region", { name: "疑似重复" })).toBeInTheDocument();
    expect(within(topicColumn).getByText(topicReviewSnapshot.review_summary)).toBeInTheDocument();
    expect(within(topicColumn).getByText("账号定位匹配度高，能直接进入脚本创作。")).toBeInTheDocument();
    expect(within(topicColumn).getByText("脚本化难")).toBeInTheDocument();
    expect(within(topicColumn).getByText("泛化")).toBeInTheDocument();
    expect(within(topicColumn).getAllByText(new RegExp(approvedTopic.title)).length).toBeGreaterThan(0);
    const rejectedCard = within(topicColumn).getByRole("article", { name: `历史选题：${scriptedTopic.title}` });
    expect(within(rejectedCard).getAllByText("疑似重复")).toHaveLength(1);
    expect(within(rejectedCard).getByText("相似选题")).toBeInTheDocument();
    expect(within(rejectedCard).getByText(approvedTopic.title)).toBeInTheDocument();
  });

  it("主题组评审在当前选题池同步展示，全部选题模式回退普通列表", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockTopicBatches({ batches: [latestTopicBatch, supplementTopicBatch, previousTopicBatch] });
    vi.mocked(api.getLatestTopicGroupReview).mockImplementation(async (_client, rootBatchId) =>
      rootBatchId === previousTopicBatch.batch_id ? topicReviewSnapshot : null,
    );
    vi.mocked(api.listContentTopics).mockImplementation(async (_client, _projectId, filters = {}) => {
      if (filters.batch_id === previousTopicBatch.batch_id) {
        return {
          topics: [{ ...approvedTopic, batch_id: previousTopicBatch.batch_id }],
          stats: { total: 1, idea: 0, approved: 1, scripted: 0, archived: 0 },
        };
      }
      if (filters.batch_id === supplementTopicBatch.batch_id) {
        return {
          topics: [
            { ...ideaTopic, batch_id: supplementTopicBatch.batch_id, source: "agent" },
            { ...scriptedTopic, batch_id: supplementTopicBatch.batch_id, source: "agent" },
          ],
          stats: { total: 2, idea: 1, approved: 0, scripted: 1, archived: 0 },
        };
      }
      return {
        topics: [
          { ...approvedTopic, batch_id: previousTopicBatch.batch_id },
          { ...ideaTopic, batch_id: supplementTopicBatch.batch_id, source: "agent" },
          { ...scriptedTopic, batch_id: supplementTopicBatch.batch_id, source: "agent" },
        ],
        stats: { total: 3, idea: 1, approved: 1, scripted: 1, archived: 0 },
      };
    });

    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "历史生成" }));
    const history = await screen.findByRole("region", { name: "历史生成列表页" });
    fireEvent.click(within(history).getByRole("button", { name: /^上一批 AI 内容流水线选题/ }));
    expect(await screen.findByRole("region", { name: "优先推荐" })).toBeInTheDocument();

    fireEvent.click(within(screen.getByRole("navigation", { name: "视频工作台菜单" })).getByRole("button", {
      name: "当前选题池",
    }));

    const topicPool = await screen.findByRole("region", { name: "选题池" });
    expect(await within(topicPool).findByRole("region", { name: "优先推荐" })).toBeInTheDocument();
    expect(within(topicPool).getByRole("region", { name: "可备选" })).toBeInTheDocument();
    const rejectedCard = within(topicPool).getByRole("article", { name: `评审选题：${scriptedTopic.title}` });
    fireEvent.click(within(rejectedCard).getByRole("button", { name: approvedTopic.title }));
    const detail = screen.getByRole("region", { name: "选题详情" });
    expect(within(detail).getByRole("heading", { name: approvedTopic.title })).toBeInTheDocument();
    expect(within(topicPool).getByRole("article", { name: `评审选题：${approvedTopic.title}` })).toHaveClass(
      "selected",
    );

    fireEvent.click(within(topicPool).getByRole("button", { name: "查看全部选题" }));

    await waitFor(() => {
      expect(within(topicPool).queryByRole("region", { name: "优先推荐" })).not.toBeInTheDocument();
    });
    expect(within(topicPool).getByText("查看全部选题时不展示主题组评审")).toBeInTheDocument();
    expect(within(topicPool).getByRole("button", { name: new RegExp(approvedTopic.title) })).toBeInTheDocument();
  });

  it("当前选题池支持移除未成稿选题并保留已成稿保护", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    vi.mocked(api.listContentTopics)
      .mockResolvedValueOnce({
        topics: [approvedTopic, scriptedTopic],
        stats: { total: 2, idea: 0, approved: 1, scripted: 1, archived: 0 },
      })
      .mockResolvedValueOnce({
        topics: [scriptedTopic],
        stats: { total: 1, idea: 0, approved: 0, scripted: 1, archived: 0 },
      });
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    vi.mocked(api.deleteContentTopic).mockResolvedValue({
      topic_id: approvedTopic.topic_id,
      deleted_at: "2026-07-08T10:00:00Z",
    });

    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    const topicPool = await screen.findByRole("region", { name: "选题池" });
    const removableCard = await within(topicPool).findByRole("article", { name: `选题：${approvedTopic.title}` });
    const lockedCard = await within(topicPool).findByRole("article", { name: `选题：${scriptedTopic.title}` });
    expect(within(removableCard).getByRole("button", { name: "移除" })).toBeEnabled();
    expect(within(lockedCard).queryByRole("button", { name: "移除" })).not.toBeInTheDocument();

    fireEvent.click(within(removableCard).getByRole("button", { name: "移除" }));

    await waitFor(() => {
      expect(api.deleteContentTopic).toHaveBeenCalledWith(expect.anything(), approvedTopic.topic_id);
    });
    expect(confirmSpy).toHaveBeenCalledWith(`确认从管理视图移除「${approvedTopic.title}」吗？`);
    await waitFor(() => {
      expect(within(topicPool).queryByRole("article", { name: `选题：${approvedTopic.title}` })).not.toBeInTheDocument();
    });
    confirmSpy.mockRestore();
  });

  it("评审分层列表保留确认、归档、移除和生成脚本动作", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockTopicBatches({ batches: [latestTopicBatch, supplementTopicBatch, previousTopicBatch] });
    vi.mocked(api.getLatestTopicGroupReview).mockImplementation(async (_client, rootBatchId) =>
      rootBatchId === previousTopicBatch.batch_id ? topicReviewSnapshot : null,
    );
    vi.mocked(api.listContentTopics).mockImplementation(async (_client, _projectId, filters = {}) => {
      if (filters.batch_id === previousTopicBatch.batch_id) {
        return {
          topics: [{ ...approvedTopic, batch_id: previousTopicBatch.batch_id }],
          stats: { total: 1, idea: 0, approved: 1, scripted: 0, archived: 0 },
        };
      }
      if (filters.batch_id === supplementTopicBatch.batch_id) {
        return {
          topics: [
            { ...ideaTopic, batch_id: supplementTopicBatch.batch_id, source: "agent" },
            { ...scriptedTopic, batch_id: supplementTopicBatch.batch_id, source: "agent" },
          ],
          stats: { total: 2, idea: 1, approved: 0, scripted: 1, archived: 0 },
        };
      }
      return {
        topics: [],
        stats: { total: 0, idea: 0, approved: 0, scripted: 0, archived: 0 },
      };
    });
    vi.mocked(api.updateContentTopicStatus)
      .mockResolvedValueOnce({ ...ideaTopic, status: "approved" })
      .mockResolvedValueOnce({ ...approvedTopic, status: "archived" });

    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "历史生成" }));
    const history = await screen.findByRole("region", { name: "历史生成列表页" });
    fireEvent.click(within(history).getByRole("button", { name: /^上一批 AI 内容流水线选题/ }));

    const backupSection = await screen.findByRole("region", { name: "可备选" });
    const prioritySection = await screen.findByRole("region", { name: "优先推荐" });
    expect(within(backupSection).getByRole("button", { name: "确认选题" })).toBeEnabled();
    expect(within(prioritySection).getByRole("button", { name: "生成脚本" })).toBeEnabled();
    expect(within(prioritySection).getByRole("button", { name: "归档选题" })).toBeEnabled();
    expect(within(prioritySection).getByRole("button", { name: "移除" })).toBeEnabled();

    fireEvent.click(within(backupSection).getByRole("button", { name: "确认选题" }));
    await waitFor(() => {
      expect(api.updateContentTopicStatus).toHaveBeenCalledWith(expect.anything(), ideaTopic.topic_id, "approved");
    });

    fireEvent.click(within(prioritySection).getByRole("button", { name: "生成脚本" }));
    await waitFor(() => {
      expect(api.prepareScriptFromTopic).toHaveBeenCalledWith(expect.anything(), approvedTopic.topic_id, {
        style: "knowledge",
        scene_count: 6,
      });
    });
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

  it("历史生成页补充选题后保持原始主题组选中并展示关联补充批次", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    vi.mocked(api.listTopicGenerationBatches)
      .mockResolvedValueOnce({
        batches: [latestTopicBatch, previousTopicBatch],
      })
      .mockResolvedValueOnce({
        batches: [supplementTopicBatch, latestTopicBatch, previousTopicBatch],
      });
    vi.mocked(api.listContentTopics).mockImplementation(async (_client, _projectId, filters = {}) => {
      if (filters.batch_id === previousTopicBatch.batch_id) {
        return {
          topics: [
            {
              ...approvedTopic,
              batch_id: previousTopicBatch.batch_id,
              title: "历史批次选题",
            },
          ],
          stats: { total: 3, idea: 1, approved: 1, scripted: 1, archived: 0 },
        };
      }
      if (filters.batch_id === supplementTopicBatch.batch_id) {
        return {
          topics: [
            {
              ...ideaTopic,
              batch_id: supplementTopicBatch.batch_id,
              title: "补充批次选题",
              source: "agent",
            },
          ],
          stats: { total: 4, idea: 2, approved: 1, scripted: 1, archived: 0 },
        };
      }
      return {
        topics: [
          {
            ...ideaTopic,
            batch_id: latestTopicBatch.batch_id,
            title: "最新批次选题",
            source: "agent",
          },
        ],
        stats: { total: 3, idea: 1, approved: 1, scripted: 1, archived: 0 },
      };
    });
    vi.mocked(api.createAgentConversation).mockResolvedValue(topicConversation);
    vi.mocked(api.sendAgentMessage).mockResolvedValue({
      user_message: topicUserMessage,
      assistant_message: {
        ...topicAssistantMessage,
        metadata: {
          ...topicAssistantMessage.metadata,
          batch_id: supplementTopicBatch.batch_id,
          supplement_of_batch_id: previousTopicBatch.batch_id,
          topic_count: 2,
        },
      },
      run: topicAgentRun,
    });

    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "历史生成" }));
    const history = await screen.findByRole("region", { name: "历史生成列表页" });
    fireEvent.click(within(history).getByRole("button", { name: /上一批 AI 内容流水线选题/ }));

    const supplementRegion = await screen.findByRole("region", { name: "补充选题" });
    fireEvent.change(within(supplementRegion).getByLabelText("补充要求"), {
      target: { value: "补充遗漏的 AI 工作流复盘角度" },
    });
    fireEvent.click(within(supplementRegion).getByRole("button", { name: "补充生成" }));

    await waitFor(() => {
      expect(api.createAgentConversation).toHaveBeenCalledWith(expect.anything(), {
        project_id: project.project_id,
        agent_type: "topic",
        title: "选题 Agent 对话",
      });
    });
    expect(api.sendAgentMessage).toHaveBeenCalledWith(expect.anything(), topicConversation.conversation_id, {
      content: "补充遗漏的 AI 工作流复盘角度",
      model_id: textModel.model_id,
      supplement_of_batch_id: previousTopicBatch.batch_id,
    });
    await waitFor(() => {
      expect(api.listTopicGenerationBatches).toHaveBeenCalledTimes(2);
      expect(api.listContentTopics).toHaveBeenCalledWith(expect.anything(), project.project_id, {
        status: "all",
        source: "all",
        batch_id: previousTopicBatch.batch_id,
      });
      expect(api.listContentTopics).toHaveBeenCalledWith(expect.anything(), project.project_id, {
        status: "all",
        source: "all",
        batch_id: supplementTopicBatch.batch_id,
      });
    });
    const batchColumn = within(history).getByRole("complementary", { name: "历史生成批次" });
    expect(within(batchColumn).queryByRole("button", { name: /补充上一批 AI 内容流水线选题/ })).not.toBeInTheDocument();
    expect(within(batchColumn).getByRole("button", { name: /^上一批 AI 内容流水线选题/ })).toHaveClass("selected");
    const supplementList = await screen.findByRole("region", { name: "关联补充批次" });
    expect(within(supplementList).getByRole("button", { name: /补充上一批 AI 内容流水线选题/ })).toHaveClass(
      "selected",
    );
    expect(await screen.findByRole("article", { name: "历史选题：补充批次选题" })).toBeInTheDocument();
  });

  it("历史生成页补充失败时保留当前批次并展示原因", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockTopicBatches({ batches: [latestTopicBatch, previousTopicBatch] });
    mockTopics({
      topics: [{ ...approvedTopic, batch_id: previousTopicBatch.batch_id, title: "历史批次选题" }],
      stats: { total: 1, idea: 0, approved: 1, scripted: 0, archived: 0 },
    });
    vi.mocked(api.createAgentConversation).mockResolvedValue(topicConversation);
    vi.mocked(api.sendAgentMessage).mockRejectedValue(new Error("该历史生成批次不可补充"));

    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "历史生成" }));
    const history = await screen.findByRole("region", { name: "历史生成列表页" });
    fireEvent.click(within(history).getByRole("button", { name: /上一批 AI 内容流水线选题/ }));

    const supplementRegion = await screen.findByRole("region", { name: "补充选题" });
    fireEvent.change(within(supplementRegion).getByLabelText("补充要求"), {
      target: { value: "补充 1 个新角度" },
    });
    fireEvent.click(within(supplementRegion).getByRole("button", { name: "补充生成" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("该历史生成批次不可补充");
    expect(within(history).getByRole("button", { name: /上一批 AI 内容流水线选题/ })).toHaveClass("selected");
    expect(api.listTopicGenerationBatches).toHaveBeenCalledTimes(1);
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
      model_id: textModel.model_id,
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

  it("选题 Agent 消息展示质量闸门通过数、淘汰数和重写状态", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    vi.mocked(api.createAgentConversation).mockResolvedValue(topicConversation);
    vi.mocked(api.sendAgentMessage).mockResolvedValue({
      user_message: topicUserMessage,
      assistant_message: {
        ...topicAssistantMessage,
        metadata: {
          ...topicAssistantMessage.metadata,
          quality_pass_count: 2,
          quality_reject_count: 1,
          quality_rewrite_triggered: true,
          quality_evaluation_id: topicQualityEvaluation.evaluation_id,
        },
      },
      run: topicAgentRun,
    });
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    const agentPanel = await screen.findByRole("region", { name: "选题 Agent" });
    fireEvent.change(within(agentPanel).getByLabelText("生成要求"), {
      target: { value: topicUserMessage.content },
    });
    fireEvent.click(within(agentPanel).getByRole("button", { name: "生成选题" }));

    expect(await within(agentPanel).findByText("质量闸门")).toBeInTheDocument();
    expect(within(agentPanel).getByText("通过 2")).toBeInTheDocument();
    expect(within(agentPanel).getByText("淘汰 1")).toBeInTheDocument();
    expect(within(agentPanel).getByText("已重写")).toBeInTheDocument();
  });

  it("当前选题池卡片展示质量分和风险标签", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    const qualityTopic: ContentTopic = {
      ...approvedTopic,
      metadata: {
        quality_gate: {
          evaluation_id: topicQualityEvaluation.evaluation_id,
          candidate_key: "candidate-1",
          quality_score: 88,
          flags: ["hard_to_script"],
          reason: "贴合账号定位，但脚本化案例需要补强。",
        },
      },
    };
    mockTopics({
      topics: [qualityTopic],
      stats: { total: 1, idea: 0, approved: 1, scripted: 0, archived: 0 },
    });
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    const topicPool = await screen.findByRole("region", { name: "选题池" });
    const qualityCard = await within(topicPool).findByRole("article", { name: `选题：${qualityTopic.title}` });

    expect(within(qualityCard).getByText("质量 88")).toBeInTheDocument();
    expect(within(qualityCard).getByText("脚本化难")).toBeInTheDocument();
  });

  it("历史生成批次卡展示质量摘要", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockTopicBatches({ batches: [latestTopicBatch] });
    mockTopicQualityEvaluation(topicQualityEvaluation);
    mockTopics({
      topics: [{ ...approvedTopic, batch_id: latestTopicBatch.batch_id }],
      stats: { total: 1, idea: 0, approved: 1, scripted: 0, archived: 0 },
    });
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "历史生成" }));
    const history = await screen.findByRole("region", { name: "历史生成列表页" });
    const batchColumn = within(history).getByRole("complementary", { name: "历史生成批次" });
    const batchCard = await within(batchColumn).findByRole("button", { name: /最新一批 AI 工具选题/ });

    await waitFor(() => {
      expect(api.getLatestTopicQualityEvaluation).toHaveBeenCalledWith(
        expect.anything(),
        latestTopicBatch.batch_id,
        project.project_id,
      );
      expect(batchCard).toHaveTextContent("质量：通过 2 · 淘汰 1 · 已重写");
    });
  });

  it("历史生成页右侧展示质量报告，淘汰候选只读无确认、脚本、归档或移除操作", async () => {
    vi.mocked(api.listWorkspaceMenus).mockResolvedValue(contentStrategyWorkspaceMenus);
    mockProjects({ projects: [project] });
    mockTopicBatches({ batches: [latestTopicBatch] });
    mockTopicQualityEvaluation(topicQualityEvaluation);
    mockTopics({
      topics: [{ ...approvedTopic, batch_id: latestTopicBatch.batch_id }],
      stats: { total: 1, idea: 0, approved: 1, scripted: 0, archived: 0 },
    });
    render(createElement(Home));

    fireEvent.click(await screen.findByRole("button", { name: /内容策略/ }));
    fireEvent.click(await screen.findByRole("button", { name: "历史生成" }));
    const qualityReport = await screen.findByRole("region", { name: "质量报告" });
    const rejectedCandidate = await within(qualityReport).findByRole("article", {
      name: "淘汰候选：人工智能是什么",
    });

    expect(within(qualityReport).getByText("重写后 3 条中 2 条通过，1 条淘汰。")).toBeInTheDocument();
    expect(within(rejectedCandidate).getByText("淘汰")).toBeInTheDocument();
    expect(within(rejectedCandidate).getByText("52 分")).toBeInTheDocument();
    expect(within(rejectedCandidate).getByText("泛化")).toBeInTheDocument();
    expect(within(rejectedCandidate).getByText("评分存疑")).toBeInTheDocument();
    for (const actionLabel of ["确认选题", "生成脚本", "归档选题", "移除"]) {
      expect(within(rejectedCandidate).queryByRole("button", { name: actionLabel })).not.toBeInTheDocument();
    }
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
    const createdManualTopic = {
      ...ideaTopic,
      topic_id: "98989898-9898-4989-8989-989898989898",
      title: "AI 产品周报选题",
    };
    vi.mocked(api.createContentTopic).mockResolvedValue(createdManualTopic);
    vi.mocked(api.updateContentTopic).mockResolvedValue({ ...ideaTopic, angle: "编辑后的角度" });
    vi.mocked(api.updateContentTopicStatus).mockImplementation(async (_client, topicId, status) => {
      if (topicId === approvedTopic.topic_id && status === "archived") {
        return { ...approvedTopic, status: "archived" };
      }
      if (topicId === createdManualTopic.topic_id && status === "approved") {
        return { ...createdManualTopic, status: "approved" };
      }
      return { ...ideaTopic, status };
    });
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

    fireEvent.click(await screen.findByRole("button", { name: /普通人如何搭建 AI 内容流水线/ }));
    let detail = screen.getByRole("region", { name: "选题详情" });
    fireEvent.click(within(detail).getByRole("button", { name: "归档选题" }));
    await waitFor(() => {
      expect(api.updateContentTopicStatus).toHaveBeenCalledWith(
        expect.anything(),
        approvedTopic.topic_id,
        "archived",
      );
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

    fireEvent.click(await screen.findByRole("button", { name: /AI 产品周报选题/ }));
    detail = screen.getByRole("region", { name: "选题详情" });
    fireEvent.click(within(detail).getByRole("button", { name: "确认选题" }));
    await waitFor(() => {
      expect(api.updateContentTopicStatus).toHaveBeenCalledWith(
        expect.anything(),
        createdManualTopic.topic_id,
        "approved",
      );
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
        model_id: textModel.model_id,
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
