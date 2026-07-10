import { type FormEvent, useMemo, useState } from "react";
import type {
  ContentTopic,
  ContentTopicStats,
  ContentTopicStatus,
  Project,
  TopicGenerationBatchSummary,
  TopicGroupSort,
  TopicGroupSummary,
  TopicQualityEvaluation,
  TopicQualityFlag,
  TopicQualityGateItem,
  TopicReviewSnapshot,
} from "../../lib/api";
import { TopicReviewList } from "./TopicReviewList";
import {
  formatTopicBatchTime,
  topicBatchStatusLabels,
} from "./topicModel";

type TopicHistoryPageProps = {
  actionError: string;
  activeTopicBatchId: string | null;
  deletingTopicId: string | null;
  error: string;
  loading: boolean;
  loadingTopicBatches: boolean;
  preparingScript: boolean;
  project?: Project;
  qualityError: string;
  qualityEvaluation: TopicQualityEvaluation | null;
  qualityLoading: boolean;
  reviewError: string;
  reviewLoading: boolean;
  reviewSnapshot: TopicReviewSnapshot | null;
  stats: ContentTopicStats;
  topicBatchError: string;
  topicBatches: TopicGenerationBatchSummary[];
  topicGroups: TopicGroupSummary[];
  topicGroupSort: TopicGroupSort;
  topics: ContentTopic[];
  writesDisabled: boolean;
  onDeleteTopic: (topic: ContentTopic) => void;
  onPrepareScript: (topic: ContentTopic) => void;
  onReviewTopicGroup: () => Promise<void>;
  onSelectTopicBatch: (batchId: string) => void;
  onSupplementTopicBatch: (batchId: string, content: string) => Promise<void>;
  onTopicGroupSortChange: (sort: TopicGroupSort) => void;
  onUpdateTopicStatus: (topic: ContentTopic, status: ContentTopicStatus) => void;
};

