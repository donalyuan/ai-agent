import {
  Activity,
  ArrowLeft,
  ArrowRight,
  Boxes,
  Check,
  ChevronDown,
  CircleAlert,
  CircleCheck,
  Clapperboard,
  FileText,
  FolderKanban,
  Layers3,
  LoaderCircle,
  Plus,
  RefreshCw,
  Settings2,
  Sparkles,
  Workflow,
} from "lucide-react";
import {
  QueryClient,
  QueryClientProvider,
  useMutation,
  useQuery,
} from "@tanstack/react-query";
import { useEffect, useState, type ReactNode } from "react";
import {
  BrowserRouter,
  Link,
  NavLink,
  Route,
  Routes,
  useLocation,
  useNavigate,
  useParams,
  useSearchParams,
} from "react-router";
import { OwnerApiError, queryKeys, workbenchApi } from "./workbench/api";
import { creativeBriefCommandSchema } from "./workbench/contracts";
import type { RunDetail, SceneProjection } from "./workbench/contracts";
import {
  EMPTY_EPISODE_SLICE,
  episodeSliceKey,
  usePresentationStore,
} from "./workbench/presentation-store";
import { AssetCenterPage } from "./pages/AssetCenterPage";
import { AssetEditReviewPage } from "./pages/AssetEditReviewPage";
import { assetCenterApi } from "./asset-center/api";
import { TimelineEditorPage } from "./pages/TimelineEditorPage";
import { ProviderSettingsPage } from "./pages/ProviderSettingsPage";
import { StorageProfilePage } from "./pages/StorageProfilePage";
import { ExportsPage } from "./pages/ExportsPage";

export const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false, staleTime: 5_000 } },
});

type Readiness = "checking" | "ready" | "unavailable";

function useReadiness() {
  const [state, setState] = useState<Readiness>("checking");
  useEffect(() => {
    const controller = new AbortController();
    fetch("/api/v1/health/ready", { signal: controller.signal })
      .then((response) => setState(response.ok ? "ready" : "unavailable"))
      .catch(() => {
        if (!controller.signal.aborted) setState("unavailable");
      });
    return () => controller.abort();
  }, []);
  return state;
}

function Readiness({ state }: { state: Readiness }) {
  if (state === "ready")
    return (
      <span className="signal-pill good">
        <CircleCheck size={14} /> API 已就绪
      </span>
    );
  if (state === "unavailable")
    return (
      <span className="signal-pill bad">
        <CircleAlert size={14} /> API 不可用
      </span>
    );
  return (
    <span className="signal-pill waiting">
      <LoaderCircle className="spin" size={14} /> 检查 API
    </span>
  );
}

function ErrorNotice({ error }: { error: unknown }) {
  if (!error) return null;
  const message =
    error instanceof Error ? error.message : "owner projection unavailable";
  return (
    <div className="data-notice unavailable" role="status">
      <CircleAlert size={15} />
      <span>{message}</span>
    </div>
  );
}

function QueryNotice({
  isPending,
  error,
  empty,
}: {
  isPending: boolean;
  error: unknown;
  empty: string;
}) {
  if (isPending)
    return (
      <div className="data-notice loading">
        <LoaderCircle className="spin" size={15} /> 正在读取 owner projection...
      </div>
    );
  if (error) return <ErrorNotice error={error} />;
  return <div className="data-notice empty">{empty}</div>;
}

