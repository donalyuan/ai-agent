"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import type {
  ApiClient,
  ApiError,
  PublicationPlanDetails,
  PublicationPlanSummary,
  PublicationPlatform,
  PublicationTarget,
  PublicationTargetStatus,
  WorkDetails,
} from "../../lib/api";
import {
  auditPublicationCopy,
  auditPublicationDownload,
  cancelPublicationTarget,
  confirmPublicationPublished,
  correctPublicationResult,
  generatePublicationPackage,
  getPublication,
  getPublicationDownloads,
  getWork,
  handoffPublicationTarget,
  listPublications,
  markPublicationNeedsAttention,
  savePublicationTarget,
} from "../../lib/api";

type Props = { client: ApiClient; planId: string | null; writesDisabled: boolean; onSelectPlan?: (planId: string) => void };
type WorkbenchView = "pending" | "history";
type PlatformFilter = "all" | PublicationPlatform;
type TimeFilter = "all" | "overdue" | "today" | "week";
type Draft = { title: string; body: string; tags: string; coverArtifactId: string; plannedAt: string };

const platforms: PublicationPlatform[] = ["douyin", "xiaohongshu"];
const pendingStatuses: PublicationTargetStatus[] = ["draft", "ready", "handed_off", "needs_attention"];
const historyStatuses: PublicationTargetStatus[] = ["published", "cancelled"];

