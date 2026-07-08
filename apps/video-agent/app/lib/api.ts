export type ScriptStatus = "draft" | "approved" | "archived";
export type ScriptStyle = "knowledge" | "story" | "tutorial";
export type ContentTopicStatus = "idea" | "approved" | "scripted" | "archived";
export type ContentTopicSource = "manual" | "agent";

export type Project = {
  project_id: string;
  name: string;
  positioning: string;
  description: string;
  status: string;
  created_at: string;
  updated_at: string;
};

export type ProjectListResponse = {
  projects: Project[];
};

export type WorkspaceMenuStatus = "active" | "planned" | "disabled";
export type WorkspaceMenuType = "section" | "page" | "group";

export type WorkspaceMenuNode = {
  menu_id: string;
  menu_key: string;
  label: string;
  description: string;
  route_path: string | null;
  icon: string;
  menu_type: WorkspaceMenuType;
  module_key: string | null;
  agent_key: string | null;
  sort_order: number;
  is_enabled: boolean;
  status: WorkspaceMenuStatus;
  metadata: Record<string, unknown>;
  children: WorkspaceMenuNode[];
};

export type WorkspaceMenuListResponse = {
  menus: WorkspaceMenuNode[];
};

export type CreateProjectPayload = {
  name: string;
  positioning: string;
  description: string;
};

export type ScriptSummary = {
  script_id: string;
  topic_id: string | null;
  source_topic_title: string | null;
  title: string;
  status: ScriptStatus;
  scene_count: number;
  parent_id: string | null;
  created_at: string;
};

export type Scene = {
  scene_id: string;
  sequence: number;
  narration: string;
  visual_description: string;
  emotion: string;
  duration_sec: number;
};

export type ScriptDetail = {
  script_id: string;
  project_id: string;
  topic_id: string | null;
  topic_snapshot?: ContentTopicSnapshot | null;
  title: string;
  hook: string;
  scenes: Scene[];
  status: ScriptStatus;
  parent_id: string | null;
  created_at: string;
  updated_at: string;
};

export type ContentTopic = {
  topic_id: string;
  project_id: string;
  batch_id: string | null;
  title: string;
  angle: string;
  target_audience: string;
  hook_points: string[];
  content_type: string;
  score: number | null;
  score_reason: string;
  tags: string[];
  source: ContentTopicSource;
  status: ContentTopicStatus;
  metadata: Record<string, unknown>;
  deleted_at: string | null;
  created_at: string;
  updated_at: string;
};

export type ContentTopicSnapshot = {
  topic_id: string;
  title: string;
  angle: string;
  target_audience: string;
  hook_points: string[];
  content_type: string;
  score: number | null;
  score_reason: string;
  tags: string[];
  source: ContentTopicSource;
  status: ContentTopicStatus;
  created_at: string;
};

export type ContentTopicStats = {
  total: number;
  idea: number;
  approved: number;
  scripted: number;
  archived: number;
};

export type TopicGenerationBatchStatus = "running" | "succeeded" | "failed";
export type TopicReviewSnapshotStatus = "succeeded" | "failed";
export type TopicReviewPriority = "priority" | "backup" | "reject";
export type TopicReviewRiskFlag =
  | "too_generic"
  | "duplicate"
  | "hard_to_script"
  | "off_positioning"
  | "compliance_risk";

export type TopicGenerationBatchSummary = {
  batch_id: string;
  project_id: string;
  supplement_of_batch_id: string | null;
  prompt: string;
  requested_count: number;
  topic_count: number;
  status: TopicGenerationBatchStatus;
  error_message: string | null;
  created_at: string;
  updated_at: string;
};

export type ContentTopicListResponse = {
  topics: ContentTopic[];
  stats: ContentTopicStats;
};

export type TopicGenerationBatchListResponse = {
  batches: TopicGenerationBatchSummary[];
};

export type TopicReviewItem = {
  topic_id: string;
  priority: TopicReviewPriority;
  reason: string;
  risk_flags: TopicReviewRiskFlag[];
  similar_topic_ids: string[];
};

export type TopicReviewResult = {
  topic_reviews: TopicReviewItem[];
};

export type TopicReviewSnapshot = {
  snapshot_id: string;
  project_id: string;
  root_batch_id: string;
  source_run_id: string | null;
  status: TopicReviewSnapshotStatus;
  review_summary: string;
  result: TopicReviewResult;
  error_message: string | null;
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
};

export type DeletedContentTopicResponse = {
  topic_id: string;
  deleted_at: string;
};

export type ContentTopicFilters = {
  status?: ContentTopicStatus | "all";
  source?: ContentTopicSource | "all";
  batch_id?: string | null;
};

export type ContentTopicPayload = {
  title: string;
  angle: string;
  target_audience: string;
  hook_points: string[];
  content_type: string;
  score?: number | null;
  score_reason: string;
  tags: string[];
};

export type PrepareScriptFromTopicPayload = {
  style?: ScriptStyle;
  scene_count?: number;
};