function Shell() {
  const readiness = useReadiness();
  const location = useLocation();
  const navigate = useNavigate();
  const projectId = location.pathname.match(/^\/projects\/([^/]+)/)?.[1] ?? "";
  const links = projectId
    ? [
        {
          label: "项目工作台",
          icon: FolderKanban,
          to: `/projects/${projectId}/workbench`,
        },
        {
          label: "候选审核",
          icon: Layers3,
          to: `/projects/${projectId}/review`,
        },
        { label: "项目资产", icon: Boxes, to: `/projects/${projectId}/assets` },
        {
          label: "集时间线",
          icon: Activity,
          to: `/projects/${projectId}/episodes/select/timeline`,
        },
        {
          label: "项目导出",
          icon: ArrowRight,
          to: `/projects/${projectId}/exports`,
        },
        {
          label: "模型设置",
          icon: Settings2,
          to: `/projects/${projectId}/settings`,
        },
      ]
    : [{ label: "项目入口", icon: FolderKanban, to: "/projects" }];
  return (
    <div className="studio-shell">
      <aside className="studio-sidebar">
        <button
          className="studio-brand"
          onClick={() =>
            navigate(
              projectId ? `/projects/${projectId}/workbench` : "/projects",
            )
          }
        >
          <span className="brand-glyph">
            <Clapperboard size={20} />
          </span>
          <span>帧间制片</span>
        </button>
        <div className="sidebar-rail" aria-hidden="true">
          <span />
          <span />
          <span />
        </div>
        <div className="project-switcher">
          <span className="micro-label">当前上下文</span>
          <Link
            to={projectId ? `/projects/${projectId}/workbench` : "/projects"}
          >
            <span className="project-led" />
            {projectId ? `项目 ${projectId.slice(0, 10)}` : "选择项目"}
            <ChevronDown size={14} />
          </Link>
        </div>
        <nav className="studio-nav" aria-label="工作台导航">
          {links.map(({ label, icon: Icon, to }) => (
            <NavLink
              key={label}
              to={to}
              className={({ isActive }) =>
                isActive ? "studio-nav-item active" : "studio-nav-item"
              }
            >
              <Icon size={17} />
              <span>{label}</span>
            </NavLink>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <span className="micro-label">阶段一 / MVP-A</span>
          <span className="local-chip">
            <span className="project-led" /> Mock + Local offline
          </span>
          <span className="sidebar-footnote">adapter: local_workspace</span>
        </div>
      </aside>
      <main className="studio-main">
        <header className="studio-topbar">
          <div className="topbar-context">
            <Link
              className="back-link"
              to={projectId ? `/projects/${projectId}/workbench` : "/projects"}
            >
              <ArrowLeft size={14} /> 项目入口
            </Link>
            <div className="topbar-title-row">
              <span className="micro-label">PROJECT / OWNER VIEW</span>
              <span className="legacy-phase-label">阶段 0 / 工程基线</span>
            </div>
            <h1>{projectId ? "创作控制台" : "项目索引"}</h1>
          </div>
          <div className="topbar-actions">
            <Readiness state={readiness} />
            <button
              className="icon-button"
              title="项目设置"
              onClick={() =>
                projectId && navigate(`/projects/${projectId}/settings`)
              }
            >
              <Settings2 size={18} />
            </button>
          </div>
        </header>
        {readiness === "unavailable" && (
          <div className="legacy-diagnostic" role="status">
            <span>无法连接 /api/v1/health/ready</span>
            <span>；保留 owner 原始不可用状态</span>
          </div>
        )}
        <Routes>
          <Route path="/projects" element={<Projects />} />
          <Route
            path="/projects/:projectId/workbench"
            element={<Workbench />}
          />
          <Route path="/projects/:projectId/review" element={<Review />} />
          <Route
            path="/projects/:projectId/assets"
            element={<AssetCenterPage />}
          />
          <Route
            path="/projects/:projectId/episodes/:episodeId/timeline"
            element={<TimelineEditorPage />}
          />
          <Route
            path="/projects/:projectId/episodes/select/timeline"
            element={<TimelineSelector />}
          />
          <Route
            path="/projects/:projectId/exports"
            element={<ExportsPage />}
          />
          <Route
            path="/projects/:projectId/settings"
            element={<ProviderSettingsPage projectId={projectId} />}
          />
          <Route
            path="/projects/:projectId/settings/storage-profiles/:storageProfileId"
            element={<StorageProfilePage />}
          />
          <Route path="*" element={<Projects />} />
        </Routes>
      </main>
    </div>
  );
}

function PageIntro({
  eyebrow,
  title,
  detail,
  action,
  onAction,
}: {
  eyebrow: string;
  title: string;
  detail: string;
  action?: string;
  onAction?: () => void;
}) {
  return (
    <div className="page-intro">
      <div>
        <span className="micro-label accent">{eyebrow}</span>
        <h2>{title}</h2>
        <p>{detail}</p>
      </div>
      {action && (
        <button className="primary-button" onClick={onAction}>
          <Sparkles size={16} /> {action}
        </button>
      )}
    </div>
  );
}

function SurfaceHeading({
  label,
  title,
  trailing,
}: {
  label: string;
  title: string;
  trailing?: ReactNode;
}) {
  return (
    <div className="surface-heading">
      <div>
        <span className="micro-label">{label}</span>
        <h3>{title}</h3>
      </div>
      {trailing}
    </div>
  );
}

function Projects() {
  const navigate = useNavigate();
  const projects = useQuery({
    queryKey: queryKeys.projects,
    queryFn: workbenchApi.listProjects,
  });
  const [name, setName] = useState("");
  const [editing, setEditing] = useState<{
    id: string;
    name: string;
    revision: number;
  } | null>(null);
  const create = useMutation({
    mutationFn: workbenchApi.createProject,
    onSuccess: (project) => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.projects });
      navigate(`/projects/${project.id}/workbench`);
    },
  });
  const update = useMutation({
    mutationFn: () =>
      editing
        ? workbenchApi.updateProject(
            editing.id,
            editing.name.trim(),
            editing.revision,
          )
        : Promise.reject(new Error("未选择项目")),
    onSuccess: () => {
      setEditing(null);
      void queryClient.invalidateQueries({ queryKey: queryKeys.projects });
    },
    onError: (error) => {
      if (error instanceof OwnerApiError && error.status === 409) {
        void queryClient.invalidateQueries({ queryKey: queryKeys.projects });
      }
    },
  });
  return (
    <section className="page-body project-entry">
      <PageIntro
        eyebrow="PROJECT INDEX / S01"
        title="先选择一个项目"
        detail="项目入口只读读取 projects owner；创建后必须显式进入工作台，不自动建立 Episode 或 Workflow。"
      />
      <div className="project-entry-grid">
        <section className="surface create-project-panel">
          <SurfaceHeading
            label="NEW PROJECT"
            title="建立创作上下文"
            trailing={<Plus size={18} />}
          />
          <label className="field-label">
            项目名称
            <input
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="例如：雾港来信"
            />
          </label>
          <button
            className="primary-button full"
            disabled={!name.trim() || create.isPending}
            onClick={() => create.mutate(name.trim())}
          >
            {create.isPending ? "创建中..." : "创建并进入工作台"}
            <ArrowRight size={16} />
          </button>
          {create.error && <ErrorNotice error={create.error} />}
          <div className="empty-note">
            <CircleCheck size={15} /> 默认测试组合：Mock Provider + Local
            test/offline
          </div>
        </section>
        <section className="surface project-list-panel">
          <SurfaceHeading
            label="OWNER PROJECTS"
            title="已有项目"
            trailing={
              <span className="count-chip">{projects.data?.length ?? 0}</span>
            }
          />
          {projects.isPending && (
            <QueryNotice isPending error={null} empty="" />
          )}
          {projects.error && <ErrorNotice error={projects.error} />}
          {!projects.isPending &&
            !projects.error &&
            projects.data?.length === 0 && (
              <QueryNotice
                isPending={false}
                error={null}
                empty="尚无项目；从左侧创建第一个项目"
              />
            )}
          <div className="project-list">
            {projects.data?.map((project) => (
              <div className="project-row" key={project.id}>
                <button
                  className="project-row-main"
                  onClick={() => navigate(`/projects/${project.id}/workbench`)}
                >
                  <span className="project-row-mark">
                    <FolderKanban size={16} />
                  </span>
                  <span>
                    <strong>{project.name}</strong>
                    <small className="mono">
                      {project.id} / rev {project.revision}
                    </small>
                  </span>
                  <ArrowRight size={16} />
                </button>
                <button
                  className="icon-button"
                  title="编辑项目名称"
                  onClick={() =>
                    setEditing({
                      id: project.id,
                      name: project.name,
                      revision: project.revision,
                    })
                  }
                >
                  <Settings2 size={15} />
                </button>
              </div>
            ))}
          </div>
          {editing && (
            <div className="edit-project-row">
              <label className="field-label">
                项目名称
                <input
                  value={editing.name}
                  onChange={(event) =>
                    setEditing({ ...editing, name: event.target.value })
                  }
                />
              </label>
              <button
                className="secondary-button"
                onClick={() => setEditing(null)}
              >
                取消
              </button>
              <button
                className="primary-button"
                disabled={!editing.name.trim() || update.isPending}
                onClick={() => update.mutate()}
              >
                {update.isPending ? "保存中..." : "保存名称"}
                <Check size={15} />
              </button>
              {update.error && <ErrorNotice error={update.error} />}
            </div>
          )}
        </section>
      </div>
    </section>
  );
}

type BriefDraft = {
  subject: string;
  genre: string;
  audience: string;
  characterPremise: string;
  style: string;
  episodeDurationSeconds: number;
  episodeCount: number;
  scenesPerEpisode: number;
  shotsPerScene: number;
};
const emptyBrief: BriefDraft = {
  subject: "",
  genre: "",
  audience: "",
  characterPremise: "",
  style: "",
  episodeDurationSeconds: 60,
  episodeCount: 1,
  scenesPerEpisode: 1,
  shotsPerScene: 1,
};

