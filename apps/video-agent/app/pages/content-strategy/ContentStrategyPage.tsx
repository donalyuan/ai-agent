import type { FormEvent } from "react";
import type {
  AgentMessage,
  ContentTopic,
  ContentTopicStats,
  ContentTopicStatus,
  PrepareScriptFromTopicResponse,
  Project,
  ScriptStyle,
} from "../../lib/api";
import {
  getTopicContentTypeLabel,
  scriptStyleLabels,
  topicPoolStatusFilters,
  topicSourceLabels,
  topicStatusClassNames,
  topicStatusLabels,
  type TopicFormState,
} from "./topicModel";

type ContentStrategyPageProps = {
  actionError: string;
  agentDraft: string;
  agentError: string;
  agentMessages: AgentMessage[];
  editingTopicId: string | null;
  error: string;
  loading: boolean;
  preparingScript: boolean;
  project?: Project;
  savingTopic: boolean;
  selectedTopic: ContentTopic | null;
  sendingAgentMessage: boolean;
  showTopicForm: boolean;
  stats: ContentTopicStats;
  statusFilter: "all" | ContentTopicStatus;
  activeTopicBatchId: string | null;
  showingAllTopicBatches: boolean;
  topicForm: TopicFormState;
  topics: ContentTopic[];
  writesDisabled: boolean;
  onCancelTopicForm: () => void;
  onClearTopicBatchFilter: () => void;
  onEditTopic: (topic: ContentTopic) => void;
  onNewTopic: () => void;
  onPrepareScript: (topic: ContentTopic) => void;
  onSelectTopic: (topicId: string) => void;
  onStatusFilterChange: (status: "all" | ContentTopicStatus) => void;
  onSubmitAgentMessage: (event: FormEvent<HTMLFormElement>) => void;
  onSubmitTopic: (event: FormEvent<HTMLFormElement>) => void;
  onTopicFormChange: (field: keyof TopicFormState, value: string) => void;
  onUpdateTopicStatus: (topic: ContentTopic, status: ContentTopicStatus) => void;
  setAgentDraft: (value: string) => void;
};

