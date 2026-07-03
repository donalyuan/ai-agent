"use client";

import { FormEvent, useEffect, useMemo, useState } from "react";
import {
  ApiClient,
  ApiError,
  Project,
  ScriptDetail,
  ScriptStatus,
  ScriptStyle,
  ScriptSummary,
  checkHealth,
  createApiClient,
  generateScript,
  getScript,
  listProjects,
  listScripts,
  updateScriptStatus,
} from "./lib/api";

const agents = [
  { name: "选题智能体", state: "预留" },
  { name: "脚本智能体", state: "当前" },
  { name: "素材智能体", state: "预留" },
  { name: "视频智能体", state: "预留" },
  { name: "发布智能体", state: "预留" },
  { name: "优化智能体", state: "预留" },
];

const statusOptions: Array<{ value: "all" | ScriptStatus; label: string }> = [
  { value: "all", label: "全部" },
  { value: "draft", label: "草稿" },
  { value: "approved", label: "已通过" },
  { value: "archived", label: "已归档" },
];

const styleOptions: Array<{ value: ScriptStyle; label: string }> = [
  { value: "knowledge", label: "知识科普" },
  { value: "story", label: "故事叙述" },
  { value: "tutorial", label: "教程讲解" },
];

const sceneCountOptions = Array.from({ length: 10 }, (_, index) => index + 3);

const statusLabels: Record<ScriptStatus, string> = {
  draft: "草稿",
  approved: "已通过",
  archived: "已归档",
};

const statusClassNames: Record<ScriptStatus, string> = {
  draft: "statusDraft",
  approved: "statusApproved",
  archived: "statusArchived",
};

type GenerateForm = {
  topic: string;
  style: ScriptStyle;
  sceneCount: number;
};

const defaultGenerateForm: GenerateForm = {
  topic: "",
  style: "knowledge",
  sceneCount: 6,
};

