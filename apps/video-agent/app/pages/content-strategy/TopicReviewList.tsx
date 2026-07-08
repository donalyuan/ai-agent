import { useState } from "react";

import type {
  ContentTopic,
  ContentTopicStatus,
  TopicReviewItem,
  TopicReviewPriority,
  TopicReviewRiskFlag,
  TopicReviewSnapshot,
} from "../../lib/api";
import {
  getTopicContentTypeLabel,
  topicSourceLabels,
  topicStatusClassNames,
  topicStatusLabels,
} from "./topicModel";

const reviewPrioritySections: Array<{ priority: TopicReviewPriority; label: string }> = [
  { priority: "priority", label: "优先推荐" },
  { priority: "backup", label: "可备选" },
  { priority: "reject", label: "建议淘汰" },
];

const riskFlagLabels: Record<TopicReviewRiskFlag, string> = {
  too_generic: "泛化",
  duplicate: "疑似重复",
  hard_to_script: "脚本化难",
  off_positioning: "偏离定位",
  compliance_risk: "合规风险",
};

type TopicReviewListProps = {
  activeRootBatchId?: string | null;
  deletingTopicId?: string | null;
  mode: "history" | "pool";
  preparingScript?: boolean;
  reviewSnapshot: TopicReviewSnapshot | null;
  selectedTopicId?: string | null;
  topics: ContentTopic[];
  writesDisabled: boolean;
  onDeleteTopic?: (topic: ContentTopic) => void;
  onPrepareScript?: (topic: ContentTopic) => void;
  onSelectTopic?: (topicId: string) => void;
  onUpdateTopicStatus?: (topic: ContentTopic, status: ContentTopicStatus) => void;
};

