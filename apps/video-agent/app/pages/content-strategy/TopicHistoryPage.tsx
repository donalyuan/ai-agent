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
}: TopicHistoryPageProps) {
  const selectedBatch =
    topicBatches.find((batch) => batch.batch_id === activeTopicBatchId) || topicBatches[0] || null;

  return (
    <div className="contentStrategyWorkspace topicHistoryWorkspace">
      <section className="strategySummaryPanel">
        <div>
          <p className="sectionKicker">内容策略</p>
          <h2>历史生成</h2>
        </div>
        <div className="strategyStats" aria-label="历史选题统计">
          <MetricCard label="历史批次" tone="neutral" value={topicBatches.length} />
          <MetricCard label="当前批次选题" tone="success" value={topics.length} />
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
            <span>{loadingTopicBatches ? "加载中" : `${topicBatches.length} 批`}</span>
          </div>

          {topicBatchError ? <p className="errorText" role="alert">{topicBatchError}</p> : null}
          {!loadingTopicBatches && !topicBatchError && !topicBatches.length ? (
            <div className="emptyState">
              <strong>还没有历史生成</strong>
              <span>通过选题 Agent 生成候选后，这里会按批次归档。</span>
            </div>
          ) : null}

          <div className="topicHistoryBatchList">
            {topicBatches.map((batch) => (
              <button
                aria-pressed={selectedBatch?.batch_id === batch.batch_id}
                className={
                  selectedBatch?.batch_id === batch.batch_id
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

        <section aria-label="历史生成批次详情" className="topicHistoryDetailPanel">
          <div className="panelHeader compactHeader">
            <div>
              <p className="sectionKicker">Batch Detail</p>
              <h2>{selectedBatch ? selectedBatch.prompt : "选择生成批次"}</h2>
            </div>
            {project ? <span>{project.name}</span> : null}
          </div>

          {selectedBatch ? (
            <div className="topicHistoryBatchSummary" aria-label="批次详情">
              <span>{`生成时间：${formatTopicBatchTime(selectedBatch.created_at)}`}</span>
              <span>{`请求数量：${selectedBatch.requested_count}`}</span>
              <span>{`当前可见：${selectedBatch.topic_count}`}</span>
              <span>{topicBatchStatusLabels[selectedBatch.status]}</span>
            </div>
          ) : null}

          {actionError ? <p className="errorText" role="alert">{actionError}</p> : null}
          {error ? <p className="errorText" role="alert">{error}</p> : null}
          {loading ? <p className="stateText">正在加载批次选题</p> : null}

          {!loading && selectedBatch && !topics.length ? (
            <div className="emptyState">
              <strong>当前批次没有可见选题</strong>
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
      </section>
    </div>
  );
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