export default function Home() {
  const client = useMemo(() => createApiClient(), []);
  const [apiAvailable, setApiAvailable] = useState<boolean | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState("");
  const [scripts, setScripts] = useState<ScriptSummary[]>([]);
  const [selectedScript, setSelectedScript] = useState<ScriptDetail | null>(null);
  const [statusFilter, setStatusFilter] = useState<"all" | ScriptStatus>("all");
  const [generateForm, setGenerateForm] = useState<GenerateForm>(defaultGenerateForm);
  const [loadingProjects, setLoadingProjects] = useState(true);
  const [loadingScripts, setLoadingScripts] = useState(false);
  const [loadingScriptDetail, setLoadingScriptDetail] = useState(false);
  const [generatingScript, setGeneratingScript] = useState(false);
  const [updatingStatus, setUpdatingStatus] = useState(false);
  const [projectError, setProjectError] = useState("");
  const [scriptError, setScriptError] = useState("");
  const [generateError, setGenerateError] = useState("");
  const [statusError, setStatusError] = useState("");

  const selectedProject = projects.find((project) => project.project_id === selectedProjectId);
  const writesDisabled = apiAvailable === false;

  useEffect(() => {
    let active = true;

    async function loadInitialData() {
      setLoadingProjects(true);
      setProjectError("");
      const healthy = await checkHealth(client);
      if (!active) {
        return;
      }
      setApiAvailable(healthy);

      try {
        const response = await listProjects(client);
        if (!active) {
          return;
        }
        setProjects(response.projects);
        setSelectedProjectId(response.projects[0]?.project_id || "");
      } catch (error) {
        if (!active) {
          return;
        }
        setProjectError(errorToMessage(error));
        setApiAvailable(false);
      } finally {
        if (active) {
          setLoadingProjects(false);
        }
      }
    }

    loadInitialData();

    return () => {
      active = false;
    };
  }, [client]);

  useEffect(() => {
    if (!selectedProjectId) {
      setScripts([]);
      setSelectedScript(null);
      return;
    }

    let active = true;

    async function loadProjectScripts() {
      setLoadingScripts(true);
      setScriptError("");
      setSelectedScript(null);

      try {
        const response = await listScripts(client, selectedProjectId, { status: statusFilter });
        if (!active) {
          return;
        }
        setScripts(response.scripts);

        if (response.scripts[0]) {
          await openScript(client, response.scripts[0].script_id, active, setLoadingScriptDetail, setSelectedScript, setScriptError);
        }
      } catch (error) {
        if (active) {
          setScriptError(errorToMessage(error));
        }
      } finally {
        if (active) {
          setLoadingScripts(false);
        }
      }
    }

    loadProjectScripts();

    return () => {
      active = false;
    };
  }, [client, selectedProjectId, statusFilter]);

  async function handleGenerateScript(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setGenerateError("");

    if (!selectedProjectId) {
      setGenerateError("请先选择项目");
      return;
    }

    if (generateForm.topic.trim().length < 10) {
      setGenerateError("选题至少需要 10 个字符");
      return;
    }

    setGeneratingScript(true);
    try {
      const script = await generateScript(client, {
        project_id: selectedProjectId,
        topic: generateForm.topic.trim(),
        style: generateForm.style,
        scene_count: generateForm.sceneCount,
      });
      setSelectedScript(script);
      setScripts((currentScripts) => upsertSummary(currentScripts, script));
      setStatusFilter("all");
    } catch (error) {
      setGenerateError(errorToMessage(error));
    } finally {
      setGeneratingScript(false);
    }
  }

  async function handleOpenScript(scriptId: string) {
    await openScript(client, scriptId, true, setLoadingScriptDetail, setSelectedScript, setScriptError);
  }

  async function handleUpdateStatus(status: ScriptStatus) {
    if (!selectedScript || selectedScript.status === status) {
      return;
    }

    setStatusError("");
    setUpdatingStatus(true);

    try {
      const response = await updateScriptStatus(client, selectedScript.script_id, status);
      setSelectedScript({
        ...selectedScript,
        status: response.status,
        updated_at: response.updated_at,
      });
      setScripts((currentScripts) =>
        currentScripts.map((script) =>
          script.script_id === response.script_id ? { ...script, status: response.status } : script,
        ),
      );
    } catch (error) {
      setStatusError(errorToMessage(error));
    } finally {
      setUpdatingStatus(false);
    }
  }

  return (
    <main className="workspaceShell">
      <aside className="agentRail">
        <div className="brandBlock">
          <div className="brandMark">AI</div>
          <div>
            <p>AI-AGENT</p>
            <span>智能体工作台</span>
          </div>
        </div>

        <nav aria-label="智能体菜单" className="agentMenu">
          {agents.map((agent) => (
            <button
              className={agent.name === "脚本智能体" ? "agentItem active" : "agentItem"}
              disabled={agent.name !== "脚本智能体"}
              key={agent.name}
              type="button"
            >
              <span>{agent.name}</span>
              <small>{agent.state}</small>
            </button>
          ))}
        </nav>
      </aside>

      <section className="workbench">
        <header className="topbar">
          <div>
            <p className="sectionKicker">AI-AGENT</p>
            <h1>智能体工作台</h1>
          </div>
          <div className="topbarActions">
            <span className={apiAvailable === false ? "healthBadge down" : "healthBadge"}>
              {apiAvailable === null ? "服务检测中" : apiAvailable ? "API 正常" : "API 不可用"}
            </span>
            <label className="projectSelectLabel">
              当前项目
              <select
                aria-label="当前项目"
                disabled={!projects.length}
                onChange={(event) => setSelectedProjectId(event.target.value)}
                value={selectedProjectId}
              >
                {projects.length ? null : <option value="">暂无项目</option>}
                {projects.map((project) => (
                  <option key={project.project_id} value={project.project_id}>
                    {project.name}
                  </option>
                ))}
              </select>
            </label>
          </div>
        </header>

        <div className="workspaceGrid">
          <section className="scriptColumn" aria-label="脚本列表">
            <div className="panelHeader compactHeader">
              <div>
                <p className="sectionKicker">脚本智能体</p>
                <h2>脚本列表</h2>
              </div>
              <span>{scripts.length} 条</span>
            </div>

            <div className="statusFilter" aria-label="脚本状态筛选">
              {statusOptions.map((option) => (
                <button
                  className={statusFilter === option.value ? "filterButton selected" : "filterButton"}
                  key={option.value}
                  onClick={() => setStatusFilter(option.value)}
                  type="button"
                >
                  {option.label}
                </button>
              ))}
            </div>

            {loadingProjects || loadingScripts ? <p className="stateText">正在加载脚本</p> : null}
            {scriptError ? <p className="errorText">{scriptError}</p> : null}
            {!loadingProjects && selectedProjectId && !scripts.length ? (
              <div className="emptyState">
                <strong>还没有脚本</strong>
                <span>在右侧输入选题后生成第一版结构化脚本。</span>
              </div>
            ) : null}
            {!selectedProjectId && !loadingProjects ? (
              <div className="emptyState">
                <strong>等待项目</strong>
                <span>从顶部选择项目后会在这里显示脚本记录。</span>
              </div>
            ) : null}

            <div className="scriptList">
              {scripts.map((script) => (
                <button
                  className={selectedScript?.script_id === script.script_id ? "scriptItem selected" : "scriptItem"}
                  key={script.script_id}
                  onClick={() => handleOpenScript(script.script_id)}
                  type="button"
                >
                  <span className="scriptTitle">{script.title}</span>
                  <span className="scriptMeta">
                    {script.scene_count} 镜 · {formatDate(script.created_at)}
                  </span>
                  <StatusBadge status={script.status} />
                </button>
              ))}
            </div>
          </section>

          <section className="detailColumn" aria-label="脚本详情">
            <ScriptDetailView
              loading={loadingScriptDetail}
              script={selectedScript}
              statusError={statusError}
              updatingStatus={updatingStatus}
              writesDisabled={writesDisabled}
              onUpdateStatus={handleUpdateStatus}
            />
          </section>

          <aside className="actionColumn" aria-label="生成操作">
            {projectError ? <p className="errorText" role="alert">{projectError}</p> : null}
            <GeneratePanel
              disabled={writesDisabled || !selectedProjectId}
              error={generateError}
              form={generateForm}
              generating={generatingScript}
              selectedProject={selectedProject}
              setForm={setGenerateForm}
              onSubmit={handleGenerateScript}
            />
          </aside>
        </div>
      </section>
    </main>
  );
}