export function TopicHistoryPage({
  actionError,
  activeTopicBatchId,
  deletingTopicId,
  error,
  loading,
  loadingTopicBatches,
  preparingScript,
  project,
  qualityError,
  qualityEvaluation,
  qualityLoading,
  reviewError,
  reviewLoading,
  reviewSnapshot,
  stats,
  topicBatchError,
  topicBatches,
  topicGroups,
  topicGroupSort,
  topics,
  writesDisabled,
  onDeleteTopic,
  onPrepareScript,
  onReviewTopicGroup,
  onSelectTopicBatch,
  onSupplementTopicBatch,
  onTopicGroupSortChange,
  onUpdateTopicStatus,
}: TopicHistoryPageProps) {
  const [supplementDraft, setSupplementDraft] = useState("");
  const [supplementError, setSupplementError] = useState("");
  const [supplementing, setSupplementing] = useState(false);
  const rootBatches = useMemo(
    () => topicBatches.filter((batch) => !batch.supplement_of_batch_id),
    [topicBatches],
  );
  const topicGroupsByRootId = useMemo(
    () => new Map(topicGroups.map((group) => [group.root_batch_id, group])),
    [topicGroups],
  );
  const displayGroupCount = topicGroups.length || rootBatches.length;
  const selectedBatch =
    topicBatches.find((batch) => batch.batch_id === activeTopicBatchId) || rootBatches[0] || topicBatches[0] || null;
  const rootBatchId = selectedBatch?.supplement_of_batch_id || selectedBatch?.batch_id || null;
  const selectedTopicGroup = rootBatchId ? topicGroupsByRootId.get(rootBatchId) || null : null;
  const rootBatch = rootBatchId
    ? topicBatches.find((batch) => batch.batch_id === rootBatchId) || selectedBatch
    : null;
  const relatedSupplementBatches = useMemo(
    () =>
      rootBatchId
        ? topicBatches.filter((batch) => batch.supplement_of_batch_id === rootBatchId)
        : [],
    [rootBatchId, topicBatches],
  );
  const supplementDisabled = writesDisabled || supplementing || !selectedBatch;

  async function handleSupplementSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedBatch) {
      return;
    }
    const content = supplementDraft.trim();
    if (!content) {
      setSupplementError("请输入补充要求");
      return;
    }

    setSupplementError("");
    setSupplementing(true);
    try {
      await onSupplementTopicBatch(selectedBatch.batch_id, content);
      setSupplementDraft("");
    } catch (error) {
      setSupplementError(errorToMessage(error));
    } finally {
      setSupplementing(false);
    }
  }

  return (
    <div className="contentStrategyWorkspace topicHistoryWorkspace">
      <section className="strategySummaryPanel">
        <div>
          <p className="sectionKicker">内容策略</p>
          <h2>历史生成</h2>
        </div>
        <div className="strategyStats" aria-label="历史选题统计">
          <MetricCard label="历史主题" tone="neutral" value={rootBatches.length} />
          <MetricCard label="当前主题选题" tone="success" value={topics.length} />
          <MetricCard label="已成稿" tone="primary" value={stats.scripted} />
        </div>
      </section>

      <section aria-label="历史生成列表页" className="topicHistoryLayout">
        <aside aria-label="历史生成批次" className="topicHistoryBatchPanel">
          <div className="panelHeader compactHeader">
            <div>
              <p className="sectionKicker">生成记录</p>
              <h2>历史生成</h2>
            </div>
            <span>{loadingTopicBatches ? "加载中" : `${displayGroupCount} 组`}</span>
          </div>

          <div aria-label="主题组排序" className="topicGroupSortToggle">
            <button
              aria-pressed={topicGroupSort === "script_priority"}
              className={topicGroupSort === "script_priority" ? "active" : ""}
              onClick={() => onTopicGroupSortChange("script_priority")}
              type="button"
            >
              脚本优先
            </button>
            <button
              aria-pressed={topicGroupSort === "created_at"}
              className={topicGroupSort === "created_at" ? "active" : ""}
              onClick={() => onTopicGroupSortChange("created_at")}
              type="button"
            >
              按时间
            </button>
          </div>

          {topicBatchError ? <p className="errorText" role="alert">{topicBatchError}</p> : null}
          {!loadingTopicBatches && !topicBatchError && !displayGroupCount ? (
            <div className="emptyState">
              <strong>还没有历史生成</strong>
              <span>通过选题 Agent生成候选后，这里会按批次归档。</span>
            </div>
          ) : null}

          <div className="topicHistoryBatchList">
            {topicGroups.length
              ? topicGroups.map((group) => (
                  <TopicGroupPriorityCard
                    group={group}
                    key={group.root_batch_id}
                    qualityEvaluation={qualityEvaluation}
                    selected={rootBatchId === group.root_batch_id}
                    onSelect={onSelectTopicBatch}
                  />
                ))
              : rootBatches.map((batch) => (
                  <button
                    aria-pressed={rootBatchId === batch.batch_id}
                    className={
                      rootBatchId === batch.batch_id
                        ? "topicHistoryBatchItem selected"
                        : "topicHistoryBatchItem"
                    }
                    key={batch.batch_id}
                    onClick={() => onSelectTopicBatch(batch.batch_id)}
                    type="button"
                  >
                    <strong>{batch.prompt}</strong>
                    <span>
                    {`${formatTopicBatchTime(batch.created_at)} · ${batch.topic_count} 条 · ${topicBatchStatusLabels[batch.status]}`}
                  </span>
                    {qualityEvaluation?.batch_id === batch.batch_id ? (
                      <TopicQualityBatchSummary evaluation={qualityEvaluation} />
                    ) : null}
                  </button>
                ))}
          </div>
        </aside>

        <section aria-label="当前主题选题" className="topicHistoryTopicPanel">
          <div className="panelHeader compactHeader">
            <div>
              <p className="sectionKicker">主题组</p>
              <h2>当前主题选题</h2>
            </div>
            {rootBatch ? <span>{rootBatch.prompt}</span> : project ? <span>{project.name}</span> : null}
          </div>

          {selectedBatch ? (
            <div className="topicHistoryBatchSummary" aria-label="批次详情">
              <span>{`原始生成：${rootBatch ? formatTopicBatchTime(rootBatch.created_at) : "-"}`}</span>
              <span>{`生成批次：${1 + relatedSupplementBatches.length}`}</span>
              <span>{`主题可见：${topics.length}`}</span>
              {selectedTopicGroup?.script_priority.score !== undefined ? (
                <span>{scriptPrioritySummary(selectedTopicGroup)}</span>
              ) : null}
              <span>{selectedBatch.supplement_of_batch_id ? "当前选中：补充批次" : "当前选中：原始批次"}</span>
              {selectedBatch.supplement_of_batch_id && rootBatch ? (
                <span>{`补充自：${rootBatch.prompt}`}</span>
              ) : null}
            </div>
          ) : null}

          {actionError ? <p className="errorText" role="alert">{actionError}</p> : null}
          {error ? <p className="errorText" role="alert">{error}</p> : null}
          {reviewError ? <p className="errorText" role="alert">{reviewError}</p> : null}
          {loading ? <p className="stateText">正在加载批次选题</p> : null}
          {reviewLoading ? <p className="stateText">正在加载主题组评审</p> : null}

          {!loading && selectedBatch && !topics.length ? (
            <div className="emptyState">
              <strong>当前主题没有可见选题</strong>
              <span>已移除的选题不会继续显示在管理视图中。</span>
            </div>
          ) : null}

          <TopicReviewList
            activeRootBatchId={rootBatchId}
            deletingTopicId={deletingTopicId}
            mode="history"
            preparingScript={preparingScript}
            reviewSnapshot={reviewSnapshot}
            topics={topics}
            writesDisabled={writesDisabled}
            onDeleteTopic={onDeleteTopic}
            onPrepareScript={onPrepareScript}
            onUpdateTopicStatus={onUpdateTopicStatus}
          />
        </section>

        <aside aria-label="补充操作" className="topicHistorySupplementPanel">
          <div className="panelHeader compactHeader topicHistorySupplementMainHeader">
            <div>
              <h2>补充操作</h2>
            </div>
            <span>{selectedBatch ? "批次上下文" : "待选择"}</span>
          </div>

          {rootBatch ? (
            <div className="topicHistoryRootBatch" aria-label="原始批次">
              <span>原始批次</span>
              <strong>{rootBatch.prompt}</strong>
              <small>{`${formatTopicBatchTime(rootBatch.created_at)} · ${rootBatch.topic_count} 条可见选题`}</small>
            </div>
          ) : (
            <div className="emptyState">
              <strong>请选择历史批次</strong>
              <span>选择批次后可以查看补充关系并继续生成。</span>
            </div>
          )}

          {selectedBatch ? (
            <TopicQualityReport
              error={qualityError}
              evaluation={qualityEvaluation}
              loading={qualityLoading}
            />
          ) : null}

          {selectedBatch ? (
            <section aria-label="主题组评审" className="topicHistoryReviewPanel">
              <div className="panelHeader compactHeader topicHistorySupplementSectionHeader">
                <div>
                  <h3>主题组评审</h3>
                </div>
                <span>{reviewSnapshot ? "已同步" : "待评审"}</span>
              </div>
              <button
                className="primaryButton"
                disabled={writesDisabled || reviewLoading || !rootBatchId}
                onClick={() => void onReviewTopicGroup()}
                type="button"
              >
                {reviewLoading ? "评审中" : "评审当前主题组"}
              </button>
              <p>评审只生成辅助分层，不会自动确认、归档或移除选题。</p>
            </section>
          ) : null}

          {selectedBatch ? (
            <section aria-label="补充选题" className="topicHistorySupplementForm">
              <div className="panelHeader compactHeader topicHistorySupplementSectionHeader">
                <div>
                  <h3>补充选题</h3>
                </div>
                <span>{supplementing ? "生成中" : "可补充"}</span>
              </div>
              {supplementError ? <p className="errorText" role="alert">{supplementError}</p> : null}
              <form className="agentChatForm" onSubmit={handleSupplementSubmit}>
                <label>
                  补充要求
                  <textarea
                    disabled={supplementDisabled}
                    onChange={(event) => setSupplementDraft(event.target.value)}
                    rows={4}
                    value={supplementDraft}
                  />
                </label>
                <button className="primaryButton" disabled={supplementDisabled} type="submit">
                  {supplementing ? "补充生成中" : "补充生成"}
                </button>
              </form>
            </section>
          ) : null}

          {selectedBatch && relatedSupplementBatches.length ? (
            <section aria-label="关联补充批次" className="topicHistorySupplementList">
              <div className="panelHeader compactHeader topicHistorySupplementSectionHeader">
                <div>
                  <h3>关联补充批次</h3>
                </div>
                <span>{`${relatedSupplementBatches.length} 批`}</span>
              </div>
              <div className="topicHistorySupplementItems">
                {relatedSupplementBatches.map((batch) => (
                  <button
                    className={
                      selectedBatch.batch_id === batch.batch_id
                        ? "topicHistorySupplementItem selected"
                        : "topicHistorySupplementItem"
                    }
                    key={batch.batch_id}
                    onClick={() => onSelectTopicBatch(batch.batch_id)}
                    type="button"
                  >
                    <strong>{batch.prompt}</strong>
                    <span>{`${formatTopicBatchTime(batch.created_at)} · ${batch.topic_count} 条`}</span>
                  </button>
                ))}
              </div>
            </section>
          ) : null}

          {selectedBatch ? (
            <p className="topicHistorySupplementHint">
              补充会创建新的生成批次，原始批次保持不变；再次补充时仍归入同一个原始批次。
            </p>
          ) : null}
        </aside>
      </section>
    </div>
  );
}

