import { expect, test, type Page } from "@playwright/test";

const projectId = "11111111-1111-4111-8111-111111111111";
const scriptId = "22222222-2222-4222-8222-222222222222";
const previousTopicBatchId = "77777777-7777-4777-8777-777777777777";
const supplementTopicBatchId = "99999999-9999-4999-8999-999999999901";
const textModelId = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const imageModelId = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const ttsModelId = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
const openAiTtsModelId = "cececece-cece-4cec-8cec-cececececece";
const asrModelId = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
const videoModelId = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";
const silentVideoModelId = "ffffffff-ffff-4fff-8fff-ffffffffffff";

const textModelOption = {
  model_id: textModelId,
  display_name: "默认文本模型",
  model_type: "text",
  provider_name: "OpenAI",
  api_protocol: "openai_responses",
  upstream_model: "gpt-test",
  is_default: true,
};

const imageModelOption = {
  model_id: imageModelId,
  display_name: "默认图片模型",
  model_type: "image",
  provider_name: "OpenAI",
  api_protocol: "openai_images",
  upstream_model: "gpt-image-test",
  is_default: true,
};

const videoModelOptions = [
  {
    model_id: videoModelId,
    display_name: "Seedance 2.0",
    model_type: "video",
    provider_name: "火山引擎",
    api_protocol: "volcengine_ark_video",
    upstream_model: "doubao-seedance-2-0-pro",
    is_default: true,
    capabilities: {
      aspect_ratios: ["16:9", "9:16"],
      resolutions: ["720p", "1080p"],
      min_duration_seconds: 4,
      max_duration_seconds: 15,
      max_reference_images: 9,
      max_prompt_chars: 500,
      generate_audio: true,
    },
  },
  {
    model_id: silentVideoModelId,
    display_name: "静音视频模型",
    model_type: "video",
    provider_name: "测试供应商",
    api_protocol: "volcengine_ark_video",
    upstream_model: "silent-video-test",
    is_default: false,
    capabilities: {
      aspect_ratios: ["1:1"],
      resolutions: ["720p"],
      min_duration_seconds: 4,
      max_duration_seconds: 15,
      max_reference_images: 9,
      max_prompt_chars: 500,
      generate_audio: false,
    },
  },
];

const speechModelOptions = [
  {
    model_id: ttsModelId,
    display_name: "豆包 TTS",
    model_type: "speech",
    provider_name: "火山引擎",
    api_protocol: "volcengine_tts_v3",
    upstream_model: "doubao-seed-tts-2.0",
    is_default: true,
  },
  {
    model_id: openAiTtsModelId,
    display_name: "ZeekAI Seed TTS",
    model_type: "speech",
    provider_name: "ZeekAI",
    api_protocol: "openai_audio_speech",
    upstream_model: "doubao-seed-tts-2.0",
    is_default: false,
  },
  {
    model_id: asrModelId,
    display_name: "豆包 ASR",
    model_type: "speech",
    provider_name: "火山引擎",
    api_protocol: "volcengine_asr_v3",
    upstream_model: "doubao-seed-asr-2.0",
    is_default: true,
  },
];

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
  topic_id: null,
  source_topic_title: null,
  title: "程序员必看：ChatGPT工作流",
  status: "draft",
  scene_count: 6,
  parent_id: null,
  created_at: "2026-07-02T00:05:00Z",
  updated_at: "2026-07-02T00:05:00Z",
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
  topic_id: null,
  source_topic_title: null,
  title: "ChatGPT 工作流新脚本",
  status: "draft",
  scene_count: 3,
  parent_id: null,
  created_at: "2026-07-02T00:12:00Z",
  updated_at: "2026-07-02T00:12:00Z",
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
];

const workGenerationWorkspaceMenus = [
  ...materialWorkspaceMenus.slice(0, 3),
  {
    ...menuNode("production", "作品生产", true, "active", 40),
    children: [
      {
        ...menuNode("work-generation", "作品生成", true, "active", 10),
        agent_key: "work-generation-agent",
        menu_type: "page",
        module_key: "production.work-generation",
      },
    ],
  },
  menuNode("publishing", "发布运营", false, "planned", 50),
  menuNode("analytics", "数据分析", false, "planned", 60),
  menuNode("workflow-tasks", "工作流任务", false, "planned", 70),
];

const workGenerationTaskWorkspaceMenus = workGenerationWorkspaceMenus.map((menu) =>
  menu.menu_key === "production"
    ? {
        ...menu,
        children: [
          ...menu.children,
          {
            ...menuNode("work-generation-task", "生成任务", true, "active", 20),
            agent_key: "work-generation-agent",
            menu_type: "page",
            module_key: "production.work-generation-task",
          },
          {
            ...menuNode("work-library", "作品库", true, "active", 30),
            agent_key: "work-generation-agent",
            menu_type: "page",
            module_key: "production.work-library",
          },
        ],
      }
    : menu,
);

function soundSubtitleMenuNode() {
  return {
    ...menuNode("sound-subtitle-generation", "声音与字幕生成", true, "active", 30),
    agent_key: "sound-generation-agent",
    menu_type: "page",
    module_key: "materials.sound-subtitle-generation",
  };
}

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

const uploadedImageMaterial = {
  ...subtitleMaterial,
  material_id: "cdcdcdcd-cdcd-4cdc-8cdc-cdcdcdcdcdcd",
  material_type: "image",
  file_url: `/assets/uploads/${projectId}/office.png`,
  thumbnail_url: null,
  file_name: "办公桌面近景",
  tags: ["办公", "场景"],
  metadata: {
    source: "user_upload",
    storage_provider: "local",
    mime_type: "image/png",
    format: "png",
    file_size_bytes: 68,
    width: 1,
    height: 1,
  },
  source: "user_upload",
};

const generatedTtsMaterial = {
  ...subtitleMaterial,
  material_id: "edededed-eded-4ded-8ded-edededededed",
  material_type: "audio",
  file_url: `/assets/generated/${projectId}/tts.wav`,
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

const uploadedAudioMaterial = {
  ...generatedTtsMaterial,
  material_id: "fefefefe-fefe-4efe-8efe-fefefefefefe",
  file_url: `/assets/uploads/${projectId}/city-morning.wav`,
  file_name: "城市清晨环境声",
  tags: ["城市", "清晨", "环境声"],
  metadata: { format: "wav", duration_sec: 42, file_size_bytes: 19_503_514 },
  source: "user_upload",
  audio_usage: "ambient",
  work_id: null,
  work_version_id: null,
  generation: null,
};

const png1x1 = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
  "base64",
);

const assetGenerationPayload = {
  model_id: imageModelId,
  image_candidates_per_scene: 3,
  use_reference_materials: true,
};

const assetGenerationPlan = {
  script_id: scriptId,
  scene_count: scriptDetail.scenes.length,
  image_candidate_count: scriptDetail.scenes.length * assetGenerationPayload.image_candidates_per_scene,
  max_image_candidate_count: 48,
  model_id: imageModelId,
  provider: "gpt-image-2",
  reference_material_count: 1,
  can_create: true,
  warnings: [],
};

const imageAssetGenerationTask = {
  task_id: "17171717-1717-4717-8717-171717171717",
  project_id: projectId,
  script_id: scriptId,
  scene_id: null,
  model_id: imageModelId,
  model_snapshot: null,
  provider: "gpt-image-2",
  task_type: "image_candidates",
  status: "pending",
  candidate_count: assetGenerationPlan.image_candidate_count,
  reference_material_ids: ["24242424-2424-4242-8242-242424242424"],
  params: { image_candidates_per_scene: assetGenerationPayload.image_candidates_per_scene },
  result: {},
  error_message: null,
  retry_count: 0,
  dismissed_at: null,
  read_only: false,
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
  read_only: true,
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
    route_path: workspaceRoutePath(menuKey),
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
  await page.route(/\/api\/model-options\?type=(text|image|video|speech)$/, async (route) => {
    const modelType = new URL(route.request().url()).searchParams.get("type");
    await route.fulfill({
      contentType: "application/json",
      json: {
        models: modelType === "text"
          ? [textModelOption]
          : modelType === "image"
            ? [imageModelOption]
            : modelType === "video"
              ? videoModelOptions
              : modelType === "speech" ? speechModelOptions : [],
      },
    });
  });
});

