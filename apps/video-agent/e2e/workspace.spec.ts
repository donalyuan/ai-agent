import { expect, test, type Page } from "@playwright/test";

const projectId = "11111111-1111-4111-8111-111111111111";
const scriptId = "22222222-2222-4222-8222-222222222222";
const previousTopicBatchId = "77777777-7777-4777-8777-777777777777";
const supplementTopicBatchId = "99999999-9999-4999-8999-999999999901";

const emptyStrategyProfile = {
  target_audience: "",
  content_pillars: [],
  tone_style: "",
  forbidden_topics: [],
  reference_accounts: [],
  topic_preferences: "",
};

const project = {
  project_id: projectId,
  name: "科技博主",
  positioning: "科技知识账号",
  description: "面向程序员的知识短视频",
  strategy_profile: emptyStrategyProfile,
  status: "active",
  created_at: "2026-07-02T00:00:00Z",
  updated_at: "2026-07-02T00:00:00Z",
};

const updatedStrategyProfile = {
  target_audience: "内容运营负责人",
  content_pillars: ["AI 工具", "内容生产"],
  tone_style: "直接清晰",
  forbidden_topics: ["夸大收益"],
  reference_accounts: ["参考账号A"],
  topic_preferences: "优先教程和案例",
};

const accountStrategyProject = {
  ...project,
  strategy_profile: updatedStrategyProfile,
  updated_at: "2026-07-08T02:00:00Z",
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
        ...menuNode("asset-generation", "素材生成", true, "active", 20),
        agent_key: "material-generation-agent",
        menu_type: "page",
        module_key: "materials.asset-generation",
      },
    ],
  },
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
        ...menuNode("asset-generation", "素材生成", true, "active", 20),
        agent_key: "material-generation-agent",
        menu_type: "page",
        module_key: "materials.asset-generation",
      },
    ],
  },
  menuNode("production", "作品生产", false, "planned", 40),
  menuNode("publishing", "发布运营", false, "planned", 50),
  menuNode("analytics", "数据分析", false, "planned", 60),
  menuNode("workflow-tasks", "工作流任务", false, "planned", 70),
];