function errorToMessage(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }
  return "补充生成失败";
}

function TopicGroupPriorityCard({
  group,
  qualityEvaluation,
  selected,
  onSelect,
}: {
  group: TopicGroupSummary;
  qualityEvaluation: TopicQualityEvaluation | null;
  selected: boolean;
  onSelect: (batchId: string) => void;
}) {
  const candidateCount = group.script_priority.recommended_topic_ids.length;
  return (
    <button
      aria-pressed={selected}
      className={selected ? "topicHistoryBatchItem topicGroupPriorityItem selected" : "topicHistoryBatchItem topicGroupPriorityItem"}
      onClick={() => onSelect(group.root_batch_id)}
      type="button"
    >
      <strong>{`${topicGroupStatusLabel(group)} · ${group.prompt}`}</strong>
      <span>{`${formatTopicBatchTime(group.created_at)} · ${topicGroupScoreLabel(group, candidateCount)}`}</span>
      <span>{`主题可见 ${group.topic_count} · 补充批次 ${group.supplement_batch_count}`}</span>
      <em>{topicGroupRiskSummary(group)}</em>
      {qualityEvaluation?.batch_id === group.root_batch_id ? (
        <TopicQualityBatchSummary evaluation={qualityEvaluation} />
      ) : null}
    </button>
  );
}