async function mockWorkGenerationWorkflow(page: Page) {
  const workId = "31313131-3131-4131-8131-313131313131";
  const workVersionId = "32323232-3232-4232-8232-323232323232";
  const planIds = [
    "33333333-3333-4333-8333-333333333331",
    "33333333-3333-4333-8333-333333333332",
  ];
  const runId = "34343434-3434-4434-8434-343434343434";
  const workConversationId = "35353535-3535-4535-8535-353535353535";
  const audioMaterialId = "36363636-3636-4636-8636-363636363636";
  const ttsVoiceType = "zh_female_fixture";
  const manifest = {
    script_id: scriptId,
    project_id: projectId,
    script_title: scriptDetail.title,
    script_updated_at: scriptDetail.updated_at,
    input_version: "manifest-v1",
    scenes: scriptDetail.scenes.map((scene, index) => ({
      ...scene,
      visual_description: `${scene.visual_description} 主角在连续镜头中保持服装、面部、光线和空间关系一致，并完整展示当前镜头动作。`,
      candidate_id: `37373737-3737-4737-8737-37373737373${index}`,
      material_id: `38383838-3838-4838-8838-38383838383${index}`,
      file_url: `https://cdn.example.com/work-scene-${index + 1}.png`,
      thumbnail_url: null,
      source_snapshot: { source: "selected_primary" },
    })),
  };
  const planRequests: Array<Record<string, unknown>> = [];
  let confirmCount = 0;

  await page.unroute(/\/api\/video-workspace\/menus$/);
  await page.route(/\/api\/video-workspace\/menus$/, async (route) => {
    await route.fulfill({ contentType: "application/json", json: { menus: workGenerationWorkspaceMenus } });
  });
  await page.route(new RegExp(`/api/projects/${projectId}/scripts(\\?.*)?$`), async (route) => {
    await route.fulfill({ contentType: "application/json", json: { scripts: [scriptSummary], total: 1, limit: 20, offset: 0 } });
  });
  await page.route(new RegExp(`/api/scripts/${scriptId}$`), async (route) => {
    await route.fulfill({ contentType: "application/json", json: scriptDetail });
  });
  await page.route(new RegExp(`/api/scripts/${scriptId}/asset-candidates(?:\\?.*)?$`), async (route) => {
    await route.fulfill({ contentType: "application/json", json: { candidates: [] } });
  });
  await page.route(new RegExp(`/api/scripts/${scriptId}/asset-generation-tasks(?:\\?.*)?$`), async (route) => {
    await route.fulfill({ contentType: "application/json", json: { script_id: scriptId, tasks: [] } });
  });
  await page.route(new RegExp(`/api/scripts/${scriptId}/scene-visual-manifest$`), async (route) => {
    await route.fulfill({ contentType: "application/json", json: manifest });
  });
  await page.route(new RegExp(`/api/scripts/${scriptId}/scene-visual-manifest/validate$`), async (route) => {
    expect(route.request().postDataJSON()).toEqual({ expected_input_version: manifest.input_version });
    await route.fulfill({ contentType: "application/json", json: manifest });
  });
  await page.route(new RegExp(`/api/speech/models/${ttsModelId}/voice-catalog$`), async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: {
        model_id: ttsModelId,
        source_model_id: ttsModelId,
        model_settings: {},
        last_sync: null,
        voices: [{
          voice_id: "39393939-3939-4939-8939-393939393939",
          voice_type: ttsVoiceType,
          resource_id: "seed-tts-2.0",
          name: "灿灿",
          avatar_url: null,
          gender: "female",
          age: "adult",
          categories: [],
          normal_labels: ["清晰"],
          special_labels: [],
          trial_url: null,
          short_trial_url: null,
          languages: [{ Language: "zh-cn", Text: "试听" }],
          emotions: [],
          description: "中文女声",
          is_available: true,
          catalog_version: 1,
          created_at: "2026-07-20T00:00:00Z",
          updated_at: "2026-07-20T00:00:00Z",
        }, {
          voice_id: "39393939-3939-4939-8939-393939393938",
          voice_type: "en_male_alastor",
          resource_id: "seed-tts-2.0",
          name: "Alastor 2.0",
          avatar_url: null,
          gender: "male",
          age: "young",
          categories: [],
          normal_labels: ["侵略性"],
          special_labels: [],
          trial_url: null,
          short_trial_url: null,
          languages: [{ Language: "en", Text: "Preview" }],
          emotions: [],
          description: "声音尖锐，有侵略性",
          is_available: true,
          catalog_version: 1,
          created_at: "2026-07-20T00:00:00Z",
          updated_at: "2026-07-20T00:00:00Z",
        }],
      },
    });
  });
  await page.route(new RegExp(`/api/speech/models/${openAiTtsModelId}/voice-catalog$`), async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: {
        model_id: openAiTtsModelId,
        source_model_id: openAiTtsModelId,
        model_settings: {},
        last_sync: null,
        voices: [{
          voice_id: "39393939-3939-4939-8939-393939393937",
          voice_type: "gateway_female_fixture",
          resource_id: "seed-tts-2.0",
          name: "中转女声",
          avatar_url: null,
          gender: "female",
          age: "adult",
          categories: [],
          normal_labels: [],
          special_labels: [],
          trial_url: null,
          short_trial_url: null,
          languages: [{ Language: "zh-cn", Text: "试听" }],
          emotions: [],
          description: "仅中转模型可用",
          is_available: true,
          catalog_version: 1,
          created_at: "2026-07-20T00:00:00Z",
          updated_at: "2026-07-20T00:00:00Z",
        }],
      },
    });
  });
  await page.route(new RegExp(`/api/projects/${projectId}/materials\\?type=audio&status=active$`), async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: {
        materials: [{
          material_id: audioMaterialId,
          project_id: projectId,
          material_type: "audio",
          file_url: "https://cdn.example.com/background.wav",
          thumbnail_url: null,
          file_name: "科技感背景音乐.wav",
          tags: ["BGM"],
          metadata: {},
          source: "user_upload",
          audio_usage: "bgm",
          work_id: null,
          work_version_id: null,
          generation: null,
          usage_count: 0,
          status: "active",
          created_at: "2026-07-20T00:00:00Z",
          updated_at: "2026-07-20T00:00:00Z",
        }],
      },
    });
  });
  await page.route(/https:\/\/cdn\.example\.com\/.*\.(png|wav)$/, async (route) => {
    await route.fulfill({ status: 200, contentType: "image/png", body: png1x1 });
  });
  await page.route(new RegExp(`/api/scripts/${scriptId}/work-generation/plans$`), async (route) => {
    const request = route.request().postDataJSON() as Record<string, unknown>;
    planRequests.push(request);
    const version = planRequests.length;
    const segmentPrompts = Array.isArray(request.segment_prompts)
      ? request.segment_prompts as string[]
      : ["开场提示词", "收束提示词"];
    await route.fulfill({
      contentType: "application/json",
      json: {
        work_id: workId,
        work_title: scriptDetail.title,
        plan_id: planIds[version - 1],
        work_version_id: workVersionId,
        plan_version: version,
        status: "ready",
        input_fingerprint: `fingerprint-${version}`,
        model_snapshot: {
          llm_model_id: request.llm_model_id,
          video_model_id: request.video_model_id,
          tts_model_id: request.tts_model_id,
        },
        capability_snapshot: {},
        output_snapshot: {},
        prompt_snapshot: {},
        timeline_snapshot: {},
        resource_usage: { video_task_count: 2, video_seconds: 30, tts_characters: 38, asr_seconds: 0 },
        warnings: request.audio_mode === "seedance_original_and_tts" ? ["可能出现双重人声"] : [],
        segments: segmentPrompts.map((prompt, index) => ({ sequence: index + 1, duration_seconds: 15, prompt })),
        can_confirm: true,
        blockers: [],
        created_at: "2026-07-20T00:01:00Z",
      },
    });
  });
  await page.route(new RegExp(`/api/work-generation/plans/${planIds[1]}/confirm$`), async (route) => {
    confirmCount += 1;
    expect(route.request().headers()["idempotency-key"]).toMatch(/^[0-9a-f-]{36}$/);
    await route.fulfill({
      status: 201,
      contentType: "application/json",
      json: { run_id: runId, work_id: workId, work_version_id: workVersionId, work_plan_id: planIds[1], status: "queued", created: true, resource_usage: { video_task_count: 2 } },
    });
  });
  await page.route(/\/api\/agent\/conversations$/, async (route) => {
    expect(route.request().postDataJSON()).toMatchObject({ agent_type: "work", project_id: projectId, subject_type: "work", subject_id: workId });
    await route.fulfill({ contentType: "application/json", json: { ...conversation, conversation_id: workConversationId, agent_type: "work", subject_type: "work", subject_id: workId, title: "作品生成 Agent" } });
  });
  await page.route(new RegExp(`/api/agent/conversations/${workConversationId}/messages$`), async (route) => {
    const content = route.request().postDataJSON().content;
    await route.fulfill({
      contentType: "application/json",
      json: {
        user_message: { ...userMessage, conversation_id: workConversationId, content },
        assistant_message: { ...assistantMessage, conversation_id: workConversationId, content: "已保留角色连续性并更新下一版草稿。" },
        run: { ...agentRun, conversation_id: workConversationId, agent_type: "work" },
      },
    });
  });

  return { audioMaterialId, manifest, planRequests, ttsVoiceType, confirmCount: () => confirmCount };
}

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
    expect(route.request().postDataJSON()).toEqual({ content: userMessage.content, model_id: textModelId });
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
    expect(route.request().postDataJSON()).toEqual({ content: generatedUserMessage.content, model_id: textModelId });
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
  let dismissAssetGenerationTaskRequestCount = 0;
  let selectedCandidateId = selectedPrimaryAssetCandidate.candidate_id;
  let failedTaskDismissed = false;

  await page.route(/https:\/\/cdn\.example\.com\/.*\.png$/, async (route) => {
    await route.fulfill({ status: 200, contentType: "image/png", body: png1x1 });
  });

  await page.route(new RegExp(`/api/scripts/${scriptId}/asset-generation-plan(?:\\?.*)?$`), async (route) => {
    assetGenerationPlanRequestCount += 1;
    expect(route.request().method()).toBe("POST");
    expect(route.request().postDataJSON()).toEqual(assetGenerationPayload);
    await route.fulfill({ contentType: "application/json", json: assetGenerationPlan });
  });
  await page.route(new RegExp(`/api/scripts/${scriptId}/scene-visual-manifest$`), async (route) => {
    await route.fulfill({
      contentType: "application/json",
      status: 409,
      json: {
        error: "主画面清单不完整",
        code: "scene_visual_manifest_incomplete",
        script_id: scriptId,
        blockers: [
          {
            scene_id: scriptDetail.scenes[1].scene_id,
            sequence: scriptDetail.scenes[1].sequence,
            reason: "selected_image_missing",
          },
        ],
      },
    });
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
  return {
    assetGenerationPlanRequestCount: () => assetGenerationPlanRequestCount,
    assetGenerationTasksRequestCount: () => assetGenerationTasksRequestCount,
    assetCandidatesRequestCount: () => assetCandidatesRequestCount,
    selectAssetCandidateRequestCount: () => selectAssetCandidateRequestCount,
    dismissAssetGenerationTaskRequestCount: () => dismissAssetGenerationTaskRequestCount,
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
    publishing: "/publishing",
    analytics: "/analytics",
    "workflow-tasks": "/workflow-tasks",
  }[menuKey] ?? `/${menuKey}`;
}

async function mockContentStrategyWorkflow(page: Page) {
  let generatedFromTopic = false;
  const topicRequest = "本周 AI 工具方向，生成 8 个选题";

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
      model_id: textModelId,
      topic_id: approvedTopic.topic_id,
      style: "knowledge",
      scene_count: 6,
    });
    generatedFromTopic = true;
    await route.fulfill({ contentType: "application/json", json: topicScriptDetail });
  });
  await page.route(/\/api\/agent\/conversations$/, async (route) => {
    expect(route.request().postDataJSON()).toMatchObject({ project_id: projectId, agent_type: "topic" });
    await route.fulfill({ contentType: "application/json", json: topicConversation });
  });
  await page.route(new RegExp(`/api/agent/conversations/${topicConversationId}/messages$`), async (route) => {
    if (route.request().method() === "GET") {
      await route.fulfill({ contentType: "application/json", json: { messages: [] } });
      return;
    }
    expect(route.request().postDataJSON()).toEqual({ content: topicRequest, model_id: textModelId });
    await route.fulfill({
      contentType: "application/json",
      json: {
        user_message: { ...supplementUserMessage, content: topicRequest },
        assistant_message: {
          ...supplementAssistantMessage,
          content: "已生成 8 个候选选题。",
          metadata: { ...supplementAssistantMessage.metadata, batch_id: latestTopicBatch.batch_id },
        },
        run: supplementAgentRun,
      },
    });
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
  await page.route(new RegExp(`/api/projects/${projectId}/strategy-profile/draft$`), async (route) => {
    expect(route.request().method()).toBe("POST");
    expect(route.request().postDataJSON()).toEqual({
      direction_notes: "面向内容运营负责人",
      model_id: textModelId,
    });
    await route.fulfill({
      contentType: "application/json",
      json: { draft: updatedStrategyProfile, draft_summary: "已按补充方向生成策略草稿。" },
    });
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
    expect(route.request().postDataJSON()).toEqual({ model_id: textModelId });
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
      model_id: textModelId,
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

async function mockMaterialLibraryWorkflow(
  page: Page,
  options: {
    initialMaterials?: Array<Record<string, unknown>>;
    uploadResponse?: Record<string, unknown>;
    onUpload?: (postData: string) => void;
  } = {},
) {
  const uploadResponse = options.uploadResponse || uploadedImageMaterial;
  const materials: Array<Record<string, unknown>> = [...(options.initialMaterials || [])];
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
  await page.route(new RegExp(`/api/projects/${projectId}/materials/upload$`), async (route) => {
    expect(route.request().method()).toBe("POST");
    expect(route.request().headers()["content-type"]).toContain("multipart/form-data; boundary=");
    options.onUpload?.(route.request().postData() || "");
    materials.splice(0, materials.length, uploadResponse);
    await route.fulfill({ status: 201, contentType: "application/json", json: uploadResponse });
  });
  await page.route(new RegExp(`/api/projects/${projectId}/materials(\\?.*)?$`), async (route) => {
    await route.fulfill({ contentType: "application/json", json: { materials } });
  });
  await page.route(/\/assets\/uploads\/.*\/office\.png$/, async (route) => {
    await route.fulfill({ status: 200, contentType: "image/png", body: png1x1 });
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
  await expect(page.getByRole("region", { name: "画面生成图片候选" })).toHaveCount(0);

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

test("工作台深层路由在刷新和浏览器前进后退后保持当前页面", async ({ page }) => {
  await page.goto("/materials/sound-subtitle-generation");

  const workspaceMenu = page.getByRole("navigation", { name: "视频工作台菜单" });
  await expect(page.getByRole("heading", { name: "声音与字幕生成" })).toBeVisible();
  await expect(workspaceMenu.getByRole("button", { name: /素材管理/ })).toHaveClass(/active/);
  await expect(workspaceMenu.getByRole("button", { name: "声音与字幕生成" })).toHaveClass(/active/);
  await expect(page).toHaveURL(/\/materials\/sound-subtitle-generation$/);

  await page.reload();
  await expect(page.getByRole("heading", { name: "声音与字幕生成" })).toBeVisible();
  await expect(page).toHaveURL(/\/materials\/sound-subtitle-generation$/);

  await workspaceMenu.getByRole("button", { name: "素材库" }).click();
  await expect(page).toHaveURL(/\/materials\/library$/);
  await expect(page.getByRole("heading", { name: "素材库" })).toBeVisible();

  await page.goBack();
  await expect(page).toHaveURL(/\/materials\/sound-subtitle-generation$/);
  await expect(page.getByRole("heading", { name: "声音与字幕生成" })).toBeVisible();

  await page.goForward();
  await expect(page).toHaveURL(/\/materials\/library$/);
  await expect(page.getByRole("heading", { name: "素材库" })).toBeVisible();
});

test("画面生成页支持生成、预览和选择主画面且不提供旧视频操作", async ({ page }) => {
  await mockExistingScriptWorkflow(page);
  const assetWorkflow = await mockScriptAssetWorkflow(page);
  await page.goto("/");
  const workspaceMenu = page.getByRole("navigation", { name: "视频工作台菜单" });
  await workspaceMenu.getByRole("button", { name: /素材管理/ }).click();
  await expect(workspaceMenu.getByLabel("素材管理二级菜单").getByRole("button"))
    .toHaveText(["素材库", "画面生成", "声音与字幕生成"]);
  await workspaceMenu.getByRole("button", { name: "画面生成" }).click();

  await expect(page.getByRole("heading", { name: "画面生成" })).toBeVisible();
  const panel = await page.getByRole("region", { name: "画面生成图片候选" });
  await expect(panel).toBeVisible();
  await expect(panel.getByText("分镜列表")).toBeVisible();
  await expect(panel.getByText("候选素材")).toBeVisible();
  await expect(panel.getByText("生成设置与任务")).toBeVisible();
  await expect(panel.getByRole("combobox", { name: "图片模型" })).toHaveValue(imageModelId);
  await expect(panel.getByText("2 分镜 × 3 = 6 张图片候选")).toBeVisible();
  await expect(panel.getByText("单次最多 48 张")).toBeVisible();
  await expect(panel.getByText("当前主素材")).toBeVisible();
  await expect(panel.getByRole("heading", { name: "当前主素材" })).toHaveCount(0);
  const existingCandidateSection = panel.getByRole("region", { name: "旧素材候选" });
  await expect(existingCandidateSection.locator("article.selected"))
    .toContainText("current-primary.png");
  await expect(panel.getByText("AI 图片候选")).toBeVisible();
  await expect(panel.getByText("历史逐分镜视频任务")).toBeVisible();
  await expect(panel.getByText("只读审计")).toBeVisible();
  await expect(panel.getByRole("button", { name: "确认生成视频" })).toHaveCount(0);
  await expect(panel.getByRole("button", { name: "进入作品生成" })).toBeDisabled();
  await expect(panel.getByText("还缺 1 个主画面")).toBeVisible();
  await expect(panel.getByText(scriptDetail.scenes[0].narration)).toBeVisible();
  await expect(panel.getByText(scriptDetail.scenes[0].visual_description)).toBeVisible();
  const visualColumns = await panel.locator(".assetCandidateGrid").evaluate((grid) => {
    const gridRect = grid.getBoundingClientRect();
    return {
      grid: { left: gridRect.left, right: gridRect.right },
      columns: Array.from(grid.children).map((column) => {
        const rect = column.getBoundingClientRect();
        return { left: rect.left, right: rect.right };
      }),
    };
  });
  expect(visualColumns.columns).toHaveLength(3);
  expect(visualColumns.columns[0].left).toBeGreaterThanOrEqual(visualColumns.grid.left);
  expect(visualColumns.columns[1].left).toBeGreaterThanOrEqual(visualColumns.columns[0].right);
  expect(visualColumns.columns[2].left).toBeGreaterThanOrEqual(visualColumns.columns[1].right);
  expect(visualColumns.columns[2].right).toBeLessThanOrEqual(visualColumns.grid.right);
  await expect(panel.getByRole("button", { name: "生成图片候选" })).toHaveCount(0);
  await expect.poll(() => assetWorkflow.assetGenerationPlanRequestCount()).toBeGreaterThan(0);
  await expect.poll(() => assetWorkflow.assetCandidatesRequestCount()).toBeGreaterThan(0);

  await page.getByRole("button", { name: "生成图片候选" }).click();
  await expect.poll(() => assetWorkflow.assetGenerationTasksRequestCount()).toBe(1);
  await expect(panel.getByText("scene-1.png")).toBeVisible();
  await expect(panel.getByRole("button", { name: "选择为主素材" }).first()).toBeVisible();

  const previewTrigger = panel.getByRole("button", { name: "查看scene-1.png大图" });
  await expect(previewTrigger).toBeVisible();
  await expect(panel.getByRole("button", { name: "查看current-primary.png大图" })).toHaveCount(0);
  await previewTrigger.click();
  const imageDialog = page.getByRole("dialog", { name: "图片大图预览" });
  await expect(imageDialog).toBeVisible();
  await expect(imageDialog.getByText("scene-1.png")).toBeVisible();
  await expect(imageDialog.getByText("AI 生成图片候选")).toBeVisible();
  await imageDialog.getByRole("button", { name: "放大图片" }).click();
  await expect(imageDialog.getByText("125%")).toBeVisible();
  await imageDialog.getByRole("button", { name: "关闭大图预览" }).click();
  await expect(imageDialog).toHaveCount(0);
  await expect(previewTrigger).toBeFocused();

  await panel.getByRole("button", { name: "选择为主素材" }).first().click();
  await expect.poll(() => assetWorkflow.selectAssetCandidateRequestCount()).toBe(1);
  await expect(panel.getByText("当前主素材")).toBeVisible();
  await expect(panel.locator(".assetCurrentCandidateSummary")).toContainText("scene-1.png");
  const aiCandidateSection = panel.getByRole("region", { name: "AI 图片候选" });
  await expect(aiCandidateSection.locator("article.selected")).toContainText("scene-1.png");
  await expect(previewTrigger).toBeVisible();
  await previewTrigger.click();
  await expect(imageDialog).toBeVisible();
  await expect(imageDialog.getByText("scene-1.png")).toBeVisible();
  await imageDialog.getByRole("button", { name: "关闭大图预览" }).click();

  await panel.getByRole("button", { name: "清理失败任务" }).click();
  const dismissDialog = page.getByRole("dialog", { name: "清理失败任务？" });
  await expect(dismissDialog).toBeVisible();
  await expect(dismissDialog.getByText(/不会重新调用供应商，也不会产生额外费用/)).toBeVisible();
  await dismissDialog.getByRole("button", { name: "确认清理" }).click();
  await expect.poll(() => assetWorkflow.dismissAssetGenerationTaskRequestCount()).toBe(1);
  await expect(panel.getByRole("button", { name: "清理失败任务" })).toHaveCount(0);
  await expect(dismissDialog).toHaveCount(0);
});

test("作品生成从完整主画面一次确认、按能力重规划并保持幂等", async ({ page }) => {
  await page.setViewportSize({ width: 1920, height: 952 });
  const workflow = await mockWorkGenerationWorkflow(page);
  await page.goto("/materials/generation");

  const workspaceMenu = page.getByRole("navigation", { name: "视频工作台菜单" });
  await workspaceMenu.getByRole("button", { name: /素材管理/ }).click();
  await workspaceMenu.getByRole("button", { name: "画面生成" }).click();
  const assetPanel = page.getByRole("region", { name: "画面生成图片候选" });
  await expect(assetPanel.getByText("还缺")).toHaveCount(0);
  const enterButton = assetPanel.getByRole("button", { name: "进入作品生成" });
  await expect(enterButton).toBeEnabled();
  await enterButton.click();

  await expect(page).toHaveURL(/\/production\/generation$/);
  await expect(page.getByRole("heading", { name: scriptDetail.title })).toBeVisible();
  await expect(page.getByText("一次确认创建一部作品")).toBeVisible();
  await expect(page.getByRole("heading", { name: "作品 Agent" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "作品计划预览" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "参数确认" })).toBeVisible();
  await expect(page.getByText("生成流程")).toBeVisible();
  await expect(page.getByText("主画面", { exact: true })).toBeVisible();
  await expect(page.getByLabel("全片提示词")).toBeVisible();
  await expect(page.getByText("资源用量", { exact: true })).toBeVisible();
  await expect(page.getByText("第 1 镜")).toBeVisible();
  await expect(page.getByText("第 2 镜")).toBeVisible();

  const agentPanel = page.getByRole("region", { name: "作品 Agent" });
  const planPanel = page.getByRole("region", { name: "作品计划预览" });
  const paramsPanel = page.getByRole("region", { name: "参数确认" });
  const generationFontSizes = await page.evaluate(() => {
    const fontSize = (selector: string) => {
      const element = document.querySelector(selector);
      return element ? getComputedStyle(element).fontSize : null;
    };
    return {
      panelTitle: fontSize(".workGenerationPanelHeader h3"),
      eyebrow: fontSize(".workGenerationEyebrow"),
      binding: fontSize(".workGenerationBinding dt"),
      message: fontSize(".workGenerationMessages p"),
      auditMeta: fontSize(".workGenerationAudit small"),
      sceneDescription: fontSize(".workGenerationScenes p"),
      prompt: fontSize(".workGenerationPromptField textarea"),
      formLabel: fontSize(".workGenerationParamsSection > label"),
    };
  });
  expect(generationFontSizes).toEqual({
    panelTitle: "16px",
    eyebrow: "12px",
    binding: "13px",
    message: "13px",
    auditMeta: "12px",
    sceneDescription: "12px",
    prompt: "13px",
    formLabel: "13px",
  });
  const clippedSceneDescriptions = await planPanel.locator(".workGenerationScenes p").evaluateAll((nodes) =>
    nodes
      .map((node, index) => ({
        index,
        clientHeight: node.clientHeight,
        scrollHeight: node.scrollHeight,
      }))
      .filter((metrics) => metrics.scrollHeight > metrics.clientHeight),
  );
  expect(clippedSceneDescriptions).toEqual([]);
  const [agentBox, planBox, paramsBox] = await Promise.all([
    agentPanel.boundingBox(),
    planPanel.boundingBox(),
    paramsPanel.boundingBox(),
  ]);
  expect(agentBox).not.toBeNull();
  expect(planBox).not.toBeNull();
  expect(paramsBox).not.toBeNull();
  expect(Math.round(agentBox!.width)).toBe(330);
  expect(planBox!.width).toBeGreaterThan(700);
  expect(Math.round(paramsBox!.width)).toBe(436);
  expect(agentBox!.x + agentBox!.width).toBeLessThan(planBox!.x);
  expect(planBox!.x + planBox!.width).toBeLessThan(paramsBox!.x);
  const viewportMetrics = await page.evaluate(() => ({
    clientHeight: document.documentElement.clientHeight,
    clientWidth: document.documentElement.clientWidth,
    scrollHeight: document.documentElement.scrollHeight,
    scrollWidth: document.documentElement.scrollWidth,
    topbarHeight: document.querySelector(".topbar")?.getBoundingClientRect().height,
    workspaceHeight: document.querySelector(".workGenerationWorkspace")?.getBoundingClientRect().height,
  }));
  expect(viewportMetrics.scrollWidth).toBeLessThanOrEqual(viewportMetrics.clientWidth);
  expect(viewportMetrics.scrollHeight, JSON.stringify(viewportMetrics)).toBeLessThanOrEqual(viewportMetrics.clientHeight);
  expect(viewportMetrics.workspaceHeight).toBe(viewportMetrics.clientHeight - Number(viewportMetrics.topbarHeight));
  const fullHeightLayout = await page.evaluate(() => {
    const panels = Array.from(document.querySelectorAll<HTMLElement>(".workGenerationPanel"));
    const planScroll = document.querySelector<HTMLElement>(".workGenerationPlanScroll");
    const paramsScroll = document.querySelector<HTMLElement>(".workGenerationParamsScroll");
    const paramsPanel = document.querySelector<HTMLElement>(".workGenerationParamsPanel");
    const paramsActions = document.querySelector<HTMLElement>(".workGenerationActions");
    const rail = document.querySelector<HTMLElement>(".agentRail");
    return {
      windowScrollY: window.scrollY,
      panelBottomGaps: panels.map((panel) => Math.round(window.innerHeight - panel.getBoundingClientRect().bottom)),
      panelHeights: panels.map((panel) => Math.round(panel.getBoundingClientRect().height)),
      planCanScrollInternally: Boolean(planScroll && planScroll.scrollHeight > planScroll.clientHeight),
      paramsCanScrollInternally: Boolean(paramsScroll && paramsScroll.scrollHeight > paramsScroll.clientHeight),
      paramsActionBottomGap: paramsPanel && paramsActions
        ? Math.round(paramsPanel.getBoundingClientRect().bottom - paramsActions.getBoundingClientRect().bottom)
        : null,
      railOverflowY: rail ? getComputedStyle(rail).overflowY : null,
    };
  });
  expect(fullHeightLayout.windowScrollY).toBe(0);
  expect(fullHeightLayout.panelBottomGaps.every((gap) => gap >= 20 && gap <= 24)).toBe(true);
  expect(new Set(fullHeightLayout.panelHeights).size).toBe(1);
  expect(fullHeightLayout.planCanScrollInternally).toBe(true);
  expect(fullHeightLayout.paramsCanScrollInternally).toBe(true);
  expect(fullHeightLayout.paramsActionBottomGap).toBeLessThanOrEqual(1);
  expect(fullHeightLayout.railOverflowY).toBe("auto");

  const llm = page.getByRole("combobox", { name: "方案 LLM" });
  const video = page.getByRole("combobox", { name: "视频模型" });
  const tts = page.getByRole("combobox", { name: "TTS 模型" });
  await expect(llm).toHaveValue(textModelId);
  await expect(video).toHaveValue(videoModelId);
  await expect(tts).toHaveValue(ttsModelId);
  const voice = page.getByRole("combobox", { name: "音色" });
  await expect(voice).toContainText("灿灿");
  await expect(page.getByRole("combobox", { name: "画面比例" })).toHaveValue("16:9");
  await expect(page.getByRole("combobox", { name: "分辨率" })).toHaveValue("1080p");
  await expect(page.getByRole("checkbox", { name: "烧录字幕" })).toBeChecked();
  await expect(page.getByLabel("已有音频素材").getByText("科技感背景音乐.wav")).toBeVisible();

  const selectMetrics = await paramsPanel.locator(".workspaceSelectField").evaluateAll((fields) => fields.map((field) => {
    const select = field.querySelector("select");
    const chevron = field.querySelector(".workspaceSelectChevron");
    if (!select || !chevron) return null;
    const style = getComputedStyle(select);
    return {
      height: Math.round(select.getBoundingClientRect().height),
      radius: style.borderRadius,
      fontSize: style.fontSize,
      background: style.backgroundColor,
      chevronVisible: getComputedStyle(chevron).display !== "none",
    };
  }));
  expect(selectMetrics.length).toBeGreaterThanOrEqual(7);
  expect(selectMetrics.every((metric) => metric && metric.height === 36 && metric.radius === "6px" && metric.fontSize === "13px" && metric.background === "rgb(255, 255, 255)" && metric.chevronVisible)).toBe(true);

  await voice.click();
  const voicePopover = page.locator(".voiceCatalogPopover");
  const voiceListbox = page.getByRole("listbox", { name: "可用音色" });
  await expect(voicePopover).toBeVisible();
  expect(Math.round((await voicePopover.boundingBox())!.width)).toBe(650);
  await expect(page.getByText("2 个可用")).toBeVisible();
  await page.getByRole("group", { name: "按语言筛选音色" }).getByRole("button", { name: "英文" }).click();
  await page.getByRole("group", { name: "按声线筛选音色" }).getByRole("button", { name: "男声" }).click();
  await page.getByRole("searchbox", { name: "搜索音色" }).fill("侵略性");
  await expect(voiceListbox.getByRole("option", { name: /Alastor 2\.0.*声音尖锐.*男.*青年.*英语/ })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(voicePopover).toHaveCount(0);

  await tts.selectOption(openAiTtsModelId);
  await expect(voice).toContainText("灿灿（已失效）");
  await expect(voice).toHaveAttribute("aria-invalid", "true");
  await page.getByRole("button", { name: "生成计划" }).click();
  await expect(page.locator(".errorText[role='alert']")).toHaveText("当前音色不适用于所选 TTS 模型，请重新选择");
  expect(workflow.planRequests).toHaveLength(0);
  await tts.selectOption(ttsModelId);
  await expect(voice).toContainText("灿灿");
  await expect(voice).not.toHaveAttribute("aria-invalid", "true");

  await page.getByRole("combobox", { name: "声音模式" }).selectOption("seedance_original_and_tts");
  await page.getByLabel("已有音频素材").getByRole("checkbox", { name: /科技感背景音乐/ }).check();
  await page.getByLabel("全片提示词").fill("统一角色、空间和光线连续性，开场快速进入主题。");
  await page.getByRole("button", { name: "生成计划" }).click();

  await expect(page.getByText("计划版本 1")).toBeVisible();
  await expect(page.getByText("2", { exact: true }).first()).toBeVisible();
  await expect(page.getByText("30 秒", { exact: true }).first()).toBeVisible();
  await expect(page.getByText("38", { exact: true })).toBeVisible();
  await expect(page.getByText("可能出现双重人声")).toBeVisible();
  await expect(page.getByLabel("第 1 段提示词")).toHaveValue("开场提示词");
  await expect(page.getByRole("button", { name: "确认生成作品" })).toBeEnabled();

  await page.getByLabel("第 1 段提示词").fill("重写后的开场提示词");
  await expect(page.getByRole("button", { name: "确认生成作品" })).toBeDisabled();
  await expect(page.getByText("计划已过期，请重新生成计划")).toBeVisible();

  await video.selectOption(silentVideoModelId);
  await expect(page.getByRole("combobox", { name: "声音模式" }).locator("option")).toHaveText(["独立 TTS"]);
  await expect(page.getByRole("combobox", { name: "画面比例" })).toHaveValue("1:1");
  await expect(page.getByRole("combobox", { name: "分辨率" })).toHaveValue("720p");
  await page.getByRole("combobox", { name: "成片时长" }).selectOption("custom");
  await page.getByLabel("自定义时长（秒）").fill("30");
  await page.getByRole("checkbox", { name: "烧录字幕" }).uncheck();
  await page.getByRole("button", { name: "重新生成计划" }).click();

  await expect(page.getByText("计划版本 2")).toBeVisible();
  expect(workflow.planRequests[1]).toMatchObject({
    video_model_id: silentVideoModelId,
    duration_strategy: "custom",
    duration_seconds: 30,
    aspect_ratio: "1:1",
    resolution: "720p",
    audio_mode: "independent_tts",
    tts_voice_type: workflow.ttsVoiceType,
    audio_material_ids: [workflow.audioMaterialId],
    burn_subtitles: false,
    segment_prompts: ["重写后的开场提示词", "收束提示词"],
  });

  const agentInput = page.getByLabel("作品 Agent 消息");
  await agentInput.fill("保持角色连续性，但把结尾收束更明确。");
  await page.getByRole("button", { name: "发送" }).click();
  await expect(page.getByText("已保留角色连续性并更新下一版草稿。")).toBeVisible();

  await expect(page.getByText(/费用|价格|金额|币种|预算|成本/)).toHaveCount(0);
  await page.getByRole("button", { name: "确认生成作品" }).click();
  await expect(page.getByText("生成中", { exact: true })).toBeVisible();
  await expect.poll(() => workflow.confirmCount()).toBe(1);

  await page.setViewportSize({ width: 1440, height: 980 });
  const compactGenerationLayout = await page.evaluate(() => {
    const workspace = document.querySelector<HTMLElement>(".workGenerationWorkspace");
    const panels = Array.from(document.querySelectorAll<HTMLElement>(".workGenerationPanel"));
    const flowLabels = Array.from(document.querySelectorAll<HTMLElement>(".workGenerationFlow strong, .workGenerationFlow small"));
    const lineCount = (element: HTMLElement) => {
      const range = document.createRange();
      range.selectNodeContents(element);
      return range.getClientRects().length;
    };
    return {
      workspaceOverflow: workspace ? workspace.scrollWidth > workspace.clientWidth : null,
      panelOverflow: panels.filter((panel) => panel.scrollWidth > panel.clientWidth).map((panel) => panel.className),
      wrappedFlowLabels: flowLabels.filter((label) => lineCount(label) > 1).map((label) => label.innerText),
    };
  });
  expect(compactGenerationLayout).toEqual({ workspaceOverflow: false, panelOverflow: [], wrappedFlowLabels: [] });
});

test("生成任务页保留完整菜单并与作品生成页共享工作台骨架", async ({ page }) => {
  await page.setViewportSize({ width: 1920, height: 980 });
  await mockWorkGenerationWorkflow(page);
  await page.unroute(/\/api\/video-workspace\/menus$/);
  await page.route(/\/api\/video-workspace\/menus$/, async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: { menus: workGenerationTaskWorkspaceMenus },
    });
  });
  const task = {
    id: "90909090-9090-4090-8090-909090909090",
    work_id: "91919191-9191-4191-8191-919191919191",
    work_version_id: "92929292-9292-4292-8292-929292929292",
    work_plan_id: "93939393-9393-4393-8393-939393939393",
    title: "夏日防晒知识短片",
    version_no: 2,
    status: "running",
    current_stage: "video_segment",
    progress_percent: 40,
    successful_steps: 2,
    running_steps: 1,
    queued_steps: 2,
    failed_steps: 0,
    can_cancel: false,
    cancel_mode: "provider",
    cancel_block_reason: "当前 provider 不支持运行中取消，任务仍需等待上游终态",
    resource_usage: { video_task_count: 2, video_seconds: 30 },
    error_category: null,
    error_summary: null,
    created_at: "2026-07-20T01:00:00Z",
    updated_at: "2026-07-20T01:01:00Z",
    dismissed_at: null,
  };
  const cancelledTask = {
    ...task,
    id: "95959595-9595-4595-8595-959595959595",
    work_version_id: "96969696-9696-4696-8696-969696969696",
    work_plan_id: "97979797-9797-4797-8797-979797979797",
    version_no: 1,
    status: "cancelled",
    current_stage: "cancelled",
    progress_percent: 16,
    successful_steps: 1,
    running_steps: 0,
    queued_steps: 0,
    cancel_mode: "none",
    cancel_block_reason: null,
  };
  const taskCounts = { pending: 0, running: 1, completed: 0, attention: 0, cancelled: 1, total: 2 };
  await page.route(new RegExp(`/api/projects/${projectId}/work-generation/tasks(?:\\?.*)?$`), async (route) => {
    const view = new URL(route.request().url()).searchParams.get("view");
    const tasks = view === "pending" ? [] : view === "cancelled" ? [cancelledTask] : [task];
    await route.fulfill({ contentType: "application/json", json: { tasks, counts: taskCounts } });
  });
  await page.route(new RegExp(`/api/work-generation/runs/${task.id}$`), async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: {
        task,
        steps: [{
          id: "94949494-9494-4494-8494-949494949494",
          step_no: 1,
          step_type: "video_segment",
          status: "running",
          is_required: true,
          depends_on: [],
          model_snapshot: { display_name: "Fake Video" },
          resource_usage: { video_seconds: 15 },
          result_material_ids: [],
          external_task_id: "fake-upstream",
          error_category: null,
          error_code: null,
          error_summary: null,
          attempts: [],
        }],
      },
    });
  });
  await page.route(new RegExp(`/api/work-generation/runs/${cancelledTask.id}$`), async (route) => {
    await route.fulfill({ contentType: "application/json", json: { task: cancelledTask, steps: [] } });
  });

  await page.goto("/production/tasks");
  await expect(page.getByRole("heading", { name: "生成任务" })).toBeVisible();
  await expect(page.getByText("共 2 个任务")).toBeVisible();
  await expect(page.getByText(task.title).first()).toBeVisible();
  await expect(page.getByText("选择一个运行")).toBeVisible();
  const taskRow = page.getByRole("button", { name: new RegExp(`${task.title}.*查看`) });
  await expect(taskRow).toHaveAttribute("aria-pressed", "false");
  await taskRow.click();
  await expect(page.getByRole("button", { name: new RegExp(`${task.title}.*查看中`) })).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByRole("heading", { name: `${task.title} · V${task.version_no}` })).toBeVisible();
  await expect(page).toHaveURL(new RegExp(`run_id=${task.id}`));
  await expect(page.getByText(task.cancel_block_reason)).toBeVisible();
  await expect(page.getByRole("button", { name: "取消运行" })).toHaveCount(0);
  const taskFontSizes = await page.evaluate(() => {
    const fontSize = (selector: string) => {
      const element = document.querySelector(selector);
      return element ? getComputedStyle(element).fontSize : null;
    };
    return {
      listTitle: fontSize(".workGenerationTaskTableHeader strong"),
      listMeta: fontSize(".workGenerationTaskTableHeader small"),
      tab: fontSize(".workGenerationTaskTabs button"),
      tableHead: fontSize(".workGenerationTaskTableRow.head"),
      taskTitle: fontSize(".workGenerationTaskTableRow:not(.head) > span > strong"),
      taskMeta: fontSize(".workGenerationTaskTableRow:not(.head) > span > small"),
      detailTitle: fontSize(".workGenerationTaskDetailHeader h3"),
      detailMeta: fontSize(".workGenerationTaskDetailHeader small"),
      stepTitle: fontSize(".workGenerationTaskDetailStep strong"),
      stepMeta: fontSize(".workGenerationTaskDetailStep small"),
    };
  });
  expect(taskFontSizes).toEqual({
    listTitle: "16px",
    listMeta: "12px",
    tab: "13px",
    tableHead: "12px",
    taskTitle: "13px",
    taskMeta: "12px",
    detailTitle: "16px",
    detailMeta: "12px",
    stepTitle: "13px",
    stepMeta: "12px",
  });

  const workspaceMenu = page.getByRole("navigation", { name: "视频工作台菜单" });
  for (const label of [
    "内容策略",
    "脚本创作",
    "素材管理",
    "作品生产",
    "发布运营",
    "数据分析",
    "工作流任务",
  ]) {
    await expect(workspaceMenu.getByRole("button", { name: new RegExp(label) })).toBeVisible();
  }
  await expect(workspaceMenu.getByRole("button", { name: "生成任务" })).toHaveClass(/active/);

  const readShellGeometry = (workspaceSelector: string, headerSelector: string) => page.evaluate(
    ({ workspaceSelector: workspace, headerSelector: header }) => {
      const rect = (selector: string) => {
        const element = document.querySelector(selector);
        if (!element) return null;
        const box = element.getBoundingClientRect();
        return {
          x: Math.round(box.x),
          y: Math.round(box.y),
          width: Math.round(box.width),
          height: Math.round(box.height),
        };
      };
      return {
        shellClass: document.querySelector(".workspaceShell")?.className,
        rail: rect(".agentRail"),
        brand: rect(".brandBlock"),
        topbar: rect(".topbar"),
        workspace: rect(workspace),
        header: rect(header),
      };
    },
    { workspaceSelector, headerSelector },
  );

  const taskGeometry = await readShellGeometry(".workGenerationTasksWorkspace", ".workGenerationTasksHeader");
  const overflowingTaskPanels = await page.locator([
    ".workGenerationTaskTablePanel",
    ".workGenerationTaskListToolbar",
    ".workGenerationTaskTable",
    ".workGenerationTaskDetailPanel",
  ].join(", ")).evaluateAll((elements) => elements
    .filter((element) => element.scrollWidth > element.clientWidth)
    .map((element) => element.className));
  expect(overflowingTaskPanels).toEqual([]);
  const [railBox, lastMenuBox] = await Promise.all([
    page.locator(".workspaceShell > .agentRail").boundingBox(),
    workspaceMenu.getByRole("button", { name: /工作流任务/ }).boundingBox(),
  ]);
  expect(taskGeometry.shellClass).toBe("workspaceShell");
  expect(railBox).not.toBeNull();
  expect(lastMenuBox).not.toBeNull();
  expect(lastMenuBox!.y + lastMenuBox!.height).toBeLessThanOrEqual(railBox!.y + railBox!.height);

  await page.getByRole("tab", { name: /未生成/ }).click();
  await expect(page.getByText("暂无任务")).toBeVisible();
  const [emptyTableBox, emptyFooterBox] = await Promise.all([
    page.locator(".workGenerationTaskTable").boundingBox(),
    page.locator(".workGenerationTaskTableFooter").boundingBox(),
  ]);
  expect(emptyTableBox).not.toBeNull();
  expect(emptyFooterBox).not.toBeNull();
  expect(Math.abs(
    (emptyTableBox!.y + emptyTableBox!.height) - (emptyFooterBox!.y + emptyFooterBox!.height),
  )).toBeLessThanOrEqual(1);

  await page.getByRole("button", { name: /更多筛选/ }).click();
  await page.getByRole("combobox", { name: "特殊状态" }).selectOption("cancelled");
  await expect(page.getByText(cancelledTask.title).first()).toBeVisible();
  await expect(page.getByText("当前显示：已取消 1 个运行")).toBeVisible();

  await workspaceMenu.getByRole("button", { name: "作品生成" }).click();
  await expect(page).toHaveURL(/\/production\/generation$/);
  await expect(page.getByRole("heading", { name: scriptDetail.title })).toBeVisible();
  const generationGeometry = await readShellGeometry(".workGenerationWorkspace", ".workGenerationHeader");

  expect(generationGeometry.shellClass).toBe("workspaceShell");
  expect(taskGeometry.rail).toEqual(generationGeometry.rail);
  expect(taskGeometry.brand).toEqual(generationGeometry.brand);
  expect(taskGeometry.topbar).toEqual(generationGeometry.topbar);
  expect(taskGeometry.workspace).toEqual(generationGeometry.workspace);
  expect(taskGeometry.header).not.toBeNull();
  expect(generationGeometry.header).not.toBeNull();
  expect({
    x: taskGeometry.header!.x,
    y: taskGeometry.header!.y,
    width: taskGeometry.header!.width,
  }).toEqual({
    x: generationGeometry.header!.x,
    y: generationGeometry.header!.y,
    width: generationGeometry.header!.width,
  });
  expect(Math.abs(taskGeometry.header!.height - generationGeometry.header!.height)).toBeLessThanOrEqual(1);

  await page.setViewportSize({ width: 1440, height: 980 });
  await workspaceMenu.getByRole("button", { name: "生成任务" }).click();
  await expect(page).toHaveURL(/\/production\/tasks$/);
  await expect(page.getByText(task.title).first()).toBeVisible();
  const compactTaskLayout = await page.evaluate(() => {
    const toolbar = document.querySelector<HTMLElement>(".workGenerationTaskListToolbar");
    const frame = document.querySelector<HTMLElement>(".workGenerationTaskWorkspaceFrame");
    const tabs = Array.from(document.querySelectorAll<HTMLElement>(".workGenerationTaskTabs button"));
    return {
      frameOverflow: frame ? frame.scrollWidth > frame.clientWidth : null,
      toolbarOverflow: toolbar ? toolbar.scrollWidth > toolbar.clientWidth : null,
      wrappedTabs: tabs.filter((tab) => tab.scrollHeight > tab.clientHeight).map((tab) => tab.innerText),
    };
  });
  expect(compactTaskLayout).toEqual({ frameOverflow: false, toolbarOverflow: false, wrappedTabs: [] });
});

test("作品库完成网格列表、版本审计、下载和发布草稿交接", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 980 });
  await page.unroute(/\/api\/video-workspace\/menus$/);
  await page.route(/\/api\/video-workspace\/menus$/, async (route) => {
    await route.fulfill({ contentType: "application/json", json: { menus: workGenerationTaskWorkspaceMenus } });
  });
  const workId = "a1111111-1111-4111-8111-111111111111";
  const versionId = "a2222222-2222-4222-8222-222222222222";
  const failedVersionId = "a3333333-3333-4333-8333-333333333333";
  const draftVersionId = "a7777777-7777-4777-8777-777777777777";
  const videoArtifactId = "a4444444-4444-4444-8444-444444444444";
  const subtitleArtifactId = "a5555555-5555-4555-8555-555555555555";
  const runId = "a6666666-6666-4666-8666-666666666666";
  const summary = {
    id: workId,
    project_id: projectId,
    script_id: scriptId,
    title: "夏日防晒知识短片",
    status: "succeeded",
    archived: false,
    current_version_id: draftVersionId,
    current_completed_version_id: versionId,
    current_completed_version_no: 5,
    aspect_ratio: "9:16",
    duration_seconds: 30,
    cover_artifact_id: videoArtifactId,
    cover_storage_path: "works/final-v2.mp4",
    created_at: "2026-07-21T00:00:00Z",
    updated_at: "2026-07-22T00:00:00Z",
  };
  const completedVersion = {
    id: versionId,
    work_id: workId,
    version_no: 5,
    status: "completed",
    source_version_id: failedVersionId,
    derivation_kind: "edit",
    source_manifest_version: "manifest-v2",
    input_snapshot: { scenes: [{ id: "scene-1", narration: "出门前注意涂抹防晒" }] },
    model_snapshot: { video: { display_name: "Seedance 2.0" } },
    parameter_snapshot: { aspect_ratio: "9:16", resolution: "1080p" },
    prompt_snapshot: { full_prompt: "海边防晒知识短片" },
    timeline_snapshot: { audio_mode: "independent_tts", burn_subtitles: true },
    created_at: "2026-07-22T00:00:00Z",
    updated_at: "2026-07-22T00:01:00Z",
    completed_at: "2026-07-22T00:05:00Z",
  };
  const failedVersion = {
    ...completedVersion,
    id: failedVersionId,
    version_no: 10,
    status: "failed",
    source_version_id: null,
    derivation_kind: "initial",
    completed_at: "2026-07-21T00:05:00Z",
  };
  const draftVersion = {
    ...completedVersion,
    id: draftVersionId,
    version_no: 11,
    status: "draft",
    source_version_id: versionId,
    derivation_kind: "edit",
    prompt_snapshot: { full_prompt: "调整开场节奏，保留已确认防晒素材" },
    completed_at: null,
  };
  const olderFailedVersions = [9, 8, 7, 6, 4].map((versionNo) => ({
    ...failedVersion,
    id: `a${versionNo}333333-3333-4333-8333-333333333333`,
    version_no: versionNo,
  }));
  const artifacts = [{
    id: videoArtifactId,
    work_version_id: versionId,
    version_status: "completed",
    role: "final_video",
    material_id: null,
    file_name: "final-v2.mp4",
    storage_path: "works/final-v2.mp4",
    mime_type: "video/mp4",
    size_bytes: 0,
    sha256: "a".repeat(64),
    metadata: {},
  }, {
    id: subtitleArtifactId,
    work_version_id: versionId,
    version_status: "completed",
    role: "subtitle",
    material_id: null,
    file_name: "final-v2.srt",
    storage_path: "works/final-v2.srt",
    mime_type: "application/x-subrip",
    size_bytes: 0,
    sha256: "b".repeat(64),
    metadata: {},
  }];
  const details = {
    ...summary,
    versions: [draftVersion, failedVersion, ...olderFailedVersions, completedVersion],
    artifacts,
    timelines: [{
      work_version_id: versionId,
      video: [{ label: "镜头 1", start_seconds: 0, duration_seconds: 15 }],
      audio: [{ label: "TTS 配音", start_seconds: 0, duration_seconds: 30 }],
      subtitles: [{ label: "中文字幕", start_seconds: 0, duration_seconds: 30 }],
    }],
    generation_audit: [{
      id: runId,
      work_version_id: failedVersionId,
      status: "failed",
      current_stage: "video_segment",
      progress_percent: 40,
      error_category: "provider",
      error_summary: "上游视频生成失败",
      attempt_count: 2,
      created_at: "2026-07-21T00:00:00Z",
      updated_at: "2026-07-21T00:01:00Z",
    }],
  };
  let handoffRequests = 0;
  let agentRequests = 0;
  const agentConversationId = "a9999999-9999-4999-8999-999999999999";
  const agentDiff = {
    id: "abbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
    work_id: workId,
    source_version_id: versionId,
    draft_version_id: draftVersionId,
    plan_version: 1,
    source_fingerprint: "a".repeat(64),
    draft_fingerprint: "b".repeat(64),
    changes: [{ path: "prompt_snapshot.full_prompt", old_value: "海边防晒知识短片", new_value: "保留配音，让画面节奏更紧凑" }],
    affected_nodes: ["video_segment:scene-1", "compose"],
    reused_artifact_ids: [subtitleArtifactId],
    resource_usage: { video_task_count: 1, video_seconds: 15, tts_characters: 0, asr_seconds: 0 },
    status: "analyzed",
    created_at: "2026-07-22T01:00:00Z",
  };
  await page.route(new RegExp(`/api/projects/${projectId}/works(?:\\?.*)?$`), async (route) => {
    await route.fulfill({ contentType: "application/json", json: { items: [summary], archived: false } });
  });
  await page.route(new RegExp(`/api/works/${workId}$`), async (route) => {
    await route.fulfill({ contentType: "application/json", json: details });
  });
  await page.route(new RegExp(`/api/work-versions/${versionId}/downloads$`), async (route) => {
    await route.fulfill({ contentType: "application/json", json: { work_version_id: versionId, artifacts: artifacts.map((artifact) => ({ artifact, integrity_status: "available" })) } });
  });
  await page.route(new RegExp(`/api/work-versions/${versionId}/publication-handoffs$`), async (route) => {
    handoffRequests += 1;
    expect(route.request().headers()["idempotency-key"]).toMatch(/^[0-9a-f-]+$/);
    await route.fulfill({ status: 201, contentType: "application/json", json: { id: "handoff-1", work_id: workId, work_version_id: versionId, final_video_artifact_id: videoArtifactId, subtitle_artifact_id: subtitleArtifactId, status: "draft", payload: {}, created_at: "2026-07-22T01:00:00Z", created: true } });
  });
  await page.route(/\/api\/agent\/conversations$/, async (route) => {
    expect(route.request().postDataJSON()).toMatchObject({ agent_type: "work", project_id: projectId, subject_type: "work", subject_id: workId });
    await route.fulfill({ status: 201, contentType: "application/json", json: { conversation_id: agentConversationId, project_id: projectId, agent_type: "work", subject_type: "work", subject_id: workId, title: "作品修改", status: "active", metadata: {}, created_at: "2026-07-22T01:00:00Z", updated_at: "2026-07-22T01:00:00Z" } });
  });
  await page.route(new RegExp(`/api/agent/conversations/${agentConversationId}/messages$`), async (route) => {
    agentRequests += 1;
    const content = route.request().postDataJSON().content;
    await route.fulfill({ contentType: "application/json", json: {
      user_message: { message_id: "accccccc-cccc-4ccc-8ccc-cccccccccccc", conversation_id: agentConversationId, role: "user", content, metadata: {}, created_at: "2026-07-22T01:00:00Z" },
      assistant_message: { message_id: "addddddd-dddd-4ddd-8ddd-dddddddddddd", conversation_id: agentConversationId, role: "assistant", content: "已保留配音并收紧画面节奏。", metadata: { draft_version_id: draftVersionId, version_no: 11, requires_confirmation: true, diff: agentDiff }, created_at: "2026-07-22T01:00:01Z" },
      run: { run_id: "aeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee", project_id: projectId, agent_type: "work", status: "succeeded", input: {}, output: {}, started_at: "2026-07-22T01:00:00Z" },
    } });
  });
  await page.route(/\/api\/work-artifacts\/[^/]+\/download$/, async (route) => {
    await route.fulfill({ status: 200, contentType: "application/octet-stream", body: "" });
  });

  await page.goto("/production/library");
  await expect(page.getByRole("heading", { name: "作品库" })).toBeVisible();
  await expect(page.getByRole("region", { name: "作品网格" })).toBeVisible();
  await page.getByRole("button", { name: "列表视图" }).click();
  await expect(page.getByRole("region", { name: "作品列表" })).toBeVisible();
  await page.getByRole("button", { name: /夏日防晒知识短片.*查看详情/ }).click();
  await expect(page.getByRole("heading", { name: "当前草稿 · 1" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "可用成片 · 1" })).toBeVisible();
  await expect(page.getByText("暂无运行产物")).toBeVisible();
  await expect(page.locator(".workLibraryTimelineRuler")).toHaveCount(0);
  await expect(page.getByText(new RegExp(versionId))).toHaveCount(0);
  await expect(page.getByRole("region", { name: "作品 Agent 对话" })).toBeVisible();
  await expect(page.getByLabel("全局提示词")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "保存草稿修改" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "分析版本差异" })).toHaveCount(0);
  const workAgentInput = page.getByLabel("作品修改要求");
  await workAgentInput.focus();
  await expect(workAgentInput).toHaveCSS("border-top-width", "0px");
  await expect(workAgentInput).toHaveCSS("box-shadow", "none");
  await workAgentInput.fill("保留配音，让画面节奏更紧凑");
  await page.getByRole("button", { name: "发送修改要求" }).click();
  await expect(page.getByText("已保留配音并收紧画面节奏。")).toBeVisible();
  await expect(page.getByRole("button", { name: "查看影响并确认" })).toBeVisible();
  expect(agentRequests).toBe(1);
  await page.getByRole("button", { name: /V5.*已完成/ }).click();
  await expect(page.getByLabel("V5 成片预览")).toHaveAttribute("src", new RegExp(`/api/work-artifacts/${videoArtifactId}/download$`));
  await expect(page.getByText("TTS 配音")).toBeVisible();
  await expect(page.getByText("中文字幕")).toBeVisible();
  const detailSurface = page.locator(".workLibraryDetailSurface");
  const collapsedSurfaceBox = await detailSurface.boundingBox();
  await page.getByRole("button", { name: /失败与早期记录.*失败 6.*未运行草稿 0/ }).click();
  const historyScroller = page.locator(".workLibraryVersionGroup.history > div");
  const versionActions = page.locator(".workLibraryVersionActions");
  const versionPanel = page.locator(".workLibraryVersionPanel");
  const [versionActionsBox, versionPanelBox, expandedSurfaceBox, historyScrollerBox] = await Promise.all([
    versionActions.boundingBox(),
    versionPanel.boundingBox(),
    detailSurface.boundingBox(),
    historyScroller.boundingBox(),
  ]);
  expect(collapsedSurfaceBox).not.toBeNull();
  expect(versionActionsBox).not.toBeNull();
  expect(versionPanelBox).not.toBeNull();
  expect(expandedSurfaceBox).not.toBeNull();
  expect(historyScrollerBox).not.toBeNull();
  expect(expandedSurfaceBox!.height).toBe(collapsedSurfaceBox!.height);
  expect(versionActionsBox!.y).toBeGreaterThanOrEqual(historyScrollerBox!.y + historyScrollerBox!.height);
  expect(versionPanelBox!.y + versionPanelBox!.height).toBeGreaterThanOrEqual(versionActionsBox!.y + versionActionsBox!.height);
  expect(expandedSurfaceBox!.y + expandedSurfaceBox!.height).toBeGreaterThanOrEqual(versionActionsBox!.y + versionActionsBox!.height);
  const historyScrollState = await historyScroller.evaluate((element) => {
    element.scrollTop = element.scrollHeight;
    return {
      overflowY: getComputedStyle(element).overflowY,
      clientHeight: element.clientHeight,
      scrollHeight: element.scrollHeight,
      scrollTop: element.scrollTop,
    };
  });
  expect(historyScrollState.overflowY).toBe("auto");
  expect(historyScrollState.scrollHeight).toBeGreaterThan(historyScrollState.clientHeight);
  expect(historyScrollState.scrollTop).toBeGreaterThan(0);
  await page.getByRole("button", { name: /V10.*失败/ }).click();
  await expect(page.getByText("上游视频生成失败")).toBeVisible();
  await expect(page.getByRole("button", { name: "查看生成任务" })).toBeVisible();
  await page.getByRole("button", { name: /V5.*已完成/ }).click();
  await page.getByRole("button", { name: "下载" }).click();
  await expect(page.getByRole("link", { name: "下载 final-v2.mp4" })).toBeVisible();
  await expect(page.getByRole("link", { name: "下载制作包" })).toHaveAttribute("href", new RegExp(`/api/work-versions/${versionId}/production-package$`));
  await page.getByRole("button", { name: "进入发布" }).click();
  await expect(page.getByText("发布草稿已创建，未自动发布")).toBeVisible();
  expect(handoffRequests).toBe(1);
  await expect(page.getByText(/费用|金额|币种|价格/)).toHaveCount(0);
  const overflow = await page.locator(".workLibraryWorkspace").evaluate((element) => element.scrollWidth > element.clientWidth);
  expect(overflow).toBe(false);
  const scrolling = await page.evaluate(() => {
    const workspace = document.querySelector<HTMLElement>(".workLibraryDetailWorkspace");
    const main = document.querySelector<HTMLElement>(".workLibraryDetailMain");
    const versions = document.querySelector<HTMLElement>(".workLibraryVersionPanel");
    const history = document.querySelector<HTMLElement>(".workLibraryVersionGroup.history > div");
    return {
      workspace: workspace ? getComputedStyle(workspace).overflowY : null,
      main: main ? getComputedStyle(main).overflowY : null,
      versions: versions ? getComputedStyle(versions).overflowY : null,
      history: history ? getComputedStyle(history).overflowY : null,
      nestedScrollable: [main, versions].some((element) => element && ["auto", "scroll"].includes(getComputedStyle(element).overflowY)),
    };
  });
  expect(scrolling).toEqual({ workspace: "auto", main: "visible", versions: "hidden", history: "auto", nestedScrollable: false });
});

