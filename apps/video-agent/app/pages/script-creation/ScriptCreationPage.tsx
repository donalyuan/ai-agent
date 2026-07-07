import type { FormEvent } from "react";
import type {
  AgentMessage,
  Project,
  ScriptDetail,
  ScriptStatus,
  ScriptSummary,
} from "../../lib/api";
import { topicSourceLabels } from "../content-strategy/topicModel";
import { formatDate, statusClassNames, statusLabels, statusOptions } from "./scriptModel";

type ScriptCreationPageProps = {
  agentDraft: string;
  agentError: string;
  agentMessages: AgentMessage[];
  loadingProjects: boolean;
  loadingScriptDetail: boolean;
  loadingScripts: boolean;
  projectError: string;
  scriptError: string;
  scripts: ScriptSummary[];
  selectedProject?: Project;
  selectedProjectId: string;
  selectedScript: ScriptDetail | null;
  selectedScriptId: string | null;
  sendingAgentMessage: boolean;
  statusError: string;
  statusFilter: "all" | ScriptStatus;
  updatingStatus: boolean;
  writesDisabled: boolean;
  onNewScript: () => void;
  onOpenScript: (scriptId: string) => void;
  onStatusFilterChange: (status: "all" | ScriptStatus) => void;
  onSubmitAgentMessage: (event: FormEvent<HTMLFormElement>) => void;
  onUpdateStatus: (status: ScriptStatus) => void;
  setAgentDraft: (value: string) => void;
};