export function ContentStrategyPage({
  project,
  topics,
  stats,
  selectedTopic,
  statusFilter,
  activeTopicBatchId,
  showingAllTopicBatches,
  loading,
  error,
  actionError,
  writesDisabled,
  showTopicForm,
  editingTopicId,
  topicForm,
  savingTopic,
  agentDraft,
  agentError,
  agentMessages,
  sendingAgentMessage,
  preparingScript,
  onSelectTopic,
  onClearTopicBatchFilter,
  onStatusFilterChange,
  onNewTopic,
  onEditTopic,
  onCancelTopicForm,
  onTopicFormChange,
  onSubmitTopic,
  onUpdateTopicStatus,
  onPrepareScript,
  setAgentDraft,
  onSubmitAgentMessage,
}: ContentStrategyPageProps) {
  return (
    <div className="contentStrategyWorkspace">
      <section className="strategySummaryPanel">
        <div>
          <p className="sectionKicker">内容策略</p>
          <h2>内容策略</h2>
        </div>
        <div className="strategyStats" aria-label="选题统计">
          <MetricCard label="全部选题" tone="neutral" value={stats.total} />
          <MetricCard label="已确认" tone="success" value={stats.approved} />
          <MetricCard label="已成稿" tone="primary" value={stats.scripted} />
        </div>
      </section>

      <div className="contentStrategyGrid">
        <div className="strategyMainColumn">
          <TopicAgentPanel
            draft={agentDraft}
            error={agentError}
            messages={agentMessages}
            project={project}
            sending={sendingAgentMessage}
            writesDisabled={writesDisabled}
            setDraft={setAgentDraft}
            onSubmit={onSubmitAgentMessage}
          />

          <section aria-label="选题池" className="topicPoolPanel">
            <div className="panelHeader compactHeader">
              <div>
                <p className="sectionKicker">Topic Pool</p>
                <h2>选题池</h2>
              </div>
              <div className="scriptHeaderActions">
                <button
                  className="secondaryButton"
                  disabled={!project || writesDisabled}
                  onClick={onNewTopic}
                  type="button"
                >
                  新增选题
                </button>
                <span>{topics.length} 条</span>
              </div>
            </div>

            {activeTopicBatchId && !showingAllTopicBatches ? (
              <div className="topicBatchNotice">
                <span>当前生成批次</span>
                <button className="filterButton" onClick={onClearTopicBatchFilter} type="button">
                  查看全部选题
                </button>
              </div>
            ) : null}

            <div aria-label="选题状态筛选" className="topicFilters">
              {topicPoolStatusFilters.map((option) => (
                <button
                  className={statusFilter === option.value ? "filterButton selected" : "filterButton"}
                  key={option.value}
                  onClick={() => onStatusFilterChange(option.value)}
                  type="button"
                >
                  {option.label}
                </button>
              ))}
            </div>

            {loading ? <p className="stateText">正在加载选题</p> : null}
            {error ? <p className="errorText" role="alert">{error}</p> : null}
            {!loading && !topics.length ? (
              <div className="emptyState">
                <strong>还没有选题</strong>
                <span>可以手动新增，或用选题 Agent 生成候选后再确认。</span>
              </div>
            ) : null}

            <div className="topicList">
              {topics.map((topic) => (
                <button
                  className={selectedTopic?.topic_id === topic.topic_id ? "topicItem selected" : "topicItem"}
                  key={topic.topic_id}
                  onClick={() => onSelectTopic(topic.topic_id)}
                  type="button"
                >
                  <span className="topicTitle">{topic.title}</span>
                  <span className="topicMeta">
                    {`来源：${topicSourceLabels[topic.source]} · 类型：${getTopicContentTypeLabel(topic.content_type)}`}
                  </span>
                  <TopicStatusBadge status={topic.status} />
                  {topic.score !== null ? <strong>{Math.round(topic.score)}</strong> : null}
                </button>
              ))}
            </div>
          </section>
        </div>

        <aside aria-label="选题详情" className="topicDetailColumn" role="region">
          {showTopicForm ? (
            <TopicForm
              actionError={actionError}
              editing={Boolean(editingTopicId)}
              form={topicForm}
              saving={savingTopic}
              onCancel={onCancelTopicForm}
              onChange={onTopicFormChange}
              onSubmit={onSubmitTopic}
            />
          ) : (
            <TopicDetail
              actionError={actionError}
              preparingScript={preparingScript}
              topic={selectedTopic}
              writesDisabled={writesDisabled}
              onEdit={onEditTopic}
              onPrepareScript={onPrepareScript}
              onUpdateStatus={onUpdateTopicStatus}
            />
          )}
        </aside>
      </div>
    </div>
  );
}

function MetricCard({ label, tone, value }: { label: string; tone: "neutral" | "success" | "primary"; value: number }) {
  return (
    <div className={`metricCard ${tone}`}>
      <strong>{value}</strong>
      <span>{label}</span>
    </div>
  );
}

