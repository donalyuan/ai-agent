"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import type {
  ApiClient,
  AgentMessage,
  ModelOption,
  Project,
  WorkDetails,
  WorkDownloadManifest,
  WorkSummary,
  WorkTimeline,
  WorkVersion,
  WorkVersionDiff,
} from "../../lib/api";
import {
  analyzeWorkVersionDiff,
  archiveWork,
  confirmWorkVersionDiff,
  createPublicationHandoff,
  createPublicationPlan,
  createAgentConversation,
  deleteWork,
  deriveWorkVersion,
  getProductionPackageDownloadUrl,
  getWork,
  getWorkArtifactDownloadUrl,
  getWorkVersionDownloads,
  listWorks,
  regenerateWorkVersion,
  restoreWork,
  sendAgentMessage,
} from "../../lib/api";

type Props = {
  client: ApiClient;
  project: Project | undefined;
  writesDisabled: boolean;
  textModels?: ModelOption[];
  onRunCreated?: (runId: string) => void;
  onOpenGenerationTask?: (runId: string) => void;
  onOpenPublicationPlan?: (planId: string) => void;
};

type LibraryView = "grid" | "list";
type PageView = "library" | "detail" | "diff";
type SortOrder = "updated_desc" | "updated_asc" | "title_asc";

const statusLabels: Record<string, string> = {
  draft: "草稿",
  confirmed: "已确认",
  queued: "排队中",
  running: "生成中",
  succeeded: "已完成",
  completed: "已完成",
  failed: "失败",
  archived: "已归档",
};

const artifactRoleLabels: Record<string, string> = {
  final_video: "成片",
  subtitle: "字幕",
  mix: "混音",
  audio_track: "声音分轨",
  production_package: "制作包",
  reusable_intermediate: "复用素材",
};