export type TopicScriptRequestPreview = {
  project_id: string;
  topic_id: string;
  topic: string;
  style: ScriptStyle;
  scene_count: number;
};

export type PrepareScriptFromTopicResponse = {
  topic: ContentTopic;
  topic_snapshot: ContentTopicSnapshot;
  script_request: TopicScriptRequestPreview;
};

export type ScriptListResponse = {
  scripts: ScriptSummary[];
  total: number;
  limit: number;
  offset: number;
};

export type GenerateScriptPayload = {
  project_id: string;
  topic?: string;
  topic_id?: string | null;
  style?: ScriptStyle;
  scene_count?: number;
  parent_id?: string | null;
};

export type UpdateScriptStatusResponse = {
  script_id: string;
  status: ScriptStatus;
  updated_at: string;
};

export type AgentType = "script" | "topic" | "material" | "video" | "publish" | "optimization";
export type AgentMessageRole = "user" | "assistant" | "system";
export type AgentRunStatus = "running" | "succeeded" | "completed" | "failed";
export type ScriptAgentIntent = "generate_script" | "edit_script";

export type AgentConversation = {
  conversation_id: string;
  project_id: string | null;
  agent_type: AgentType;
  subject_type: string | null;
  subject_id: string | null;
  title: string;
  status: "active" | "archived";
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
};

export type AgentMessage = {
  message_id: string;
  conversation_id: string;
  role: AgentMessageRole;
  content: string;
  metadata: Record<string, unknown>;
  created_at: string;
};

export type ScriptAgentTurnMetadata = {
  intent: ScriptAgentIntent;
  script_id: string | null;
  script_created: boolean;
  needs_input: boolean;
  missing_fields: string[];
};

export type AgentRun = {
  run_id: string;
  conversation_id?: string;
  project_id: string | null;
  agent_type: AgentType;
  status: AgentRunStatus;
  input: Record<string, unknown>;
  output: Record<string, unknown> | null;
  error?: string | null;
  error_message?: string | null;
  started_at: string;
  ended_at?: string | null;
  finished_at?: string | null;
};

export type CreateAgentConversationPayload = {
  project_id: string;
  agent_type: AgentType;
  subject_type?: string | null;
  subject_id?: string | null;
  title?: string;
};

export type AgentMessageListResponse = {
  messages: AgentMessage[];
};

export type SendAgentMessagePayload = {
  content: string;
  supplement_of_batch_id?: string | null;
};

export type AgentTurnResponse = {
  user_message: AgentMessage;
  assistant_message: AgentMessage;
  run: AgentRun;
};

export type ApiClient = {
  baseUrl: string;
  fetcher: typeof fetch;
};

export class ApiError extends Error {
  status: number;
  details: unknown;

  constructor(status: number, message: string, details: unknown) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.details = details;
  }
}

export function getApiBaseUrl() {
  return (process.env.NEXT_PUBLIC_API_BASE_URL || "http://localhost:18180").replace(/\/+$/, "");
}

export function createApiClient(options: Partial<ApiClient> = {}): ApiClient {
  return {
    baseUrl: (options.baseUrl || getApiBaseUrl()).replace(/\/+$/, ""),
    fetcher: options.fetcher || ((...args) => globalThis.fetch(...args)),
  };
}

export async function checkHealth(client: ApiClient): Promise<boolean> {
  try {
    const response = await client.fetcher(`${client.baseUrl}/health`, {
      headers: { accept: "application/json" },
    });
    return response.ok;
  } catch {
    return false;
  }
}

export function listProjects(client: ApiClient) {
  return request<ProjectListResponse>(client, "/api/projects");
}

export function listWorkspaceMenus(client: ApiClient) {
  return request<WorkspaceMenuListResponse>(client, "/api/video-workspace/menus");
}

export function createProject(client: ApiClient, payload: CreateProjectPayload) {
  return request<Project>(client, "/api/projects", {
    method: "POST",
    body: payload,
  });
}

export function listScripts(
  client: ApiClient,
  projectId: string,
  filters: { status?: ScriptStatus | "all" } = {},
) {
  const searchParams = new URLSearchParams();
  if (filters.status && filters.status !== "all") {
    searchParams.set("status", filters.status);
  }
  const query = searchParams.toString();
  return request<ScriptListResponse>(
    client,
    `/api/projects/${projectId}/scripts${query ? `?${query}` : ""}`,
  );
}

export function listContentTopics(
  client: ApiClient,
  projectId: string,
  filters: ContentTopicFilters = {},
) {
  const searchParams = new URLSearchParams();
  if (filters.status && filters.status !== "all") {
    searchParams.set("status", filters.status);
  }
  if (filters.source && filters.source !== "all") {
    searchParams.set("source", filters.source);
  }
  if (filters.batch_id) {
    searchParams.set("batch_id", filters.batch_id);
  }
  const query = searchParams.toString();
  return request<ContentTopicListResponse>(
    client,
    `/api/projects/${projectId}/topics${query ? `?${query}` : ""}`,
  );
}