export function PublicationWorkbenchPage({ client, planId, writesDisabled, onSelectPlan }: Props) {
  const [view, setView] = useState<WorkbenchView>("pending");
  const [plans, setPlans] = useState<PublicationPlanSummary[]>([]);
  const [selectedPlanId, setSelectedPlanId] = useState<string | null>(planId);
  const [plan, setPlan] = useState<PublicationPlanDetails | null>(null);
  const [work, setWork] = useState<WorkDetails | null>(null);
  const [drafts, setDrafts] = useState<Record<PublicationPlatform, Draft>>({
    douyin: emptyDraft(),
    xiaohongshu: emptyDraft(),
  });
  const [platformFilter, setPlatformFilter] = useState<PlatformFilter>("all");
  const [statusFilter, setStatusFilter] = useState<"all" | PublicationTargetStatus>("all");
  const [timeFilter, setTimeFilter] = useState<TimeFilter>("all");
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [readError, setReadError] = useState("");
  const [busyTargetId, setBusyTargetId] = useState<string | null>(null);
  const [actionErrors, setActionErrors] = useState<Partial<Record<PublicationPlatform, string>>>({});
  const [notices, setNotices] = useState<Partial<Record<PublicationPlatform, string>>>({});
  const [downloads, setDownloads] = useState<Record<string, Awaited<ReturnType<typeof getPublicationDownloads>>>>({});
  const [resultForms, setResultForms] = useState<Record<PublicationPlatform, { url: string; at: string }>>({
    douyin: { url: "", at: "" },
    xiaohongshu: { url: "", at: "" },
  });
  const [auditOpen, setAuditOpen] = useState<Partial<Record<PublicationPlatform, boolean>>>({});

  const load = useCallback(async (explicitPlanId: string | null) => {
    setLoading(true);
    setReadError("");
    try {
      const list = await listPublications(client);
      setPlans(list.items);
      if (!explicitPlanId) {
        setPlan(null);
        setWork(null);
        return;
      }
      const details = await getPublication(client, explicitPlanId);
      const workDetails = await getWork(client, details.work_id);
      setPlan(details);
      setWork(workDetails);
      setDrafts({
        douyin: targetToDraft(details.targets.find((target) => target.platform === "douyin")),
        xiaohongshu: targetToDraft(details.targets.find((target) => target.platform === "xiaohongshu")),
      });
      setResultForms({
        douyin: targetToResult(details.targets.find((target) => target.platform === "douyin")),
        xiaohongshu: targetToResult(details.targets.find((target) => target.platform === "xiaohongshu")),
      });
    } catch (value) {
      setPlan(null);
      setWork(null);
      setReadError(errorMessage(value, "发布计划读取失败"));
    } finally {
      setLoading(false);
    }
  }, [client]);

  useEffect(() => {
    setSelectedPlanId(planId);
    void load(planId);
  }, [load, planId]);

  const selectedSummary = plans.find((item) => item.id === selectedPlanId) ?? null;
  const workTitle = selectedSummary?.work_title || work?.title || "未命名作品";
  const versionLabel = useMemo(() => {
    const version = work?.versions.find((item) => item.id === plan?.work_version_id);
    if (version) return `V${version.version_no}`;
    const suffix = plan?.work_version_id.match(/(?:version-|v)(\d+)$/i)?.[1];
    return suffix ? `V${suffix}` : "明确版本";
  }, [plan?.work_version_id, work?.versions]);

  const visiblePlans = useMemo(() => plans.filter((item) => {
    const targets = item.targets.filter((target) => {
      const inView = view === "pending" ? pendingStatuses.includes(target.status) : historyStatuses.includes(target.status);
      return inView
        && (platformFilter === "all" || target.platform === platformFilter)
        && (statusFilter === "all" || target.status === statusFilter)
        && matchesTime(target, timeFilter);
    });
    const keyword = query.trim().toLocaleLowerCase("zh-CN");
    return targets.length > 0 && (!keyword || `${item.work_title} ${item.work_version_id}`.toLocaleLowerCase("zh-CN").includes(keyword));
  }), [plans, platformFilter, query, statusFilter, timeFilter, view]);

  async function runTargetAction(platform: PublicationPlatform, targetId: string, action: () => Promise<void>) {
    if (writesDisabled || busyTargetId) return;
    setBusyTargetId(targetId);
    setActionErrors((current) => ({ ...current, [platform]: "" }));
    setNotices((current) => ({ ...current, [platform]: "" }));
    try {
      await action();
    } catch (value) {
      setActionErrors((current) => ({ ...current, [platform]: actionErrorMessage(value) }));
    } finally {
      setBusyTargetId(null);
    }
  }

  function saveTarget(platform: PublicationPlatform, target: PublicationTarget | undefined) {
    const draft = drafts[platform];
    void runTargetAction(platform, target?.id ?? `new-${platform}`, async () => {
      await savePublicationTarget(client, plan!.id, platform, {
        expected_revision: target?.draft_revision ?? null,
        title: draft.title.trim(),
        body: draft.body.trim(),
        tags: parseTags(draft.tags),
        cover_artifact_id: draft.coverArtifactId || null,
        planned_at: localDateTimeToIso(draft.plannedAt),
      }, idempotencyKey());
      setNotices((current) => ({ ...current, [platform]: "草稿已保存，旧发布包已失效" }));
      await load(plan!.id);
    });
  }

  function generatePackage(platform: PublicationPlatform, target: PublicationTarget) {
    void runTargetAction(platform, target.id, async () => {
      await generatePublicationPackage(client, target.id, target.draft_revision, idempotencyKey());
      const manifest = await getPublicationDownloads(client, target.id);
      setDownloads((current) => ({ ...current, [target.id]: manifest }));
      setNotices((current) => ({ ...current, [platform]: "发布包已通过完整性校验" }));
      await load(plan!.id);
    });
  }

  function openOfficialPlatform(platform: PublicationPlatform, target: PublicationTarget) {
    void runTargetAction(platform, target.id, async () => {
      const result = await handoffPublicationTarget(client, target.id, idempotencyKey());
      if (!trustedOfficialEntrance(platform, result.official_entrance)) throw new Error("平台返回了非受信任官方入口，已阻止打开");
      window.open(result.official_entrance, "_blank", "noopener,noreferrer");
      setNotices((current) => ({ ...current, [platform]: "等待人工发布" }));
      await load(plan!.id);
    });
  }

  function copyText(platform: PublicationPlatform, target: PublicationTarget) {
    void runTargetAction(platform, target.id, async () => {
      const draft = drafts[platform];
      const text = [draft.title, draft.body, parseTags(draft.tags).join(" ")].filter(Boolean).join("\n\n");
      if (!navigator.clipboard?.writeText) throw new Error("当前浏览器不支持安全复制，请下载发布文案");
      await navigator.clipboard.writeText(text);
      await auditPublicationCopy(client, target.id, idempotencyKey());
      setNotices((current) => ({ ...current, [platform]: "文案已复制并记录审计" }));
    });
  }

  function downloadPackage(platform: PublicationPlatform, target: PublicationTarget) {
    void runTargetAction(platform, target.id, async () => {
      const manifest = downloads[target.id] ?? await getPublicationDownloads(client, target.id);
      setDownloads((current) => ({ ...current, [target.id]: manifest }));
      await auditPublicationDownload(client, target.id, idempotencyKey());
      window.open(manifest.package.download_url, "_blank", "noopener,noreferrer");
    });
  }

  function downloadFile(platform: PublicationPlatform, target: PublicationTarget, url: string) {
    void runTargetAction(platform, target.id, async () => {
      await auditPublicationDownload(client, target.id, idempotencyKey());
      window.open(url, "_blank", "noopener,noreferrer");
    });
  }

  function markNeedsAttention(platform: PublicationPlatform, target: PublicationTarget) {
    void runTargetAction(platform, target.id, async () => {
      await markPublicationNeedsAttention(client, target.id, idempotencyKey());
      await load(plan!.id);
    });
  }

  function cancelTarget(platform: PublicationPlatform, target: PublicationTarget) {
    void runTargetAction(platform, target.id, async () => {
      await cancelPublicationTarget(client, target.id, idempotencyKey());
      await load(plan!.id);
    });
  }

  function saveResult(platform: PublicationPlatform, target: PublicationTarget, correction: boolean) {
    const form = resultForms[platform];
    void runTargetAction(platform, target.id, async () => {
      const publishedAt = validatePublicationResult(platform, form);
      const payload = { published_url: form.url.trim(), published_at: publishedAt };
      if (correction) await correctPublicationResult(client, target.id, payload, idempotencyKey());
      else await confirmPublicationPublished(client, target.id, payload, idempotencyKey());
      setNotices((current) => ({ ...current, [platform]: correction ? "人工结果修正已记录" : "人工确认已发布" }));
      await load(plan!.id);
    });
  }

  return (
    <section className="publicationWorkspace">
      <header className="publicationHeader">
        <div><p className="sectionKicker">发布运营 / 发布工作台</p><h2>人工发布运营</h2><p>发布包准备、官方网页交接与人工结果登记</p></div>
        <div className="publicationViewTabs" aria-label="发布视图">
          <button aria-pressed={view === "pending"} className={view === "pending" ? "active" : ""} type="button" onClick={() => { setView("pending"); setStatusFilter("all"); }}>待发布</button>
          <button aria-pressed={view === "history"} className={view === "history" ? "active" : ""} type="button" onClick={() => { setView("history"); setStatusFilter("all"); }}>发布记录</button>
        </div>
      </header>

      <div className="publicationToolbar">
        <input aria-label="搜索发布计划" placeholder="搜索作品或版本" role="searchbox" value={query} onChange={(event) => setQuery(event.target.value)} />
        <select aria-label="平台筛选" value={platformFilter} onChange={(event) => setPlatformFilter(event.target.value as PlatformFilter)}><option value="all">全部平台</option><option value="douyin">抖音</option><option value="xiaohongshu">小红书</option></select>
        <select aria-label="状态筛选" value={statusFilter} onChange={(event) => setStatusFilter(event.target.value as typeof statusFilter)}><option value="all">全部状态</option>{(view === "pending" ? pendingStatuses : historyStatuses).map((status) => <option key={status} value={status}>{statusLabel(status)}</option>)}</select>
        <select aria-label="时间筛选" value={timeFilter} onChange={(event) => setTimeFilter(event.target.value as TimeFilter)}><option value="all">全部时间</option><option value="overdue">已逾期</option><option value="today">今天</option><option value="week">未来 7 天</option></select>
        <span>{visiblePlans.length} 个计划</span>
      </div>

      {readError ? <div className="publicationReadError" role="alert"><strong>发布数据读取失败</strong><span>{readError}</span><button type="button" onClick={() => void load(selectedPlanId)}>重新读取</button></div> : null}
      {loading ? <div className="publicationLoading">正在读取发布计划</div> : null}
      {!loading && !readError && !plans.length ? <EmptyState title="暂无发布计划" description="从作品库选择一个已完成版本进入发布。" /> : null}

      {!loading && !readError && plans.length ? (
        <div className="publicationLayout">
          <aside aria-label={view === "pending" ? "待发布计划" : "发布记录"} className="publicationQueue">
            {!visiblePlans.length ? <EmptyState title="没有匹配记录" description="调整平台、状态、时间或关键词筛选。" /> : visiblePlans.map((item) => (
              <button aria-pressed={item.id === selectedPlanId} className={item.id === selectedPlanId ? "selected" : ""} key={item.id} type="button" onClick={() => { setSelectedPlanId(item.id); onSelectPlan?.(item.id); void load(item.id); }}>
                <span><strong>{item.work_title}</strong><em>{planStatusLabel(item.status)}</em></span>
                <small>{compactVersion(item.work_version_id)} · {item.targets.map((target) => platformLabel(target.platform)).join(" / ")}</small>
                <small>最近动作 {formatDateTime(item.updated_at)}</small>
              </button>
            ))}
          </aside>

          <main className="publicationDetail">
            {!plan ? <EmptyState title="选择发布计划" description="从左侧选择要处理的明确作品版本。" /> : (
              <>
                <div className="publicationDetailHeader"><div><h3>{workTitle}</h3><p><strong>{versionLabel}</strong> · 计划状态 {planStatusLabel(plan.status)} · 绑定版本不可自动切换</p></div><span>{formatDateTime(plan.updated_at)}</span></div>
                <div className="publicationTargets">
                  {platforms.map((platform) => {
                    const target = plan.targets.find((item) => item.platform === platform);
                    return <TargetPanel
                      key={platform}
                      platform={platform}
                      target={target}
                      draft={drafts[platform]}
                      result={resultForms[platform]}
                      coverArtifacts={(work?.artifacts ?? []).filter((artifact) => artifact.mime_type.startsWith("image/"))}
                      busy={busyTargetId === (target?.id ?? `new-${platform}`)}
                      disabled={writesDisabled}
                      error={actionErrors[platform] ?? ""}
                      notice={notices[platform] ?? ""}
                      downloads={target ? downloads[target.id] : undefined}
                      auditOpen={auditOpen[platform] === true}
                      onAuditToggle={() => setAuditOpen((current) => ({ ...current, [platform]: !current[platform] }))}
                      onDraftChange={(patch) => setDrafts((current) => ({ ...current, [platform]: { ...current[platform], ...patch } }))}
                      onResultChange={(patch) => setResultForms((current) => ({ ...current, [platform]: { ...current[platform], ...patch } }))}
                      onSave={() => saveTarget(platform, target)}
                      onGenerate={() => target && generatePackage(platform, target)}
                      onCopy={() => target && copyText(platform, target)}
                      onDownload={() => target && downloadPackage(platform, target)}
                      onDownloadFile={(url) => target && downloadFile(platform, target, url)}
                      onHandoff={() => target && openOfficialPlatform(platform, target)}
                      onNeedsAttention={() => target && markNeedsAttention(platform, target)}
                      onCancel={() => target && cancelTarget(platform, target)}
                      onSaveResult={(correction) => target && saveResult(platform, target, correction)}
                    />;
                  })}
                </div>
              </>
            )}
          </main>
        </div>
      ) : null}
    </section>
  );
}