function Workbench() {
  const { projectId = "" } = useParams();
  const [searchParams, setSearchParams] = useSearchParams();
  const [modeOverride, setModeOverride] = useState<
    "original" | "adaptation" | null
  >(null);
  const [draftOverride, setDraftOverride] = useState<BriefDraft | null>(null);
  const [briefMessage, setBriefMessage] = useState<string | null>(null);
  const [run, setRun] = useState<RunDetail | null>(null);
  const creative = useQuery({
    queryKey: queryKeys.creative(projectId),
    queryFn: () => workbenchApi.getCreative(projectId),
    enabled: Boolean(projectId),
  });
  const episodes = useQuery({
    queryKey: queryKeys.episodes(projectId),
    queryFn: () => workbenchApi.listEpisodes(projectId),
    enabled: Boolean(projectId),
  });
  const selectedEpisodeId = searchParams.get("episodeId");
  const view =
    (searchParams.get("view") as
      | "storyboard"
      | "workflow"
      | "asset-bible"
      | null) ?? "storyboard";
  const storyboard = useQuery({
    queryKey: selectedEpisodeId
      ? queryKeys.storyboard(projectId, selectedEpisodeId)
      : ["disabled"],
    queryFn: () =>
      workbenchApi.getStoryboard(projectId, selectedEpisodeId as string),
    enabled: Boolean(selectedEpisodeId),
  });
  const workflow = useQuery({
    queryKey: queryKeys.workflow(projectId),
    queryFn: () => workbenchApi.getWorkflow(projectId),
    enabled: view === "workflow",
  });
  const currentBrief = creative.data?.creativeBrief;
  const mode = modeOverride ?? creative.data?.creationMode ?? "original";
  const draft =
    draftOverride ??
    (currentBrief
      ? {
          subject: currentBrief.subject,
          genre: currentBrief.genre,
          audience: currentBrief.audience,
          characterPremise: currentBrief.characterPremise,
          style: currentBrief.style,
          episodeDurationSeconds: currentBrief.episodeDurationSeconds,
          episodeCount: currentBrief.episodeCount,
          scenesPerEpisode: currentBrief.scenesPerEpisode,
          shotsPerScene: currentBrief.shotsPerScene,
        }
      : emptyBrief);
  const saveBrief = useMutation({
    mutationFn: () => {
      const expectedRevision = creative.data?.projectRevision ?? 0;
      const payload = {
        creationMode: mode,
        ...draft,
        schemaVersion: "1.0.0" as const,
        expectedRevision,
        expectedBriefRevision: currentBrief?.revision ?? null,
      };
      const parsed = creativeBriefCommandSchema.safeParse(payload);
      if (!parsed.success) {
        throw new OwnerApiError(
          422,
          "creative_brief_contract_invalid",
          "CreativeBrief canonical 字段不完整或无效",
        );
      }
      return workbenchApi.saveBrief(projectId, parsed.data, expectedRevision);
    },
    onSuccess: () => {
      setModeOverride(null);
      setDraftOverride(null);
      setBriefMessage("CreativeBrief 已由 projects owner 保存");
      void queryClient.invalidateQueries({
        queryKey: queryKeys.creative(projectId),
      });
    },
    onError: (error) => {
      if (error instanceof OwnerApiError && error.status === 409) {
        setModeOverride(null);
        setDraftOverride(null);
        void queryClient.invalidateQueries({
          queryKey: queryKeys.creative(projectId),
        });
      }
      setBriefMessage(error instanceof Error ? error.message : "保存失败");
    },
  });
  const startRun = useMutation({
    mutationFn: async () => {
      const routes = await workbenchApi.listSkillRoutes(projectId);
      if (routes.length !== 1) {
        throw new OwnerApiError(
          409,
          "skill_route_required",
          "请先完成唯一的当前 Skill route 裁决",
        );
      }
      const source = await workbenchApi.ensureWorkflow(projectId);
      return workbenchApi.startRun(
        projectId,
        source.id,
        source.bindingRevision,
        `text:${projectId}:${Date.now()}`,
        routes[0].id,
      );
    },
    onSuccess: setRun,
    onError: (error) =>
      setBriefMessage(error instanceof Error ? error.message : "Run 启动失败"),
  });
  const episodeList = [...(episodes.data ?? [])].sort(
    (left, right) =>
      left.number - right.number || left.id.localeCompare(right.id),
  );
  const selectedEpisode = episodeList.find(
    (episode) => episode.id === selectedEpisodeId,
  );
  const setView = (next: typeof view) => {
    const params = new URLSearchParams(searchParams);
    params.set("view", next);
    setSearchParams(params);
  };
  const setEpisode = (episodeId: string) => {
    const params = new URLSearchParams(searchParams);
    if (episodeId) params.set("episodeId", episodeId);
    else params.delete("episodeId");
    setSearchParams(params);
  };
  return (
    <section className="page-body">
      <PageIntro
        eyebrow="WORKBENCH / S02-S04"
        title="把创作意图变成可审核的镜头"
        detail="projects owner 保存 CreativeBrief；文本候选在一次完整 TextReviewBatch 后才允许进入付费媒体。"
      />
      <div className="context-band">
        <span>
          <span className="micro-label">PROJECT</span>
          <strong>{projectId || "未选择"}</strong>
        </span>
        <span>
          <span className="micro-label">CREATION MODE</span>
          <strong>{mode === "original" ? "原创" : "改编"}</strong>
        </span>
        <span>
          <span className="micro-label">PROFILE</span>
          <strong>Mock + Local / local_workspace</strong>
        </span>
        <span className="scope-lock">
          <Check size={14} /> 页面读取不产生 mutation
        </span>
      </div>
      <div className="workbench-grid">
        <section className="surface brief-surface">
          <SurfaceHeading
            label="PROJECTS OWNER / CREATIVEBRIEF"
            title={currentBrief?.subject ?? "未保存创作简报"}
            trailing={
              <span className="revision-mark">
                rev {creative.data?.projectRevision ?? "--"}
              </span>
            }
          />
          {creative.isPending && (
            <QueryNotice isPending error={null} empty="" />
          )}
          {creative.error && <ErrorNotice error={creative.error} />}
          <div className="mode-segment" role="tablist" aria-label="创作模式">
            <button
              className={mode === "original" ? "selected" : ""}
              onClick={() => setModeOverride("original")}
              role="tab"
              aria-selected={mode === "original"}
            >
              原创
            </button>
            <button
              className={mode === "adaptation" ? "selected" : ""}
              onClick={() => setModeOverride("adaptation")}
              role="tab"
              aria-selected={mode === "adaptation"}
            >
              改编
            </button>
          </div>
          <div className="brief-fields">
            {(
              [
                ["主题", "subject"],
                ["题材", "genre"],
                ["受众", "audience"],
                ["人物设想", "characterPremise"],
                ["风格", "style"],
              ] as const
            ).map(([label, field]) => (
              <label className="field-label" key={field}>
                {label}
                <input
                  value={draft[field]}
                  onChange={(event) =>
                    setDraftOverride({ ...draft, [field]: event.target.value })
                  }
                  placeholder={`填写 ${label}`}
                />
              </label>
            ))}
            <div className="number-fields">
              {(
                [
                  ["每集时长", "episodeDurationSeconds"],
                  ["集数", "episodeCount"],
                  ["每集场数", "scenesPerEpisode"],
                  ["每场镜头", "shotsPerScene"],
                ] as const
              ).map(([label, field]) => (
                <label className="field-label" key={field}>
                  {label}
                  <input
                    type="number"
                    min="1"
                    value={draft[field]}
                    onChange={(event) =>
                      setDraftOverride({
                        ...draft,
                        [field]: Number(event.target.value),
                      })
                    }
                  />
                </label>
              ))}
            </div>
          </div>
          {mode === "adaptation" && (
            <SourceMaterialPanel projectId={projectId} />
          )}
          <div className="surface-actions">
            <span className="schema-note mono">
              schemaVersion 1.0.0 / owner revision{" "}
              {currentBrief?.revision ?? "--"}
            </span>
            <button
              className="secondary-button"
              disabled={
                !creative.data || saveBrief.isPending || !draft.subject.trim()
              }
              onClick={() => saveBrief.mutate()}
            >
              {saveBrief.isPending ? "保存中..." : "保存 CreativeBrief"}
              <Check size={15} />
            </button>
          </div>
          {briefMessage && (
            <div className="inline-confirm">
              <CircleCheck size={15} /> {briefMessage}
            </div>
          )}
        </section>
        <section className="surface run-surface">
          <SurfaceHeading
            label="WORKFLOWS / RUNS OWNER"
            title={run ? `Run ${run.id.slice(0, 8)}` : "尚未开始文本 Run"}
            trailing={
              <span className={`status-tag ${run ? run.status : "neutral"}`}>
                {run?.status ?? "idle"}
              </span>
            }
          />
          <div className="run-meter">
            <div className={`meter-fill ${run ? "active" : ""}`} />
          </div>
          <div className="run-rows">
            <div>
              <span>WorkflowVersion</span>
              <strong>drama-mvp-a-default / published</strong>
            </div>
            <div>
              <span>Selection</span>
              <strong>Mock Provider + Local offline</strong>
            </div>
            <div>
              <span>Adapter</span>
              <strong className="mono">local_workspace</strong>
            </div>
            <div>
              <span>BudgetGate</span>
              <strong className="warning-text">unknown / 需确认</strong>
            </div>
          </div>
          <button
            className="primary-button full"
            disabled={!currentBrief || startRun.isPending}
            onClick={() => startRun.mutate()}
          >
            {startRun.isPending
              ? "确保 Workflow 并启动..."
              : run
                ? "再次读取 owner Run"
                : "显式生成文本 Run"}
            <ArrowRight size={16} />
          </button>
          {run && (
            <RunSummary projectId={projectId} run={run} onUpdate={setRun} />
          )}
          {startRun.error && <ErrorNotice error={startRun.error} />}
        </section>
      </div>
      <SkillRoutePanel projectId={projectId} />
      <section className="surface projection-surface">
        <SurfaceHeading
          label="PROJECT / EPISODE S04-S09"
          title="同一事实源的项目投影"
          trailing={
            <span className="read-only-label">
              <CircleCheck size={14} /> read-only source
            </span>
          }
        />
        <div className="episode-bar">
          <label className="field-label compact">
            显式选择 Episode
            <select
              value={selectedEpisodeId ?? ""}
              onChange={(event) => setEpisode(event.target.value)}
            >
              <option value="">选择一个 Episode...</option>
              {episodeList.map((episode) => (
                <option key={episode.id} value={episode.id}>
                  {String(episode.number).padStart(2, "0")} / {episode.title}
                </option>
              ))}
            </select>
          </label>
          {selectedEpisode && (
            <span className="episode-revision mono">
              {selectedEpisode.id} / rev {selectedEpisode.revision}
            </span>
          )}
        </div>
        <div className="projection-tabs" role="tablist" aria-label="项目投影">
          <button
            className={view === "storyboard" ? "active" : ""}
            onClick={() => setView("storyboard")}
            role="tab"
          >
            Storyboard
          </button>
          <button
            className={view === "workflow" ? "active" : ""}
            onClick={() => setView("workflow")}
            role="tab"
          >
            Workflow source
          </button>
          <button
            className={view === "asset-bible" ? "active" : ""}
            onClick={() => setView("asset-bible")}
            role="tab"
          >
            AssetBible
          </button>
        </div>
        {view === "storyboard" && (
          <StoryboardView
            episodeId={selectedEpisodeId}
            state={storyboard}
            projectId={projectId}
          />
        )}
        {view === "workflow" && <WorkflowView state={workflow} />}
        {view === "asset-bible" && <AssetBibleView projectId={projectId} />}
      </section>
    </section>
  );
}