function GeneratePanel({
  disabled,
  error,
  form,
  generating,
  selectedProject,
  setForm,
  onSubmit,
}: {
  disabled: boolean;
  error: string;
  form: GenerateForm;
  generating: boolean;
  selectedProject?: Project;
  setForm: (form: GenerateForm) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  return (
    <section className="sidePanel">
      <div className="panelHeader">
        <div>
          <p className="sectionKicker">生成</p>
          <h2>生成脚本</h2>
        </div>
      </div>

      <p className="helperText">
        {selectedProject ? `当前绑定：${selectedProject.name}` : "请先从顶部选择项目后再生成脚本。"}
      </p>
      {error ? <p className="errorText" role="alert">{error}</p> : null}

      <form className="stackForm" onSubmit={onSubmit}>
        <label>
          选题
          <textarea
            disabled={disabled || generating}
            onChange={(event) => setForm({ ...form, topic: event.target.value })}
            placeholder="例如：ChatGPT 如何改变程序员工作流"
            rows={5}
            value={form.topic}
          />
        </label>
        <label>
          脚本风格
          <select
            disabled={disabled || generating}
            onChange={(event) => setForm({ ...form, style: event.target.value as ScriptStyle })}
            value={form.style}
          >
            {styleOptions.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <label>
          分镜数
          <select
            disabled={disabled || generating}
            onChange={(event) => setForm({ ...form, sceneCount: Number(event.target.value) })}
            value={form.sceneCount}
          >
            {sceneCountOptions.map((count) => (
              <option key={count} value={count}>
                {count} 镜
              </option>
            ))}
          </select>
        </label>
        <button className="primaryButton" disabled={disabled || generating} type="submit">
          {generating ? "生成中" : "生成脚本"}
        </button>
      </form>
    </section>
  );
}

function ScriptDetailView({
  loading,
  script,
  statusError,
  updatingStatus,
  writesDisabled,
  onUpdateStatus,
}: {
  loading: boolean;
  script: ScriptDetail | null;
  statusError: string;
  updatingStatus: boolean;
  writesDisabled: boolean;
  onUpdateStatus: (status: ScriptStatus) => void;
}) {
  if (loading) {
    return <p className="stateText">正在读取脚本详情</p>;
  }

  if (!script) {
    return (
      <div className="detailEmpty">
        <p className="sectionKicker">时间轴对照视图</p>
        <h2>选择脚本后查看分镜</h2>
        <span>生成脚本或从左侧列表选择脚本后，这里会展示旁白与画面指令。</span>
      </div>
    );
  }

  const totalDuration = script.scenes.reduce((sum, scene) => sum + scene.duration_sec, 0);

  return (
    <article className="detailPanel">
      <div className="detailHeader">
        <div>
          <p className="sectionKicker">时间轴对照视图</p>
          <h2>{script.title}</h2>
          <p>{script.hook}</p>
        </div>
        <div className="detailStats">
          <StatusBadge status={script.status} />
          <span>{script.scenes.length} 镜</span>
          <span>{totalDuration} 秒</span>
        </div>
      </div>

      <div className="statusActions" aria-label="脚本状态更新">
        {(["draft", "approved", "archived"] as ScriptStatus[]).map((status) => (
          <button
            className={script.status === status ? "filterButton selected" : "filterButton"}
            disabled={writesDisabled || updatingStatus || script.status === status}
            key={status}
            onClick={() => onUpdateStatus(status)}
            type="button"
          >
            {statusLabels[status]}
          </button>
        ))}
      </div>
      {statusError ? <p className="errorText" role="alert">{statusError}</p> : null}

      <div className="timelineList">
        {[...script.scenes]
          .sort((left, right) => left.sequence - right.sequence)
          .map((scene) => (
            <section className="timelineRow" key={scene.scene_id}>
              <div className="timelineMarker">
                <span>第 {scene.sequence} 镜</span>
                <strong>{scene.duration_sec} 秒</strong>
              </div>
              <div className="sceneCompare">
                <div>
                  <h3>旁白</h3>
                  <p>{scene.narration}</p>
                </div>
                <div>
                  <h3>画面指令</h3>
                  <p>{scene.visual_description}</p>
                  <span>{scene.emotion}</span>
                </div>
              </div>
            </section>
          ))}
      </div>
    </article>
  );
}

function StatusBadge({ status }: { status: ScriptStatus }) {
  return <span className={`statusBadge ${statusClassNames[status]}`}>{statusLabels[status]}</span>;
}

async function openScript(
  client: ApiClient,
  scriptId: string,
  active: boolean,
  setLoading: (value: boolean) => void,
  setScript: (script: ScriptDetail | null) => void,
  setError: (message: string) => void,
) {
  setLoading(true);
  setError("");
  try {
    const script = await getScript(client, scriptId);
    if (active) {
      setScript(script);
    }
  } catch (error) {
    if (active) {
      setError(errorToMessage(error));
    }
  } finally {
    if (active) {
      setLoading(false);
    }
  }
}

function upsertSummary(scripts: ScriptSummary[], script: ScriptDetail): ScriptSummary[] {
  const summary: ScriptSummary = {
    script_id: script.script_id,
    title: script.title,
    status: script.status,
    scene_count: script.scenes.length,
    parent_id: script.parent_id,
    created_at: script.created_at,
  };
  const nextScripts = scripts.filter((item) => item.script_id !== script.script_id);
  return [summary, ...nextScripts];
}

function errorToMessage(error: unknown) {
  if (error instanceof ApiError) {
    return error.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "请求失败";
}

function formatDate(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "未知时间";
  }
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}