function TopicQualityBatchSummary({ evaluation }: { evaluation: TopicQualityEvaluation }) {
  return (
    <span className="topicQualitySummaryText">{`质量：通过 ${evaluation.pass_count} · 淘汰 ${evaluation.reject_count} · ${
      evaluation.rewrite_triggered ? "已重写" : "未重写"
    }`}</span>
  );
}

function TopicQualityReport({
  error,
  evaluation,
  loading,
}: {
  error: string;
  evaluation: TopicQualityEvaluation | null;
  loading: boolean;
}) {
  const rejectedItems = evaluation?.result.items.filter((item) => item.decision === "reject") ?? [];
  return (
    <section aria-label="质量报告" className="topicQualityReportPanel">
      <div className="panelHeader compactHeader topicHistorySupplementSectionHeader">
        <div>
          <h3>质量报告</h3>
        </div>
        <span>{loading ? "加载中" : evaluation ? "已同步" : "无报告"}</span>
      </div>
      {error ? <p className="errorText" role="alert">{error}</p> : null}
      {loading ? <p className="stateText">正在加载质量报告</p> : null}
      {!loading && !evaluation ? (
        <p className="topicQualityReportEmpty">当前批次暂无质量报告。</p>
      ) : null}
      {evaluation ? (
        <>
          <div className="topicQualityReportStats" aria-label="质量评估统计">
            <span>{`通过 ${evaluation.pass_count}`}</span>
            <span>{`淘汰 ${evaluation.reject_count}`}</span>
            <span>{evaluation.rewrite_triggered ? "已重写" : "未重写"}</span>
          </div>
          <p className="topicQualityReportSummary">{evaluation.result.summary}</p>
          <div className="topicQualityRejectedList">
            {rejectedItems.length ? (
              rejectedItems.map((item) => <TopicQualityRejectedItem item={item} key={item.candidate_key} />)
            ) : (
              <p className="topicQualityReportEmpty">没有淘汰候选。</p>
            )}
          </div>
        </>
      ) : null}
    </section>
  );
}