function SourceMaterialPanel({ projectId }: { projectId: string }) {
  const [materialType, setMaterialType] = useState<
    "novel" | "synopsis" | "existing_script"
  >("novel");
  const [inputMode, setInputMode] = useState<"inline_text" | "uploaded_file">(
    "inline_text",
  );
  const [content, setContent] = useState("");
  const [assetVersionId, setAssetVersionId] = useState("");
  const [projection, setProjection] = useState<Record<string, unknown> | null>(
    null,
  );
  const create = useMutation({
    mutationFn: async () => {
      if (inputMode === "inline_text" && !content.trim())
        throw new Error("inline_text 需要显式内容");
      if (inputMode === "uploaded_file" && !assetVersionId.trim())
        throw new Error("uploaded_file 需要已验证 AssetVersion owner ID");
      const source = (await workbenchApi.createSourceMaterial(
        projectId,
        materialType,
        inputMode,
      )) as { id?: string; revision?: number };
      if (!source.id || !source.revision)
        throw new Error("SourceMaterial owner response 缺少 id/revision");
      return workbenchApi.appendSourceMaterial(
        source.id,
        source.revision,
        inputMode,
        inputMode === "inline_text" ? content : null,
        inputMode === "uploaded_file" ? assetVersionId.trim() : null,
      );
    },
    onSuccess: (value) => setProjection(value as Record<string, unknown>),
  });
  return (
    <div className="source-panel">
      <div className="source-panel-title">
        <FileText size={17} />
        <strong>SourceMaterial</strong>
        <span className="status-tag neutral">text owner</span>
      </div>
      <div className="source-options">
        <select
          value={materialType}
          onChange={(event) =>
            setMaterialType(event.target.value as typeof materialType)
          }
          aria-label="material type"
        >
          <option value="novel">novel</option>
          <option value="synopsis">synopsis</option>
          <option value="existing_script">existing_script</option>
        </select>
        <select
          value={inputMode}
          onChange={(event) =>
            setInputMode(event.target.value as typeof inputMode)
          }
          aria-label="input mode"
        >
          <option value="inline_text">inline_text</option>
          <option value="uploaded_file">uploaded_file</option>
        </select>
      </div>
      {inputMode === "inline_text" ? (
        <textarea
          value={content}
          onChange={(event) => setContent(event.target.value)}
          placeholder="粘贴待解析文本；不会创建 Storage session"
          aria-label="source inline content"
        />
      ) : (
        <input
          value={assetVersionId}
          onChange={(event) => setAssetVersionId(event.target.value)}
          placeholder="已验证 AssetVersion owner ID"
          aria-label="source asset version"
        />
      )}
      <p>
        SourceMaterial 先创建 immutable owner，再
        append/parse/validate。上传本体必须先由 Storage/Assets owner
        完成，不在此处伪造。
      </p>
      <button
        className="secondary-button"
        disabled={create.isPending}
        onClick={() => create.mutate()}
      >
        {create.isPending ? "导入并验证..." : "显式导入 SourceMaterial"}{" "}
        <ArrowRight size={14} />
      </button>
      {projection && (
        <div className="inline-confirm">
          <CircleCheck size={15} /> source owner 已返回{" "}
          {String(projection.id ?? "unknown")} / rev{" "}
          {String(projection.revision ?? "--")}；仍需 projects owner 显式
          binding。
        </div>
      )}
      {create.error && <ErrorNotice error={create.error} />}
    </div>
  );
}

function SkillRoutePanel({ projectId }: { projectId: string }) {
  const [route, setRoute] = useState<Awaited<
    ReturnType<typeof workbenchApi.resolveSkillRoute>
  > | null>(null);
  const resolve = useMutation({
    mutationFn: () => workbenchApi.resolveSkillRoute(projectId),
    onSuccess: (value) => setRoute(value),
  });
  const select = useMutation({
    mutationFn: (candidate: { name: string; version: string }) => {
      if (!route) return Promise.reject(new Error("尚未读取 Skill route"));
      return workbenchApi.selectSkillRoute(
        projectId,
        route.id,
        candidate.name,
        candidate.version,
        route.revision,
      );
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: queryKeys.skillRoutes(projectId),
      });
    },
  });
  return (
    <section className="surface route-panel">
      <SurfaceHeading
        label="SKILL ROUTE / EXPLICIT DECISION"
        title="文本 Skill 候选"
        trailing={<span className="status-tag neutral">页面读取不路由</span>}
      />
      <p className="muted route-copy">
        只有显式请求候选并选择固定 revision 后，Run 才能携带 route
        decision；设置页不会替代本次裁决。
      </p>
      <button
        className="secondary-button"
        disabled={resolve.isPending}
        onClick={() => resolve.mutate()}
      >
        {resolve.isPending ? "读取候选..." : "请求 Skill 候选"}{" "}
        <RefreshCw size={14} />
      </button>
      {resolve.error && <ErrorNotice error={resolve.error} />}
      {route && (
        <div className="route-candidate-list">
          <div className="mono">
            {route.id} / rev {route.revision} /{" "}
            {route.needsManualSelection ? "需人工选择" : "可冻结"}
          </div>
          {route.fallbackReason && (
            <div className="warning-line">
              <CircleAlert size={14} /> {route.fallbackReason}
            </div>
          )}
          {route.candidates.map((candidate) => (
            <div
              className="route-candidate"
              key={`${candidate.name}@${candidate.version}`}
            >
              <span>
                <strong>{candidate.name}</strong>
                <small className="mono">
                  {candidate.version} / score {candidate.score}
                </small>
              </span>
              <button
                className="secondary-button"
                disabled={select.isPending}
                onClick={() => select.mutate(candidate)}
              >
                冻结此 revision <Check size={14} />
              </button>
            </div>
          ))}
          {Boolean(select.data) && (
            <div className="inline-confirm">
              <CircleCheck size={15} /> Skill route selection 已冻结，后续 Run
              使用 owner 返回的 revision。
            </div>
          )}
          {select.error && <ErrorNotice error={select.error} />}
        </div>
      )}
    </section>
  );
}