function TargetPanel({ platform, target, draft, result, coverArtifacts, busy, disabled, error, notice, downloads, auditOpen, onAuditToggle, onDraftChange, onResultChange, onSave, onGenerate, onCopy, onDownload, onDownloadFile, onHandoff, onNeedsAttention, onCancel, onSaveResult }: {
  platform: PublicationPlatform;
  target?: PublicationTarget;
  draft: Draft;
  result: { url: string; at: string };
  coverArtifacts: WorkDetails["artifacts"];
  busy: boolean;
  disabled: boolean;
  error: string;
  notice: string;
  downloads?: Awaited<ReturnType<typeof getPublicationDownloads>>;
  auditOpen: boolean;
  onAuditToggle: () => void;
  onDraftChange: (patch: Partial<Draft>) => void;
  onResultChange: (patch: Partial<{ url: string; at: string }>) => void;
  onSave: () => void;
  onGenerate: () => void;
  onCopy: () => void;
  onDownload: () => void;
  onDownloadFile: (url: string) => void;
  onHandoff: () => void;
  onNeedsAttention: () => void;
  onCancel: () => void;
  onSaveResult: (correction: boolean) => void;
}) {
  const status = target?.status ?? "draft";
  const locked = status === "published" || status === "cancelled";
  return (
    <section aria-label={`${platformLabel(platform)}发布目标`} className={`publicationTarget ${status}`} role="region">
      <header><div><strong>{platformLabel(platform)}</strong><span>官方创作者网页人工交接</span></div><em className={`publicationStatus ${status}`}>{statusLabel(status)}</em></header>
      {target?.overdue && !locked ? <p className="publicationOverdue">计划时间已逾期，状态不会自动改变</p> : null}
      {status === "handed_off" ? <p className="publicationWaiting">等待人工发布</p> : null}
      {status === "published" ? <p className="publicationPublished">人工确认已发布</p> : null}
      {error ? <p className="publicationActionError" role="alert">{error}</p> : null}
      {notice ? <p className="publicationNotice">{notice}</p> : null}

      <div className="publicationForm">
        <label>平台标题<input aria-label="平台标题" disabled={locked || busy || disabled} value={draft.title} onChange={(event) => onDraftChange({ title: event.target.value })} /></label>
        <label>发布正文<textarea aria-label="发布正文" disabled={locked || busy || disabled} value={draft.body} onChange={(event) => onDraftChange({ body: event.target.value })} /></label>
        <label>标签<input aria-label="平台标签" disabled={locked || busy || disabled} placeholder="#效率 #创作" value={draft.tags} onChange={(event) => onDraftChange({ tags: event.target.value })} /></label>
        <div className="publicationInlineFields">
          <label>计划时间<input aria-label="计划发布时间" disabled={locked || busy || disabled} type="datetime-local" value={draft.plannedAt} onChange={(event) => onDraftChange({ plannedAt: event.target.value })} /></label>
          <label>封面<select aria-label="平台封面" disabled={locked || busy || disabled} value={draft.coverArtifactId} onChange={(event) => onDraftChange({ coverArtifactId: event.target.value })}><option value="">不单独设置</option>{coverArtifacts.map((artifact) => <option key={artifact.id} value={artifact.id}>{artifact.file_name}</option>)}</select></label>
        </div>
      </div>

      <div className="publicationChecklist"><strong>准备检查</strong><span className={draft.title.trim() || draft.body.trim() ? "done" : ""}>发布文案</span><span className={target?.status === "ready" || target?.status === "handed_off" || target?.status === "published" ? "done" : ""}>artifact 完整性</span><span className={downloads ? "done" : ""}>当前 revision 发布包</span></div>

      {!locked ? <div className="publicationActions">
        <button disabled={busy || disabled} type="button" onClick={onSave}>保存草稿</button>
        {target ? <button disabled={busy || disabled || !draft.title.trim() && !draft.body.trim()} type="button" onClick={onGenerate}>生成发布包</button> : null}
        {target ? <button disabled={busy || disabled} type="button" onClick={onCopy}>复制文案</button> : null}
        {target && status === "ready" ? <button className="primaryButton" disabled={busy || disabled} type="button" onClick={onHandoff}>去平台发布</button> : null}
        {target && status === "handed_off" ? <button disabled={busy || disabled} type="button" onClick={onNeedsAttention}>标记需处理</button> : null}
        {target ? <button className="dangerTextButton" disabled={busy || disabled} type="button" onClick={onCancel}>取消目标</button> : null}
      </div> : null}

      {target && ["ready", "handed_off", "published"].includes(status) ? <div className="publicationDownloads"><button disabled={busy} type="button" onClick={onDownload}>下载完整发布包</button>{downloads ? <><button disabled={busy} type="button" onClick={() => onDownloadFile(downloads.video.download_url)}>下载 MP4</button>{downloads.cover ? <button disabled={busy} type="button" onClick={() => onDownloadFile(downloads.cover!.download_url)}>下载封面</button> : null}</> : null}</div> : null}

      {target && (status === "handed_off" || status === "published") ? <div className="publicationResultForm"><strong>{status === "published" ? "修正人工结果" : "登记人工发布结果"}</strong><label>官方作品链接<input aria-label="官方作品链接" disabled={busy || disabled} placeholder="https://..." value={result.url} onChange={(event) => onResultChange({ url: event.target.value })} /></label><label>实际发布时间<input aria-label="实际发布时间" disabled={busy || disabled} type="datetime-local" value={result.at} onChange={(event) => onResultChange({ at: event.target.value })} /></label><button disabled={busy || disabled || !result.url || !result.at} type="button" onClick={() => onSaveResult(status === "published")}>{status === "published" ? "保存结果修正" : "人工确认已发布"}</button></div> : null}

      {target ? <div className="publicationAudit"><button aria-expanded={auditOpen} type="button" onClick={onAuditToggle}>审计记录</button>{auditOpen ? <div><span>最近动作：{statusLabel(status)}</span><span>更新时间：{formatDateTime(target.updated_at)}</span><span>草稿 revision：{target.draft_revision}</span>{target.published_url ? <a href={target.published_url} rel="noreferrer" target="_blank">查看官方作品</a> : null}</div> : null}</div> : null}
    </section>
  );
}

