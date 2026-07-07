import { type FormEvent, useMemo, useState } from "react";
import type {
  ContentTopic,
  ContentTopicStats,
  Project,
  TopicGenerationBatchSummary,
} from "../../lib/api";
import { TopicStatusBadge } from "./ContentStrategyPage";
import {
  formatTopicBatchTime,
  getTopicContentTypeLabel,
  topicBatchStatusLabels,
  topicSourceLabels,
} from "./topicModel";

type TopicHistoryPageProps = {
  actionError: string;
  activeTopicBatchId: string | null;
  deletingTopicId: string | null;
  error: string;
  loading: boolean;
  loadingTopicBatches: boolean;
  project?: Project;
  stats: ContentTopicStats;
  topicBatchError: string;
  topicBatches: TopicGenerationBatchSummary[];
  topics: ContentTopic[];
  writesDisabled: boolean;
  onDeleteTopic: (topic: ContentTopic) => void;
  onSelectTopicBatch: (batchId: string) => void;
  onSupplementTopicBatch: (batchId: string, content: string) => Promise<void>;
};

export function TopicHistoryPage({
  actionError,
  activeTopicBatchId,
  deletingTopicId,
  error,
  loading,
  loadingTopicBatches,
  project,
  stats,
  topicBatchError,
  topicBatches,
  topics,
  writesDisabled,
  onDeleteTopic,
  onSelectTopicBatch,
  onSupplementTopicBatch,
}: TopicHistoryPageProps) {
  const [supplementDraft, setSupplementDraft] = useState("");
  const [supplementError, setSupplementError] = useState("");
  const [supplementing, setSupplementing] = useState(false);
  const rootBatches = useMemo(
    () => topicBatches.filter((batch) => !batch.supplement_of_batch_id),
    [topicBatches],
  );
  const selectedBatch =
    topicBatches.find((batch) => batch.batch_id === activeTopicBatchId) || rootBatches[0] || topicBatches[0] || null;
  const rootBatchId = selectedBatch?.supplement_of_batch_id || selectedBatch?.batch_id || null;
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
              <p className="sectionKicker">Generation History</p>
              <h2>历史生成</h2>
            </div>
            <span>{loadingTopicBatches ? "加载中" : `${rootBatches.length} 组`}</span>
          </div>

          {topicBatchError ? <p className="errorText" role="alert">{topicBatchError}</p> : null}
          {!loadingTopicBatches && !topicBatchError && !rootBatches.length ? (
            <div className="emptyState">
              <strong>还没有历史生成</strong>
              <span>通过选题 Agent 生成候选后，这里会按批次归档。</span>
            </div>
          ) : null}

          <div className="topicHistoryBatchList">
            {rootBatches.map((batch) => (
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
              </button>
            ))}
          </div>
        </aside>

        <section aria-label="当前主题选题" className="topicHistoryTopicPanel">
          <div className="panelHeader compactHeader">
            <div>
              <p className="sectionKicker">Topic Group</p>
              <h2>当前主题选题</h2>
            </div>
            {rootBatch ? <span>{rootBatch.prompt}</span> : project ? <span>{project.name}</span> : null}
          </div>

          {selectedBatch ? (
            <div className="topicHistoryBatchSummary" aria-label="批次详情">
              <span>{`原始生成：${rootBatch ? formatTopicBatchTime(rootBatch.created_at) : "-"}`}</span>
              <span>{`生成批次：${1 + relatedSupplementBatches.length}`}</span>
              <span>{`主题可见：${topics.length}`}</span>
              <span>{selectedBatch.supplement_of_batch_id ? "当前选中：补充批次" : "当前选中：原始批次"}</span>
              {selectedBatch.supplement_of_batch_id && rootBatch ? (
                <span>{`补充自：${rootBatch.prompt}`}</span>
              ) : null}
            </div>
          ) : null}

          {actionError ? <p className="errorText" role="alert">{actionError}</p> : null}
          {error ? <p className="errorText" role="alert">{error}</p> : null}
          {loading ? <p className="stateText">正在加载批次选题</p> : null}

          {!loading && selectedBatch && !topics.length ? (
            <div className="emptyState">
              <strong>当前主题没有可见选题</strong>
              <span>已移除的选题不会继续显示在管理视图中。</span>
            </div>
          ) : null}

          <div className="topicHistoryTopicList">
            {topics.map((topic) => (
              <article
                aria-label={`历史选题：${topic.title}`}
                className="topicHistoryTopicRow"
                key={topic.topic_id}
              >
                <div className="topicHistoryTopicMain">
                  <strong>{topic.title}</strong>
                  <span>
                    {`来源：${topicSourceLabels[topic.source]} · 类型：${getTopicContentTypeLabel(topic.content_type)}`}
                  </span>
                </div>
                <TopicStatusBadge status={topic.status} />
                <span
                  className={
                    topic.batch_id === rootBatchId
                      ? "topicHistoryOriginBadge"
                      : "topicHistoryOriginBadge supplement"
                  }
                >
                  {topic.batch_id === rootBatchId ? "原始生成" : "补充生成"}
                </span>
                {topic.score !== null ? <span className="topicHistoryScore">{Math.round(topic.score)} 分</span> : null}
                <div className="topicHistoryActions">
                  {topic.status === "scripted" ? (
                    <span className="topicHistoryLocked">已生成脚本，不可删除</span>
                  ) : (
                    <button
                      className="secondaryButton dangerButton"
                      disabled={writesDisabled || deletingTopicId === topic.topic_id}
                      onClick={() => onDeleteTopic(topic)}
                      type="button"
                    >
                      {deletingTopicId === topic.topic_id ? "移除中" : "移除"}
                    </button>
                  )}
                </div>
              </article>
            ))}
          </div>
        </section>

        <aside aria-label="补充操作" className="topicHistorySupplementPanel">
          <div className="panelHeader compactHeader">
            <div>
              <p className="sectionKicker">Supplement</p>
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
            <section aria-label="补充选题" className="topicHistorySupplementForm">
              <div className="panelHeader compactHeader">
                <div>
                  <p className="sectionKicker">Prompt</p>
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
              <div className="panelHeader compactHeader">
                <div>
                  <p className="sectionKicker">Related</p>
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
