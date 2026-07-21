"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ApiClient, Material, Project, WorkGenerationStep, WorkGenerationTask, WorkGenerationTaskCounts, WorkGenerationTaskDetails } from "../../lib/api";
import { cancelWorkGenerationRun, dismissWorkGenerationRun, getMaterial, getWorkGenerationTask, listWorkGenerationTasks, retryWorkGenerationStep } from "../../lib/api";

type Props = { client: ApiClient; project: Project | undefined; writesDisabled: boolean; onOpenMaterialLibrary?: () => void };
type View = "pending" | "running" | "completed" | "attention" | "cancelled";

const emptyCounts: WorkGenerationTaskCounts = { pending: 0, running: 0, completed: 0, attention: 0, cancelled: 0, total: 0 };

const stepLabels: Record<string, string> = { plan: "方案规划", tts: "TTS 配音", video_segment: "Seedance 分段", asr: "ASR 识别", subtitle: "字幕构建", mix: "本地混音", compose: "最终合成" };
const stepOrder = ["plan", "tts", "video_segment", "asr", "subtitle", "mix", "compose"];

export function WorkGenerationTaskPage({ client, project, writesDisabled, onOpenMaterialLibrary }: Props) {
  const initialRunId = useRef<string | null>(typeof window === "undefined" ? null : new URLSearchParams(window.location.search).get("run_id"));
  const [view, setView] = useState<View>("running");
  const [tasks, setTasks] = useState<WorkGenerationTask[]>([]);
  const [taskCounts, setTaskCounts] = useState<WorkGenerationTaskCounts>(emptyCounts);
  const [details, setDetails] = useState<WorkGenerationTaskDetails | null>(null);
  const [resultMaterials, setResultMaterials] = useState<Material[]>([]);
  const [selectedRunId, setSelectedRunId] = useState<string | null>(initialRunId.current);
  const selectedRunIdRef = useRef<string | null>(initialRunId.current);
  const detailRequestIdRef = useRef(0);
  const [deepLinkPending, setDeepLinkPending] = useState(Boolean(initialRunId.current));
  const [selectedStepId, setSelectedStepId] = useState<string | null>(null);
  const [showFilters, setShowFilters] = useState(false);
  const [query, setQuery] = useState("");
  const [stage, setStage] = useState("");
  const [includeHidden, setIncludeHidden] = useState(false);
  const [loading, setLoading] = useState(false);
  const [detailsLoading, setDetailsLoading] = useState(Boolean(initialRunId.current));
  const [error, setError] = useState("");

  const loadResultMaterials = useCallback(async (value: WorkGenerationTaskDetails) => {
    const ids = [...new Set(resultMaterialIds(value))];
    if (!ids.length) {
      setResultMaterials([]);
      return;
    }
    const materials = await Promise.all(ids.map(async (id) => {
      try { return await getMaterial(client, id); } catch { return null; }
    }));
    setResultMaterials(materials.filter((material): material is Material => Boolean(material)));
  }, [client]);

  const loadTaskDetails = useCallback(async (runId: string, showLoading: boolean) => {
    const requestId = ++detailRequestIdRef.current;
    if (showLoading) {
      setDetails(null);
      setResultMaterials([]);
      setSelectedStepId(null);
      setDetailsLoading(true);
    }
    try {
      const next = await getWorkGenerationTask(client, runId);
      if (detailRequestIdRef.current !== requestId || selectedRunIdRef.current !== runId) return null;
      setDetails(next);
      void loadResultMaterials(next);
      setSelectedStepId((current) => next.steps.some((step) => step.id === current) ? current : next.steps[0]?.id || null);
      return next;
    } catch (value) {
      if (detailRequestIdRef.current === requestId && selectedRunIdRef.current === runId) {
        setError(value instanceof Error ? value.message : "任务详情读取失败");
        selectedRunIdRef.current = null;
        setSelectedRunId(null);
      }
      return null;
    } finally {
      if (detailRequestIdRef.current === requestId) setDetailsLoading(false);
    }
  }, [client, loadResultMaterials]);

  const loadTasks = useCallback(async () => {
    if (!project || deepLinkPending) return;
    setLoading(true);
    setError("");
    try {
      const response = await listWorkGenerationTasks(client, project.project_id, { view, stage: stage || undefined, query: query.trim() || undefined, include_hidden: includeHidden });
      setTasks(response.tasks);
      setTaskCounts(response.counts);
      const selected = selectedRunIdRef.current;
      if (selected && response.tasks.some((task) => task.id === selected)) {
        await loadTaskDetails(selected, false);
      } else if (selected) {
        detailRequestIdRef.current += 1;
        selectedRunIdRef.current = null;
        setSelectedRunId(null);
        setDetails(null);
        setResultMaterials([]);
        setSelectedStepId(null);
        setDetailsLoading(false);
      }
    } catch (value) {
      setError(value instanceof Error ? value.message : "生成任务读取失败");
    } finally { setLoading(false); }
  }, [client, deepLinkPending, includeHidden, loadTaskDetails, project, query, stage, view]);

  useEffect(() => {
    const runId = initialRunId.current;
    if (!runId) return;
    initialRunId.current = null;
    const requestId = ++detailRequestIdRef.current;
    selectedRunIdRef.current = runId;
    void getWorkGenerationTask(client, runId)
      .then((next) => {
        if (detailRequestIdRef.current !== requestId || selectedRunIdRef.current !== runId) return;
        setDetails(next);
        void loadResultMaterials(next);
        setSelectedRunId(runId);
        setSelectedStepId(next.steps[0]?.id || null);
        setView(viewForTask(next.task));
      })
      .catch((value) => {
        if (detailRequestIdRef.current !== requestId) return;
        setError(value instanceof Error ? value.message : "任务详情读取失败");
        selectedRunIdRef.current = null;
        setSelectedRunId(null);
      })
      .finally(() => {
        if (detailRequestIdRef.current === requestId) setDetailsLoading(false);
        setDeepLinkPending(false);
      });
  }, [client, loadResultMaterials]);

  useEffect(() => { void loadTasks(); }, [loadTasks]);
  useEffect(() => { const timer = window.setInterval(() => { void loadTasks(); }, 5000); return () => window.clearInterval(timer); }, [loadTasks]);

  async function selectTask(runId: string) {
    selectedRunIdRef.current = runId;
    setSelectedRunId(runId);
    setError("");
    const params = new URLSearchParams(window.location.search);
    params.set("run_id", runId);
    window.history.replaceState(window.history.state, "", `${window.location.pathname}?${params}`);
    const next = await loadTaskDetails(runId, true);
    if (!next && selectedRunIdRef.current === null) {
      params.delete("run_id");
      window.history.replaceState(window.history.state, "", params.toString() ? `${window.location.pathname}?${params}` : window.location.pathname);
    }
  }

  async function cancelRun() {
    if (!details) return;
    const confirmation = details.task.cancel_mode === "provider"
      ? "确认向 provider 请求取消当前运行吗？取消结果以上游最终状态为准。"
      : "确认取消这个尚未开始外部调用的任务吗？";
    if (!window.confirm(confirmation)) return;
    try { setDetails(await cancelWorkGenerationRun(client, details.task.id)); await loadTasks(); }
    catch (value) { setError(value instanceof Error ? value.message : "任务取消失败"); }
  }

  async function dismissRun() {
    if (!details || !window.confirm("确认隐藏该失败任务吗？审计记录仍会保留。")) return;
    try { setDetails(await dismissWorkGenerationRun(client, details.task.id)); await loadTasks(); }
    catch (value) { setError(value instanceof Error ? value.message : "任务隐藏失败"); }
  }

  async function retryStep(step: WorkGenerationStep) {
    if (!details || !window.confirm(retryConfirmation(step, details.steps))) return;
    try { await retryWorkGenerationStep(client, step.id, crypto.randomUUID()); setDetails(await getWorkGenerationTask(client, details.task.id)); await loadTasks(); }
    catch (value) { setError(value instanceof Error ? value.message : "节点重试失败"); }
  }

  const displayedSteps = useMemo(() => buildDisplayedSteps(details?.steps || []), [details]);
  const selectedStep = displayedSteps.find((step) => step.id === selectedStepId) || displayedSteps[0] || null;
  const counts = useMemo(() => ({ succeeded: displayedSteps.filter((step) => step.status === "succeeded").length, running: displayedSteps.filter((step) => step.status === "running").length, queued: displayedSteps.filter((step) => step.status === "queued" || step.status === "blocked").length, failed: displayedSteps.filter((step) => step.status === "failed" || step.status === "waiting_manual").length }), [displayedSteps]);
  const runningCount = taskCounts.running;

  return <section aria-label="生成任务" className="workGenerationTasksWorkspace">
    <header className="workGenerationTasksHeader">
      <div><p className="sectionKicker">作品生产 / 生成任务</p><div className="workGenerationTaskTitleLine"><h2>生成任务</h2><span className="workGenerationTaskHeaderBadge">{runningCount} 个生成中</span><span className="workGenerationTaskHeaderBadge neutral">共 {taskCounts.total} 个任务</span></div></div>
      <button className="workGenerationTaskRefresh" type="button" onClick={() => void loadTasks()} disabled={loading}>刷新任务</button>
    </header>
    <div className="workGenerationTaskWorkspaceFrame">
      <section className="workGenerationTaskTablePanel" aria-label="作品生成任务列表">
        <header className="workGenerationTaskTableHeader"><strong>作品生成任务</strong><small>{tasks.length} 个运行 · 服务端聚合进度 · 最近更新 {tasks[0] ? formatTime(tasks[0].updated_at) : "--"}</small></header>
        <div className="workGenerationTaskListToolbar">
          <div className="workGenerationTaskTabs" role="tablist" aria-label="任务状态">{([["pending", "未生成"], ["running", "生成中"], ["completed", "已完成"]] as const).map(([key, label]) => <button key={key} className={view === key ? "active" : ""} onClick={() => changeView(key, setView)} role="tab" aria-selected={view === key} type="button">{label} <small>{taskCounts[key]}</small></button>)}</div>
          <button className={view === "attention" ? "workGenerationAttentionButton active" : "workGenerationAttentionButton"} onClick={() => changeView("attention", setView)} type="button"><span aria-hidden="true">!</span><strong>需处理&nbsp;&nbsp;{taskCounts.attention}</strong></button>
          <div className="workGenerationMoreFilterAnchor"><button className={showFilters || view === "cancelled" ? "workGenerationMoreFilter active" : "workGenerationMoreFilter"} onClick={() => setShowFilters((value) => !value)} type="button"><span aria-hidden="true">☷</span><strong>更多筛选</strong></button>{showFilters ? <div className="workGenerationTaskFilterPopover"><label>阶段<select aria-label="按阶段筛选" value={stage} onChange={(event) => setStage(event.target.value)}><option value="">全部阶段</option><option value="plan">方案规划</option><option value="tts">TTS 配音</option><option value="video_segment">Seedance 分段</option><option value="subtitle">字幕构建</option><option value="compose">最终合成</option></select></label><label>特殊状态<select aria-label="特殊状态" value={view === "cancelled" ? "cancelled" : ""} onChange={(event) => changeView(event.target.value === "cancelled" ? "cancelled" : "running", setView)}><option value="">主状态视图</option><option value="cancelled">已取消（{taskCounts.cancelled}）</option></select></label><label className="workGenerationHiddenFilter"><input type="checkbox" checked={includeHidden} onChange={(event) => setIncludeHidden(event.target.checked)} />显示已隐藏任务</label></div> : null}</div>
          <label className="workGenerationTaskSearch"><span aria-hidden="true">⌕</span><input aria-label="搜索作品或任务 ID" placeholder="搜索作品 / 任务 ID" value={query} onChange={(event) => setQuery(event.target.value)} /></label>
        </div>
        <div className="workGenerationTaskTable">
          <div className="workGenerationTaskTableRow head"><span>作品 / 版本</span><span>状态 / 阶段</span><span>聚合进度</span><span>子任务</span><span>资源用量</span><span>更新时间</span><span>操作</span></div>
          <div className="workGenerationTaskTableBody">{tasks.length ? tasks.map((task) => <TaskTableRow key={task.id} task={task} selected={task.id === selectedRunId} onSelect={() => void selectTask(task.id)} />) : <div className="workGenerationTaskEmpty"><strong>暂无任务</strong><span>当前状态下没有作品运行。</span></div>}</div>
          <footer className="workGenerationTaskTableFooter"><span>当前显示：{viewLabel(view)} {tasks.length} 个运行</span><span>1 / 1</span></footer>
        </div>
      </section>
      <aside className="workGenerationTaskDetailPanel" aria-label="任务步骤详情">
        {details ? <><header className="workGenerationTaskDetailHeader"><div><h3>{details.task.title} · V{details.task.version_no}</h3><small>{details.task.id.slice(0, 8)} · 作品版本 V{details.task.version_no} · 创建于 {formatTime(details.task.created_at)}</small></div><StatusBadge task={details.task} />{details.task.can_cancel ? <button className="workGenerationCancelRun" disabled={writesDisabled} onClick={() => void cancelRun()} type="button">取消运行</button> : null}</header><div className="workGenerationTaskDetailScroll">{details.task.cancel_block_reason ? <p className="workGenerationCancelBlockReason">{details.task.cancel_block_reason}</p> : null}<section className="workGenerationTaskProgressCard"><div><span>服务端聚合进度</span><strong>{details.task.progress_percent}%</strong></div><div className="workGenerationTaskProgress"><span style={{ width: `${Math.max(0, Math.min(100, details.task.progress_percent))}%` }} /></div><footer><span>成功 {details.steps.length ? counts.succeeded : details.task.successful_steps}</span><span>运行 {details.steps.length ? counts.running : details.task.running_steps}</span><span>排队 {details.steps.length ? counts.queued : details.task.queued_steps}</span><span>失败 {details.steps.length ? counts.failed : details.task.failed_steps}</span><span>{resourceLabel(details.task.resource_usage)}</span></footer></section>{resultMaterials.length ? <section className="workGenerationTaskResultCard" aria-label="生成成品"><div><strong>生成成品</strong><small>{resultMaterials.length} 个产物已登记到素材库</small></div>{resultMaterials.map((material) => <div className="workGenerationTaskResultItem" key={material.material_id}><video controls preload="metadata" src={material.file_url} /><div><strong>{material.file_name}</strong><button className="secondaryButton" type="button" onClick={onOpenMaterialLibrary}>在素材库查看</button></div></div>)}</section> : details.task.status === "succeeded" ? <p className="workGenerationTaskArtifactMissing">任务已完成，但成品尚未登记。请刷新任务，若仍未出现请将任务转为需处理。</p> : null}<div className="workGenerationTaskStepsHeading"><strong>执行步骤与调用审计</strong><small>按实际执行顺序 · 历史 attempt 永久保留</small></div><div className="workGenerationTaskDetailSteps">{displayedSteps.map((step) => <DetailStep key={step.id} step={step} selected={step.id === selectedStep?.id} onSelect={() => setSelectedStepId(step.id)} />)}</div></div>{details.task.status === "failed" || details.task.status === "waiting_manual" ? <footer className="workGenerationTaskDetailFooter"><button className="secondaryButton" disabled={writesDisabled} onClick={() => void dismissRun()} type="button">隐藏任务</button>{selectedStep && ["failed", "waiting_manual"].includes(selectedStep.status) ? <button className="primaryButton" disabled={writesDisabled} onClick={() => void retryStep(selectedStep)} type="button">重试失败节点</button> : null}</footer> : null}</> : detailsLoading ? <div className="workGenerationTaskEmpty" role="status"><strong>正在读取任务详情</strong><span>请稍候，正在加载步骤和调用审计。</span></div> : <div className="workGenerationTaskEmpty"><strong>选择一个运行</strong><span>右侧将展示步骤和调用审计。</span></div>}
      </aside>
    </div>
    {error ? <p className="errorText workGenerationTaskError">{error}</p> : null}
  </section>;
}

