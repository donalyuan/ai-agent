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

async function request<T>(
  client: ApiClient,
  path: string,
  options: { method?: "GET" | "POST" | "PUT"; body?: unknown } = {},
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
