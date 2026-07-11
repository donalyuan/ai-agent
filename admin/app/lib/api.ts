export type ScriptStatus = "draft" | "approved" | "archived";
export type ScriptStyle = "knowledge" | "story" | "tutorial";

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

export type CreateProjectPayload = {
  name: string;
  positioning: string;
  description: string;
};

export type ScriptSummary = {
  script_id: string;
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
  title: string;
  hook: string;
  scenes: Scene[];
  status: ScriptStatus;
  parent_id: string | null;
  created_at: string;
  updated_at: string;
};

export type ScriptListResponse = {
  scripts: ScriptSummary[];
  total: number;
  limit: number;
  offset: number;
};

export type GenerateScriptPayload = {
  project_id: string;
  topic: string;
  style: ScriptStyle;
  scene_count: number;
  parent_id?: string | null;
};

export type UpdateScriptStatusResponse = {
  script_id: string;
  status: ScriptStatus;
  updated_at: string;
};

export type ApiClient = {
  baseUrl: string;
  fetcher: typeof fetch;
};

export type AiModelType = "text" | "image" | "video";
export type AiModelStatus = "enabled" | "disabled" | "deleted";
export type AiModelProtocol =
  | "openai_responses"
  | "openai_chat_completions"
  | "openai_images"
  | "jimeng_visual"
  | "runway_api"
  | "kling_api";
export type AuthScheme = "bearer" | "access_key_secret";

export type AiModel = {
  model_id: string;
  display_name: string;
  model_type: AiModelType;
  provider_name: string;
  api_protocol: AiModelProtocol;
  protocol_version: string;
  auth_scheme: AuthScheme;
  request_base_url: string;
  upstream_model: string;
  api_key_masked: string;
  api_secret_masked: string | null;
  api_key_configured: boolean;
  api_secret_configured: boolean;
  timeout_seconds: number;
  reasoning_effort: string | null;
  max_output_tokens: number | null;
  settings: Record<string, unknown>;
  sort_order: number;
  remark: string;
  status: AiModelStatus;
  is_default: boolean;
  last_call_status: "never" | "success" | "failed";
  last_call_at: string | null;
  last_error_summary: string | null;
  source: string;
  version: number;
  deleted_at: string | null;
  created_at: string;
  updated_at: string;
};

export type AiModelPayload = {
  display_name: string;
  model_type: AiModelType;
  provider_name: string;
  api_protocol: AiModelProtocol;
  protocol_version: string;
  auth_scheme: AuthScheme;
  request_base_url: string;
  upstream_model: string;
  api_key: string;
  api_secret: string | null;
  timeout_seconds: number;
  reasoning_effort: string | null;
  max_output_tokens: number | null;
  settings: Record<string, unknown>;
  sort_order: number;
  remark: string;
  is_default: boolean;
};

export type VersionedModelAction = {
  version: number;
  replacement_model_id?: string | null;
  allow_no_default?: boolean;
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

export function listAiModels(
  client: ApiClient,
  filters: {
    type?: AiModelType;
    status?: AiModelStatus;
    provider?: string;
    protocol?: AiModelProtocol;
    q?: string;
  } = {},
) {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(filters)) {
    if (value) search.set(key, value);
  }
  const query = search.toString();
  return request<{ models: AiModel[] }>(
    client,
    `/api/admin/models${query ? `?${query}` : ""}`,
  );
}

export function createAiModel(client: ApiClient, payload: AiModelPayload) {
  return request<AiModel>(client, "/api/admin/models", { method: "POST", body: payload });
}

export function updateAiModel(
  client: ApiClient,
  modelId: string,
  payload: AiModelPayload & VersionedModelAction,
) {
  return request<AiModel>(client, `/api/admin/models/${modelId}`, {
    method: "PUT",
    body: payload,
  });
}

export function setDefaultAiModel(
  client: ApiClient,
  modelId: string,
  payload: { version: number },
) {
  return request<AiModel>(client, `/api/admin/models/${modelId}/default`, {
    method: "PUT",
    body: payload,
  });
}

export function changeAiModelStatus(
  client: ApiClient,
  modelId: string,
  payload: VersionedModelAction & { status: "enabled" | "disabled" },
) {
  return request<AiModel>(client, `/api/admin/models/${modelId}/status`, {
    method: "PUT",
    body: payload,
  });
}

export function deleteAiModel(
  client: ApiClient,
  modelId: string,
  payload: VersionedModelAction,
) {
  return request<{ deletion: "physical" | "logical"; model_id?: string; model?: AiModel }>(
    client,
    `/api/admin/models/${modelId}`,
    { method: "DELETE", body: payload },
  );
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