export function TopicReviewList({
  activeRootBatchId,
  deletingTopicId = null,
  mode,
  preparingScript = false,
  reviewSnapshot,
  selectedTopicId = null,
  topics,
  writesDisabled,
  onDeleteTopic,
  onPrepareScript,
  onSelectTopic,
  onUpdateTopicStatus,
}: TopicReviewListProps) {
  const [locatedTopicId, setLocatedTopicId] = useState<string | null>(null);
  const effectiveSelectedTopicId = locatedTopicId || selectedTopicId;

  function handleSelectTopic(topicId: string) {
    setLocatedTopicId(null);
    onSelectTopic?.(topicId);
  }

  function handleLocateTopic(topicId: string) {
    setLocatedTopicId(topicId);
    onSelectTopic?.(topicId);
    window.requestAnimationFrame(() => {
      const target = document.querySelector(`[data-topic-id="${topicId}"]`);
      if (target && typeof target.scrollIntoView === "function") {
        target.scrollIntoView({
          block: "nearest",
          behavior: "smooth",
        });
      }
    });
  }

  if (!reviewSnapshot) {
    return mode === "history" ? (
      <HistoryTopicRows
        activeRootBatchId={activeRootBatchId}
        deletingTopicId={deletingTopicId}
        topics={topics}
        writesDisabled={writesDisabled}
        onDeleteTopic={onDeleteTopic}
      />
    ) : (
      <PoolTopicRows
        deletingTopicId={deletingTopicId}
        selectedTopicId={effectiveSelectedTopicId}
        topics={topics}
        writesDisabled={writesDisabled}
        onDeleteTopic={onDeleteTopic}
        onSelectTopic={handleSelectTopic}
      />
    );
  }

  const topicsById = new Map(topics.map((topic) => [topic.topic_id, topic]));
  const reviewItems = reviewSnapshot.result.topic_reviews;
  const reviewedTopicIds = new Set(reviewItems.map((item) => item.topic_id));
  const duplicateItems = reviewItems.filter(
    (item) => item.risk_flags.includes("duplicate") || item.similar_topic_ids.length > 0,
  );
  const unreviewedTopics = topics.filter((topic) => !reviewedTopicIds.has(topic.topic_id));

  return (
    <div className="topicReviewList" aria-label="主题组评审分层">
      <div className="topicReviewSummary">
        <span>最新评审</span>
        <p>{reviewSnapshot.review_summary}</p>
      </div>

      {reviewPrioritySections.map((section) => {
        const sectionItems = reviewItems
          .filter((item) => item.priority === section.priority)
          .map((item) => ({ item, topic: topicsById.get(item.topic_id) }))
          .filter((entry): entry is { item: TopicReviewItem; topic: ContentTopic } => Boolean(entry.topic));

        return (
          <section aria-label={section.label} className="topicReviewSection" key={section.priority}>
            <div className="topicReviewSectionHeader">
              <h3>{section.label}</h3>
              <span>{sectionItems.length} 条</span>
            </div>
            {sectionItems.length ? (
              <div className="topicReviewCards">
                {sectionItems.map(({ item, topic }) => (
                  <ReviewedTopicCard
                    activeRootBatchId={activeRootBatchId}
                    deletingTopicId={deletingTopicId}
                    item={item}
                    key={topic.topic_id}
                    mode={mode}
                    preparingScript={preparingScript}
                    selectedTopicId={effectiveSelectedTopicId}
                    topic={topic}
                    topicsById={topicsById}
                    writesDisabled={writesDisabled}
                    onDeleteTopic={onDeleteTopic}
                    onLocateTopic={handleLocateTopic}
                    onPrepareScript={onPrepareScript}
                    onSelectTopic={onSelectTopic ? handleSelectTopic : undefined}
                    onUpdateTopicStatus={onUpdateTopicStatus}
                  />
                ))}
              </div>
            ) : (
              <p className="topicReviewEmpty">暂无选题</p>
            )}
          </section>
        );
      })}

      <section aria-label="疑似重复" className="topicReviewSection">
        <div className="topicReviewSectionHeader">
          <h3>疑似重复</h3>
          <span>{duplicateItems.length} 组</span>
        </div>
        {duplicateItems.length ? (
          <div className="topicDuplicateList">
            {duplicateItems.map((item) => {
              const topic = topicsById.get(item.topic_id);
              if (!topic) {
                return null;
              }
              const similarTitles = item.similar_topic_ids
                .map((topicId) => topicsById.get(topicId)?.title || topicId)
                .join("、");
              return (
                <article className="topicDuplicateItem" key={item.topic_id}>
                  <strong>{topic.title}</strong>
                  <span>{similarTitles ? `疑似重复：${similarTitles}` : "疑似重复：需人工复核"}</span>
                </article>
              );
            })}
          </div>
        ) : (
          <p className="topicReviewEmpty">暂无疑似重复</p>
        )}
      </section>

      {unreviewedTopics.length ? (
        <section aria-label="未评审选题" className="topicReviewSection">
          <div className="topicReviewSectionHeader">
            <h3>未评审选题</h3>
            <span>{unreviewedTopics.length} 条</span>
          </div>
          <div className="topicReviewCards">
            {unreviewedTopics.map((topic) => (
              <ReviewedTopicCard
                activeRootBatchId={activeRootBatchId}
                deletingTopicId={deletingTopicId}
                item={null}
                key={topic.topic_id}
                mode={mode}
                preparingScript={preparingScript}
                selectedTopicId={effectiveSelectedTopicId}
                topic={topic}
                topicsById={topicsById}
                writesDisabled={writesDisabled}
                onDeleteTopic={onDeleteTopic}
                onLocateTopic={handleLocateTopic}
                onPrepareScript={onPrepareScript}
                onSelectTopic={onSelectTopic ? handleSelectTopic : undefined}
                onUpdateTopicStatus={onUpdateTopicStatus}
              />
            ))}
          </div>
        </section>
      ) : null}
    </div>
  );
}