function RunSummary({
  projectId,
  run,
  onUpdate,
}: {
  projectId: string;
  run: RunDetail;
  onUpdate: (run: RunDetail) => void;
}) {
  const [error, setError] = useState<unknown>(null);
  const [snapshotId, setSnapshotId] = useState<string | null>(null);
  const cancel = useMutation({
    mutationFn: () => workbenchApi.cancelRun(projectId, run.id, run.revision),
    onSuccess: onUpdate,
    onError: setError,
  });
  const canCancel =
    run.status === "queued" ||
    run.status === "running" ||
    run.status === "waiting_review";
  const events = useQuery({
    queryKey: ["runs", run.id, "events", run.latestEventSequence],
    queryFn: () =>
      workbenchApi.getRunEvents(projectId, run.id, run.latestEventSequence),
    enabled: false,
  });
  const snapshots = useQuery({
    queryKey: ["projects", projectId, "run-input-snapshots"],
    queryFn: () => workbenchApi.listRunInputSnapshots(projectId),
    enabled: false,
  });
  const successor = useMutation({
    mutationFn: () =>
      workbenchApi.createSuccessorRun(projectId, run.id, run.revision),
    onSuccess: onUpdate,
    onError: setError,
  });
  const rerun = useMutation({
    mutationFn: () => {
      if (!snapshotId)
        return Promise.reject(new Error("请先选择历史 RunInputSnapshot"));
      const item = (
        snapshots.data as Array<{ id: string; revision: number }> | undefined
      )?.find((value) => value.id === snapshotId);
      if (!item)
        return Promise.reject(
          new Error("历史 snapshot 不在当前项目 owner scope"),
        );
      return workbenchApi.rerunHistorical(projectId, item.id, item.revision);
    },
    onSuccess: onUpdate,
    onError: setError,
  });
  return (
    <div className="run-summary">
      <div>
        <span className="micro-label">RUN DETAIL</span>
        <strong className="mono">{run.id}</strong>
      </div>
      <div className="run-summary-grid">
        <span>
          event seq <b>{run.latestEventSequence}</b>
        </span>
        <span>
          workflow rev <b>{run.workflowVersionNumber}</b>
        </span>
        <span>
          nodes <b>{run.nodes.length}</b>
        </span>
      </div>
      {run.failure && (
        <div className="warning-line">
          <CircleAlert size={14} /> {String(run.failure.code)}:{" "}
          {String(run.failure.message)}
        </div>
      )}
      {canCancel && (
        <button
          className="danger-button"
          disabled={cancel.isPending}
          onClick={() => cancel.mutate()}
        >
          <CircleAlert size={15} />{" "}
          {cancel.isPending ? "取消请求中..." : "取消 Run"}
        </button>
      )}
      <div className="run-detail-actions">
        <button
          className="secondary-button"
          onClick={() => void events.refetch()}
        >
          <RefreshCw size={15} /> 读取新事件
        </button>
        {run.allowedActions.createSuccessor && (
          <button
            className="secondary-button"
            disabled={successor.isPending}
            onClick={() => successor.mutate()}
          >
            <ArrowRight size={15} /> 从失败节点继续
          </button>
        )}
        <button
          className="secondary-button"
          onClick={() => void snapshots.refetch()}
        >
          <HistoryIcon /> 历史输入快照
        </button>
      </div>
      {events.error && <ErrorNotice error={events.error} />}
      {events.data && (
        <div className="event-list">
          {(
            events.data as Array<{
              id: string;
              sequence: number;
              eventType: string;
            }>
          ).map((event) => (
            <span className="event-chip mono" key={event.id}>
              #{event.sequence} {event.eventType}
            </span>
          ))}
        </div>
      )}
      {snapshots.data !== undefined && snapshots.data !== null && (
        <div className="snapshot-picker">
          <label className="field-label compact">
            选择 immutable RunInputSnapshot
            <select
              value={snapshotId ?? ""}
              onChange={(event) => setSnapshotId(event.target.value || null)}
            >
              <option value="">请选择...</option>
              {(
                snapshots.data as Array<{
                  id: string;
                  revision: number;
                  runnable?: boolean;
                  diagnostic?: string | null;
                }>
              ).map((item) => (
                <option
                  key={item.id}
                  value={item.id}
                  disabled={item.runnable === false}
                >
                  {item.id.slice(0, 8)} / rev {item.revision}
                  {item.runnable === false ? " / unrunnable" : ""}
                </option>
              ))}
            </select>
          </label>
          <button
            className="secondary-button"
            disabled={!snapshotId || rerun.isPending}
            onClick={() => rerun.mutate()}
          >
            {rerun.isPending ? "创建新 Run..." : "按精确快照重新运行"}
          </button>
        </div>
      )}
      <div className="node-detail-list">
        {run.nodes.map((node) => (
          <div className="node-detail-row" key={node.id}>
            <span className="mono">{node.nodeKey}</span>
            <span className={`status-tag ${node.status}`}>{node.status}</span>
            <small className="mono">{node.logicalOperation}</small>
          </div>
        ))}
      </div>
      {error != null && <ErrorNotice error={error} />}
    </div>
  );
}

function HistoryIcon() {
  return <RefreshCw size={15} />;
}