const materialWorkspaceMenus = [
  ...contentStrategyWorkspaceMenus.slice(0, 2),
  {
    ...menuNode("material-management", "素材管理", true, "active", 30),
    children: [
      {
        ...menuNode("material-library", "素材库", true, "active", 10),
        menu_type: "page",
        module_key: "materials.library",
      },
      {
        ...menuNode("asset-generation", "素材生成", true, "active", 20),
        agent_key: "material-generation-agent",
        menu_type: "page",
        module_key: "materials.asset-generation",
      },
    ],
  },
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

const topicReviewSnapshot = {
  snapshot_id: "18181818-1818-4818-8818-181818181818",
  project_id: projectId,
  root_batch_id: previousTopicBatchId,
  source_run_id: "19191919-1919-4919-8919-191919191919",
  status: "succeeded",
  review_summary: "主题组中优先推进历史批次选题，补充选题需要再次评审。",
  result: {
    topic_reviews: [
      {
        topic_id: approvedTopic.topic_id,
        priority: "priority",
        reason: "账号定位匹配度高，适合直接进入脚本创作。",
        risk_flags: [],
        similar_topic_ids: [],
      },
    ],
  },
  error_message: null,
  metadata: {},
  created_at: "2026-07-07T09:00:00Z",
  updated_at: "2026-07-07T09:00:15Z",
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

const subtitleMaterial = {
  material_id: "abababab-abab-4aba-8aba-abababababab",
  project_id: projectId,
  material_type: "subtitle",
  file_url: "https://cdn.example.com/subtitles/demo.vtt",
  thumbnail_url: null,
  file_name: "demo.vtt",
  tags: ["字幕", "中英双语"],
  metadata: { language: "zh-CN", subtitle_format: "vtt" },
  usage_count: 0,
  status: "active",
  created_at: "2026-07-09T00:00:00Z",
  updated_at: "2026-07-09T00:00:00Z",
};

const assetGenerationPayload = {
  provider: "gpt-image-2",
  image_candidates_per_scene: 3,
  use_reference_materials: true,
};

const assetGenerationPlan = {
  script_id: scriptId,
  scene_count: scriptDetail.scenes.length,
  image_candidate_count: scriptDetail.scenes.length * assetGenerationPayload.image_candidates_per_scene,
  max_image_candidate_count: 48,
  provider: assetGenerationPayload.provider,
  enabled_providers: ["gpt-image-2", "jimeng"],
  reference_material_count: 1,
  video_task_count: scriptDetail.scenes.length,
  can_create: true,
  warnings: [],
};

const imageAssetGenerationTask = {
  task_id: "17171717-1717-4717-8717-171717171717",
  project_id: projectId,
  script_id: scriptId,
  scene_id: null,
  provider: assetGenerationPayload.provider,
  task_type: "image_candidates",
  status: "pending",
  candidate_count: assetGenerationPlan.image_candidate_count,
  reference_material_ids: ["24242424-2424-4242-8242-242424242424"],
  params: { image_candidates_per_scene: assetGenerationPayload.image_candidates_per_scene },
  result: {},
  error_message: null,
  retry_count: 0,
  dismissed_at: null,
  created_at: "2026-07-09T00:30:00Z",
  updated_at: "2026-07-09T00:30:00Z",
};

const failedAssetGenerationTask = {
  ...imageAssetGenerationTask,
  task_id: "25252525-2525-4525-8525-252525252525",
  scene_id: scriptDetail.scenes[0].scene_id,
  status: "failed",
  candidate_count: assetGenerationPayload.image_candidates_per_scene,
  error_message: "图片供应商返回生成失败",
  created_at: "2026-07-09T00:35:00Z",
  updated_at: "2026-07-09T00:36:00Z",
};

const videoDraftAssetGenerationTask = {
  ...imageAssetGenerationTask,
  task_id: "18181818-1818-4818-8818-181818181818",
  scene_id: scriptDetail.scenes[0].scene_id,
  task_type: "video_draft",
  status: "draft",
  candidate_count: 0,
  params: { requires_manual_confirmation: true },
};

const selectedPrimaryAssetCandidate = {
  candidate_id: "19191919-1919-4919-8919-191919191919",
  project_id: projectId,
  script_id: scriptId,
  scene_id: scriptDetail.scenes[0].scene_id,
  material_id: "24242424-2424-4242-8242-242424242424",
  candidate_type: "image",
  source: "existing_material",
  status: "selected",
  rank: 0,
  generation_task_id: null,
  metadata: { role: "primary" },
  file_url: "https://cdn.example.com/materials/current-primary.png",
  thumbnail_url: "https://cdn.example.com/materials/current-primary-thumb.png",
  file_name: "current-primary.png",
  created_at: "2026-07-09T00:31:00Z",
  updated_at: "2026-07-09T00:31:00Z",
};

const aiImageAssetCandidate = {
  ...selectedPrimaryAssetCandidate,
  candidate_id: "20202020-2020-4020-8020-202020202020",
  material_id: "21212121-2121-4121-8121-212121212121",
  source: "ai_generated",
  status: "candidate",
  rank: 1,
  generation_task_id: imageAssetGenerationTask.task_id,
  file_url: "https://cdn.example.com/assets/generated/images/task/scene-1.png",
  thumbnail_url: "https://cdn.example.com/assets/generated/images/task/scene-1.png",
  file_name: "scene-1.png",
};

const additionalAiImageAssetCandidates = [2, 3].map((rank) => ({
  ...aiImageAssetCandidate,
  candidate_id: `${rank}0202020-2020-4020-8020-20202020202${rank}`,
  material_id: `${rank}1212121-2121-4121-8121-21212121212${rank}`,
  rank,
  file_url: `https://cdn.example.com/assets/generated/images/task/scene-${rank}.png`,
  thumbnail_url: `https://cdn.example.com/assets/generated/images/task/scene-${rank}.png`,
  file_name: `scene-${rank}.png`,
}));

const videoTaskAssetCandidate = {
  ...selectedPrimaryAssetCandidate,
  candidate_id: "23232323-2323-4232-8232-232323232323",
  material_id: null,
  candidate_type: "video",
  source: "video_task",
  status: "candidate",
  rank: 10000,
  generation_task_id: videoDraftAssetGenerationTask.task_id,
  file_url: null,
  thumbnail_url: null,
  file_name: null,
  metadata: { requires_manual_confirmation: true },
};

const failedAssetCandidate = {
  ...aiImageAssetCandidate,
  candidate_id: "26262626-2626-4626-8626-262626262626",
  material_id: null,
  status: "failed",
  rank: 2,
  generation_task_id: failedAssetGenerationTask.task_id,
  file_url: null,
  thumbnail_url: null,
  file_name: null,
  metadata: { error_message: "图片供应商返回生成失败" },
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

function buildScriptAssetCandidates(selectedCandidateId: string, includeFailedTask = true) {
  const candidates = [
    {
      ...selectedPrimaryAssetCandidate,
      status:
        selectedCandidateId === selectedPrimaryAssetCandidate.candidate_id ? "selected" : "candidate",
    },
    {
      ...aiImageAssetCandidate,
      status: selectedCandidateId === aiImageAssetCandidate.candidate_id ? "selected" : "candidate",
    },
    ...additionalAiImageAssetCandidates,
    videoTaskAssetCandidate,
  ];
  return includeFailedTask ? [...candidates, failedAssetCandidate] : candidates;
}

async function mockScriptAssetWorkflow(page: Page) {
  let assetGenerationPlanRequestCount = 0;
  let assetGenerationTasksRequestCount = 0;
  let assetCandidatesRequestCount = 0;
  let selectAssetCandidateRequestCount = 0;
  let confirmAssetGenerationTaskRequestCount = 0;
  let dismissAssetGenerationTaskRequestCount = 0;
  let selectedCandidateId = selectedPrimaryAssetCandidate.candidate_id;
  let failedTaskDismissed = false;

  await page.route(new RegExp(`/api/scripts/${scriptId}/asset-generation-plan(?:\\?.*)?$`), async (route) => {
    assetGenerationPlanRequestCount += 1;
    expect(route.request().method()).toBe("POST");
    expect(route.request().postDataJSON()).toEqual(assetGenerationPayload);
    await route.fulfill({ contentType: "application/json", json: assetGenerationPlan });
  });
  await page.route(
    new RegExp(`/api/scripts/${scriptId}/asset-generation-tasks(?:\\?.*)?$`),
    async (route) => {
      if (route.request().method() === "GET") {
        await route.fulfill({
          contentType: "application/json",
          json: {
            script_id: scriptId,
            tasks: [
              imageAssetGenerationTask,
              videoDraftAssetGenerationTask,
              ...(failedTaskDismissed ? [] : [failedAssetGenerationTask]),
            ],
          },
        });
        return;
      }

      assetGenerationTasksRequestCount += 1;
      expect(route.request().method()).toBe("POST");
      expect(route.request().postDataJSON()).toEqual(assetGenerationPayload);
      await route.fulfill({
        contentType: "application/json",
        status: 201,
        json: {
          script_id: scriptId,
          tasks: [
            imageAssetGenerationTask,
            videoDraftAssetGenerationTask,
            ...(failedTaskDismissed ? [] : [failedAssetGenerationTask]),
          ],
        },
      });
    },
  );
  await page.route(new RegExp(`/api/scripts/${scriptId}/asset-candidates(?:\\?.*)?$`), async (route) => {
    assetCandidatesRequestCount += 1;
    await route.fulfill({
      contentType: "application/json",
      json: { candidates: buildScriptAssetCandidates(selectedCandidateId, !failedTaskDismissed) },
    });
  });
  await page.route(
    new RegExp(
      `/api/scenes/${scriptDetail.scenes[0].scene_id}/asset-candidates/${aiImageAssetCandidate.candidate_id}/select$`,
    ),
    async (route) => {
      selectAssetCandidateRequestCount += 1;
      selectedCandidateId = aiImageAssetCandidate.candidate_id;
      expect(route.request().method()).toBe("PUT");
      await route.fulfill({
        contentType: "application/json",
        json: { ...aiImageAssetCandidate, status: "selected" },
      });
    },
  );
  await page.route(
    new RegExp(`/api/asset-generation-tasks/${failedAssetGenerationTask.task_id}/dismiss$`),
    async (route) => {
      dismissAssetGenerationTaskRequestCount += 1;
      failedTaskDismissed = true;
      expect(route.request().method()).toBe("POST");
      await route.fulfill({
        contentType: "application/json",
        json: { ...failedAssetGenerationTask, dismissed_at: "2026-07-10T00:00:00Z" },
      });
    },
  );
  await page.route(
    new RegExp(`/api/asset-generation-tasks/${videoDraftAssetGenerationTask.task_id}/confirm$`),
    async (route) => {
      confirmAssetGenerationTaskRequestCount += 1;
      expect(route.request().method()).toBe("POST");
      await route.fulfill({
        contentType: "application/json",
        json: {
          ...videoDraftAssetGenerationTask,
          task_type: "video_generation",
          status: "pending",
        },
      });
    },
  );

  return {
    assetGenerationPlanRequestCount: () => assetGenerationPlanRequestCount,
    assetGenerationTasksRequestCount: () => assetGenerationTasksRequestCount,
    assetCandidatesRequestCount: () => assetCandidatesRequestCount,
    selectAssetCandidateRequestCount: () => selectAssetCandidateRequestCount,
    confirmAssetGenerationTaskRequestCount: () => confirmAssetGenerationTaskRequestCount,
    dismissAssetGenerationTaskRequestCount: () => dismissAssetGenerationTaskRequestCount,
  };
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

async function mockAccountStrategyWorkflow(page: Page) {
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
  await page.route(new RegExp(`/api/projects/${projectId}/strategy-profile$`), async (route) => {
    expect(route.request().method()).toBe("PUT");
    expect(route.request().postDataJSON()).toEqual({
      name: project.name,
      positioning: project.positioning,
      description: project.description,
      strategy_profile: updatedStrategyProfile,
    });
    await route.fulfill({ contentType: "application/json", json: accountStrategyProject });
  });
}

async function mockContentStrategyHistoryWorkflow(page: Page) {
  let supplementGenerated = false;
  let reviewCreated = false;

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
  await page.route(/\/api\/topic-groups\/[^/]+\/reviews\/latest$/, async (route) => {
    const rootBatchId = route.request().url().split("/topic-groups/")[1]?.split("/")[0];
    await route.fulfill({
      contentType: "application/json",
      json: reviewCreated && rootBatchId === previousTopicBatchId ? topicReviewSnapshot : null,
    });
  });
  await page.route(/\/api\/topic-groups\/[^/]+\/reviews$/, async (route) => {
    const rootBatchId = route.request().url().split("/topic-groups/")[1]?.split("/")[0];
    expect(route.request().method()).toBe("POST");
    expect(rootBatchId).toBe(previousTopicBatchId);
    reviewCreated = true;
    await route.fulfill({
      contentType: "application/json",
      status: 201,
      json: topicReviewSnapshot,
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

async function mockMaterialLibraryWorkflow(page: Page) {
  await page.unroute(/\/api\/video-workspace\/menus$/);
  await page.route(/\/api\/video-workspace\/menus$/, async (route) => {
    await route.fulfill({ contentType: "application/json", json: { menus: materialWorkspaceMenus } });
  });
  await page.route(new RegExp(`/api/projects/${projectId}/topic-generation-batches$`), async (route) => {
    await route.fulfill({ contentType: "application/json", json: { batches: [] } });
  });
  await page.route(new RegExp(`/api/projects/${projectId}/topic-groups(\\?.*)?$`), async (route) => {
    await route.fulfill({ contentType: "application/json", json: { topic_groups: [] } });
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
  await page.route(new RegExp(`/api/projects/${projectId}/materials(\\?.*)?$`), async (route) => {
    await route.fulfill({ contentType: "application/json", json: { materials: [subtitleMaterial] } });
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
  for (const label of ["选题 Agent", "脚本 Agent", "素材智能体", "视频智能体", "发布智能体", "优化智能体"]) {
    await expect(workspaceMenu.getByText(label)).toHaveCount(0);
  }
  await expect(workspaceMenu.getByRole("button", { name: /内容策略/ })).toHaveClass(/active/);
  await expect(page.getByRole("heading", { name: "内容策略" })).toBeVisible();
  await workspaceMenu.getByRole("button", { name: /脚本创作/ }).click();
  await expect(workspaceMenu.getByRole("button", { name: /脚本创作/ })).toHaveClass(/active/);
  await expect(workspaceMenu.getByRole("button", { name: /内容策略/ })).toBeEnabled();
  await expect(workspaceMenu.getByRole("button", { name: /素材管理/ })).toBeEnabled();

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
  await expect(page.getByRole("region", { name: "脚本详情素材候选" })).toHaveCount(0);

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

test("素材生成页支持生成选择主素材并确认生成视频", async ({ page }) => {
  await mockExistingScriptWorkflow(page);
  const assetWorkflow = await mockScriptAssetWorkflow(page);
  await page.goto("/");
  const workspaceMenu = page.getByRole("navigation", { name: "视频工作台菜单" });
  await workspaceMenu.getByRole("button", { name: /素材管理/ }).click();
  await workspaceMenu.getByRole("button", { name: "素材生成" }).click();

  await expect(page.getByRole("heading", { name: "脚本详情素材候选" })).toBeVisible();
  const panel = await page.getByRole("region", { name: "脚本详情素材候选" });
  await expect(panel).toBeVisible();
  await expect(panel.getByText("分镜列表")).toBeVisible();
  await expect(panel.getByText("候选素材")).toBeVisible();
  await expect(panel.getByText("生成设置与任务")).toBeVisible();
  await expect(panel.getByRole("button", { name: "gpt-image-2" })).toHaveClass(/selected/);
  await expect(panel.getByRole("button", { name: "即梦" })).toBeVisible();
  await expect(panel.getByText("2 分镜 × 3 = 6 张图片候选")).toBeVisible();
  await expect(panel.getByText("单次最多 48 张")).toBeVisible();
  await expect(panel.getByText("当前主素材")).toBeVisible();
  await expect(panel.getByText("AI 图片候选")).toBeVisible();
  await expect(panel.getByText("AI 视频二次确认")).toBeVisible();
  await expect(panel.getByText(scriptDetail.scenes[0].narration)).toBeVisible();
  await expect(panel.getByText(scriptDetail.scenes[0].visual_description)).toBeVisible();
  await expect(panel.getByRole("button", { name: "生成素材候选" })).toHaveCount(0);
  await expect.poll(() => assetWorkflow.assetGenerationPlanRequestCount()).toBeGreaterThan(0);
  await expect.poll(() => assetWorkflow.assetCandidatesRequestCount()).toBeGreaterThan(0);

  await page.getByRole("button", { name: "生成素材候选" }).click();
  await expect.poll(() => assetWorkflow.assetGenerationTasksRequestCount()).toBe(1);
  await expect(panel.getByText("scene-1.png")).toBeVisible();
  await expect(panel.getByRole("button", { name: "选择为主素材" }).first()).toBeVisible();

  await panel.getByRole("button", { name: "选择为主素材" }).first().click();
  await expect.poll(() => assetWorkflow.selectAssetCandidateRequestCount()).toBe(1);
  await expect(panel.getByText("当前主素材")).toBeVisible();
  await expect(panel.getByText("scene-1.png")).toBeVisible();

  await panel.getByRole("button", { name: "确认生成视频" }).first().click();
  await expect.poll(() => assetWorkflow.confirmAssetGenerationTaskRequestCount()).toBe(1);

  await panel.getByRole("button", { name: "清理失败任务" }).click();
  const dismissDialog = page.getByRole("dialog", { name: "清理失败任务？" });
  await expect(dismissDialog).toBeVisible();
  await expect(dismissDialog.getByText(/不会重新调用供应商，也不会产生额外费用/)).toBeVisible();
  await dismissDialog.getByRole("button", { name: "确认清理" }).click();
  await expect.poll(() => assetWorkflow.dismissAssetGenerationTaskRequestCount()).toBe(1);
  await expect(panel.getByRole("button", { name: "清理失败任务" })).toHaveCount(0);
  await expect(dismissDialog).toHaveCount(0);
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
  await expect(page.getByRole("region", { name: "选题池" }).getByText("策略资料状态")).toHaveCount(0);
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
  expect(poolLayout.filtersTop!).toBeLessThanOrEqual(180);
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

test("内容策略账号策略页保存资料后当前选题池不展示账号策略区块", async ({ page }) => {
  await page.setViewportSize({ width: 1756, height: 980 });
  await mockAccountStrategyWorkflow(page);
  await page.goto("/");

  const workspaceMenu = page.getByRole("navigation", { name: "视频工作台菜单" });
  await workspaceMenu.getByRole("button", { name: /内容策略/ }).click();
  const contentStrategySubMenu = workspaceMenu.getByLabel("内容策略二级菜单");
  await expect(contentStrategySubMenu.getByRole("button")).toHaveText(["账号策略", "历史生成", "当前选题池"]);
  await contentStrategySubMenu.getByRole("button", { name: "账号策略" }).click();

  const accountPage = page.getByRole("region", { name: "账号策略资料" });
  await expect(accountPage.getByText("内容策略 / 账号策略")).toBeVisible();
  await expect(accountPage.getByRole("button", { name: "返回当前选题池" })).toBeVisible();
  await expect(accountPage.getByRole("button", { name: "AI 生成草稿" })).toHaveCount(0);
  await expect(accountPage.getByText("当前正式策略资料待补齐")).toBeVisible();
  await expect(accountPage.getByRole("region", { name: "基础资料" })).toBeVisible();
  await expect(accountPage.getByRole("region", { name: "结构化策略" })).toBeVisible();
  await expect(accountPage.getByRole("region", { name: "保存后应用到选题链路" })).toBeVisible();
  await expect(accountPage.getByRole("region", { name: "AI 生成策略草稿" })).toBeVisible();
  const accountLayout = await page.locator(".accountStrategyPage").evaluate((element) => {
    const pageRect = element.getBoundingClientRect();
    const workbenchRect = document.querySelector(".workbench")?.getBoundingClientRect();
    return workbenchRect
      ? {
          rightGap: Math.round(workbenchRect.right - pageRect.right),
          width: Math.round(pageRect.width),
          workbenchWidth: Math.round(workbenchRect.width),
        }
      : null;
  });
  expect(accountLayout).not.toBeNull();
  expect(accountLayout!.rightGap).toBeLessThanOrEqual(2);
  expect(accountLayout!.width).toBeGreaterThan(1168);
  const accountPanelGeometry = await page.locator(".accountStrategyBodyGrid").evaluate((element) => {
    const basics = element.querySelector(".strategyBasicsPanel")?.getBoundingClientRect();
    const context = element.querySelector(".strategyContextPanel")?.getBoundingClientRect();
    const structured = element.querySelector(".strategyStructuredPanel")?.getBoundingClientRect();
    const draft = element.querySelector(".accountDraftPanel")?.getBoundingClientRect();
    return basics && context && structured && draft
      ? {
          topDelta: Math.abs(Math.round(structured.top - basics.top)),
          bottomDelta: Math.abs(Math.round(structured.bottom - draft.bottom)),
          basicsToContextGap: Math.round(context.top - basics.bottom),
          contextToDraftGap: Math.round(draft.top - context.bottom),
        }
      : null;
  });
  expect(accountPanelGeometry).not.toBeNull();
  expect(accountPanelGeometry!.topDelta).toBeLessThanOrEqual(2);
  expect(accountPanelGeometry!.bottomDelta).toBeLessThanOrEqual(2);
  expect(accountPanelGeometry!.basicsToContextGap).toBeGreaterThanOrEqual(18);
  expect(accountPanelGeometry!.basicsToContextGap).toBeLessThanOrEqual(22);
  expect(accountPanelGeometry!.contextToDraftGap).toBeGreaterThanOrEqual(18);
  expect(accountPanelGeometry!.contextToDraftGap).toBeLessThanOrEqual(22);
  await accountPage.getByLabel("目标受众").fill(updatedStrategyProfile.target_audience);
  await accountPage.getByLabel("内容支柱").fill(updatedStrategyProfile.content_pillars.join("\n"));
  await accountPage.getByLabel("表达风格").fill(updatedStrategyProfile.tone_style);
  await accountPage.getByLabel("禁区方向").fill(updatedStrategyProfile.forbidden_topics.join("\n"));
  await accountPage.getByLabel("参考账号").fill(updatedStrategyProfile.reference_accounts.join("\n"));
  await accountPage.getByLabel("选题偏好").fill(updatedStrategyProfile.topic_preferences);
  await accountPage.getByRole("button", { name: "保存并应用" }).click();

  const structured = accountPage.getByRole("region", { name: "结构化策略" });
  await expect(structured.getByText("目标受众：内容运营负责人")).toBeVisible();
  await expect(structured.getByText("内容支柱：AI 工具 / 内容生产")).toBeVisible();
  await expect(page.getByLabel("当前账号")).toHaveValue(projectId);

  await contentStrategySubMenu.getByRole("button", { name: "当前选题池" }).click();
  const topicPool = page.getByRole("region", { name: "选题池" });
  await expect(topicPool.getByRole("heading", { name: "账号策略" })).toHaveCount(0);
  await expect(topicPool.getByText("策略资料状态")).toHaveCount(0);
  await expect(topicPool.getByText("账号策略摘要")).toHaveCount(0);
  await expect(topicPool.getByText("内容运营负责人")).toHaveCount(0);
  await expect(topicPool.getByText("表达风格")).toHaveCount(0);
  await expect(topicPool.getByText("选题偏好")).toHaveCount(0);
  await expect(topicPool.getByLabel("账号名称")).toHaveCount(0);
});

test("内容策略当前选题池在超宽桌面保持可读比例", async ({ page }) => {
  await page.setViewportSize({ width: 2552, height: 1308 });
  await mockContentStrategyWorkflow(page);
  await page.goto("/");

  await expect(page.getByRole("region", { name: "选题池" })).toBeVisible();
  const wideLayout = await page.locator(".contentStrategyWorkspace").evaluate((workspace) => {
    const workspaceRect = workspace.getBoundingClientRect();
    const pool = document.querySelector('[aria-label="选题池"]')?.getBoundingClientRect();
    const detail = document.querySelector(".topicDetailColumn")?.getBoundingClientRect();
    const agent = document.querySelector('[aria-label="选题 Agent"]')?.getBoundingClientRect();
    return {
      agentWidth: agent ? Math.round(agent.width) : null,
      detailWidth: detail ? Math.round(detail.width) : null,
      poolWidth: pool ? Math.round(pool.width) : null,
      workspaceWidth: Math.round(workspaceRect.width),
    };
  });

  expect(wideLayout.agentWidth).toBe(360);
  expect(wideLayout.workspaceWidth).toBeLessThanOrEqual(1800);
  expect(wideLayout.poolWidth).not.toBeNull();
  expect(wideLayout.poolWidth!).toBeLessThanOrEqual(1040);
  expect(wideLayout.detailWidth).not.toBeNull();
  expect(wideLayout.detailWidth!).toBeGreaterThanOrEqual(340);
  expect(wideLayout.detailWidth!).toBeLessThanOrEqual(420);

  const overflowingTopicItemChildren = await page.locator(".topicItem").evaluateAll((items) =>
    items.flatMap((item, itemIndex) => {
      const itemRect = item.getBoundingClientRect();
      return Array.from(
        item.querySelectorAll(".topicPoolBadges span, .topicPoolActions button, .topicScore"),
      )
        .map((child) => {
          const childRect = child.getBoundingClientRect();
          return {
            bottomOverflow: Math.round(childRect.bottom - itemRect.bottom),
            itemIndex,
            rightOverflow: Math.round(childRect.right - itemRect.right),
            text: child.textContent?.trim() || "",
          };
        })
        .filter((entry) => entry.bottomOverflow > 0 || entry.rightOverflow > 0);
    }),
  );
  expect(overflowingTopicItemChildren).toEqual([]);
});

test("内容策略历史生成列表页展示批次并限制已成稿选题删除", async ({ page }) => {
  await mockContentStrategyHistoryWorkflow(page);
  await page.goto("/");

  const workspaceMenu = page.getByRole("navigation", { name: "视频工作台菜单" });
  const contentStrategySubMenu = workspaceMenu.getByLabel("内容策略二级菜单");
  await expect(contentStrategySubMenu.getByRole("button")).toHaveText(["账号策略", "历史生成", "当前选题池"]);
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
  const topicGroupPanel = historyPage.getByRole("region", { name: "当前主题选题" });
  await expect(topicGroupPanel.getByRole("region", { name: "优先推荐" })).toHaveCount(0);
  await page.getByRole("button", { name: "评审当前主题组" }).click();
  await expect(topicGroupPanel.getByRole("region", { name: "优先推荐" })).toBeVisible();
  await expect(topicGroupPanel.getByText(topicReviewSnapshot.review_summary)).toBeVisible();
  await expect(topicGroupPanel.getByText("账号定位匹配度高，适合直接进入脚本创作。")).toBeVisible();

  const supplementPanel = page.getByRole("region", { name: "补充选题" });
  await supplementPanel.getByLabel("补充要求").fill(supplementUserMessage.content);
  await supplementPanel.getByRole("button", { name: "补充生成" }).click();
  await expect(historyPage.getByRole("button", { name: /补充上一批 AI 内容流水线选题/ }).first()).toHaveClass(/selected/);
  await expect(topicGroupPanel.getByRole("article", { name: "历史选题：历史批次选题" })).toBeVisible();
  await expect(topicGroupPanel.getByRole("article", { name: "历史选题：补充批次选题" })).toBeVisible();
  await expect(topicGroupPanel.getByText("原始生成", { exact: true })).toBeVisible();
  await expect(topicGroupPanel.getByText("补充生成", { exact: true })).toBeVisible();
  await expect(page.getByRole("region", { name: "关联补充批次" })).toBeVisible();

  await contentStrategySubMenu.getByRole("button", { name: "当前选题池" }).click();
  const topicPool = page.getByRole("region", { name: "选题池" });
  await expect(topicPool.getByRole("region", { name: "优先推荐" })).toBeVisible();
  await expect(topicPool.getByRole("button", { name: /补充批次选题/ })).toBeVisible();
  await expect(topicPool.getByRole("button", { name: /最新批次选题/ })).toHaveCount(0);
  await topicPool.getByRole("button", { name: "查看全部选题" }).click();
  await expect(topicPool.getByText("查看全部选题时不展示主题组评审")).toBeVisible();
  await expect(topicPool.getByRole("region", { name: "优先推荐" })).toHaveCount(0);
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
  expect(filtersTop).toBeLessThanOrEqual(180);
  expect(emptyTop).toBeLessThanOrEqual(260);
  expect(emptyHeight).toBeLessThanOrEqual(150);
});

test("素材库画布展示第一版素材管理闭环", async ({ page }) => {
  await mockMaterialLibraryWorkflow(page);
  await page.goto("/");
  await page.getByRole("navigation", { name: "视频工作台菜单" }).getByRole("button", { name: /素材管理/ }).click();

  await expect(page.getByRole("heading", { name: "素材库" })).toBeVisible();
  const headingInset = await page.locator(".materialLibraryPage").evaluate((pageElement) => {
    const pageRect = pageElement.getBoundingClientRect();
    const headingRect = pageElement.querySelector("h1")?.getBoundingClientRect();
    return headingRect ? Math.round(headingRect.left - pageRect.left) : null;
  });
  expect(headingInset).not.toBeNull();
  expect(headingInset!).toBeGreaterThanOrEqual(16);
  await expect(page.getByLabel("素材画布")).toBeVisible();
  await expect(page.getByLabel("素材资产浮层")).toBeVisible();
  await expect(page.getByLabel("素材详情浮层")).toBeVisible();
  await expect(page.getByLabel("画布工具栏")).toBeVisible();
  await expect(page.locator(".materialCanvasWorkspace canvas")).toBeVisible();
  const canvasCoverage = await page.locator(".materialCanvasWorkspace").evaluate((workspace) => {
    const workspaceRect = workspace.getBoundingClientRect();
    const canvasRect = workspace.querySelector("canvas")?.getBoundingClientRect();
    const detailRect = workspace.querySelector(".materialDetailPanel")?.getBoundingClientRect();
    return canvasRect && detailRect
      ? {
          canvasRightGap: Math.round(workspaceRect.right - canvasRect.right),
          canvasBottomGap: Math.round(workspaceRect.bottom - canvasRect.bottom),
          detailCoveredByCanvas: canvasRect.right >= detailRect.right - 1,
        }
      : null;
  });
  expect(canvasCoverage).not.toBeNull();
  expect(canvasCoverage!.canvasRightGap).toBeLessThanOrEqual(2);
  expect(canvasCoverage!.canvasBottomGap).toBeLessThanOrEqual(2);
  expect(canvasCoverage!.detailCoveredByCanvas).toBe(true);
  await expect(page.getByRole("button", { name: /demo\.vtt/ })).toBeVisible();
  await expect(page.getByText("字幕").first()).toBeVisible();
  await expect(page.getByLabel("素材 URL")).toBeVisible();
  await expect(page.getByText("语义检索")).toHaveCount(0);
  await expect(page.getByText("分镜候选")).toHaveCount(0);
  await expect(page.getByText("素材清单确认")).toHaveCount(0);
});