function TopicAgentPanel({
  draft,
  error,
  messages,
  project,
  sending,
  writesDisabled,
  setDraft,
  onSubmit,
}: {
  draft: string;
  error: string;
  messages: AgentMessage[];
  project?: Project;
  sending: boolean;
  writesDisabled: boolean;
  setDraft: (value: string) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  const disabled = !project || writesDisabled || sending;
  return (
    <section aria-label="选题 Agent" className="sidePanel topicAgentPanel">
      <div className="panelHeader">
        <div>
          <p className="sectionKicker">Agent</p>
          <h2>选题 Agent</h2>
        </div>
        <span className={!disabled ? "agentChatState ready" : "agentChatState"}>
          {!project ? "未选择" : writesDisabled ? "不可用" : "可生成"}
        </span>
      </div>
      {error ? <p className="errorText" role="alert">{error}</p> : null}
      <form className="agentChatForm" onSubmit={onSubmit}>
        <label>
          生成要求
          <textarea
            disabled={disabled}
            onChange={(event) => setDraft(event.target.value)}
            rows={3}
            value={draft}
          />
        </label>
        <button className="primaryButton" disabled={disabled} type="submit">
          {sending ? "生成中" : "生成选题"}
        </button>
      </form>
      <div className="agentMessages compactMessages" aria-label="选题 Agent 消息">
        {messages.length ? (
          messages.map((message) => (
            <article className={`agentMessage ${message.role}`} key={message.message_id}>
              <span>{message.role === "user" ? "你" : "Agent"}</span>
              <p>{message.content}</p>
            </article>
          ))
        ) : (
          <div className="agentEmptyState">
            <strong>等待生成要求</strong>
            <span>例如：本周 AI 工具方向，生成 8 个选题。</span>
          </div>
        )}
      </div>
    </section>
  );
}

function TopicForm({
  actionError,
  editing,
  form,
  saving,
  onCancel,
  onChange,
  onSubmit,
}: {
  actionError: string;
  editing: boolean;
  form: TopicFormState;
  saving: boolean;
  onCancel: () => void;
  onChange: (field: keyof TopicFormState, value: string) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  return (
    <section className="topicDetailPanel">
      <div className="panelHeader">
        <div>
          <p className="sectionKicker">{editing ? "Edit Topic" : "New Topic"}</p>
          <h2>{editing ? "编辑选题" : "新增选题"}</h2>
        </div>
      </div>
      {actionError ? <p className="errorText" role="alert">{actionError}</p> : null}
      <form className="topicForm" onSubmit={onSubmit}>
        <label>
          选题标题
          <input
            aria-label="选题标题"
            onChange={(event) => onChange("title", event.target.value)}
            value={form.title}
          />
        </label>
        <label>
          选题角度
          <textarea
            aria-label="选题角度"
            onChange={(event) => onChange("angle", event.target.value)}
            rows={3}
            value={form.angle}
          />
        </label>
        <label>
          目标受众
          <input
            aria-label="目标受众"
            onChange={(event) => onChange("target_audience", event.target.value)}
            value={form.target_audience}
          />
        </label>
        <label>
          核心看点
          <textarea
            aria-label="核心看点"
            onChange={(event) => onChange("hook_points", event.target.value)}
            rows={3}
            value={form.hook_points}
          />
        </label>
        <div className="topicFormGrid">
          <label>
            内容类型
            <input
              aria-label="内容类型"
              onChange={(event) => onChange("content_type", event.target.value)}
              value={form.content_type}
            />
          </label>
          <label>
            评分
            <input
              aria-label="评分"
              inputMode="numeric"
              onChange={(event) => onChange("score", event.target.value)}
              value={form.score}
            />
          </label>
        </div>
        <label>
          评分理由
          <textarea
            aria-label="评分理由"
            onChange={(event) => onChange("score_reason", event.target.value)}
            rows={3}
            value={form.score_reason}
          />
        </label>
        <label>
          标签
          <input
            aria-label="标签"
            onChange={(event) => onChange("tags", event.target.value)}
            value={form.tags}
          />
        </label>
        <div className="topicFormActions">
          <button className="secondaryButton" onClick={onCancel} type="button">
            取消
          </button>
          <button className="primaryButton" disabled={saving} type="submit">
            {saving ? "保存中" : "保存选题"}
          </button>
        </div>
      </form>
    </section>
  );
}

function TopicDetail({
  actionError,
  preparingScript,
  topic,
  writesDisabled,
  onEdit,
  onPrepareScript,
  onUpdateStatus,
}: {
  actionError: string;
  preparingScript: boolean;
  topic: ContentTopic | null;
  writesDisabled: boolean;
  onEdit: (topic: ContentTopic) => void;
  onPrepareScript: (topic: ContentTopic) => void;
  onUpdateStatus: (topic: ContentTopic, status: ContentTopicStatus) => void;
}) {
  if (!topic) {
    return (
      <div className="topicDetailPanel detailEmpty">
        <p className="sectionKicker">选题详情</p>
        <h2>选择选题后查看详情</h2>
        <span>选中选题后会显示角度、受众、看点、评分和下一步操作。</span>
      </div>
    );
  }

  return (
    <section className="topicDetailPanel">
      <div className="topicDetailHeader">
        <div>
          <p className="sectionKicker">选题详情</p>
          <h2>{topic.title}</h2>
        </div>
        <TopicStatusBadge status={topic.status} />
      </div>
      {actionError ? <p className="errorText" role="alert">{actionError}</p> : null}

      <div className="topicDetailMeta">
        <span>{`来源：${topicSourceLabels[topic.source]}`}</span>
        <span>{`类型：${getTopicContentTypeLabel(topic.content_type)}`}</span>
        {topic.score !== null ? <span>{Math.round(topic.score)} 分</span> : null}
      </div>

      <section className="topicDetailSection">
        <h3>角度</h3>
        <p>{topic.angle || "未填写"}</p>
      </section>
      <section className="topicDetailSection">
        <h3>目标受众</h3>
        <p>{topic.target_audience || "未填写"}</p>
      </section>
      <section className="topicDetailSection">
        <h3>核心看点</h3>
        {topic.hook_points.length ? (
          <ul>
            {topic.hook_points.map((hookPoint) => (
              <li key={hookPoint}>{hookPoint}</li>
            ))}
          </ul>
        ) : (
          <p>未填写</p>
        )}
      </section>
      <section className="topicDetailSection">
        <h3>评分理由</h3>
        <p>{topic.score_reason || "未填写"}</p>
      </section>
      <div className="tagList">
        {topic.tags.map((tag) => (
          <span key={tag}>{tag}</span>
        ))}
      </div>

      <div className="topicActions">
        <button className="secondaryButton" disabled={writesDisabled} onClick={() => onEdit(topic)} type="button">
          编辑选题
        </button>
        {topic.status === "idea" ? (
          <button
            className="primaryButton"
            disabled={writesDisabled}
            onClick={() => onUpdateStatus(topic, "approved")}
            type="button"
          >
            确认选题
          </button>
        ) : null}
        {topic.status === "approved" ? (
          <button
            className="primaryButton"
            disabled={writesDisabled || preparingScript}
            onClick={() => onPrepareScript(topic)}
            type="button"
          >
            {preparingScript ? "准备中" : "生成脚本"}
          </button>
        ) : null}
        {topic.status !== "archived" ? (
          <button
            className="secondaryButton dangerButton"
            disabled={writesDisabled}
            onClick={() => onUpdateStatus(topic, "archived")}
            type="button"
          >
            归档选题
          </button>
        ) : null}
      </div>
    </section>
  );
}

export function ScriptPreparationDialog({
  error,
  generating,
  options,
  preparation,
  onClose,
  onConfirm,
  onOptionsChange,
}: {
  error: string;
  generating: boolean;
  options: { style: ScriptStyle; scene_count: number };
  preparation: PrepareScriptFromTopicResponse;
  onClose: () => void;
  onConfirm: () => void;
  onOptionsChange: (options: { style: ScriptStyle; scene_count: number }) => void;
}) {
  return (
    <div className="modalBackdrop">
      <section aria-label="脚本生成确认" aria-modal="true" className="scriptConfirmDialog" role="dialog">
        <div className="panelHeader">
          <div>
            <p className="sectionKicker">Script Request</p>
            <h2>脚本生成确认</h2>
          </div>
          <button className="secondaryButton" disabled={generating} onClick={onClose} type="button">
            关闭
          </button>
        </div>

        <div className="confirmSnapshot">
          <span>来源选题</span>
          <strong>{preparation.topic_snapshot.title}</strong>
          <p>{preparation.topic_snapshot.angle}</p>
          <div className="tagList">
            {preparation.topic_snapshot.tags.map((tag) => (
              <span key={tag}>{tag}</span>
            ))}
          </div>
        </div>

        <div className="confirmControls">
          <label>
            脚本风格
            <select
              aria-label="脚本风格"
              onChange={(event) =>
                onOptionsChange({ ...options, style: event.target.value as ScriptStyle })
              }
              value={options.style}
            >
              {(["knowledge", "story", "tutorial"] as ScriptStyle[]).map((style) => (
                <option key={style} value={style}>
                  {scriptStyleLabels[style]}
                </option>
              ))}
            </select>
          </label>
          <label>
            分镜数
            <input
              aria-label="分镜数"
              max={12}
              min={3}
              onChange={(event) =>
                onOptionsChange({ ...options, scene_count: Number(event.target.value) || 6 })
              }
              type="number"
              value={options.scene_count}
            />
          </label>
        </div>

        {error ? <p className="errorText" role="alert">{error}</p> : null}

        <div className="dialogActions">
          <button className="secondaryButton" disabled={generating} onClick={onClose} type="button">
            取消
          </button>
          <button className="primaryButton" disabled={generating} onClick={onConfirm} type="button">
            {generating ? "生成中" : "确认生成"}
          </button>
        </div>
      </section>
    </div>
  );
}

export function TopicStatusBadge({ status }: { status: ContentTopicStatus }) {
  return (
    <span className={`statusBadge ${topicStatusClassNames[status]}`}>
      {topicStatusLabels[status]}
    </span>
  );
}