export function listTopicGenerationBatches(client: ApiClient, projectId: string) {
  return request<TopicGenerationBatchListResponse>(
    client,
    `/api/projects/${projectId}/topic-generation-batches`,
  );
}

export function createTopicGroupReview(client: ApiClient, rootBatchId: string) {
  return request<TopicReviewSnapshot>(client, `/api/topic-groups/${rootBatchId}/reviews`, {
    method: "POST",
  });
}

export function getLatestTopicGroupReview(client: ApiClient, rootBatchId: string) {
  return request<TopicReviewSnapshot | null>(
    client,
    `/api/topic-groups/${rootBatchId}/reviews/latest`,
  );
}

export function createContentTopic(
  client: ApiClient,
  projectId: string,
  payload: ContentTopicPayload,
) {
  return request<ContentTopic>(client, `/api/projects/${projectId}/topics`, {
    method: "POST",
    body: payload,
  });
}

export function updateContentTopic(
  client: ApiClient,
  topicId: string,
  payload: ContentTopicPayload,
) {
  return request<ContentTopic>(client, `/api/topics/${topicId}`, {
    method: "PUT",
    body: payload,
  });
}

export function updateContentTopicStatus(
  client: ApiClient,
  topicId: string,
  status: ContentTopicStatus,
) {
  return request<ContentTopic>(client, `/api/topics/${topicId}/status`, {
    method: "PUT",
    body: { status },
  });
}

export function deleteContentTopic(client: ApiClient, topicId: string) {
  return request<DeletedContentTopicResponse>(client, `/api/topics/${topicId}`, {
    method: "DELETE",
  });
}

export function prepareScriptFromTopic(
  client: ApiClient,
  topicId: string,
  payload: PrepareScriptFromTopicPayload = {},
) {
  return request<PrepareScriptFromTopicResponse>(client, `/api/topics/${topicId}/prepare-script`, {
    method: "POST",
    body: payload,
  });
}

export function getScript(client: ApiClient, scriptId: string) {
  return request<ScriptDetail>(client, `/api/scripts/${scriptId}`);
}

export function generateScript(client: ApiClient, payload: GenerateScriptPayload) {
  return request<ScriptDetail>(client, "/api/scripts/generate", {
    method: "POST",
    body: payload,
  });
}

export function updateScriptStatus(
  client: ApiClient,
  scriptId: string,
  status: ScriptStatus,
) {
  return request<UpdateScriptStatusResponse>(client, `/api/scripts/${scriptId}/status`, {
    method: "PUT",
    body: { status },
  });
}

export function createAgentConversation(
  client: ApiClient,
  payload: CreateAgentConversationPayload,
) {
  return request<AgentConversation>(client, "/api/agent/conversations", {
    method: "POST",
    body: payload,
  });
}

export function listAgentMessages(client: ApiClient, conversationId: string) {
  return request<AgentMessageListResponse>(
    client,
    `/api/agent/conversations/${conversationId}/messages`,
  );
}

export function sendAgentMessage(
  client: ApiClient,
  conversationId: string,
  payload: SendAgentMessagePayload,
) {
  return request<AgentTurnResponse>(
    client,
    `/api/agent/conversations/${conversationId}/messages`,
    {
      method: "POST",
      body: payload,
    },
  );
}

export function getScriptAgentTurnMetadata(message: AgentMessage): ScriptAgentTurnMetadata {
  const metadata = message.metadata;
  return {
    intent: getScriptAgentIntent(metadata.intent),
    script_id: typeof metadata.script_id === "string" ? metadata.script_id : null,
    script_created: metadata.script_created === true,
    needs_input: metadata.needs_input === true,
    missing_fields: Array.isArray(metadata.missing_fields)
      ? metadata.missing_fields.filter((field): field is string => typeof field === "string")
      : [],
  };
}

function getScriptAgentIntent(value: unknown): ScriptAgentIntent {
  return value === "edit_script" ? "edit_script" : "generate_script";
}

async function request<T>(
  client: ApiClient,
  path: string,
  options: { method?: "GET" | "POST" | "PUT" | "DELETE"; body?: unknown } = {},
): Promise<T> {
  const headers: HeadersInit = { accept: "application/json" };
  const init: RequestInit = { headers };

  if (options.method) {
    init.method = options.method;
  }

  if (options.body !== undefined) {
    headers["content-type"] = "application/json";
    init.body = JSON.stringify(options.body);
  }

  const response = await client.fetcher(`${client.baseUrl}${path}`, init);
  const body = await parseJson(response);

  if (!response.ok) {
    throw new ApiError(response.status, errorMessage(body), body);
  }

  return body as T;
}

async function parseJson(response: Response): Promise<unknown> {
  const text = await response.text();
  if (!text) {
    return null;
  }

  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

function errorMessage(body: unknown) {
  if (body && typeof body === "object" && "error" in body) {
    const error = (body as { error: unknown }).error;
    if (typeof error === "string" && error.trim()) {
      return error;
    }
  }

  return "请求失败";
}