function TopicQualityRejectedItem({ item }: { item: TopicQualityGateItem }) {
  return (
    <article aria-label={`淘汰候选：${item.title}`} className="topicQualityRejectedItem">
      <div>
        <strong>{item.title}</strong>
        <span>淘汰</span>
      </div>
      <div className="topicQualityRejectedMeta">
        <span>{`${item.quality_score} 分`}</span>
        {item.flags.map((flag) => (
          <span className="topicQualityFlag" key={flag}>
            {topicQualityFlagLabel(flag)}
          </span>
        ))}
      </div>
      <p>{item.reason}</p>
    </article>
  );
}

function topicQualityFlagLabel(flag: TopicQualityFlag) {
  const labels: Record<TopicQualityFlag, string> = {
    too_generic: "泛化",
    duplicate: "疑似重复",
    off_positioning: "偏离定位",
    hard_to_script: "脚本化难",
    compliance_risk: "合规风险",
    score_untrusted: "评分存疑",
  };
  return labels[flag];
}

function topicGroupStatusLabel(group: TopicGroupSummary) {
  if (group.script_priority.status === "needs_review") {
    return group.review_freshness === "stale" ? "需重新评审" : "待评审";
  }
  if (group.script_priority.status === "ready_for_script") {
    return "建议立刻出脚本";
  }
  if (group.script_priority.status === "needs_supplement") {
    return "需补充";
  }
  return "暂缓";
}

function topicGroupScoreLabel(group: TopicGroupSummary, candidateCount: number) {
  if (group.script_priority.score === null) {
    const freshnessText = group.review_freshness === "stale" ? "评审已过期" : "缺少评审";
    return `${freshnessText} · ${group.topic_count} 条选题`;
  }
  return `${group.script_priority.score} 分 · ${candidateCount} 个候选 · ${group.script_priority.metrics.priority_count} 个优先`;
}

function topicGroupRiskSummary(group: TopicGroupSummary) {
  const metrics = group.script_priority.metrics;
  const risks = [
    metrics.duplicate_count ? `重复 ${metrics.duplicate_count}` : "",
    metrics.hard_to_script_count ? `脚本化难 ${metrics.hard_to_script_count}` : "",
    metrics.off_positioning_count ? `偏离定位 ${metrics.off_positioning_count}` : "",
    metrics.compliance_risk_count ? `合规风险 ${metrics.compliance_risk_count}` : "",
  ].filter(Boolean);
  return risks.length ? risks.join(" · ") : "风险低";
}

function scriptPrioritySummary(group: TopicGroupSummary) {
  if (group.script_priority.score === null) {
    return topicGroupStatusLabel(group);
  }
  return `脚本优先：${group.script_priority.score} 分`;
}

function MetricCard({
  label,
  tone,
  value,
}: {
  label: string;
  tone: "neutral" | "success" | "primary";
  value: number;
}) {
  return (
    <div className={`metricCard ${tone}`}>
      <strong>{value}</strong>
      <span>{label}</span>
    </div>
  );
}
