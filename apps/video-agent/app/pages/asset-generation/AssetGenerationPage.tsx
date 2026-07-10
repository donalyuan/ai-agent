import type {
  Project,
  ScriptDetail,
  ScriptStatus,
  ScriptSummary,
} from "../../lib/api";
import { AssetCandidatePanel, type AssetCandidatePanelProps } from "../script-creation/AssetCandidatePanel";
import { statusClassNames, statusLabels } from "../script-creation/scriptModel";

type AssetGenerationPageProps = {
  assetCandidatePanel: Omit<AssetCandidatePanelProps, "script" | "writesDisabled"> | null;
  loadingProjects: boolean;
  loadingScriptDetail: boolean;
  loadingScripts: boolean;
  project?: Project;
  scriptError: string;
  scripts: ScriptSummary[];
  selectedProjectId: string;
  selectedScript: ScriptDetail | null;
  selectedScriptId: string | null;
  writesDisabled: boolean;
  onOpenScript: (scriptId: string) => void;
};

export function AssetGenerationPage({
  assetCandidatePanel,
  loadingProjects,
  loadingScriptDetail,
  loadingScripts,
  project,
  scriptError,
  scripts,
  selectedProjectId,
  selectedScript,
  selectedScriptId,
  writesDisabled,
  onOpenScript,
}: AssetGenerationPageProps) {
  const generateCandidatesDisabled =
    writesDisabled ||
    Boolean(assetCandidatePanel?.actionInProgress) ||
    (assetCandidatePanel?.plan ? !assetCandidatePanel.plan.can_create : false);

  return (
    <div className="assetGenerationPage">
      <section className="assetGenerationHeader">
        <div>
          <p className="sectionKicker">素材管理 / 素材生成</p>
          <div className="assetGenerationTitleRow">
            <h2>脚本详情素材候选</h2>
            {selectedScript ? (
              <>
                <StatusBadge status={selectedScript.status} />
                <span>{selectedScript.scenes.length} 分镜</span>
              </>
            ) : null}
          </div>
        </div>
        <div className="assetScriptSelectorGroup">
          <label>
            当前脚本
            <select
              aria-label="当前脚本"
              disabled={loadingProjects || loadingScripts || !scripts.length}
              onChange={(event) => onOpenScript(event.target.value)}
              value={selectedScriptId || ""}
            >
              {scripts.length ? null : <option value="">暂无脚本</option>}
              {scripts.map((script) => (
                <option key={script.script_id} value={script.script_id}>
                  {script.title}
                </option>
              ))}
            </select>
          </label>
          <span>{scripts.length} 条脚本</span>
          {selectedScript && assetCandidatePanel ? (
            <button
              className="primaryButton assetGenerateCandidatesButton"
              disabled={generateCandidatesDisabled}
              onClick={assetCandidatePanel.onGenerateCandidates}
              type="button"
            >
              {assetCandidatePanel.actionInProgress ? "处理中" : "生成素材候选"}
            </button>
          ) : null}
        </div>
      </section>

      {scriptError ? <p className="errorText assetGenerationPageError">{scriptError}</p> : null}

      <section className="assetGenerationWorkArea" aria-label="素材生成工作区">
        {loadingProjects || loadingScripts || loadingScriptDetail ? (
          <p className="stateText">正在读取素材生成工作区</p>
        ) : null}
        {!loadingProjects && !selectedProjectId ? (
          <div className="detailEmpty">
            <p className="sectionKicker">分镜素材候选</p>
            <h2>等待账号</h2>
            <span>选择账号后会在这里显示可生成素材的脚本。</span>
          </div>
        ) : null}
        {!loadingProjects && selectedProjectId && !loadingScripts && !scripts.length ? (
          <div className="detailEmpty">
            <p className="sectionKicker">分镜素材候选</p>
            <h2>还没有脚本</h2>
            <span>先在脚本生成中产出脚本，再进入这里生成分镜素材。</span>
          </div>
        ) : null}
        {!loadingScriptDetail && scripts.length > 0 && !selectedScript ? (
          <div className="detailEmpty">
            <p className="sectionKicker">分镜素材候选</p>
            <h2>选择脚本后生成素材候选</h2>
            <span>
              当前账号：{project?.name || "未选择"}。素材生成会按分镜拆分候选，AI 视频生成必须人工二次确认。
            </span>
          </div>
        ) : null}
        {!loadingScriptDetail && selectedScript && assetCandidatePanel ? (
          <AssetCandidatePanel
            {...assetCandidatePanel}
            script={selectedScript}
            writesDisabled={writesDisabled}
          />
        ) : null}
      </section>
    </div>
  );
}

function StatusBadge({ status }: { status: ScriptStatus }) {
  return <span className={`statusBadge ${statusClassNames[status]}`}>{statusLabels[status]}</span>;
}