function EmptyState({ title, description }: { title: string; description: string }) { return <div className="publicationEmpty"><strong>{title}</strong><span>{description}</span></div>; }
function emptyDraft(): Draft { return { title: "", body: "", tags: "", coverArtifactId: "", plannedAt: "" }; }
function targetToDraft(target?: PublicationTarget): Draft { return target ? { title: target.title, body: target.body, tags: target.tags.join(" "), coverArtifactId: target.cover_artifact_id ?? "", plannedAt: isoToLocalDateTime(target.planned_at) } : emptyDraft(); }
function targetToResult(target?: PublicationTarget) { return { url: target?.published_url ?? "", at: isoToLocalDateTime(target?.published_at ?? null) }; }
function platformLabel(platform: PublicationPlatform) { return platform === "douyin" ? "抖音" : "小红书"; }
function statusLabel(status: PublicationTargetStatus) { return ({ draft: "草稿", ready: "准备完成", handed_off: "等待人工发布", needs_attention: "需处理", published: "人工确认已发布", cancelled: "已取消" } as Record<PublicationTargetStatus, string>)[status]; }
function planStatusLabel(status: PublicationPlanDetails["status"]) { return ({ draft: "草稿", ready: "准备完成", handed_off: "等待人工发布", needs_attention: "需处理", partially_published: "部分完成", published: "已发布", cancelled: "已取消" } as Record<PublicationPlanDetails["status"], string>)[status]; }
function compactVersion(value: string) { const suffix = value.match(/(?:version-|v)(\d+)$/i)?.[1]; return suffix ? `V${suffix}` : "明确版本"; }
function parseTags(value: string) { return Array.from(new Set(value.split(/[\s,，]+/).map((item) => item.trim()).filter(Boolean))); }
function isoToLocalDateTime(value: string | null) { if (!value) return ""; const date = new Date(value); if (Number.isNaN(date.getTime())) return ""; const local = new Date(date.getTime() - date.getTimezoneOffset() * 60_000); return local.toISOString().slice(0, 16); }
function localDateTimeToIso(value: string) { if (!value) return null; const date = new Date(value); if (Number.isNaN(date.getTime())) throw new Error("计划时间格式无效"); return date.toISOString(); }
function formatDateTime(value: string) { const date = new Date(value); return Number.isNaN(date.getTime()) ? value : date.toLocaleString("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", hour12: false }); }
function matchesTime(target: PublicationTarget, filter: TimeFilter) { if (filter === "all") return true; if (filter === "overdue") return target.overdue === true; if (!target.planned_at) return false; const at = new Date(target.planned_at); const now = new Date(); if (filter === "today") return at.toDateString() === now.toDateString(); return at >= now && at.getTime() <= now.getTime() + 7 * 24 * 60 * 60 * 1000; }
function trustedOfficialEntrance(platform: PublicationPlatform, value: string) { try { const url = new URL(value); const expected = platform === "douyin" ? "creator.douyin.com" : "creator.xiaohongshu.com"; return url.protocol === "https:" && url.hostname === expected && !url.username && !url.password; } catch { return false; } }
function trustedPublishedUrl(platform: PublicationPlatform, value: string) { try { const url = new URL(value); const root = platform === "douyin" ? "douyin.com" : "xiaohongshu.com"; return url.protocol === "https:" && !url.search && (url.hostname === root || url.hostname.endsWith(`.${root}`)); } catch { return false; } }
function validatePublicationResult(platform: PublicationPlatform, result: { url: string; at: string }) { if (!trustedPublishedUrl(platform, result.url.trim())) throw new Error(`请输入${platformLabel(platform)} HTTPS 官方作品链接，且不要包含查询参数`); const date = new Date(result.at); if (Number.isNaN(date.getTime())) throw new Error("实际发布时间格式无效"); if (date.getTime() > Date.now() + 60_000) throw new Error("实际发布时间不能晚于当前时间"); return date.toISOString(); }
function errorMessage(value: unknown, fallback: string) { return value instanceof Error ? value.message : fallback; }
function actionErrorMessage(value: unknown) { const error = value as Partial<ApiError>; if (error.status === 409 && typeof error.details === "object" && error.details && "code" in error.details) { const code = String((error.details as { code?: unknown }).code ?? ""); if (code === "publication_conflict") return "草稿已被其他操作更新，请重新读取后再试"; if (code === "publication_artifact_integrity") return `artifact 完整性校验失败：${errorMessage(value, "来源文件缺失或损坏")}`; } return errorMessage(value, "发布操作失败"); }
function idempotencyKey() { const bytes = new Uint8Array(16); globalThis.crypto.getRandomValues(bytes); return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join(""); }