function TaskTableRow({ task, selected, onSelect }: { task: WorkGenerationTask; selected: boolean; onSelect: () => void }) { return <button aria-pressed={selected} className={selected ? "workGenerationTaskTableRow selected" : "workGenerationTaskTableRow"} onClick={onSelect} type="button"><span><strong>{task.title}</strong><small>V{task.version_no} · RUN-{task.id.slice(0, 8)}</small></span><span><StatusBadge task={task} /><small>{stepLabel(task.current_stage)}</small></span><span><strong>{task.progress_percent}% · {task.successful_steps + task.running_steps}/{Math.max(task.successful_steps + task.running_steps + task.queued_steps + task.failed_steps, 1)}</strong><i><b style={{ width: `${task.progress_percent}%` }} /></i></span><span>成功 {task.successful_steps} · 运行 {task.running_steps}<br />排队 {task.queued_steps} · 失败 {task.failed_steps}</span><span>{resourceLabel(task.resource_usage)}</span><span>{formatTime(task.updated_at)}<br /><small>最近更新</small></span><span className="workGenerationTaskViewAction">{selected ? "查看中" : "查看 →"}</span></button>; }
function DetailStep({ step, selected, onSelect }: { step: WorkGenerationStep; selected: boolean; onSelect: () => void }) { return <button className={selected ? "workGenerationTaskDetailStep selected" : "workGenerationTaskDetailStep"} type="button" onClick={onSelect}><span className={`workGenerationTaskStepIcon ${step.status}`}>{step.status === "succeeded" ? "✓" : step.status === "running" ? "◌" : step.status === "failed" ? "!" : "·"}</span><span><strong>{stepLabel(step.step_type)}</strong><small>{modelLabel(step.model_snapshot)} · {step.error_summary || (step.status === "queued" ? "等待依赖" : resourceLabel(step.resource_usage))}</small></span><em className={`workGenerationStepStatus ${step.status}`}>{statusLabel(step.status, step.is_required)}</em></button>; }
function StatusBadge({ task }: { task: WorkGenerationTask }) { const attention = task.failed_steps > 0 || ["failed", "waiting_manual"].includes(task.status); return <span className={`workGenerationTaskStatus ${attention ? "attention" : task.status}`}>{attention ? "需处理" : taskStatus(task.status)}</span>; }
function buildDisplayedSteps(steps: WorkGenerationStep[]) { if (steps.length) return [...steps].sort((a, b) => a.step_no - b.step_no); return stepOrder.map((stepType, index) => ({ id: `planned-${stepType}`, step_no: index + 1, step_type: stepType, status: index === 0 ? "succeeded" : "queued", is_required: true, depends_on: [], model_snapshot: {}, resource_usage: {}, result_material_ids: [], external_task_id: null, error_category: null, error_code: null, error_summary: null, attempts: [] } satisfies WorkGenerationStep)); }
function resultMaterialIds(value: WorkGenerationTaskDetails) { return value.steps.flatMap((step) => Array.isArray(step.result_material_ids) ? step.result_material_ids.map(String) : []); }
function viewLabel(view: View) { return ({ pending: "未生成", running: "生成中", completed: "已完成", attention: "需处理", cancelled: "已取消" } as Record<View, string>)[view]; }
function changeView(view: View, setter: (view: View) => void) { const params = new URLSearchParams(window.location.search); params.delete("run_id"); window.history.replaceState(window.history.state, "", params.toString() ? `${window.location.pathname}?${params}` : window.location.pathname); setter(view); }
function viewForTask(task: WorkGenerationTask): View { return task.failed_steps || ["failed", "waiting_manual"].includes(task.status) ? "attention" : task.status === "queued" ? "pending" : task.status === "succeeded" ? "completed" : task.status === "cancelled" ? "cancelled" : "running"; }
function taskStatus(status: string) { return status === "queued" ? "未生成" : status === "succeeded" ? "已完成" : status === "cancelled" ? "已取消" : status === "failed" || status === "waiting_manual" ? "需处理" : "生成中"; }
function statusLabel(status: string, required: boolean) { if (!required) return "未规划"; return status === "succeeded" ? "已完成" : status === "running" ? "生成中" : status === "queued" ? "排队" : status === "blocked" ? "等待依赖" : status === "failed" ? "失败" : status === "waiting_manual" ? "需处理" : status === "cancelled" ? "已取消" : status; }
function stepLabel(step: string) { return stepLabels[step] || (step === "queued" ? "等待执行" : step.replaceAll("_", " ")); }
function modelLabel(snapshot: Record<string, unknown>) { const value = snapshot.display_name || snapshot.model_id || snapshot.model || snapshot.tool || snapshot.provider; return value ? String(value).slice(0, 28) : "待执行时锁定"; }
function resourceLabel(resource: Record<string, unknown>) { const values = Object.entries(resource).filter(([key, value]) => typeof value === "number" && !/(cost|price|fee|budget|currency|金额|费用)/i.test(key)).slice(0, 3).map(([key, value]) => `${usageLabel(key)} ${value}`); return values.length ? values.join(" · ") : "暂无用量"; }
function usageLabel(key: string) { return ({ video_seconds: "视频", video_task_count: "视频任务", tts_characters: "TTS", asr_seconds: "ASR" } as Record<string, string>)[key] || key; }
function dependencyLabel(value: unknown) { return Array.isArray(value) && value.length ? value.map((item) => `步骤 ${item}`).join("、") : "无前置依赖"; }
function materialLabel(value: unknown) { return Array.isArray(value) && value.length ? `${value.length} 个结果素材` : "暂无结果素材"; }
function retryConfirmation(step: WorkGenerationStep, steps: WorkGenerationStep[]) {
  const affected = downstreamSteps(step.id, steps);
  const reusedMaterials = steps
    .filter((item) => item.status === "succeeded" && item.id !== step.id && !affected.has(item.id))
    .reduce((total, item) => total + (Array.isArray(item.result_material_ids) ? item.result_material_ids.length : 0), 0);
  return [
    `确认重试“${stepLabel(step.step_type)}”吗？`,
    `模型/工具：${modelLabel(step.model_snapshot)}`,
    `再次执行：1 个任务 · ${resourceLabel(step.resource_usage)}`,
    `必要下游：${affected.size} 个步骤`,
    `继续复用：${reusedMaterials} 个成功素材`,
  ].join("\n");
}
function downstreamSteps(stepId: string, steps: WorkGenerationStep[]) {
  const affected = new Set<string>();
  let frontier = [stepId];
  while (frontier.length) {
    const parents = new Set(frontier);
    frontier = steps
      .filter((step) => !affected.has(step.id) && dependencyIds(step.depends_on).some((id) => parents.has(id)))
      .map((step) => step.id);
    frontier.forEach((id) => affected.add(id));
  }
  return affected;
}
function dependencyIds(value: unknown) { return Array.isArray(value) ? value.map(String) : []; }
function formatTime(value: string) { const date = new Date(value); return Number.isNaN(date.valueOf()) ? value : date.toLocaleString("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit" }); }