test("声音与字幕工作区使用动态中文语言并在确认后创建任务", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 980 });
  await mockMaterialLibraryWorkflow(page);
  let taskCreated = false;
  const voice = {
    voice_id: "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee",
    voice_type: "zh_female_fixture",
    resource_id: "seed-tts-2.0",
    name: "测试女声",
    avatar_url: null,
    gender: "female",
    age: "adult",
    categories: [],
    normal_labels: ["沉稳"],
    special_labels: [],
    trial_url: null,
    short_trial_url: null,
    languages: [{ Language: "zh-cn", Text: "这是一段试听文案，不是语言名称。" }],
    emotions: [{ Label: "", Value: "", Icon: "" }],
    description: "适合知识旁白",
    is_available: true,
    catalog_version: 1,
    created_at: "2026-07-15T00:00:00Z",
    updated_at: "2026-07-15T00:00:00Z",
  };
  const alastorVoice = {
    ...voice,
    voice_id: "edededed-eded-4ded-8ded-edededededed",
    voice_type: "ICL_uranus_en_male_alastor_tob",
    name: "Alastor 2.0",
    gender: "男",
    age: "青年",
    normal_labels: [],
    languages: [{ Language: "en", Text: "Smile, smile darling, this is audition copy." }],
    description: "恐怖电影里的小丑，声音尖锐，有侵略性，擅长英语",
  };
  const sourceScriptId = "12121212-1212-4212-8212-121212121212";
  const sourceSceneIds = [
    "13131313-1313-4313-8313-131313131313",
    "14141414-1414-4414-8414-141414141414",
  ];
  const sourceScriptUpdatedAt = "2026-07-16T08:24:00Z";
  await page.route(new RegExp(`/api/projects/${projectId}/scripts(?:\\?.*)?$`), async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: {
        scripts: [
          {
            script_id: sourceScriptId,
            topic_id: null,
            source_topic_title: "停止内耗，从拆小目标开始",
            title: "别硬扛：稳定前进的方法",
            status: "approved",
            scene_count: 2,
            parent_id: null,
            created_at: "2026-07-15T00:00:00Z",
            updated_at: sourceScriptUpdatedAt,
          },
          {
            script_id: "15151515-1515-4515-8515-151515151515",
            topic_id: null,
            source_topic_title: "历史选题",
            title: "已经归档的脚本",
            status: "archived",
            scene_count: 1,
            parent_id: null,
            created_at: "2026-07-14T00:00:00Z",
            updated_at: "2026-07-14T08:24:00Z",
          },
        ],
        total: 2,
        limit: 100,
        offset: 0,
      },
    });
  });
  await page.route(new RegExp(`/api/scripts/${sourceScriptId}$`), async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: {
        script_id: sourceScriptId,
        project_id: projectId,
        topic_id: null,
        topic_snapshot: { title: "停止内耗，从拆小目标开始" },
        title: "别硬扛：稳定前进的方法",
        hook: "停止内耗",
        status: "approved",
        parent_id: null,
        created_at: "2026-07-15T00:00:00Z",
        updated_at: sourceScriptUpdatedAt,
        scenes: [
          { scene_id: sourceSceneIds[0], sequence: 1, narration: "允许自己停一停。", visual_description: "停顿", emotion: "温暖", duration_sec: 5 },
          { scene_id: sourceSceneIds[1], sequence: 2, narration: "把目标拆小。", visual_description: "拆分", emotion: "平静", duration_sec: 5 },
        ],
      },
    });
  });
  await page.route(new RegExp(`/api/speech/models/${ttsModelId}/voice-catalog(?:\\?.*)?$`), async (route) => {
    await route.fulfill({
      contentType: "application/json",
      json: {
        model_id: ttsModelId,
        model_settings: {
          supported_audio_formats: ["mp3"],
          default_audio_format: "mp3",
          supported_sample_rates: [24000],
          default_sample_rate: 24000,
          max_input_characters: 3000,
          supports_word_timestamps: true,
          word_timestamp_languages: ["zh-cn"],
          parameters: { speed_ratio: { type: "number", minimum: 0.5, maximum: 2 } },
        },
        last_sync: {
          sync_id: "ffffffff-ffff-4fff-8fff-ffffffffffff",
          model_id: ttsModelId,
          trigger_source: "admin",
          status: "succeeded",
          page_limit: 100,
          page_count: 1,
          speaker_count: 2,
          error_summary: null,
          requested_at: "2026-07-15T00:00:00Z",
          started_at: "2026-07-15T00:00:00Z",
          completed_at: "2026-07-15T00:01:00Z",
          created_at: "2026-07-15T00:00:00Z",
          updated_at: "2026-07-15T00:01:00Z",
        },
        voices: [voice, alastorVoice],
      },
    });
  });
  await page.route(new RegExp(`/api/projects/${projectId}/sound-subtitle/tasks/preflight$`), async (route) => {
    expect(route.request().postDataJSON()).toMatchObject({
      task_type: "tts",
      model_id: ttsModelId,
      text_content: "你好世界",
      voice_type: alastorVoice.voice_type,
      language: "en",
      parameters: { audio_format: "mp3", sample_rate: 24000, speed_ratio: 1 },
      generate_subtitle: false,
      source_script_id: sourceScriptId,
      source_script_updated_at: sourceScriptUpdatedAt,
      source_script_scene_ids: [sourceSceneIds[0]],
    });
    await route.fulfill({
      contentType: "application/json",
      json: {
        task_type: "tts",
        model_id: ttsModelId,
        model_display_name: "豆包 TTS",
        voice_snapshot: { name: alastorVoice.name },
        resource_usage: { character_count: 4, task_count: 1, output_count: 1 },
        normalized_parameters: { audio_format: "mp3", sample_rate: 24000, speed_ratio: 1 },
        confirmation_token: "confirmation-token",
      },
    });
  });
  const failedSoundTasks = Array.from({ length: 8 }, (_, index) => ({
    task_id: `99999999-9999-4999-8999-99999999999${index}`,
    project_id: projectId,
    parent_task_id: null,
    task_type: "tts",
    status: "failed",
    model_id: ttsModelId,
    audio_inspection_id: null,
    source_audio_material_id: null,
    source_script_id: null,
    source_script_snapshot: null,
    output_audio_material_id: null,
    output_subtitle_material_id: null,
    text_content: `第 ${index + 1} 条项目旁白完整配音`,
    voice_type: alastorVoice.voice_type,
    language: "en",
    emotion: null,
    parameters: { audio_format: "mp3", sample_rate: 24000, speed_ratio: 1 },
    generate_subtitle: false,
    subtitle_segments: [],
    model_snapshot: {
      display_name: "豆包 TTS",
      upstream_model: "doubao-seed-tts-2.0",
      api_protocol: "volcengine_tts_v3",
    },
    voice_snapshot: { name: alastorVoice.name },
    resource_usage: { character_count: 12, task_count: 1, output_count: 1 },
    timeline: null,
    result: null,
    request_id: index === 0
      ? "f1f273d6-82da-4101-a284-6c4b54b89910"
      : `abababab-abab-4bab-8bab-abababababa${index}`,
    upstream_log_id: index === 0 ? "20260717150632A1B2C3D4E5F60789" : null,
    attempt_count: 1,
    max_attempts: 2,
    error_code: "tts_http_error",
    error_summary: index === 0 ? "语音供应商返回 HTTP 403" : "语音供应商返回 HTTP 401",
    error_details: index === 0 ? {
      http_status: 403,
      provider_error_code: "45000020",
      provider_error_message: "Permission denied",
    } : {
      http_status: 401,
      provider_error_code: "45000010",
      provider_error_message: "Invalid X-Api-Key",
    },
    staging_status: "none",
    cleanup_attempt_count: 0,
    cleanup_error_summary: null,
    started_at: "2026-07-17T07:06:28Z",
    completed_at: "2026-07-17T07:06:29Z",
    created_at: `2026-07-17T07:0${6 - Math.min(index, 6)}:28Z`,
    updated_at: "2026-07-17T07:06:29Z",
  }));
  await page.route(new RegExp(`/api/projects/${projectId}/sound-subtitle/tasks$`), async (route) => {
    if (route.request().method() === "GET") {
      await route.fulfill({ contentType: "application/json", json: { tasks: failedSoundTasks } });
      return;
    }
    taskCreated = true;
    expect(route.request().headers()["idempotency-key"]).toBeTruthy();
    expect(route.request().postDataJSON()).toMatchObject({ confirmation_token: "confirmation-token" });
    await route.fulfill({
      status: 201,
      contentType: "application/json",
      json: {
        task_id: "99999999-9999-4999-8999-999999999999",
        project_id: projectId,
        parent_task_id: null,
        task_type: "tts",
        status: "queued",
        model_id: ttsModelId,
        audio_inspection_id: null,
        source_audio_material_id: null,
        output_audio_material_id: null,
        output_subtitle_material_id: null,
        text_content: "你好世界",
        voice_type: alastorVoice.voice_type,
        language: "en",
        emotion: null,
        parameters: { audio_format: "mp3", sample_rate: 24000, speed_ratio: 1 },
        generate_subtitle: false,
        subtitle_segments: [],
        model_snapshot: { display_name: "豆包 TTS" },
        voice_snapshot: { name: alastorVoice.name },
        resource_usage: { character_count: 4, task_count: 1, output_count: 1 },
        timeline: null,
        result: null,
        request_id: "abababab-abab-4bab-8bab-abababababab",
        upstream_log_id: null,
        attempt_count: 0,
        max_attempts: 2,
        error_code: null,
        error_summary: null,
        staging_status: "not_required",
        cleanup_attempt_count: 0,
        cleanup_error_summary: null,
        started_at: null,
        completed_at: null,
        created_at: "2026-07-15T00:02:00Z",
        updated_at: "2026-07-15T00:02:00Z",
      },
    });
  });

  await page.goto("/");
  const workspaceMenu = page.getByRole("navigation", { name: "视频工作台菜单" });
  await workspaceMenu.getByRole("button", { name: /素材管理/ }).click();
  await workspaceMenu.getByRole("button", { name: "声音与字幕生成" }).click();

  await expect(page.getByRole("heading", { name: "声音与字幕生成" })).toBeVisible();
  await expect(page.getByRole("tab", { name: "TTS配音" })).toBeVisible();
  await expect(page.getByRole("tab", { name: "字幕" })).toBeVisible();
  await expect(page.getByText("情绪风格")).toHaveCount(0);
  const languageSelector = page.getByRole("combobox", { name: "语言 / 口音" });
  await languageSelector.click();
  const languageListbox = page.getByRole("listbox", { name: "语言 / 口音选项" });
  await expect(languageListbox.getByRole("option", { name: "简体中文" })).toHaveAttribute("data-value", "zh-cn");
  const languagePlacement = await Promise.all([
    languageSelector.boundingBox(),
    languageListbox.boundingBox(),
  ]);
  expect(languagePlacement[0]).not.toBeNull();
  expect(languagePlacement[1]).not.toBeNull();
  expect(languagePlacement[1]!.y).toBeGreaterThanOrEqual(languagePlacement[0]!.y + languagePlacement[0]!.height);
  await languageListbox.getByRole("option", { name: "简体中文" }).click();
  await expect(page.getByText("这是一段试听文案，不是语言名称。")).toHaveCount(0);
  await expect(page.getByText("素材管理 / 声音与字幕生成")).toBeVisible();
  await expect(page.getByRole("button", { name: "新建 TTS 任务" })).toBeVisible();
  await expect(page.getByRole("complementary", { name: "配音任务列表" })).toBeVisible();
  const failedDetail = page.getByLabel("当前失败任务详情");
  await expect(failedDetail).toContainText("HTTP 403");
  await expect(failedDetail).toContainText("Permission denied");
  await expect(failedDetail).toContainText("45000020");
  await expect(failedDetail).toContainText("f1f273d6-82da-4101-a284-6c4b54b89910");
  await expect(failedDetail).toContainText("20260717150632A1B2C3D4E5F60789");
  const taskListLayout = await page.getByRole("complementary", { name: "配音任务列表" }).evaluate((panel) => {
    const list = panel.querySelector(".soundTaskCards") as HTMLElement | null;
    const filters = panel.querySelector(".soundTaskFilters")?.getBoundingClientRect();
    const concurrency = panel.querySelector(".soundConcurrencyStatus")?.getBoundingClientRect();
    const cards = Array.from(panel.querySelectorAll<HTMLElement>(".soundTaskCard"));
    if (!list || !filters || !concurrency) return null;
    const listRect = list.getBoundingClientRect();
    return {
      count: cards.length,
      verticalOverflow: list.scrollHeight - list.clientHeight,
      horizontalOverflow: list.scrollWidth - list.clientWidth,
      overflowX: getComputedStyle(list).overflowX,
      cardsClipped: cards.some((card) => card.scrollHeight - card.clientHeight > 1),
      cardsHorizontalOverflow: cards.some((card) => card.scrollWidth - card.clientWidth > 1),
      ordered: filters.bottom <= listRect.top && listRect.bottom <= concurrency.top,
      concurrencyContained: concurrency.bottom <= panel.getBoundingClientRect().bottom,
    };
  });
  expect(taskListLayout).not.toBeNull();
  expect(taskListLayout!.count).toBe(8);
  expect(taskListLayout!.verticalOverflow).toBeGreaterThan(0);
  expect(taskListLayout!.horizontalOverflow).toBeLessThanOrEqual(1);
  expect(taskListLayout!.overflowX).toBe("hidden");
  expect(taskListLayout!.cardsClipped).toBe(false);
  expect(taskListLayout!.cardsHorizontalOverflow).toBe(false);
  expect(taskListLayout!.ordered).toBe(true);
  expect(taskListLayout!.concurrencyContained).toBe(true);
  await expect(page.getByText("试听音频")).toBeVisible();
  await expect(page.getByText("当前任务")).toBeVisible();
  await expect(page.getByRole("slider", { name: "语速" })).toHaveValue("1");
  await expect(page.locator(".soundTaskSection")).toHaveCount(0);
  await expect(page.getByRole("table")).toHaveCount(0);
  const ttsModelSelector = page.getByRole("combobox", { name: "TTS 模型" });
  await expect(ttsModelSelector).toHaveJSProperty("tagName", "BUTTON");
  await ttsModelSelector.click();
  const ttsModelListbox = page.getByRole("listbox", { name: "TTS 模型选项" });
  await expect(ttsModelListbox.getByRole("option", { name: /豆包 TTS/ })).toBeVisible();
  const modelPlacement = await Promise.all([
    ttsModelSelector.boundingBox(),
    ttsModelListbox.boundingBox(),
  ]);
  expect(modelPlacement[0]).not.toBeNull();
  expect(modelPlacement[1]).not.toBeNull();
  expect(modelPlacement[1]!.x).toBeCloseTo(modelPlacement[0]!.x, 0);
  expect(modelPlacement[1]!.width).toBeCloseTo(modelPlacement[0]!.width, 0);
  expect(modelPlacement[1]!.y).toBeGreaterThanOrEqual(modelPlacement[0]!.y + modelPlacement[0]!.height);
  expect(await ttsModelListbox.evaluate((node) => getComputedStyle(node).backgroundColor)).toBe("rgb(255, 255, 255)");
  await page.keyboard.press("Escape");
  await expect(ttsModelListbox).toHaveCount(0);
  await expect(ttsModelSelector).toBeFocused();
  const layout = await page.locator(".soundWorkspaceGrid").evaluate((grid) => {
    const gridRect = grid.getBoundingClientRect();
    const taskRect = grid.querySelector(".soundTaskPanel")?.getBoundingClientRect();
    const editorRect = grid.querySelector(".soundEditorPanel")?.getBoundingClientRect();
    const agentRect = grid.querySelector(".soundAgentPanel")?.getBoundingClientRect();
    return taskRect && editorRect && agentRect ? {
      gridWidth: gridRect.width,
      gridHeight: gridRect.height,
      taskWidth: taskRect.width,
      editorWidth: editorRect.width,
      agentWidth: agentRect.width,
      firstGap: editorRect.left - taskRect.right,
      secondGap: agentRect.left - editorRect.right,
      aligned: taskRect.top === editorRect.top && editorRect.top === agentRect.top,
      ordered: taskRect.left < editorRect.left && editorRect.left < agentRect.left,
      withinGrid: agentRect.right <= gridRect.right + 1,
    } : null;
  });
  expect(layout).not.toBeNull();
  expect(layout!.gridWidth).toBeCloseTo(1118, 0);
  expect(layout!.gridHeight).toBeCloseTo(712, 0);
  expect(layout!.taskWidth).toBeCloseTo(250, 0);
  expect(layout!.editorWidth).toBeCloseTo(520, 0);
  expect(layout!.agentWidth).toBeCloseTo(276, 0);
  expect(layout!.firstGap).toBeCloseTo(16, 0);
  expect(layout!.secondGap).toBeCloseTo(16, 0);
  expect(layout!.aligned).toBe(true);
  expect(layout!.ordered).toBe(true);
  expect(layout!.withinGrid).toBe(true);
  const panelOverflow = await page.locator(".soundWorkspaceGrid").evaluate((grid) => (
    [".soundTaskPanel", ".soundEditorPanel", ".soundAgentPanel"].map((selector) => {
      const panel = grid.querySelector(selector) as HTMLElement | null;
      return panel ? {
        horizontal: panel.scrollWidth - panel.clientWidth,
        vertical: panel.scrollHeight - panel.clientHeight,
      } : null;
    })
  ));
  expect(panelOverflow).not.toContain(null);
  expect(panelOverflow.every((overflow) => overflow && overflow.horizontal <= 1 && overflow.vertical <= 1)).toBe(true);
  await page.setViewportSize({ width: 1700, height: 980 });
  const intermediateOverflow = await page.locator(".soundEditorPanel").evaluate((editor) => {
    const editorRect = editor.getBoundingClientRect();
    const model = editor.querySelector(".soundModelCatalogRow")?.getBoundingClientRect();
    const preview = editor.querySelector(".soundPreviewPlayer")?.getBoundingClientRect();
    const form = editor.querySelector(".soundFormGrid")?.getBoundingClientRect();
    const narration = editor.querySelector(".soundNarrationField")?.getBoundingClientRect();
    const parameter = editor.querySelector(".speechParameterGrid label")?.getBoundingClientRect();
    const actions = editor.querySelector(".soundPrimaryActions")?.getBoundingClientRect();
    const current = editor.querySelector(".soundCurrentTask")?.getBoundingClientRect();
    return model && preview && form && narration && parameter && actions && current ? {
      horizontal: editor.scrollWidth - editor.clientWidth,
      contentRight: editorRect.right - 18,
      modelWidth: Math.round(model.width),
      topRowAligned: Math.round(model.top) === Math.round(preview.top),
      previewRight: Math.round(preview.right),
      formRight: Math.round(form.right),
      narrationRight: Math.round(narration.right),
      actionRowAligned: Math.round(parameter.top) === Math.round(actions.top),
      actionsRight: Math.round(actions.right),
      currentRight: Math.round(current.right),
    } : null;
  });
  expect(intermediateOverflow).not.toBeNull();
  if (!intermediateOverflow) {
    throw new Error("中间桌面宽度下声音配置布局元素缺失");
  }
  expect(intermediateOverflow.horizontal).toBeLessThanOrEqual(1);
  expect(intermediateOverflow.modelWidth).toBe(484);
  expect(intermediateOverflow.topRowAligned).toBe(true);
  expect(intermediateOverflow.actionRowAligned).toBe(true);
  for (const right of [
    intermediateOverflow.previewRight,
    intermediateOverflow.formRight,
    intermediateOverflow.narrationRight,
    intermediateOverflow.actionsRight,
    intermediateOverflow.currentRight,
  ]) {
    expect(right).toBeCloseTo(intermediateOverflow.contentRight, 0);
  }
  await page.setViewportSize({ width: 1920, height: 980 });
  const wideLayout = await page.locator(".soundWorkspaceGrid").evaluate((grid) => {
    const gridRect = grid.getBoundingClientRect();
    const editorRect = grid.querySelector(".soundEditorPanel")?.getBoundingClientRect();
    const pageRect = document.querySelector(".soundSubtitlePage")?.getBoundingClientRect();
    return editorRect && pageRect ? {
      gridWidth: gridRect.width,
      editorWidth: editorRect.width,
      leftInset: gridRect.left - pageRect.left,
      rightInset: pageRect.right - gridRect.right,
    } : null;
  });
  expect(wideLayout).not.toBeNull();
  expect(wideLayout!.gridWidth).toBeCloseTo(1598, 0);
  expect(wideLayout!.editorWidth).toBeCloseTo(1000, 0);
  expect(wideLayout!.leftInset).toBeCloseTo(24, 0);
  expect(wideLayout!.rightInset).toBeCloseTo(26, 0);
  const readableFontSizes = await page.locator(".soundSubtitlePage").evaluate((root) => {
    const fontSize = (selector: string) => {
      const element = root.querySelector(selector);
      if (!element) throw new Error(`缺少字号验收元素：${selector}`);
      return getComputedStyle(element).fontSize;
    };
    return {
      editorMeta: fontSize(".soundEditorHeader p"),
      taskTitle: fontSize(".soundTaskCard > strong"),
      taskModel: fontSize(".soundTaskCard > p"),
      taskStatus: fontSize(".soundTaskStatus"),
      taskTime: fontSize(".soundTaskCardFooter small"),
      taskError: fontSize(".soundTaskFailure"),
      retryButton: fontSize(".soundTaskActions button"),
      formLabel: fontSize(".soundModelTriggerCopy span"),
      formValue: fontSize(".soundModelTriggerCopy strong"),
      narration: fontSize(".soundNarrationField textarea"),
      primaryButton: fontSize(".soundPrimaryActions .primaryAction"),
      failureTitle: fontSize(".soundFailureHeader strong"),
      failureMessage: fontSize(".soundFailureMessage"),
      failureFact: fontSize(".soundFailureFacts dd"),
      agentTitle: fontSize(".soundAgentTitleRow strong"),
      agentMeta: fontSize(".soundAgentSessionRow > span"),
      agentComposer: fontSize(".soundAgentComposer textarea"),
    };
  });
  expect(readableFontSizes).toEqual({
    editorMeta: "12px",
    taskTitle: "13px",
    taskModel: "12px",
    taskStatus: "11px",
    taskTime: "11px",
    taskError: "11px",
    retryButton: "11px",
    formLabel: "12px",
    formValue: "13px",
    narration: "14px",
    primaryButton: "13px",
    failureTitle: "13px",
    failureMessage: "12px",
    failureFact: "11px",
    agentTitle: "16px",
    agentMeta: "11px",
    agentComposer: "12px",
  });
  const agentHeaderLayout = await page.locator(".soundAgentHeader").evaluate((header) => {
    const rect = (selector: string) => header.querySelector(selector)?.getBoundingClientRect();
    const titleRow = rect(".soundAgentTitleRow");
    const title = rect(".soundAgentTitleRow strong");
    const online = rect(".soundAgentTitleRow > span");
    const sessionRow = rect(".soundAgentSessionRow");
    const session = rect(".soundAgentSessionRow > span");
    const model = rect(".soundAgentSessionRow select");
    return titleRow && title && online && sessionRow && session && model ? {
      titleRowDisplay: getComputedStyle(header.querySelector(".soundAgentTitleRow")!).display,
      sessionRowDisplay: getComputedStyle(header.querySelector(".soundAgentSessionRow")!).display,
      rowsSeparated: titleRow.bottom <= sessionRow.top,
      titleOnlineAligned: Math.abs((title.top + title.height / 2) - (online.top + online.height / 2)) <= 1,
      sessionModelAligned: Math.abs((session.top + session.height / 2) - (model.top + model.height / 2)) <= 1,
      titleBeforeOnline: title.right <= online.left,
      sessionBeforeModel: session.right <= model.left,
      contained: online.right <= titleRow.right && model.right <= sessionRow.right,
    } : null;
  });
  expect(agentHeaderLayout).toEqual({
    titleRowDisplay: "flex",
    sessionRowDisplay: "flex",
    rowsSeparated: true,
    titleOnlineAligned: true,
    sessionModelAligned: true,
    titleBeforeOnline: true,
    sessionBeforeModel: true,
    contained: true,
  });
  const voiceSelector = page.getByRole("combobox", { name: "音色" });
  await expect(voiceSelector).toContainText("测试女声");
  await expect(voiceSelector).toContainText("适合知识旁白");
  await expect(page.getByText("适合知识旁白", { exact: true })).toHaveCount(1);
  const formControlRects = await page.locator(".soundFormGrid").first().evaluate((grid) => (
    Array.from(grid.children).map((field) => {
      const control = field.getBoundingClientRect();
      return { top: Math.round(control.top), bottom: Math.round(control.bottom), width: Math.round(control.width), height: Math.round(control.height) };
    })
  ));
  expect(formControlRects).not.toContain(null);
  expect(new Set(formControlRects.map((rect) => rect?.top)).size).toBe(1);
  expect(new Set(formControlRects.map((rect) => rect?.bottom)).size).toBe(1);
  expect(new Set(formControlRects.map((rect) => rect?.height))).toEqual(new Set([56]));
  expect(formControlRects.map((rect) => rect?.width)).toEqual([650, 302]);
  const wideEditorRects = await page.locator(".soundEditorPanel").evaluate((editor) => {
    const rect = (selector: string) => editor.querySelector(selector)?.getBoundingClientRect();
    const model = rect(".soundModelCatalogRow");
    const preview = rect(".soundPreviewPlayer");
    const narration = rect(".soundNarrationField");
    const parameter = rect(".speechParameterGrid label");
    const actions = Array.from(editor.querySelectorAll(".soundPrimaryActions button")).map((button) => button.getBoundingClientRect());
    const current = rect(".soundCurrentTask");
    return model && preview && narration && parameter && current ? {
      model: { width: Math.round(model.width), height: Math.round(model.height), top: Math.round(model.top) },
      preview: { width: Math.round(preview.width), height: Math.round(preview.height), top: Math.round(preview.top) },
      narration: { width: Math.round(narration.width), height: Math.round(narration.height) },
      parameter: { width: Math.round(parameter.width), top: Math.round(parameter.top) },
      actions: actions.map((action) => ({ width: Math.round(action.width), top: Math.round(action.top) })),
      current: { width: Math.round(current.width), height: Math.round(current.height) },
    } : null;
  });
  expect(wideEditorRects).not.toBeNull();
  expect(wideEditorRects!.model).toEqual({ width: 484, height: 54, top: wideEditorRects!.preview.top });
  expect(wideEditorRects!.preview).toEqual({ width: 462, height: 54, top: wideEditorRects!.model.top });
  expect(wideEditorRects!.narration).toEqual({ width: 964, height: 180 });
  expect(wideEditorRects!.parameter.width).toBe(154);
  expect(wideEditorRects!.actions.map((action) => action.width)).toEqual([200, 586]);
  expect(wideEditorRects!.actions.every((action) => action.top === wideEditorRects!.parameter.top)).toBe(true);
  expect(wideEditorRects!.current).toEqual({ width: 964, height: 140 });
  await page.getByLabel("配音文本").fill("旧旁白");
  await page.getByRole("button", { name: "导入脚本" }).click();
  const importDialog = page.getByRole("dialog", { name: "从脚本创作导入旁白" });
  await expect(importDialog).toBeVisible();
  await expect(importDialog.getByText("别硬扛：稳定前进的方法", { exact: true })).toBeVisible();
  await expect(importDialog.getByText("已经归档的脚本", { exact: true })).toHaveCount(0);
  await expect(importDialog.getByRole("checkbox", { name: "镜头 01" })).toBeChecked();
  await expect(importDialog.getByRole("checkbox", { name: "镜头 02" })).toBeChecked();
  const importLayout = await importDialog.evaluate((dialogNode) => {
    const dialog = dialogNode.getBoundingClientRect();
    const list = dialogNode.querySelector(".soundScriptImportList")?.getBoundingClientRect();
    const scenes = dialogNode.querySelector(".soundScriptScenePicker")?.getBoundingClientRect();
    return list && scenes ? {
      width: Math.round(dialog.width),
      ordered: list.right <= scenes.left,
      aligned: Math.round(list.top) === Math.round(scenes.top),
      contained: list.left >= dialog.left && scenes.right <= dialog.right,
      horizontalOverflow: dialogNode.scrollWidth - dialogNode.clientWidth,
    } : null;
  });
  expect(importLayout).toEqual({ width: 900, ordered: true, aligned: true, contained: true, horizontalOverflow: 0 });
  await importDialog.getByRole("checkbox", { name: "镜头 02" }).uncheck();
  await importDialog.getByRole("button", { name: "替换并导入" }).click();
  await expect(page.getByLabel("配音文本")).toHaveValue("允许自己停一停。");
  await expect(page.getByText("来源：别硬扛：稳定前进的方法")).toBeVisible();
  expect(taskCreated).toBe(false);
  await voiceSelector.click();
  const voiceListbox = page.getByRole("listbox", { name: "可用音色" });
  const voicePopover = page.locator(".voiceCatalogPopover");
  const voicePlacement = await Promise.all([voiceSelector.boundingBox(), voicePopover.boundingBox()]);
  expect(voicePlacement[0]?.width).toBeCloseTo(650, 0);
  expect(voicePlacement[1]?.width).toBeCloseTo(650, 0);
  expect(voicePlacement[1]!.y).toBeGreaterThanOrEqual(voicePlacement[0]!.y + voicePlacement[0]!.height);
  const languageFilters = page.getByRole("group", { name: "按语言筛选音色" });
  const genderFilters = page.getByRole("group", { name: "按声线筛选音色" });
  await expect(languageFilters.getByRole("button")).toHaveText(["中文", "英文", "多语言"]);
  await expect(genderFilters.getByRole("button")).toHaveText(["男声", "女声"]);
  await languageFilters.getByRole("button", { name: "中文" }).click();
  await genderFilters.getByRole("button", { name: "男声" }).click();
  await expect(voiceListbox.getByText("没有匹配的音色")).toBeVisible();
  await genderFilters.getByRole("button", { name: "男声" }).click();
  await expect(voiceListbox.getByRole("option", { name: /测试女声/ })).toBeVisible();
  await languageFilters.getByRole("button", { name: "英文" }).click();
  await expect(voiceListbox.getByRole("option", { name: /测试女声/ })).toHaveCount(0);
  await expect(voiceListbox.getByRole("option", { name: /Alastor 2\.0.*恐怖电影里的小丑.*男.*青年.*英语/ })).toBeVisible();
  await page.getByRole("searchbox", { name: "搜索音色" }).fill("侵略性");
  await voiceListbox.getByRole("option", { name: /Alastor 2\.0/ }).click();
  await expect(voiceSelector).toContainText("Alastor 2.0");
  await expect(voiceSelector).toContainText("恐怖电影里的小丑");
  await expect(page.getByRole("combobox", { name: "语言 / 口音" })).toContainText("英语");
  await page.getByLabel("配音文本").fill("你好世界");
  await page.getByRole("button", { name: "生成配音" }).click();

  const dialog = page.getByRole("dialog", { name: "确认声音任务" });
  await expect(dialog.getByText("4 字符")).toBeVisible();
  await expect(dialog.getByText("1 个任务")).toBeVisible();
  expect(taskCreated).toBe(false);
  await dialog.getByRole("button", { name: "确认生成" }).click();
  await expect.poll(() => taskCreated).toBe(true);
  await expect(page.getByRole("complementary", { name: "配音任务列表" }).getByText("排队中", { exact: true })).toBeVisible();
  await expect(page.getByRole("region", { name: "TTS 配音配置" }).getByText("排队中", { exact: true })).toBeVisible();
});