function PoolTopicRows({
  deletingTopicId,
  selectedTopicId,
  topics,
  writesDisabled,
  onDeleteTopic,
  onSelectTopic,
}: {
  deletingTopicId: string | null;
  selectedTopicId: string | null;
  topics: ContentTopic[];
  writesDisabled: boolean;
  onDeleteTopic?: (topic: ContentTopic) => void;
  onSelectTopic?: (topicId: string) => void;
}) {
  return (
    <div className="topicList">
      {topics.map((topic) => (
        <article
          aria-label={`选题：${topic.title}`}
          className={selectedTopicId === topic.topic_id ? "topicItem selected" : "topicItem"}
          data-topic-id={topic.topic_id}
          key={topic.topic_id}
        >
          <button className="topicPoolTitleButton" onClick={() => onSelectTopic?.(topic.topic_id)} type="button">
            <span className="topicTitle">{topic.title}</span>
          </button>
          <span className="topicMeta">
            {`来源：${topicSourceLabels[topic.source]} · 类型：${getTopicContentTypeLabel(topic.content_type)}`}
          </span>
          <div className="topicPoolBadges">
            <TopicStatusBadge status={topic.status} />
          </div>
          {topic.score !== null ? <strong className="topicScore">{Math.round(topic.score)}</strong> : null}
          <div className="topicPoolActions">
            {topic.status === "scripted" ? (
              <span className="topicHistoryLocked">已成稿不可移除</span>
            ) : (
              <button
                className="secondaryButton dangerButton"
                disabled={writesDisabled || deletingTopicId === topic.topic_id}
                onClick={() => onDeleteTopic?.(topic)}
                type="button"
              >
                {deletingTopicId === topic.topic_id ? "移除中" : "移除"}
              </button>
            )}
          </div>
        </article>
      ))}
    </div>
  );
}

function HistoryTopicRows({
  activeRootBatchId,
  deletingTopicId,
  topics,
  writesDisabled,
  onDeleteTopic,
}: {
  activeRootBatchId?: string | null;
  deletingTopicId: string | null;
  topics: ContentTopic[];
  writesDisabled: boolean;
  onDeleteTopic?: (topic: ContentTopic) => void;
}) {
  return (
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
          <OriginBadge activeRootBatchId={activeRootBatchId} topic={topic} />
          {topic.score !== null ? <span className="topicHistoryScore">{Math.round(topic.score)} 分</span> : null}
          <div className="topicHistoryActions">
            {topic.status === "scripted" ? (
              <span className="topicHistoryLocked">已生成脚本，不可删除</span>
            ) : (
              <button
                className="secondaryButton dangerButton"
                disabled={writesDisabled || deletingTopicId === topic.topic_id}
                onClick={() => onDeleteTopic?.(topic)}
                type="button"
              >
                {deletingTopicId === topic.topic_id ? "移除中" : "移除"}
              </button>
            )}
          </div>
        </article>
      ))}
    </div>
  );
}

function ReviewedTopicCard({
  activeRootBatchId,
  deletingTopicId,
  item,
  mode,
  preparingScript,
  selectedTopicId,
  topic,
  topicsById,
  writesDisabled,
  onDeleteTopic,
  onLocateTopic,
  onPrepareScript,
  onSelectTopic,
  onUpdateTopicStatus,
}: {
  activeRootBatchId?: string | null;
  deletingTopicId: string | null;
  item: TopicReviewItem | null;
  mode: "history" | "pool";
  preparingScript: boolean;
  selectedTopicId: string | null;
  topic: ContentTopic;
  topicsById: Map<string, ContentTopic>;
  writesDisabled: boolean;
  onDeleteTopic?: (topic: ContentTopic) => void;
  onLocateTopic: (topicId: string) => void;
  onPrepareScript?: (topic: ContentTopic) => void;
  onSelectTopic?: (topicId: string) => void;
  onUpdateTopicStatus?: (topic: ContentTopic, status: ContentTopicStatus) => void;
}) {
  return (
    <article
      aria-label={`${mode === "history" ? "历史选题" : "评审选题"}：${topic.title}`}
      className={selectedTopicId === topic.topic_id ? "topicReviewCard selected" : "topicReviewCard"}
      data-topic-id={topic.topic_id}
    >
      <div className="topicReviewCardMain">
        {onSelectTopic ? (
          <button className="topicReviewTitleButton" onClick={() => onSelectTopic(topic.topic_id)} type="button">
            {topic.title}
          </button>
        ) : (
          <strong>{topic.title}</strong>
        )}
        <span>
          {`来源：${topicSourceLabels[topic.source]} · 类型：${getTopicContentTypeLabel(topic.content_type)}`}
        </span>
      </div>

      <div className="topicReviewMeta">
        <TopicStatusBadge status={topic.status} />
        <OriginBadge activeRootBatchId={activeRootBatchId} topic={topic} />
        {topic.score !== null ? <span>{Math.round(topic.score)} 分</span> : null}
      </div>

      {item ? (
        <div className="topicReviewReason">
          <p>{item.reason}</p>
          {item.risk_flags.length ? (
            <div className="topicReviewRiskTags">
              {item.risk_flags.map((flag) => (
                <span key={flag}>{riskFlagLabels[flag]}</span>
              ))}
            </div>
          ) : null}
          {item.similar_topic_ids.length ? (
            <div className="topicReviewDuplicateRefs">
              <span className="topicReviewDuplicateLabel">相似选题</span>
              {item.similar_topic_ids.map((topicId) => {
                const similarTopic = topicsById.get(topicId);
                return similarTopic ? (
                  <button
                    className="topicReviewDuplicateTitle"
                    key={topicId}
                    onClick={() => onLocateTopic(topicId)}
                    type="button"
                  >
                    {similarTopic.title}
                  </button>
                ) : (
                  <span className="topicReviewDuplicateTitle" key={topicId}>
                    {topicId}
                  </span>
                );
              })}
            </div>
          ) : null}
        </div>
      ) : (
        <div className="topicReviewReason">
          <p>新补充选题暂未纳入本次评审。</p>
        </div>
      )}

      <TopicReviewActions
        deleting={deletingTopicId === topic.topic_id}
        preparingScript={preparingScript}
        topic={topic}
        writesDisabled={writesDisabled}
        onDeleteTopic={onDeleteTopic}
        onPrepareScript={onPrepareScript}
        onUpdateTopicStatus={onUpdateTopicStatus}
      />
    </article>
  );
}