function StoryboardView({
  episodeId,
  state,
  projectId,
}: {
  episodeId: string | null;
  state: { data?: SceneProjection[]; isPending: boolean; error: unknown };
  projectId: string;
}) {
  const moveItem = <T,>(items: T[], from: number, to: number): T[] => {
    const next = items.slice();
    const [item] = next.splice(from, 1);
    next.splice(to, 0, item);
    return next;
  };
  const slice = usePresentationStore((store) =>
    episodeId
      ? (store.slices[episodeSliceKey(projectId, episodeId)] ??
        EMPTY_EPISODE_SLICE)
      : null,
  );
  const patchSlice = usePresentationStore((store) => store.patchSlice);
  const storyboardKey = episodeId
    ? queryKeys.storyboard(projectId, episodeId)
    : (["disabled"] as const);
  const reorderScenes = useMutation({
    mutationFn: (sceneIds: string[]) => {
      if (!episodeId) return Promise.reject(new Error("请先选择 Episode"));
      const current =
        queryClient.getQueryData<SceneProjection[]>(storyboardKey);
      const expectedRevision = current?.[0]?.sceneOrderRevision ?? 1;
      return workbenchApi.reorderScenes(
        projectId,
        episodeId,
        sceneIds,
        expectedRevision,
      );
    },
    onMutate: async (sceneIds) => {
      if (!episodeId) return undefined;
      await queryClient.cancelQueries({ queryKey: storyboardKey });
      const previous =
        queryClient.getQueryData<SceneProjection[]>(storyboardKey);
      if (previous) {
        const byId = new Map(previous.map((scene) => [scene.id, scene]));
        queryClient.setQueryData(
          storyboardKey,
          sceneIds.map((id, index) => ({
            ...byId.get(id)!,
            number: index + 1,
          })),
        );
      }
      return { previous };
    },
    onError: (_error, _sceneIds, context) => {
      if (context?.previous)
        queryClient.setQueryData(storyboardKey, context.previous);
      void queryClient.invalidateQueries({ queryKey: storyboardKey });
    },
    onSuccess: (data) => queryClient.setQueryData(storyboardKey, data),
  });
  const reorderShots = useMutation({
    mutationFn: ({
      scene,
      shotIds,
    }: {
      scene: SceneProjection;
      shotIds: string[];
    }) => {
      if (!episodeId) return Promise.reject(new Error("请先选择 Episode"));
      return workbenchApi.reorderShots(
        projectId,
        episodeId,
        scene.id,
        shotIds,
        scene.revision,
      );
    },
    onMutate: async ({ scene, shotIds }) => {
      if (!episodeId) return undefined;
      await queryClient.cancelQueries({ queryKey: storyboardKey });
      const previous =
        queryClient.getQueryData<SceneProjection[]>(storyboardKey);
      if (previous) {
        queryClient.setQueryData(
          storyboardKey,
          previous.map((item) => {
            if (item.id !== scene.id) return item;
            const byId = new Map(item.shots.map((shot) => [shot.id, shot]));
            return {
              ...item,
              shots: shotIds.map((id, index) => ({
                ...byId.get(id)!,
                number: index + 1,
              })),
            };
          }),
        );
      }
      return { previous };
    },
    onError: (_error, _variables, context) => {
      if (context?.previous)
        queryClient.setQueryData(storyboardKey, context.previous);
      void queryClient.invalidateQueries({ queryKey: storyboardKey });
    },
    onSuccess: (data) => queryClient.setQueryData(storyboardKey, data),
  });
  if (!episodeId)
    return (
      <div className="projection-empty">
        <Activity size={24} />
        <strong>请先选择 Episode</strong>
        <span>
          Timeline、Shot 选择与折叠状态都必须绑定明确的 projectId + episodeId。
        </span>
      </div>
    );
  if (state.isPending) return <QueryNotice isPending error={null} empty="" />;
  if (state.error) return <ErrorNotice error={state.error} />;
  const scenes = state.data ?? [];
  const filter = slice?.filters.status ?? "all";
  const toggleScene = (sceneId: string) => {
    const collapsed = new Set(slice?.collapsedSceneIds ?? []);
    if (collapsed.has(sceneId)) collapsed.delete(sceneId);
    else collapsed.add(sceneId);
    patchSlice(projectId, episodeId, { collapsedSceneIds: [...collapsed] });
  };
  return (
    <div className="storyboard-view">
      <div className="projection-toolbar">
        <span className="micro-label">
          {scenes.length} scenes /{" "}
          {scenes.reduce((sum, scene) => sum + scene.shots.length, 0)} shots
        </span>
        <div className="toolbar-right">
          <select
            value={filter}
            onChange={(event) =>
              patchSlice(projectId, episodeId, {
                filters: {
                  ...(slice?.filters ?? { model: "all", review: "all" }),
                  status: event.target.value,
                },
              })
            }
            aria-label="筛选镜头"
          >
            <option value="all">所有状态</option>
            <option value="ready">ready</option>
            <option value="pending">pending</option>
            <option value="stale">stale</option>
          </select>
          <span className="read-only-label">projection only</span>
        </div>
      </div>
      {scenes.length === 0 && (
        <div className="projection-empty">
          <Layers3 size={24} />
          <strong>当前 Episode 尚无 Scene/Shot</strong>
          <span>不会自动选择其他 Episode，也不会创建模板或 Run。</span>
        </div>
      )}
      {scenes.map((scene, sceneIndex) => {
        const collapsed = slice?.collapsedSceneIds.includes(scene.id) ?? false;
        const shots = scene.shots.filter(
          (shot) => filter === "all" || shot.status === filter,
        );
        return (
          <div className="scene-block" key={scene.id}>
            <div className="scene-header">
              <button
                className="scene-toggle"
                onClick={() => toggleScene(scene.id)}
                aria-expanded={!collapsed}
              >
                <span className="scene-index">
                  SC {String(scene.number).padStart(2, "0")}
                </span>
                <span>
                  <strong>{scene.title || "未命名场次"}</strong>
                  <small className="mono">
                    {scene.id} / rev {scene.revision}
                  </small>
                </span>
                <span className="scene-count">{shots.length} shots</span>
                <ChevronDown
                  className={collapsed ? "chevron-closed" : ""}
                  size={16}
                />
              </button>
              <div className="scene-reorder-actions" aria-label="场次排序">
                <button
                  className="icon-button"
                  title="场次上移"
                  aria-label="场次上移"
                  disabled={sceneIndex === 0 || reorderScenes.isPending}
                  onClick={() =>
                    reorderScenes.mutate(
                      moveItem(
                        scenes.map((item) => item.id),
                        sceneIndex,
                        sceneIndex - 1,
                      ),
                    )
                  }
                >
                  <ArrowLeft size={14} />
                </button>
                <button
                  className="icon-button"
                  title="场次下移"
                  aria-label="场次下移"
                  disabled={
                    sceneIndex === scenes.length - 1 || reorderScenes.isPending
                  }
                  onClick={() =>
                    reorderScenes.mutate(
                      moveItem(
                        scenes.map((item) => item.id),
                        sceneIndex,
                        sceneIndex + 1,
                      ),
                    )
                  }
                >
                  <ArrowRight size={14} />
                </button>
              </div>
            </div>
            {!collapsed && (
              <div className="shot-grid">
                {shots.map((shot, shotIndex) => (
                  <ShotCard
                    key={shot.id}
                    projectId={projectId}
                    episodeId={episodeId}
                    scene={scene}
                    shot={shot}
                    canMoveUp={shotIndex > 0}
                    canMoveDown={shotIndex < shots.length - 1}
                    onMove={(direction) => {
                      const shotIds = scene.shots.map((item) => item.id);
                      const nextIndex =
                        direction === "up" ? shotIndex - 1 : shotIndex + 1;
                      reorderShots.mutate({
                        scene,
                        shotIds: moveItem(shotIds, shotIndex, nextIndex),
                      });
                    }}
                    onSelect={() =>
                      patchSlice(projectId, episodeId, {
                        selectedShotId: shot.id,
                      })
                    }
                  />
                ))}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}

function ShotCard({
  projectId,
  episodeId,
  scene,
  shot,
  canMoveUp,
  canMoveDown,
  onMove,
  onSelect,
}: {
  projectId: string;
  episodeId: string;
  scene: SceneProjection;
  shot: SceneProjection["shots"][number];
  canMoveUp: boolean;
  canMoveDown: boolean;
  onMove: (direction: "up" | "down") => void;
  onSelect: () => void;
}) {
  const timelineReady = Boolean(
    shot.currentVideo?.timelineReady || shot.currentImage?.timelineReady,
  );
  const continuityBlocked = Boolean(shot.continuityTasks.some((task) => task));
  const reviewMedia = shot.currentVideo ?? shot.currentImage;
  const reviewParams = new URLSearchParams({
    episodeId,
    shotId: shot.id,
  });
  if (reviewMedia && shot.continuitySnapshot) {
    reviewParams.set("assetVersionId", reviewMedia.assetVersionId);
    reviewParams.set(
      "assetVersionRevision",
      String(reviewMedia.assetVersionRevision),
    );
    reviewParams.set("assetVersionHash", reviewMedia.assetVersionHash);
    reviewParams.set("continuitySnapshotId", shot.continuitySnapshot.ownerId);
    reviewParams.set(
      "continuitySnapshotRevision",
      String(shot.continuitySnapshot.revision),
    );
    reviewParams.set(
      "continuitySnapshotHash",
      shot.continuitySnapshot.contentHash,
    );
  }
  return (
    <article className="shot-card owner-card">
      <div className="shot-art">
        <span className="scene-index">
          SC {String(scene.number).padStart(2, "0")}
        </span>
        <span className="frame-stamp mono">
          SHOT {String(shot.number).padStart(2, "0")}
        </span>
        <div className="shot-art-placeholder">
          <Clapperboard size={22} />
          <span>
            {shot.currentVideo || shot.currentImage
              ? "owner media reference"
              : "暂无 media"}
          </span>
        </div>
      </div>
      <div className="shot-copy">
        <div className="shot-card-title">
          <strong>Shot {String(shot.number).padStart(2, "0")}</strong>
          <span className={`status-tag ${shot.status}`}>{shot.status}</span>
        </div>
        <span className="mono owner-ref">
          {shot.id} / rev {shot.revision}
        </span>
        <div className="shot-facts">
          <span>
            spec <b>{shot.specRef?.ownerId ?? "--"}</b>
          </span>
          <span>
            continuity <b>{shot.continuitySnapshot?.ownerId ?? "未绑定"}</b>
          </span>
        </div>
        <div className="shot-gates">
          {continuityBlocked && (
            <span className="gate-warning">
              <CircleAlert size={13} /> continuity task pending
            </span>
          )}
          {!timelineReady && (
            <span className="gate-warning">
              <CircleAlert size={13} /> derivative 未 ready
            </span>
          )}
        </div>
        <div className="shot-actions">
          <button className="icon-button" title="选择镜头" onClick={onSelect}>
            <Check size={15} />
          </button>
          <button
            className="icon-button"
            title="镜头上移"
            aria-label="镜头上移"
            disabled={!canMoveUp}
            onClick={() => onMove("up")}
          >
            <ArrowLeft size={14} />
          </button>
          <button
            className="icon-button"
            title="镜头下移"
            aria-label="镜头下移"
            disabled={!canMoveDown}
            onClick={() => onMove("down")}
          >
            <ArrowRight size={14} />
          </button>
          <Link
            className="text-button"
            to={`/projects/${projectId}/review?${reviewParams.toString()}`}
          >
            审核 <ArrowRight size={14} />
          </Link>
          {timelineReady && !continuityBlocked ? (
            <Link
              className="text-button"
              to={`/projects/${projectId}/episodes/${episodeId}/timeline?shotId=${shot.id}`}
            >
              Timeline <ArrowRight size={14} />
            </Link>
          ) : (
            <span className="disabled-action">Timeline locked</span>
          )}
        </div>
      </div>
    </article>
  );
}

function WorkflowView({
  state,
}: {
  state: {
    data?: {
      definition: { nodes: unknown[] };
      contentHash: string;
      revision: number;
      versionNumber: number;
      templateKey: string;
    };
    isPending: boolean;
    error: unknown;
  };
}) {
  if (state.isPending) return <QueryNotice isPending error={null} empty="" />;
  if (state.error) return <ErrorNotice error={state.error} />;
  if (!state.data)
    return (
      <div className="projection-empty">
        <Workflow size={24} />
        <strong>Workflow source 尚未读取</strong>
        <span>
          仅由用户显式生成 command ensure；页面加载不会创建或升级 Workflow。
        </span>
      </div>
    );
  return (
    <div className="workflow-readonly">
      <div className="workflow-source-line">
        <span className="status-tag ready">published</span>
        <strong>{state.data.templateKey}</strong>
        <span className="mono">
          v{state.data.versionNumber} / rev {state.data.revision}
        </span>
        <span className="mono">
          sha256 {state.data.contentHash.slice(0, 14)}...
        </span>
      </div>
      <div className="workflow-node-list">
        {state.data.definition.nodes.map((node, index) => (
          <div className="workflow-node" key={index}>
            <span className="node-number">
              {String(index + 1).padStart(2, "0")}
            </span>
            <span>
              {typeof node === "object" && node && "key" in node
                ? String((node as { key: unknown }).key)
                : `node-${index + 1}`}
            </span>
            <span className="read-only-label">read-only</span>
          </div>
        ))}
      </div>
      <div className="unsupported-note">
        <CircleAlert size={15} /> graph edit / connect / draft save / publish
        属于 MVP-B，本页面没有对应 mutation。
      </div>
    </div>
  );
}

function AssetBibleView({ projectId }: { projectId: string }) {
  const entries = useQuery({
    queryKey: queryKeys.bible(projectId),
    queryFn: async () => {
      const response = await fetch(
        `/api/v1/projects/${projectId}/asset-bible/entries`,
        { headers: { "X-Project-Scope": projectId } },
      );
      if (!response.ok)
        throw new Error(`AssetBible owner unavailable (${response.status})`);
      return response.json() as Promise<unknown[]>;
    },
    enabled: Boolean(projectId),
  });
  return (
    <div className="asset-bible-view">
      {entries.isPending && <QueryNotice isPending error={null} empty="" />}
      {entries.error && <ErrorNotice error={entries.error} />}
      {!entries.isPending && !entries.error && entries.data?.length === 0 && (
        <div className="projection-empty">
          <Boxes size={24} />
          <strong>AssetBible 尚无 entry</strong>
          <span>
            entry/version/override/resolved snapshot 由 AssetBible owner
            管理，工作台不会自动创建。
          </span>
        </div>
      )}
      {entries.data?.map((entry, index) => (
        <div className="bible-row" key={index}>
          <span className="entry-glyph">
            <Boxes size={15} />
          </span>
          <span>
            <strong>
              {String(
                (entry as { entryType?: string }).entryType ?? "typed entry",
              )}
            </strong>
            <small className="mono">owner projection / immutable version</small>
          </span>
          <span className="status-tag neutral">只读</span>
        </div>
      ))}
    </div>
  );
}

function LegacyTextReview() {
  const { projectId = "" } = useParams();
  const [decision, setDecision] = useState<string | null>(null);
  const [pendingDecision, setPendingDecision] = useState<
    "accept" | "reject" | "retake" | null
  >(null);
  const [mediaGate, setMediaGate] = useState<{
    status: "ready" | "blocked";
    missingOwners: string[];
  } | null>(null);
  const batches = useQuery({
    queryKey: queryKeys.textReview(projectId),
    queryFn: () => workbenchApi.listTextReviews(projectId),
    enabled: Boolean(projectId),
  });
  const batch = batches.data?.[0];
  const decide = useMutation({
    mutationFn: (action: "accept" | "reject" | "retake") => {
      if (!batch) return Promise.reject(new Error("没有可审核 batch"));
      return workbenchApi.decideTextReview(batch.id, batch.revision, action);
    },
    onSuccess: (result) => {
      const handoff = (result as { handoff?: { id?: string } } | null)?.handoff;
      if (handoff?.id) {
        void workbenchApi
          .getMediaGate(handoff.id)
          .then((gate) => setMediaGate(gate));
      }
      void queryClient.invalidateQueries({
        queryKey: queryKeys.textReview(projectId),
      });
    },
  });
  const confirmDecision = () => {
    if (!pendingDecision) return;
    setDecision(pendingDecision);
    decide.mutate(pendingDecision);
    setPendingDecision(null);
  };
  return (
    <section className="page-body">
      <PageIntro
        eyebrow="CANDIDATE REVIEW / S04-S07"
        title="一次确认，解锁下游媒体"
        detail="只显示 owner 返回的 TextReviewBatch 与 candidate。accept / reject / retake 是唯一审核动作。"
      />
      <div className="review-layout">
        <section className="surface candidate-stage">
          {batches.isPending && <QueryNotice isPending error={null} empty="" />}
          {batches.error && <ErrorNotice error={batches.error} />}
          {!batches.isPending && !batches.error && !batch && (
            <div className="projection-empty">
              <Layers3 size={24} />
              <strong>没有待审核 TextReviewBatch</strong>
              <span>
                请从项目工作台显式生成文本 Run；页面不会自行 ensure 或提交
                Provider。
              </span>
            </div>
          )}
          {batch && (
            <>
              <div className="batch-header">
                <div>
                  <span className="micro-label">TEXT REVIEW BATCH</span>
                  <h3>{batch.id}</h3>
                </div>
                <span className={`status-tag ${batch.status}`}>
                  {batch.status}
                </span>
              </div>
              <div className="candidate-list">
                {batch.candidates.map((candidate) => (
                  <div className="candidate-row" key={candidate.id}>
                    <FileText size={16} />
                    <span>
                      <strong>{candidate.kind}</strong>
                      <small className="mono">
                        {candidate.id} / rev {candidate.revision} /{" "}
                        {candidate.payloadHash.slice(0, 12)}...
                      </small>
                    </span>
                    <span className="status-tag neutral">
                      {candidate.status}
                    </span>
                  </div>
                ))}
              </div>
              <div className="decision-row">
                <button
                  className={`decision reject ${decision === "reject" ? "selected" : ""}`}
                  disabled={
                    decide.isPending || batch.status !== "pending_review"
                  }
                  onClick={() => {
                    setPendingDecision("reject");
                  }}
                >
                  Reject
                </button>
                <button
                  className={`decision retake ${decision === "retake" ? "selected" : ""}`}
                  disabled={
                    decide.isPending || batch.status !== "pending_review"
                  }
                  onClick={() => {
                    setPendingDecision("retake");
                  }}
                >
                  Retake
                </button>
                <button
                  className={`decision accept ${decision === "accept" ? "selected" : ""}`}
                  disabled={
                    decide.isPending || batch.status !== "pending_review"
                  }
                  onClick={() => {
                    setPendingDecision("accept");
                  }}
                >
                  Accept <CircleCheck size={16} />
                </button>
              </div>
              {decision && (
                <div className="inline-confirm">
                  <CircleCheck size={15} /> 已选择 {decision}；提交仍需 owner
                  revision、candidate hash 与全部 ack。
                </div>
              )}
              {decide.error && <ErrorNotice error={decide.error} />}
              {pendingDecision && (
                <div
                  className="decision-confirm"
                  role="dialog"
                  aria-label="确认审核动作"
                >
                  <strong>确认 {pendingDecision} 当前 TextReviewBatch？</strong>
                  <span>
                    只发送一次 owner command；旧 batch 与候选保持 immutable。
                  </span>
                  <div className="setting-actions">
                    <button
                      className="secondary-button"
                      onClick={() => setPendingDecision(null)}
                    >
                      取消
                    </button>
                    <button
                      className="primary-button"
                      onClick={confirmDecision}
                    >
                      确认提交
                    </button>
                  </div>
                </div>
              )}
            </>
          )}
        </section>
        <aside className="surface evidence-panel">
          <span className="micro-label">OWNER GATE</span>
          <h3>媒体入口状态</h3>
          <div className="evidence-list">
            <div>
              <span>Project/Episode ack</span>
              <strong
                className={
                  mediaGate?.status === "ready"
                    ? "success-text"
                    : "warning-text"
                }
              >
                {mediaGate?.status === "ready"
                  ? "ready"
                  : mediaGate?.missingOwners?.length
                    ? `缺少 ${mediaGate.missingOwners.join(", ")}`
                    : "待审核"}
              </strong>
            </div>
            <div>
              <span>AssetBible snapshot</span>
              <strong className="warning-text">待确认</strong>
            </div>
            <div>
              <span>Provider submit</span>
              <strong>blocked</strong>
            </div>
          </div>
          <div className="warning-line">
            <CircleAlert size={15} /> 未完成 batch ack 前不显示可执行媒体入口
          </div>
        </aside>
      </div>
    </section>
  );
}

function Review() {
  const { projectId = "" } = useParams();
  return (
    <>
      <AssetEditReviewPage projectId={projectId} />
      <LegacyTextReview />
    </>
  );
}

function TimelineSelector() {
  const { projectId = "" } = useParams();
  const [searchParams] = useSearchParams();
  const assetVersionId = searchParams.get("assetVersionId");
  const assetVersionRevision = searchParams.get("assetVersionRevision");
  const assetVersionHash = searchParams.get("assetVersionHash");
  const episodes = useQuery({
    queryKey: queryKeys.episodes(projectId),
    queryFn: () => workbenchApi.listEpisodes(projectId),
    enabled: Boolean(projectId),
  });
  const episodeList = [...(episodes.data ?? [])].sort(
    (left, right) =>
      left.number - right.number || left.id.localeCompare(right.id),
  );
  const navigate = useNavigate();
  const handoff = useMutation({
    mutationFn: async (episodeId: string) => {
      if (!assetVersionId) return null;
      return assetCenterApi.timelineSelection(
        projectId,
        assetVersionId,
        episodeId,
      );
    },
    onSuccess: (selection, episodeId) => {
      const suffix = selection
        ? `?assetVersionId=${encodeURIComponent(assetVersionId ?? "")}&assetVersionRevision=${encodeURIComponent(assetVersionRevision ?? "")}&assetVersionHash=${encodeURIComponent(assetVersionHash ?? "")}`
        : "";
      navigate(
        `/projects/${projectId}/episodes/${episodeId}/timeline${suffix}`,
      );
    },
  });
  return (
    <section className="page-body">
      <PageIntro
        eyebrow="EPISODE TIMELINE / S09"
        title="先选择一集"
        detail="Timeline 是 Episode owner 范围；不会从全部集视图推断 current Cut。"
      />
      <section className="surface selector-panel">
        <SurfaceHeading label="EPISODE SCOPE" title="显式选择 Episode" />
        {assetVersionId && (
          <div className="selection-handoff">
            <Boxes size={15} />
            <span>
              AssetVersion <span className="mono">{assetVersionId}</span>
            </span>
            <small className="mono">
              rev {assetVersionRevision ?? "?"} /{" "}
              {assetVersionHash?.slice(0, 12)}
            </small>
          </div>
        )}
        {episodes.isPending && <QueryNotice isPending error={null} empty="" />}
        {episodes.error && <ErrorNotice error={episodes.error} />}
        {!episodes.isPending &&
          !episodes.error &&
          episodes.data?.length === 0 && (
            <QueryNotice
              isPending={false}
              error={null}
              empty="当前项目尚无 Episode；不会创建模板入口。"
            />
          )}
        <div className="episode-list">
          {episodeList.map((episode) => (
            <button
              className="episode-row"
              key={episode.id}
              onClick={() =>
                assetVersionId
                  ? handoff.mutate(episode.id)
                  : navigate(
                      `/projects/${projectId}/episodes/${episode.id}/timeline`,
                    )
              }
              disabled={handoff.isPending}
            >
              <span className="scene-index">
                EP {String(episode.number).padStart(2, "0")}
              </span>
              <span>
                <strong>{episode.title}</strong>
                <small className="mono">
                  {episode.id} / rev {episode.revision}
                </small>
              </span>
              <ArrowRight size={16} />
            </button>
          ))}
        </div>
        {handoff.error && <ErrorNotice error={handoff.error} />}
      </section>
    </section>
  );
}

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Shell />
      </BrowserRouter>
    </QueryClientProvider>
  );
}