export function WorkLibraryPage({ client, project, writesDisabled, textModels = [], onRunCreated, onOpenGenerationTask, onOpenPublicationPlan }: Props) {
  const [pageView, setPageView] = useState<PageView>("library");
  const [libraryView, setLibraryView] = useState<LibraryView>("grid");
  const [sortOrder, setSortOrder] = useState<SortOrder>("updated_desc");
  const [archived, setArchived] = useState(false);
  const [query, setQuery] = useState("");
  const [works, setWorks] = useState<WorkSummary[]>([]);
  const [selectedWorkId, setSelectedWorkId] = useState<string | null>(null);
  const [details, setDetails] = useState<WorkDetails | null>(null);
  const [selectedVersionId, setSelectedVersionId] = useState<string | null>(null);
  const [diff, setDiff] = useState<WorkVersionDiff | null>(null);
  const [agentConversationId, setAgentConversationId] = useState<string | null>(null);
  const [agentMessages, setAgentMessages] = useState<AgentMessage[]>([]);
  const [agentDraft, setAgentDraft] = useState("");
  const [sendingAgentMessage, setSendingAgentMessage] = useState(false);
  const [downloads, setDownloads] = useState<WorkDownloadManifest | null>(null);
  const [historyExpanded, setHistoryExpanded] = useState(false);
  const [technicalExpanded, setTechnicalExpanded] = useState(false);
  const [loading, setLoading] = useState(false);
  const [detailsLoading, setDetailsLoading] = useState(false);
  const [actionBusy, setActionBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");

  const loadWorks = useCallback(async () => {
    if (!project) {
      setWorks([]);
      return;
    }
    setLoading(true);
    setError("");
    try {
      const response = await listWorks(client, project.project_id, {
        archived,
        query: query.trim() || undefined,
      });
      setWorks(response.items);
    } catch (value) {
      setError(errorMessage(value, "作品库读取失败"));
    } finally {
      setLoading(false);
    }
  }, [archived, client, project, query]);

  useEffect(() => { void loadWorks(); }, [loadWorks]);

  const sortedWorks = useMemo(() => [...works].sort((left, right) => {
    if (sortOrder === "title_asc") return left.title.localeCompare(right.title, "zh-CN");
    const difference = Date.parse(left.updated_at) - Date.parse(right.updated_at);
    return sortOrder === "updated_asc" ? difference : -difference;
  }), [sortOrder, works]);

  const selectedVersion = details?.versions.find((version) => version.id === selectedVersionId) ?? null;
  const selectedArtifacts = details?.artifacts.filter((artifact) => artifact.work_version_id === selectedVersionId) ?? [];
  const selectedTimeline = details?.timelines.find((timeline) => timeline.work_version_id === selectedVersionId) ?? null;
  const selectedAudit = details?.generation_audit.filter((audit) => audit.work_version_id === selectedVersionId) ?? [];
  const finalVideo = selectedArtifacts.find((artifact) => artifact.role === "final_video");
  const currentDrafts = details?.versions.filter((version) => version.id === details.current_version_id && version.status === "draft") ?? [];
  const completedVersions = details?.versions.filter((version) => version.status === "completed") ?? [];
  const historyVersions = details?.versions.filter((version) => !currentDrafts.some((current) => current.id === version.id) && version.status !== "completed") ?? [];
  const failedHistoryCount = historyVersions.filter((version) => version.status === "failed").length;
  const earlyDraftCount = historyVersions.filter((version) => version.status === "draft").length;
  const textModelId = textModels.find((model) => model.is_default)?.model_id ?? textModels[0]?.model_id ?? "";

  async function openWork(workId: string) {
    setSelectedWorkId(workId);
    setPageView("detail");
    setDetailsLoading(true);
    setError("");
    setNotice("");
    setDownloads(null);
    setHistoryExpanded(false);
    setTechnicalExpanded(false);
    setAgentConversationId(null);
    setAgentMessages([]);
    setAgentDraft("");
    try {
      const value = await getWork(client, workId);
      setDetails(value);
      selectInitialVersion(value);
    } catch (value) {
      setError(errorMessage(value, "作品详情读取失败"));
    } finally {
      setDetailsLoading(false);
    }
  }

  function selectInitialVersion(value: WorkDetails, preferredId?: string) {
    const version = value.versions.find((item) => item.id === preferredId)
      ?? value.versions.find((item) => item.id === value.current_version_id && item.status === "draft")
      ?? value.versions.find((item) => item.status === "completed")
      ?? value.versions.find((item) => item.id === value.current_version_id)
      ?? value.versions[0];
    setSelectedVersionId(version?.id ?? null);
  }

  function selectVersion(version: WorkVersion) {
    setSelectedVersionId(version.id);
    setDownloads(null);
    setNotice("");
    setTechnicalExpanded(false);
  }

  async function refreshDetails(preferredId?: string) {
    if (!selectedWorkId) return;
    const value = await getWork(client, selectedWorkId);
    setDetails(value);
    selectInitialVersion(value, preferredId);
  }

  async function runAction(action: () => Promise<void>) {
    if (writesDisabled || actionBusy) return;
    setActionBusy(true);
    setError("");
    setNotice("");
    try {
      await action();
    } catch (value) {
      setError(errorMessage(value, "作品操作失败"));
    } finally {
      setActionBusy(false);
    }
  }

  function continueEditing() {
    if (!selectedVersion) return;
    void runAction(async () => {
      const draft = await deriveWorkVersion(client, selectedVersion.id, {});
      await refreshDetails(draft.id);
      setNotice(`已创建 V${draft.version_no} 草稿，原版本保持不变`);
    });
  }

  function fullRegeneration() {
    if (!selectedVersion) return;
    void runAction(async () => {
      const draft = await regenerateWorkVersion(client, selectedVersion.id);
      await refreshDetails(draft.id);
      const result = await analyzeWorkVersionDiff(client, draft.id);
      setDiff(result);
      setPageView("diff");
    });
  }

  async function submitAgentMessage() {
    if (!details || !selectedVersion || selectedVersion.status !== "draft" || !project || !agentDraft.trim() || !textModelId || sendingAgentMessage) return;
    const content = agentDraft.trim();
    const localUserMessage: AgentMessage = {
      message_id: `local-${Date.now()}`,
      conversation_id: agentConversationId ?? "pending",
      role: "user",
      content,
      metadata: {},
      created_at: new Date().toISOString(),
    };
    setAgentMessages((current) => [...current, localUserMessage]);
    setSendingAgentMessage(true);
    setError("");
    setNotice("");
    try {
      const conversationId = agentConversationId ?? (await createAgentConversation(client, {
        project_id: project.project_id,
        agent_type: "work",
        subject_type: "work",
        subject_id: details.id,
        title: `${details.title} · 作品修改`,
        metadata: {},
      })).conversation_id;
      setAgentConversationId(conversationId);
      const response = await sendAgentMessage(client, conversationId, { model_id: textModelId, content });
      setAgentMessages((current) => [...current, response.assistant_message]);
      setAgentDraft("");
      const nextDiff = workAgentDiff(response.assistant_message.metadata.diff);
      if (nextDiff) setDiff(nextDiff);
      const draftId = typeof response.assistant_message.metadata.draft_version_id === "string"
        ? response.assistant_message.metadata.draft_version_id
        : selectedVersion.id;
      await refreshDetails(draftId);
    } catch (value) {
      setError(errorMessage(value, "作品 Agent 修改失败"));
    } finally {
      setSendingAgentMessage(false);
    }
  }

  function confirmDiff() {
    if (!diff) return;
    void runAction(async () => {
      const result = await confirmWorkVersionDiff(client, diff.id, idempotencyKey());
      setNotice(result.created ? "已创建局部重生成运行" : "该差异计划已创建运行");
      onRunCreated?.(result.run_id);
    });
  }

  function loadDownloads() {
    if (!selectedVersion || selectedVersion.status !== "completed") return;
    void runAction(async () => setDownloads(await getWorkVersionDownloads(client, selectedVersion.id)));
  }

  function handoffToPublishing() {
    if (!selectedVersion || selectedVersion.status !== "completed") return;
    void runAction(async () => {
      const handoff = await createPublicationHandoff(client, selectedVersion.id, idempotencyKey());
      const plan = await createPublicationPlan(client, handoff.id, idempotencyKey());
      onOpenPublicationPlan?.(plan.id);
    });
  }

  function changeArchiveState() {
    if (!details) return;
    void runAction(async () => {
      if (details.archived) await restoreWork(client, details.id);
      else await archiveWork(client, details.id);
      setPageView("library");
      setDetails(null);
      await loadWorks();
    });
  }

  function removeBlankWork() {
    if (!details) return;
    void runAction(async () => {
      await deleteWork(client, details.id);
      setPageView("library");
      setDetails(null);
      await loadWorks();
    });
  }

  if (pageView === "diff" && diff && details) {
    return (
      <DiffConfirmation
        details={details}
        diff={diff}
        busy={actionBusy}
        error={error}
        onBack={() => setPageView("detail")}
        onConfirm={confirmDiff}
      />
    );
  }

  if (pageView === "detail") {
    return (
      <section className="workLibraryWorkspace workLibraryDetailWorkspace">
        <header className="workLibraryHeader">
          <div>
            <p className="sectionKicker">作品生产 / 作品库</p>
            <h2>作品详情</h2>
            <p>{details ? `${details.title} · ${details.versions.length} 个版本` : "查看成片、草稿、版本和调用审计"}</p>
          </div>
          {details ? (
            <div className="workLibraryHeaderActions">
              {isBlankDraft(details) ? <button className="secondaryButton" disabled={writesDisabled || actionBusy} type="button" onClick={removeBlankWork}>删除空白草稿</button> : null}
              <button className="secondaryButton" disabled={writesDisabled || actionBusy} type="button" onClick={changeArchiveState}>{details.archived ? "恢复作品" : "归档作品"}</button>
            </div>
          ) : null}
        </header>

        {error ? <p className="errorBanner">{error}</p> : null}
        {notice ? <p className="successBanner">{notice}</p> : null}
        {detailsLoading ? <div className="workLibraryLoading">正在读取作品详情</div> : null}

        {details && selectedVersion ? (
          <div className="workLibraryDetailSurface">
            <div className="workLibraryPrimaryBar">
              <button aria-label="返回作品库" className="workLibraryIconButton" title="返回作品库" type="button" onClick={() => setPageView("library")}>←</button>
              <div>
                <h3>{details.title}</h3>
                <span>{selectedVersion.status === "draft" ? `当前草稿 V${selectedVersion.version_no}` : `当前查看 V${selectedVersion.version_no}`} · {selectedVersion.source_version_id ? `来源：V${details.versions.find((version) => version.id === selectedVersion.source_version_id)?.version_no ?? "--"}` : "初始版本"} · 更新于 {formatDateTime(selectedVersion.updated_at)}</span>
              </div>
              <i className={`workLibraryStatus ${selectedVersion.status}`}>{statusLabel(selectedVersion.status)}</i>
              {selectedAudit[0] ? <button className="secondaryButton" type="button" onClick={() => onOpenGenerationTask?.(selectedAudit[0].id)}>来源任务</button> : null}
              {selectedVersion.status === "draft" ? <button className="primaryButton" type="button" onClick={() => document.getElementById("work-agent-input")?.focus()}>与 Agent 修改</button> : null}
            </div>

            <div className="workLibraryDetailLayout">
            <main className="workLibraryDetailMain">
              <div className="workLibraryDraftSummary">
                <section className="workLibraryPreviewPanel" aria-label="成片预览">
                  {finalVideo ? (
                    <video aria-label={`V${selectedVersion.version_no} 成片预览`} controls src={getWorkArtifactDownloadUrl(client, finalVideo.id)} />
                  ) : (
                    <div className="workLibraryPreviewEmpty"><strong>该版本没有可播放成片</strong><span>{selectedVersion.status === "failed" ? "可在审计区查看失败阶段与保留产物" : "完成生成后将在此显示"}</span></div>
                  )}
                </section>

                <BusinessSummary details={details} version={selectedVersion} artifacts={selectedArtifacts} />
              </div>

              <TimelinePanel
                timeline={selectedTimeline}
                sourceVersion={details.versions.find((version) => version.id === selectedVersion.source_version_id) ?? null}
                onSelectSource={selectVersion}
              />

              <section className="workLibraryAuditPanel">
                <div className="workLibrarySectionHeader"><h3>调用审计</h3><span>{selectedAudit.length} 次运行</span></div>
                {selectedAudit.length ? selectedAudit.map((audit) => (
                  <article className={`workLibraryRunAudit ${audit.status}`} key={audit.id}>
                    <div><strong>{statusLabel(audit.status)} · {stageLabel(audit.current_stage)}</strong><span>{audit.progress_percent}% · {audit.attempt_count} 次调用记录</span></div>
                    {audit.error_summary ? <p>{audit.error_summary}</p> : null}
                    <button className="secondaryButton" type="button" onClick={() => onOpenGenerationTask?.(audit.id)}>查看生成任务</button>
                  </article>
                )) : <p className="workLibraryMuted">该版本没有生成运行审计。</p>}
                <div className="workLibraryTechnicalDisclosure">
                  <button aria-expanded={technicalExpanded} className="workLibraryDisclosureButton" type="button" onClick={() => setTechnicalExpanded((value) => !value)}>
                    {technicalExpanded ? "收起技术快照" : "展开技术快照"}
                  </button>
                  {technicalExpanded ? <TechnicalSnapshot version={selectedVersion} /> : null}
                </div>
              </section>
            </main>

            <aside aria-label="版本记录" className="workLibraryVersionPanel" role="region">
              <div className="workLibrarySectionHeader"><h3>版本记录</h3><span>{details.versions.length} 个版本</span></div>
              <div className="workLibraryVersionList">
                {currentDrafts.length ? <VersionGroup title={`当前草稿 · ${currentDrafts.length}`} versions={currentDrafts} selectedId={selectedVersion.id} onSelect={selectVersion} /> : null}
                {completedVersions.length ? <VersionGroup title={`可用成片 · ${completedVersions.length}`} versions={completedVersions} selectedId={selectedVersion.id} onSelect={selectVersion} /> : null}
                {historyVersions.length ? (
                  <section className="workLibraryVersionGroup history">
                    <button
                      aria-expanded={historyExpanded}
                      className="workLibraryHistoryToggle"
                      type="button"
                      onClick={() => setHistoryExpanded((value) => !value)}
                    >
                      <span><strong>失败与早期记录</strong><small>失败 {failedHistoryCount} · 未运行草稿 {earlyDraftCount}</small></span>
                      <em>{historyExpanded ? "收起" : "展开"}</em>
                    </button>
                    {historyExpanded ? <div>{historyVersions.map((version) => <VersionButton key={version.id} version={version} selected={version.id === selectedVersion.id} onSelect={selectVersion} />)}</div> : null}
                  </section>
                ) : null}
              </div>

              {selectedVersion.status !== "draft" ? (
                <div className="workLibraryVersionActions">
                  <>
                    <button className="secondaryButton" disabled={writesDisabled || actionBusy || details.archived} type="button" onClick={continueEditing}>继续修改</button>
                    <button className="secondaryButton" disabled={writesDisabled || actionBusy || details.archived} type="button" onClick={fullRegeneration}>整体重生成</button>
                  </>
                  {selectedVersion.status === "completed" ? (
                  <>
                    <button className="secondaryButton" disabled={actionBusy} type="button" onClick={loadDownloads}>下载</button>
                    <button className="primaryButton" disabled={writesDisabled || actionBusy} type="button" onClick={handoffToPublishing}>进入发布</button>
                  </>
                  ) : null}
                </div>
              ) : null}

              {selectedVersion.status === "draft" ? (
                <WorkAgentPanel
                  draft={agentDraft}
                  messages={agentMessages}
                  version={selectedVersion}
                  sourceVersion={details.versions.find((version) => version.id === selectedVersion.source_version_id) ?? null}
                  disabled={writesDisabled || sendingAgentMessage || !textModelId}
                  diff={diff}
                  onDraftChange={setAgentDraft}
                  onOpenDiff={() => setPageView("diff")}
                  onSubmit={() => void submitAgentMessage()}
                />
              ) : null}

              {downloads ? (
                <div className="workLibraryDownloads" aria-label={`V${selectedVersion.version_no} 下载清单`}>
                  <strong>V{selectedVersion.version_no} 产物</strong>
                  {downloads.artifacts.map(({ artifact, integrity_status }) => integrity_status === "available" ? (
                    <a href={getWorkArtifactDownloadUrl(client, artifact.id)} key={artifact.id}>下载 {artifact.file_name}</a>
                  ) : (
                    <span className="workLibraryDownloadError" key={artifact.id}>{artifact.file_name}：{integrity_status === "missing" ? "文件缺失" : "完整性校验失败"}</span>
                  ))}
                  <a href={getProductionPackageDownloadUrl(client, selectedVersion.id)}>下载制作包</a>
                </div>
              ) : null}
            </aside>
            </div>
          </div>
        ) : null}
      </section>
    );
  }

  return (
    <section className="workLibraryWorkspace">
      <header className="workLibraryHeader">
        <div><p className="sectionKicker">作品生产</p><h2>作品库</h2><p>集中管理成片、版本和生产审计</p></div>
        <div className="workLibraryViewToggle" aria-label="作品视图">
          <button aria-pressed={libraryView === "grid"} className={libraryView === "grid" ? "active" : ""} type="button" onClick={() => setLibraryView("grid")}>网格视图</button>
          <button aria-pressed={libraryView === "list"} className={libraryView === "list" ? "active" : ""} type="button" onClick={() => setLibraryView("list")}>列表视图</button>
        </div>
      </header>

      <div className="workLibraryToolbar">
        <input aria-label="搜索作品" placeholder="搜索作品名称" role="searchbox" value={query} onChange={(event) => setQuery(event.target.value)} />
        <label className="workLibraryArchiveFilter"><input checked={archived} type="checkbox" onChange={(event) => setArchived(event.target.checked)} />查看归档</label>
        <label>排序
          <select aria-label="作品排序" value={sortOrder} onChange={(event) => setSortOrder(event.target.value as SortOrder)}>
            <option value="updated_desc">最近更新</option><option value="updated_asc">最早更新</option><option value="title_asc">名称 A-Z</option>
          </select>
        </label>
        <span>{sortedWorks.length} 个作品</span>
      </div>

      {error ? <p className="errorBanner">{error}</p> : null}
      {loading ? <div className="workLibraryLoading">正在读取作品库</div> : null}
      {!loading && !project ? <EmptyState title="请先选择账号" description="选择账号后查看对应作品。" /> : null}
      {!loading && project && !sortedWorks.length ? <EmptyState title={archived ? "没有归档作品" : "作品库为空"} description={query ? "没有匹配当前关键词的作品。" : "完成作品生成后，版本会自动进入作品库。"} /> : null}
      {!loading && sortedWorks.length ? libraryView === "grid" ? (
        <div className="workLibraryGrid" role="region" aria-label="作品网格">
          {sortedWorks.map((work) => <WorkGridItem client={client} key={work.id} work={work} selected={selectedWorkId === work.id} onOpen={() => void openWork(work.id)} />)}
        </div>
      ) : (
        <div className="workLibraryList" role="region" aria-label="作品列表">
          <div className="workLibraryListHeader"><span>作品</span><span>版本</span><span>时长 / 比例</span><span>状态</span><span>更新时间</span><span>操作</span></div>
          {sortedWorks.map((work) => <WorkListItem key={work.id} work={work} selected={selectedWorkId === work.id} onOpen={() => void openWork(work.id)} />)}
        </div>
      ) : null}
    </section>
  );
}

function WorkGridItem({ client, work, selected, onOpen }: { client: ApiClient; work: WorkSummary; selected: boolean; onOpen: () => void }) {
  return (
    <button aria-label={`${work.title}，查看详情`} className={`workLibraryCard ${selected ? "selected" : ""}`} type="button" onClick={onOpen}>
      <span className="workLibraryCover">
        {work.cover_artifact_id ? <video muted preload="metadata" src={getWorkArtifactDownloadUrl(client, work.cover_artifact_id)} /> : <span>暂无成片</span>}
        <em>{work.duration_seconds ? `${work.duration_seconds} 秒` : "--"}</em>
      </span>
      <span className="workLibraryCardBody"><strong>{work.title}</strong><small>V{work.current_completed_version_no ?? "--"} · {work.aspect_ratio ?? "--"}</small><span><i className={`workLibraryStatus ${work.status}`}>{statusLabel(work.status)}</i><time>{formatDateTime(work.updated_at)}</time></span></span>
    </button>
  );
}

function WorkListItem({ work, selected, onOpen }: { work: WorkSummary; selected: boolean; onOpen: () => void }) {
  return (
    <button aria-label={`${work.title}，查看详情`} className={`workLibraryListRow ${selected ? "selected" : ""}`} type="button" onClick={onOpen}>
      <strong>{work.title}</strong><span>V{work.current_completed_version_no ?? "--"}</span><span>{work.duration_seconds ? `${work.duration_seconds} 秒` : "--"} / {work.aspect_ratio ?? "--"}</span><span><i className={`workLibraryStatus ${work.status}`}>{statusLabel(work.status)}</i></span><time>{formatDateTime(work.updated_at)}</time><span>查看详情</span>
    </button>
  );
}

function WorkAgentPanel({
  draft,
  messages,
  version,
  sourceVersion,
  disabled,
  diff,
  onDraftChange,
  onOpenDiff,
  onSubmit,
}: {
  draft: string;
  messages: AgentMessage[];
  version: WorkVersion;
  sourceVersion: WorkVersion | null;
  disabled: boolean;
  diff: WorkVersionDiff | null;
  onDraftChange: (value: string) => void;
  onOpenDiff: () => void;
  onSubmit: () => void;
}) {
  return (
    <section aria-label="作品 Agent 对话" className="workLibraryAgentPanel">
      <header>
        <span aria-hidden="true">✦</span>
        <div><strong>和 Agent 讨论修改</strong><small>基于 V{sourceVersion?.version_no ?? "--"} 修改当前草稿 V{version.version_no}</small></div>
      </header>
      <div aria-live="polite" className="workLibraryAgentMessages">
        {!messages.length ? <article className="assistant"><span aria-hidden="true">✦</span><p>V{version.version_no} 还未运行。告诉我你希望如何调整这版作品。</p></article> : null}
        {messages.map((message) => (
          <article className={message.role} key={message.message_id}>
            {message.role === "assistant" ? <span aria-hidden="true">✦</span> : null}
            <p>{message.content}</p>
          </article>
        ))}
        {diff ? <button className="workLibraryAgentDiffButton" type="button" onClick={onOpenDiff}>查看影响并确认</button> : null}
      </div>
      <div className="workLibraryAgentComposer">
        <textarea
          aria-label="作品修改要求"
          id="work-agent-input"
          placeholder="例如：保留配音，让画面节奏更紧凑……"
          value={draft}
          onChange={(event) => onDraftChange(event.target.value)}
        />
        <div>
          <button aria-label="添加参考素材" disabled title="参考素材将在后续版本开放" type="button">+</button>
          <button aria-label="发送修改要求" disabled={disabled || !draft.trim()} title="发送修改要求" type="button" onClick={onSubmit}>↑</button>
        </div>
      </div>
    </section>
  );
}

function BusinessSummary({ details, version, artifacts }: { details: WorkDetails; version: WorkVersion; artifacts: WorkDetails["artifacts"] }) {
  const source = details.versions.find((item) => item.id === version.source_version_id);
  const videoModel = firstBusinessString(version.model_snapshot, [
    ["video", "display_name"],
    ["video_model", "display_name"],
    ["video_model_name"],
  ]) || modelCatalogName(details, snapshotString(version.model_snapshot, "video_model_id")) || "视频模型未记录";
  const aspectRatio = snapshotString(version.parameter_snapshot, "aspect_ratio") || "比例未设置";
  const resolution = snapshotString(version.parameter_snapshot, "resolution") || "分辨率未设置";
  const duration = snapshotNumber(version.timeline_snapshot, "duration_seconds");
  const prompt = snapshotString(version.prompt_snapshot, "full_prompt") || "尚未生成全片提示词";
  const audioMode = audioModeLabel(snapshotString(version.timeline_snapshot, "audio_mode"));
  const burnSubtitles = version.timeline_snapshot.burn_subtitles === false ? "外挂字幕" : "字幕烧录";
  const reusableCount = artifacts.filter((artifact) => artifact.role === "reusable_intermediate").length;

  return (
    <section aria-label="制作摘要" className="workLibrarySummaryPanel" role="region">
      <div className="workLibrarySectionHeader"><h3>制作摘要</h3><span>V{version.version_no} · {derivationLabel(version.derivation_kind)}</span></div>
      <div className="workLibrarySummaryGrid">
        <article><span>版本来源</span><strong>{source ? `来自 V${source.version_no}` : "初始规划"}</strong><small>{source ? "原版本与产物保持不变" : "从已确认脚本与素材开始"}</small></article>
        <article><span>制作方案</span><strong>{videoModel}</strong><small>{aspectRatio} / {resolution}{duration ? ` · ${duration} 秒` : ""}</small></article>
        <article><span>本次修改</span><strong>{prompt}</strong><small>{derivationLabel(version.derivation_kind)}</small></article>
        <article><span>声音、字幕与复用</span><strong>{audioMode} · {burnSubtitles}</strong><small>{reusableCount ? `保留 ${reusableCount} 项复用产物` : source ? "沿用来源版本已确认素材" : "暂无复用产物"}</small></article>
      </div>
    </section>
  );
}

function TimelinePanel({ timeline, sourceVersion, onSelectSource }: { timeline: WorkTimeline | null; sourceVersion: WorkVersion | null; onSelectSource: (version: WorkVersion) => void }) {
  const tracks = [
    { key: "video", label: "视频轨", values: timeline?.video ?? [] },
    { key: "audio", label: "音频轨", values: timeline?.audio ?? [] },
    { key: "subtitles", label: "字幕轨", values: timeline?.subtitles ?? [] },
  ];
  const hasTrackContent = tracks.some((track) => track.values.length > 0);
  if (!hasTrackContent) {
    return (
      <section className="workLibraryTimelinePanel empty">
        <div className="workLibrarySectionHeader"><h3>多轨时间轴</h3><span>当前选定版本</span></div>
        <div className="workLibraryTimelineEmpty">
          <div><strong>暂无运行产物</strong><span>该草稿尚未生成视频、音频或字幕轨道。</span></div>
          {sourceVersion ? <button className="secondaryButton" type="button" onClick={() => onSelectSource(sourceVersion)}>查看来源 V{sourceVersion.version_no}</button> : null}
        </div>
      </section>
    );
  }
  return (
    <section className="workLibraryTimelinePanel">
      <div className="workLibrarySectionHeader"><h3>多轨时间轴</h3><span>当前选定版本</span></div>
      <div className="workLibraryTimelineRuler"><span>00:00</span><span>00:15</span><span>00:30</span><span>00:45</span><span>01:00</span></div>
      {tracks.map((track) => (
        <div className={`workLibraryTrack ${track.key}`} key={track.key}><strong>{track.label}</strong><div>{track.values.length ? track.values.map((item, index) => <span key={index}>{timelineItemLabel(item, `${track.label} ${index + 1}`)}</span>) : <em>无轨道内容</em>}</div></div>
      ))}
    </section>
  );
}

function TechnicalSnapshot({ version }: { version: WorkVersion }) {
  return (
    <div aria-label={`V${version.version_no} 完整技术快照`} className="workLibraryTechnicalSnapshot">
      <pre>{JSON.stringify({
        input_snapshot: version.input_snapshot,
        model_snapshot: version.model_snapshot,
        parameter_snapshot: version.parameter_snapshot,
        prompt_snapshot: version.prompt_snapshot,
        timeline_snapshot: version.timeline_snapshot,
      }, null, 2)}</pre>
    </div>
  );
}

function VersionGroup({ title, versions, selectedId, onSelect }: { title: string; versions: WorkVersion[]; selectedId: string; onSelect: (version: WorkVersion) => void }) {
  return (
    <section className="workLibraryVersionGroup">
      <h4>{title}</h4>
      <div>{versions.map((version) => <VersionButton key={version.id} version={version} selected={version.id === selectedId} onSelect={onSelect} />)}</div>
    </section>
  );
}

function VersionButton({ version, selected, onSelect }: { version: WorkVersion; selected: boolean; onSelect: (version: WorkVersion) => void }) {
  return (
    <button
      aria-label={`V${version.version_no} ${statusLabel(version.status)}，${derivationLabel(version.derivation_kind)}，${formatDateTime(version.updated_at)}`}
      aria-pressed={selected}
      className={selected ? "selected" : ""}
      type="button"
      onClick={() => onSelect(version)}
    >
      <span><strong>V{version.version_no}</strong><em className={`workLibraryStatus ${version.status}`}>{statusLabel(version.status)}</em></span>
      <small>{derivationLabel(version.derivation_kind)} · {formatDateTime(version.updated_at)}</small>
    </button>
  );
}

function DiffConfirmation({ details, diff, busy, error, onBack, onConfirm }: { details: WorkDetails; diff: WorkVersionDiff; busy: boolean; error: string; onBack: () => void; onConfirm: () => void }) {
  const source = details.versions.find((version) => version.id === diff.source_version_id);
  const draft = details.versions.find((version) => version.id === diff.draft_version_id);
  return (
    <section className="workLibraryWorkspace workLibraryDiffWorkspace">
      <header className="workLibraryHeader"><div><p className="sectionKicker">作品生产 / 作品库</p><h2>版本差异确认</h2><p>{details.title} · V{source?.version_no ?? "--"} → V{draft?.version_no ?? "--"}</p></div></header>
      <div aria-label="版本差异确认" className="workLibraryDiffDialog" role="dialog">
        <div className="workLibraryDiffColumns">
          <section><h3>结构化差异</h3>{diff.changes.map((change) => <article key={change.path}><strong>{change.path}</strong><span>{summaryValue(change.old_value)} → {summaryValue(change.new_value)}</span></article>)}</section>
          <section><h3>受影响任务</h3>{diff.affected_nodes.map((node) => <span className="workLibraryDiffNode" key={node}>{node}</span>)}</section>
          <section><h3>复用素材</h3>{diff.reused_artifact_ids.length ? diff.reused_artifact_ids.map((id) => <span className="workLibraryDiffNode reused" key={id}>{artifactName(details, id)}</span>) : <p className="workLibraryMuted">没有可复用素材</p>}</section>
        </div>
        <div className="workLibraryResourceUsage">
          <strong>再次调用资源用量</strong>
          <span>视频任务 {numberValue(diff.resource_usage.video_task_count)}</span>
          <span>视频 {numberValue(diff.resource_usage.video_seconds)} 秒</span>
          <span>TTS {numberValue(diff.resource_usage.tts_characters)} 字符</span>
          <span>ASR {numberValue(diff.resource_usage.asr_seconds)} 秒</span>
        </div>
        {error ? <p className="errorBanner">{error}</p> : null}
        <footer><button className="secondaryButton" disabled={busy} type="button" onClick={onBack}>返回草稿</button><button className="primaryButton" disabled={busy} type="button" onClick={onConfirm}>确认并创建运行</button></footer>
      </div>
    </section>
  );
}

function EmptyState({ title, description }: { title: string; description: string }) {
  return <div className="workLibraryEmpty"><strong>{title}</strong><span>{description}</span></div>;
}

function statusLabel(status: string) { return statusLabels[status] ?? status; }
function stageLabel(stage: string) { return ({ plan: "方案规划", tts: "TTS 配音", video_segment: "视频分段", subtitle: "字幕", mix: "混音", compose: "最终合成" } as Record<string, string>)[stage] ?? stage; }
function derivationLabel(kind: string) { return ({ initial: "初始版本", edit: "继续修改", full_regeneration: "整体重生成" } as Record<string, string>)[kind] ?? kind; }
function formatDateTime(value: string) { const date = new Date(value); return Number.isNaN(date.getTime()) ? value : date.toLocaleString("zh-CN", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", hour12: false }); }
function snapshotText(value: Record<string, unknown> | undefined, key: string) { return typeof value?.[key] === "string" ? value[key] as string : ""; }
function snapshotString(value: Record<string, unknown>, key: string) { return typeof value[key] === "string" ? value[key] as string : ""; }
function snapshotNumber(value: Record<string, unknown>, key: string) { return typeof value[key] === "number" && Number.isFinite(value[key]) ? value[key] as number : 0; }
function firstBusinessString(value: Record<string, unknown>, paths: string[][]) { for (const path of paths) { let current: unknown = value; for (const key of path) current = current && typeof current === "object" && !Array.isArray(current) ? (current as Record<string, unknown>)[key] : undefined; if (typeof current === "string" && current.trim()) return current; } return ""; }
function audioModeLabel(mode: string) { return ({ independent_tts: "独立 TTS", seedance_original: "Seedance 原声", seedance_original_and_tts: "原声 + TTS", existing_audio: "已有音频" } as Record<string, string>)[mode] ?? (mode || "声音未设置"); }
function modelCatalogName(details: WorkDetails, modelId: string) { return modelId ? details.model_catalog?.[modelId]?.display_name ?? "" : ""; }
function errorMessage(value: unknown, fallback: string) { return value instanceof Error ? value.message : fallback; }
function numberValue(value: unknown) { return typeof value === "number" && Number.isFinite(value) ? value : 0; }
function summaryValue(value: unknown) { if (value === null || value === undefined || value === "") return "未设置"; if (typeof value === "object") return JSON.stringify(value); return String(value); }
function timelineItemLabel(value: unknown, fallback: string) { if (!value || typeof value !== "object") return String(value || fallback); const item = value as Record<string, unknown>; return String(item.label ?? item.name ?? item.title ?? item.file_name ?? fallback); }
function artifactName(details: WorkDetails, id: string) { const artifact = details.artifacts.find((item) => item.id === id); return artifact ? `${artifactRoleLabels[artifact.role] ?? artifact.role} · ${artifact.file_name}` : id; }
function workAgentDiff(value: unknown): WorkVersionDiff | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const diff = value as Partial<WorkVersionDiff>;
  return typeof diff.id === "string"
    && typeof diff.work_id === "string"
    && typeof diff.source_version_id === "string"
    && typeof diff.draft_version_id === "string"
    && Array.isArray(diff.changes)
    && Array.isArray(diff.affected_nodes)
    && Array.isArray(diff.reused_artifact_ids)
    && diff.resource_usage !== null
    && typeof diff.resource_usage === "object"
    ? diff as WorkVersionDiff
    : null;
}
function isBlankDraft(details: WorkDetails) { const version = details.versions[0]; return details.versions.length === 1 && version?.status === "draft" && details.artifacts.length === 0 && details.generation_audit.length === 0 && [version.input_snapshot, version.model_snapshot, version.parameter_snapshot, version.prompt_snapshot, version.timeline_snapshot].every((value) => Object.keys(value).length === 0); }
function idempotencyKey() { const values = new Uint32Array(4); globalThis.crypto.getRandomValues(values); return Array.from(values, (value) => value.toString(16).padStart(8, "0")).join("-"); }