function TopicReviewActions({
  deleting,
  preparingScript,
  topic,
  writesDisabled,
  onDeleteTopic,
  onPrepareScript,
  onUpdateTopicStatus,
}: {
  deleting: boolean;
  preparingScript: boolean;
  topic: ContentTopic;
  writesDisabled: boolean;
  onDeleteTopic?: (topic: ContentTopic) => void;
  onPrepareScript?: (topic: ContentTopic) => void;
  onUpdateTopicStatus?: (topic: ContentTopic, status: ContentTopicStatus) => void;
}) {
  return (
    <div className="topicReviewActions">
      {topic.status === "idea" && onUpdateTopicStatus ? (
        <button
          className="primaryButton"
          disabled={writesDisabled}
          onClick={() => onUpdateTopicStatus(topic, "approved")}
          type="button"
        >
          确认选题
        </button>
      ) : null}
      {topic.status === "approved" && onPrepareScript ? (
        <button
          className="primaryButton"
          disabled={writesDisabled || preparingScript}
          onClick={() => onPrepareScript(topic)}
          type="button"
        >
          {preparingScript ? "准备中" : "生成脚本"}
        </button>
      ) : null}
      {topic.status !== "archived" && onUpdateTopicStatus ? (
        <button
          className="secondaryButton"
          disabled={writesDisabled}
          onClick={() => onUpdateTopicStatus(topic, "archived")}
          type="button"
        >
          归档选题
        </button>
      ) : null}
      {onDeleteTopic ? (
        topic.status === "scripted" ? (
          <span className="topicHistoryLocked">已生成脚本，不可删除</span>
        ) : (
          <button
            className="secondaryButton dangerButton"
            disabled={writesDisabled || deleting}
            onClick={() => onDeleteTopic(topic)}
            type="button"
          >
            {deleting ? "移除中" : "移除"}
          </button>
        )
      ) : null}
    </div>
  );
}

function OriginBadge({
  activeRootBatchId,
  topic,
}: {
  activeRootBatchId?: string | null;
  topic: ContentTopic;
}) {
  return (
    <span
      className={
        topic.batch_id === activeRootBatchId
          ? "topicHistoryOriginBadge"
          : "topicHistoryOriginBadge supplement"
      }
    >
      {topic.batch_id === activeRootBatchId ? "原始生成" : "补充生成"}
    </span>
  );
}

function TopicStatusBadge({ status }: { status: ContentTopicStatus }) {
  return (
    <span className={`statusBadge ${topicStatusClassNames[status]}`}>
      {topicStatusLabels[status]}
    </span>
  );
}
