import type { FormEvent, ReactNode } from "react";
import type { Project } from "../../lib/api";
import {
  accountStrategyProfileIsEmpty,
  type AccountStrategyFormState,
} from "./topicModel";

type AccountStrategyPageProps = {
  draftNotes: string;
  draftSummary: string;
  error: string;
  form: AccountStrategyFormState;
  generatingDraft: boolean;
  hasUnsavedChanges: boolean;
  modelSelect: ReactNode;
  modelUnavailable: boolean;
  project?: Project;
  saving: boolean;
  writesDisabled: boolean;
  onBackToTopicPool: () => void;
  onCancel: () => void;
  onDraftNotesChange: (value: string) => void;
  onFormChange: (field: keyof AccountStrategyFormState, value: string) => void;
  onGenerateDraft: () => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
};

export function AccountStrategyPage({
  project,
  form,
  draftNotes,
  draftSummary,
  error,
  generatingDraft,
  hasUnsavedChanges,
  modelSelect,
  modelUnavailable,
  saving,
  writesDisabled,
  onBackToTopicPool,
  onCancel,
  onDraftNotesChange,
  onFormChange,
  onGenerateDraft,
  onSubmit,
}: AccountStrategyPageProps) {
  const profile = project?.strategy_profile;
  const missingProfile = !profile || accountStrategyProfileIsEmpty(profile);
  const disabled = !project || writesDisabled;

  return (
    <section aria-label="账号策略资料" className="accountStrategyPage">
      <header className="accountStrategyHero">
        <div>
          <p className="sectionKicker">内容策略 / 账号策略</p>
          <div className="accountStrategyHeroTitle">
            <h2>账号策略</h2>
            <span>content-strategy</span>
            <span className="strategyModuleTag">账号策略</span>
          </div>
        </div>
        <div className="accountStrategyHeroActions">
          <button className="secondaryButton accountBackButton" onClick={onBackToTopicPool} type="button">
            返回当前选题池
          </button>
        </div>
      </header>

      <form className="accountStrategyCard" onSubmit={onSubmit}>
        <div className="accountStrategyCardHeader">
          <h3>账号策略资料</h3>
          <p>内容策略下的独立二级页面；第一版只维护内容账号策略资料，不保存平台 Cookie、Token 或发布授权。</p>
        </div>

        <div className={missingProfile ? "strategyMissingNotice" : "strategyDraftNotice"}>
          <strong>
            {missingProfile
              ? "当前正式策略资料待补齐；AI 草稿只预填下方表单，点击保存后才写入账号策略资料。"
              : "AI 草稿只预填下方表单，点击保存后才写入账号策略资料。"}
          </strong>
        </div>
        {error ? (
          <p className="errorText" role="alert">
            {error}
          </p>
        ) : null}

        <div className="accountStrategyBodyGrid">
          <section aria-label="基础资料" className="strategyBasicsPanel">
            <h4>基础资料</h4>
            {project ? (
              <div className="strategyTextPreview">
                <p>账号名称：{displayText(project.name)}</p>
                <p>定位摘要：{displayText(project.positioning)}</p>
                <p>描述：{displayText(project.description)}</p>
              </div>
            ) : (
              <p className="strategyPlaceholder">未选择账号，请先从顶部选择当前账号。</p>
            )}
            <div className="accountStrategyEditStack">
              <label>
                账号名称
                <input
                  aria-label="账号名称"
                  disabled={disabled || saving}
                  onChange={(event) => onFormChange("name", event.target.value)}
                  value={form.name}
                />
              </label>
              <label>
                定位摘要
                <textarea
                  aria-label="定位摘要"
                  disabled={disabled || saving}
                  onChange={(event) => onFormChange("positioning", event.target.value)}
                  rows={2}
                  value={form.positioning}
                />
              </label>
              <label>
                账号描述
                <textarea
                  aria-label="账号描述"
                  disabled={disabled || saving}
                  onChange={(event) => onFormChange("description", event.target.value)}
                  rows={2}
                  value={form.description}
                />
              </label>
            </div>
          </section>

          <section aria-label="结构化策略" className="strategyStructuredPanel">
            <h4>结构化策略</h4>
            <div className="strategyTextPreview structuredPreview">
              <p>目标受众：{displayText(profile?.target_audience)}</p>
              <p>内容支柱：{displayList(profile?.content_pillars)}</p>
              <p>风格语气：{displayText(profile?.tone_style)}</p>
              <p>禁区：{displayList(profile?.forbidden_topics)}</p>
              <p>参考账号：{displayList(profile?.reference_accounts)}</p>
              <p>选题偏好：{displayText(profile?.topic_preferences)}</p>
            </div>
            <div className="accountStrategyEditGrid">
              <label>
                目标受众
                <textarea
                  aria-label="目标受众"
                  disabled={disabled || saving}
                  onChange={(event) => onFormChange("target_audience", event.target.value)}
                  rows={2}
                  value={form.target_audience}
                />
              </label>
              <label>
                表达风格
                <textarea
                  aria-label="表达风格"
                  disabled={disabled || saving}
                  onChange={(event) => onFormChange("tone_style", event.target.value)}
                  rows={2}
                  value={form.tone_style}
                />
              </label>
              <label>
                内容支柱
                <textarea
                  aria-label="内容支柱"
                  disabled={disabled || saving}
                  onChange={(event) => onFormChange("content_pillars", event.target.value)}
                  rows={3}
                  value={form.content_pillars}
                />
              </label>
              <label>
                禁区方向
                <textarea
                  aria-label="禁区方向"
                  disabled={disabled || saving}
                  onChange={(event) => onFormChange("forbidden_topics", event.target.value)}
                  rows={3}
                  value={form.forbidden_topics}
                />
              </label>
              <label>
                参考账号
                <textarea
                  aria-label="参考账号"
                  disabled={disabled || saving}
                  onChange={(event) => onFormChange("reference_accounts", event.target.value)}
                  rows={2}
                  value={form.reference_accounts}
                />
              </label>
              <label>
                选题偏好
                <textarea
                  aria-label="选题偏好"
                  disabled={disabled || saving}
                  onChange={(event) => onFormChange("topic_preferences", event.target.value)}
                  rows={2}
                  value={form.topic_preferences}
                />
              </label>
            </div>
          </section>

          <section aria-label="保存后应用到选题链路" className="strategyContextPanel">
            <h4>保存后应用到选题链路</h4>
            <p>选题生成、质量闸门、主题组评审都读取同一份已保存账号策略；草稿或未保存修改不会参与生成。</p>
          </section>

          <section aria-label="AI 生成策略草稿" className="accountDraftPanel">
            <div>
              <h4>AI 生成策略草稿</h4>
              <label>
                AI 草稿补充方向
                <textarea
                  disabled={disabled || generatingDraft}
                  onChange={(event) => onDraftNotesChange(event.target.value)}
                  placeholder="面向想用 AI 做副业的新手，优先做教程和避坑，不要夸大收益。"
                  rows={2}
                  value={draftNotes}
                />
              </label>
              {draftSummary ? (
                <p className="strategyDraftSummary">草稿摘要：{draftSummary}</p>
              ) : (
                <p className="strategyDraftSummary muted">草稿摘要：保存前不会修改正式账号资料。</p>
              )}
              {modelSelect}
            </div>
            <div className="accountDraftActions">
              <button
                className="primaryButton accountDraftButton"
                disabled={disabled || generatingDraft || modelUnavailable}
                onClick={onGenerateDraft}
                type="button"
              >
                {generatingDraft ? "生成中" : "生成草稿"}
              </button>
              <span>手动触发 / 限 token / 最多重试 1 次</span>
            </div>
          </section>
        </div>

        <div className="accountStrategyActions">
          <button
            className="secondaryButton accountCancelButton"
            disabled={disabled || saving || !hasUnsavedChanges}
            onClick={onCancel}
            type="button"
          >
            取消
          </button>
          <button className="primaryButton accountSaveButton" disabled={disabled || saving} type="submit">
            {saving ? "保存中" : "保存并应用"}
          </button>
        </div>
      </form>
    </section>
  );
}

function displayText(value?: string) {
  const trimmed = value?.trim();
  return trimmed || "未填写";
}

function displayList(value?: string[]) {
  const values = value?.map((item) => item.trim()).filter(Boolean) || [];
  return values.length ? values.join(" / ") : "未填写";
}