test("OpenAI Audio Speech 中转可生成配音但阻止 TTS 时间戳字幕", async ({ page }) => {
  await mockMaterialLibraryWorkflow(page);
  const voice = {
    voice_id: "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee",
    voice_type: "zh_female_fixture",
    resource_id: "seed-tts-2.0",
    name: "测试女声",
    avatar_url: null,
    gender: "female",
    age: "adult",
    categories: [],
    normal_labels: ["沉稳"],
    special_labels: [],
    trial_url: null,
    short_trial_url: null,
    languages: [{ Language: "zh-cn", Text: "试听文案" }],
    emotions: [],
    description: "适合知识旁白",
    is_available: true,
    catalog_version: 1,
    created_at: "2026-07-15T00:00:00Z",
    updated_at: "2026-07-15T00:00:00Z",
  };
  await page.route(/\/api\/speech\/models\/.*\/voice-catalog(?:\?.*)?$/, async (route) => {
    const selectedModelId = route.request().url().split("/models/")[1].split("/")[0];
    const isGateway = selectedModelId === openAiTtsModelId;
    await route.fulfill({
      contentType: "application/json",
      json: {
        model_id: selectedModelId,
        source_model_id: ttsModelId,
        model_settings: {
          supported_audio_formats: ["mp3"],
          default_audio_format: "mp3",
          supported_sample_rates: [24000],
          default_sample_rate: 24000,
          max_input_characters: 3000,
          supports_word_timestamps: !isGateway,
          word_timestamp_languages: isGateway ? [] : ["zh-cn"],
          parameters: { speed_ratio: { type: "number", minimum: 0.25, maximum: 4 } },
        },
        last_sync: null,
        voices: [voice],
      },
    });
  });
  await page.route(new RegExp(`/api/projects/${projectId}/sound-subtitle/tasks$`), async (route) => {
    await route.fulfill({ contentType: "application/json", json: { tasks: [] } });
  });

  await page.goto("/materials/sound-subtitle-generation");
  const modelSelector = page.getByRole("combobox", { name: "TTS 模型" });
  await expect(modelSelector).toContainText("豆包 TTS");
  await modelSelector.click();
  await page.getByRole("option", { name: /ZeekAI Seed TTS/ }).click();
  await expect(modelSelector).toContainText("ZeekAI Seed TTS");
  await expect(page.getByRole("combobox", { name: "音色" })).toContainText("测试女声");
  await page.getByLabel("配音文本").fill("你好世界");
  await expect(page.getByRole("button", { name: "生成配音" })).toBeEnabled();

  await page.getByRole("tab", { name: "字幕" }).click();
  await expect(page.getByRole("button", { name: "TTS 字词时间戳" })).toBeDisabled();
  await expect(page.getByText("当前 TTS 中转模型不返回可信字词时间戳，请使用已有音频 ASR。")).toBeVisible();
  await expect(page.getByRole("button", { name: "生成配音与字幕" })).toBeDisabled();
  await expect(page.getByRole("button", { name: "已有音频 ASR" })).toBeEnabled();
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

test("内容策略选题 Agent 使用用户选择的文本模型生成候选", async ({ page }) => {
  await mockContentStrategyWorkflow(page);
  await page.goto("/");

  const agentPanel = page.getByRole("region", { name: "选题 Agent" });
  await expect(agentPanel.getByRole("combobox", { name: "推理模型" })).toHaveValue(textModelId);
  await agentPanel.getByLabel("生成要求").fill("本周 AI 工具方向，生成 8 个选题");
  await agentPanel.getByRole("button", { name: "生成选题" }).click();

  await expect(agentPanel.getByText("已生成 8 个候选选题。")).toBeVisible();
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
  await expect(accountPage.getByRole("combobox", { name: "推理模型" })).toHaveValue(textModelId);
  await accountPage.getByLabel("AI 草稿补充方向").fill("面向内容运营负责人");
  await accountPage.getByRole("button", { name: "生成草稿" }).click();
  await expect(accountPage.getByText("草稿摘要：已按补充方向生成策略草稿。")).toBeVisible();
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
  expect(Math.round(agentInputBox!.y - agentBox!.y)).toBeLessThanOrEqual(180);
  expect(Math.round(agentButtonBox!.width)).toBeLessThanOrEqual(160);
  expect(Math.round(agentButtonBox!.y + agentButtonBox!.height - agentBox!.y)).toBeLessThanOrEqual(320);
  expect(Math.round(agentMessagesBox!.y - agentBox!.y)).toBeLessThanOrEqual(370);

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

test("素材库上传后回填系统信息并支持大图预览", async ({ page }) => {
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
  await expect(page.getByLabel("素材详情浮层")).toHaveCount(0);
  await expect(page.getByLabel("画布工具栏")).toBeVisible();
  await expect(page.locator(".materialCanvasWorkspace canvas")).toBeVisible();
  const materialTypeFilterLayout = await page.getByLabel("素材类型筛选").evaluate((filter) => {
    const buttons = Array.from(filter.querySelectorAll("button"));
    const buttonRects = buttons.map((button) => button.getBoundingClientRect());
    const topPositions = buttonRects.map((rect) => Math.round(rect.top));
    return {
      buttonCount: buttons.length,
      rowOffset: Math.max(...topPositions) - Math.min(...topPositions),
      textFits: buttons.every((button) => button.scrollWidth <= button.clientWidth),
    };
  });
  expect(materialTypeFilterLayout.buttonCount).toBe(5);
  expect(materialTypeFilterLayout.rowOffset).toBeLessThanOrEqual(1);
  expect(materialTypeFilterLayout.textFits).toBe(true);
  const materialStatusFilterLayout = await page.getByLabel("素材状态筛选").evaluate((filter) => {
    const filterRect = filter.getBoundingClientRect();
    const buttons = Array.from(filter.querySelectorAll("button"));
    const buttonRects = buttons.map((button) => button.getBoundingClientRect());
    const widths = buttonRects.map((rect) => Math.round(rect.width));
    return {
      buttonCount: buttons.length,
      leftOffset: Math.round(buttonRects[0].left - filterRect.left),
      rightOffset: Math.round(filterRect.right - buttonRects.at(-1)!.right),
      widthSpread: Math.max(...widths) - Math.min(...widths),
    };
  });
  expect(materialStatusFilterLayout.buttonCount).toBe(3);
  expect(materialStatusFilterLayout.leftOffset).toBeLessThanOrEqual(1);
  expect(materialStatusFilterLayout.rightOffset).toBeLessThanOrEqual(1);
  expect(materialStatusFilterLayout.widthSpread).toBeLessThanOrEqual(1);
  await page.getByRole("button", { name: "上传素材" }).click();
  await expect(page.getByLabel("素材详情浮层")).toBeVisible();
  await expect(page.getByRole("heading", { name: "上传素材" })).toBeVisible();
  await page.getByLabel("素材文件").setInputFiles({
    name: "办公桌面近景.png",
    mimeType: "image/png",
    buffer: png1x1,
  });
  await expect(page.getByLabel("素材名称")).toHaveValue("办公桌面近景");
  await page.getByLabel("标签（选填）").fill("办公, 场景");
  await page.getByRole("button", { name: "上传并保存" }).click();

  const materialButton = page.getByLabel("素材资产浮层").getByRole("button", { name: /办公桌面近景/ });
  await expect(materialButton).toBeVisible();
  await expect(page.getByText("图片 · PNG · 1 × 1 · 68 B")).toBeVisible();
  await expect(page.getByLabel("素材 URL")).toHaveCount(0);
  await expect(page.getByLabel("缩略图 URL")).toHaveCount(0);
  await expect(page.getByLabel("来源备注")).toHaveCount(0);
  await expect(page.getByLabel("授权备注")).toHaveCount(0);
  await expect(page.getByLabel("格式")).toHaveCount(0);
  await expect(page.getByLabel("宽度")).toHaveCount(0);
  await expect(page.getByLabel("高度")).toHaveCount(0);

  await page.getByRole("button", { name: "查看办公桌面近景大图" }).click();
  await expect(page.getByRole("dialog", { name: "图片大图预览" })).toBeVisible();
  await expect(page.getByText("100%")).toBeVisible();
  await page.getByRole("button", { name: "放大图片" }).click();
  await expect(page.getByText("125%")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: "图片大图预览" })).toHaveCount(0);

  const detailContentFlow = await page.getByLabel("素材详情浮层").evaluate((panel) => {
    const previewRect = panel.querySelector(".materialDetailPreview")?.getBoundingClientRect();
    const formRect = panel.querySelector(".materialForm")?.getBoundingClientRect();
    return previewRect && formRect
      ? {
          formTop: Math.round(formRect.top),
          previewBottom: Math.round(previewRect.bottom),
        }
      : null;
  });
  expect(detailContentFlow).not.toBeNull();
  expect(detailContentFlow!.formTop).toBeGreaterThanOrEqual(detailContentFlow!.previewBottom + 8);
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
  await expect(page.getByText("语义检索")).toHaveCount(0);
  await expect(page.getByText("分镜候选")).toHaveCount(0);
  await expect(page.getByText("素材清单确认")).toHaveCount(0);
  await page.getByRole("button", { name: "关闭素材详情" }).click();
  await expect(page.getByLabel("素材详情浮层")).toHaveCount(0);
});

test("素材库支持作品声音筛选并展示只读生成快照", async ({ page }) => {
  await mockMaterialLibraryWorkflow(page, { initialMaterials: [generatedTtsMaterial] });
  await page.goto("/");
  await page.getByRole("navigation", { name: "视频工作台菜单" })
    .getByRole("button", { name: /素材管理/ })
    .click();

  await page.getByLabel("声音用途筛选").selectOption("tts");
  await page.getByLabel("生成来源筛选").selectOption("work_generation");
  await page.getByLabel("来源作品筛选").selectOption(generatedTtsMaterial.work_id);
  const finalRequest = page.waitForRequest((request) => {
    const url = new URL(request.url());
    return url.pathname.endsWith(`/api/projects/${projectId}/materials`)
      && url.searchParams.get("work_version_id") === generatedTtsMaterial.work_version_id;
  });
  await page.getByLabel("来源版本筛选").selectOption(generatedTtsMaterial.work_version_id);

  const requestUrl = new URL((await finalRequest).url());
  expect(requestUrl.searchParams.get("audio_usage")).toBe("tts");
  expect(requestUrl.searchParams.get("source")).toBe("work_generation");
  expect(requestUrl.searchParams.get("work_id")).toBe(generatedTtsMaterial.work_id);
  expect(requestUrl.searchParams.get("work_version_id")).toBe(generatedTtsMaterial.work_version_id);

  await page.getByLabel("素材资产浮层")
    .getByRole("button", { name: /Debug不内耗-V3-旁白/ })
    .click();
  const detail = page.getByLabel("素材详情浮层");
  await expect(detail.getByRole("heading", { name: "生成来源" })).toBeVisible();
  await expect(detail.getByText("豆包语音 2.0")).toBeVisible();
  await expect(detail.getByText("灿灿")).toBeVisible();
  await expect(detail.getByText("req_7P2K8")).toBeVisible();
  await expect(detail.getByText("凭据未记录")).toBeVisible();
  await expect(page.getByText("AI 音乐")).toHaveCount(0);
  await expect(page.getByText("环境音生成")).toHaveCount(0);
  await expect(page.getByText("动作音效生成")).toHaveCount(0);
});

test("素材库上传音频时提交标准声音用途", async ({ page }) => {
  let uploadBody = "";
  await mockMaterialLibraryWorkflow(page, {
    uploadResponse: uploadedAudioMaterial,
    onUpload: (postData) => {
      uploadBody = postData;
    },
  });
  await page.goto("/");
  await page.getByRole("navigation", { name: "视频工作台菜单" })
    .getByRole("button", { name: /素材管理/ })
    .click();
  await page.getByRole("button", { name: "上传素材" }).click();
  await page.getByLabel("素材文件").setInputFiles({
    name: "城市清晨环境声.wav",
    mimeType: "audio/wav",
    buffer: Buffer.from("RIFF-test-wave"),
  });
  await page.getByLabel("声音用途（选填）").selectOption("ambient");
  await page.getByLabel("标签（选填）").fill("城市, 清晨, 环境声");
  await page.getByRole("button", { name: "上传并保存" }).click();

  await expect(page.getByLabel("素材资产浮层")
    .getByRole("button", { name: /城市清晨环境声/ }))
    .toBeVisible();
  expect(uploadBody).toContain('name="audio_usage"');
  expect(uploadBody).toContain("ambient");
  await expect(page.getByLabel("素材详情浮层").getByText("环境音")).toBeVisible();
});