export function ScriptCreationPage({
  agentDraft,
  agentError,
  agentMessages,
  loadingProjects,
  loadingScriptDetail,
  loadingScripts,
  projectError,
  scriptError,
  scripts,
  selectedProject,
  selectedProjectId,
  selectedScript,
  selectedScriptId,
  sendingAgentMessage,
  statusError,
  statusFilter,
  updatingStatus,
  writesDisabled,
  onNewScript,
  onOpenScript,
  onStatusFilterChange,
  onSubmitAgentMessage,
  onUpdateStatus,
  setAgentDraft,
}: ScriptCreationPageProps) {
  return (
    <div className="workspaceGrid">
      <section className="scriptColumn" aria-label="脚本列表">
        <div className="panelHeader compactHeader">
          <div>
            <p className="sectionKicker">脚本创作</p>
            <h2>脚本列表</h2>
          </div>
          <div className="scriptHeaderActions">
            <button
              className="secondaryButton"
              disabled={!selectedProjectId || writesDisabled}
              onClick={onNewScript}
              type="button"
            >
              新建脚本
            </button>
            <span>{scripts.length} 条</span>
          </div>
        </div>

        <div className="statusFilter" aria-label="脚本状态筛选">
          {statusOptions.map((option) => (
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

        {loadingProjects || loadingScripts ? <p className="stateText">正在加载脚本</p> : null}
        {scriptError ? <p className="errorText">{scriptError}</p> : null}
        {!loadingProjects && selectedProjectId && !scripts.length ? (
          <div className="emptyState">
            <strong>还没有脚本</strong>
            <span>在右侧脚本 Agent 对话中描述需求后生成第一版结构化脚本。</span>
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
              className={selectedScriptId === script.script_id ? "scriptItem selected" : "scriptItem"}
              key={script.script_id}
              onClick={() => onOpenScript(script.script_id)}
              type="button"
            >
              <span className="scriptTitle">{script.title}</span>
              <span className="scriptMeta">
                {script.scene_count} 镜 · {formatDate(script.created_at)}
              </span>
              {script.source_topic_title ? (
                <span className="scriptSourceTopic">来源选题：{script.source_topic_title}</span>
              ) : null}
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
          onUpdateStatus={onUpdateStatus}
        />
      </section>

      <aside className="actionColumn" aria-label="脚本 Agent 操作">
        {projectError ? <p className="errorText" role="alert">{projectError}</p> : null}
        <ScriptAgentConversationPanel
          apiUnavailable={writesDisabled}
          draft={agentDraft}
          error={agentError}
          messages={agentMessages}
          selectedProject={selectedProject}
          selectedScript={selectedScript}
          sending={sendingAgentMessage}
          setDraft={setAgentDraft}
          onSubmit={onSubmitAgentMessage}
        />
      </aside>
    </div>
  );
}

function ScriptAgentConversationPanel({
  apiUnavailable,
  draft,
  error,
  messages,
  selectedProject,
  selectedScript,
  sending,
  setDraft,
  onSubmit,
}: {
  apiUnavailable: boolean;
  draft: string;
  error: string;
  messages: AgentMessage[];
  selectedProject?: Project;
  selectedScript: ScriptDetail | null;
  sending: boolean;
  setDraft: (value: string) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  const inputDisabled = apiUnavailable || !selectedProject || sending;
  const ready = Boolean(selectedProject) && !apiUnavailable;
  const stateLabel = apiUnavailable ? "不可用" : selectedScript ? "可对话" : selectedProject ? "可生成" : "未选择";
  const bindingText = selectedProject
    ? selectedScript
      ? `当前项目：${selectedProject.name} / 脚本：${selectedScript.title}`
      : `当前项目：${selectedProject.name} / 新脚本生成`
    : "请选择项目后开始对话";
  const placeholder = selectedScript ? "描述要修改的分镜方向..." : "描述你想生成的脚本...";
  const label = selectedScript ? "修改方向" : "脚本需求";
  const emptyTitle = selectedScript ? "可直接指定分镜修改方向" : "可直接描述新脚本需求";
  const emptyHint = selectedScript
    ? "例如：把第 2 镜改得更有冲突感，画面更具体。"
    : "例如：生成一个关于 ChatGPT 工作流的 6 镜知识科普脚本。";

  return (
    <section aria-label="脚本 Agent 对话" className="sidePanel agentChatPanel">
      <div className="panelHeader">
        <div>
          <p className="sectionKicker">Agent</p>
          <h2>脚本 Agent 对话</h2>
        </div>
        <span className={ready ? "agentChatState ready" : "agentChatState"}>{stateLabel}</span>
      </div>

      <p className="helperText">{bindingText}</p>
      {error ? <p className="errorText" role="alert">{error}</p> : null}

      <div className="agentMessages" aria-label="脚本 Agent 消息">
        {messages.length ? (
          messages.map((message) => (
            <article className={`agentMessage ${message.role}`} key={message.message_id}>
              <span>{message.role === "user" ? "你" : "Agent"}</span>
              <p>{message.content}</p>
            </article>
          ))
        ) : (
          <div className="agentEmptyState">
            <strong>{emptyTitle}</strong>
            <span>{emptyHint}</span>
          </div>
        )}
      </div>

      <form className="agentChatForm" onSubmit={onSubmit}>
        <label>
          {label}
          <textarea
            disabled={inputDisabled}
            onChange={(event) => setDraft(event.target.value)}
            placeholder={placeholder}
            rows={3}
            value={draft}
          />
        </label>
        <button className="primaryButton" disabled={inputDisabled} type="submit">
          {sending ? "发送中" : "发送"}
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
        <span>通过脚本 Agent 生成脚本或从左侧列表选择脚本后，这里会展示旁白与画面指令。</span>
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

      {script.topic_snapshot ? (
        <section className="sourceTopicPanel">
          <div>
            <p className="sectionKicker">Topic Source</p>
            <h3>来源选题</h3>
          </div>
          <strong>{script.topic_snapshot.title}</strong>
          <p>{script.topic_snapshot.angle}</p>
          <div className="topicDetailMeta">
            <span>{script.topic_snapshot.content_type}</span>
            <span>{topicSourceLabels[script.topic_snapshot.source]}</span>
            {script.topic_snapshot.score !== null ? (
              <span>{Math.round(script.topic_snapshot.score)} 分</span>
            ) : null}
          </div>
        </section>
      ) : null}

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
