import { useRef, useState, type ReactNode } from "react";
import { ImagePreviewDialog } from "../../components/ImagePreviewDialog";
import type {
  AssetGenerationPlanResponse,
  AssetGenerationTask,
  SceneAssetCandidate,
  ScriptDetail,
} from "../../lib/api";
import {
  assetCandidateStatusLabels,
  assetTaskStatusLabels,
  candidatePreviewUrl,
  candidatesForScene,
  selectedCandidateForScene,
  sortScenesBySequence,
} from "./assetModel";

export type AssetCandidatePanelProps = {
  actionInProgress: boolean;
  dismissingTaskId: string | null;
  candidates: SceneAssetCandidate[];
  candidateCount: number;
  error: string;
  loadingCandidates: boolean;
  loadingPlan: boolean;
  plan: AssetGenerationPlanResponse | null;
  modelSelect: ReactNode;
  modelUnavailable: boolean;
  script: ScriptDetail;
  selectedSceneId: string | null;
  tasks: AssetGenerationTask[];
  taskToDismissId: string | null;
  useReferenceMaterials: boolean;
  writesDisabled: boolean;
  onCandidateCountChange: (count: number) => void;
  onCancelDismissTask: () => void;
  onConfirmDismissTask: () => void;
  onConfirmVideoTask: (taskId: string) => void;
  onGenerateCandidates: () => void;
  onRegenerateScene: (sceneId: string) => void;
  onRequestDismissTask: (taskId: string) => void;
  onRejectCandidate: (sceneId: string, candidateId: string) => void;
  onSelectCandidate: (sceneId: string, candidateId: string) => void;
  onSelectScene: (sceneId: string) => void;
  onUseReferenceMaterialsChange: (enabled: boolean) => void;
};

const candidateCountOptions = [1, 2, 3, 4];

export function AssetCandidatePanel({
  actionInProgress,
  dismissingTaskId,
  candidates,
  candidateCount,
  error,
  loadingCandidates,
  loadingPlan,
  plan,
  modelSelect,
  modelUnavailable,
  script,
  selectedSceneId,
  tasks,
  taskToDismissId,
  useReferenceMaterials,
  writesDisabled,
  onCandidateCountChange,
  onCancelDismissTask,
  onConfirmDismissTask,
  onConfirmVideoTask,
  onRegenerateScene,
  onRequestDismissTask,
  onRejectCandidate,
  onSelectCandidate,
  onSelectScene,
  onUseReferenceMaterialsChange,
}: AssetCandidatePanelProps) {
  const previewTriggerRef = useRef<HTMLButtonElement | null>(null);
  const [imagePreview, setImagePreview] = useState<{
    imageUrl: string;
    title: string;
  } | null>(null);
  const scenes = sortScenesBySequence(script.scenes);
  const activeScene = scenes.find((scene) => scene.scene_id === selectedSceneId) || scenes[0] || null;
  const activeSceneCandidates = activeScene ? candidatesForScene(candidates, activeScene.scene_id) : [];
  const selectedCandidate = activeScene
    ? selectedCandidateForScene(activeSceneCandidates, activeScene.scene_id)
    : null;
  const existingCandidates = activeSceneCandidates.filter(
    (candidate) => candidate.source === "existing_material" && candidate.status !== "selected",
  );
  const aiImageCandidates = activeSceneCandidates.filter(
    (candidate) =>
      candidate.source === "ai_generated" &&
      candidate.candidate_type === "image" &&
      candidate.status !== "selected",
  );
  const planSceneCount = plan?.scene_count || scenes.length;
  const planImageCount = plan?.image_candidate_count || planSceneCount * candidateCount;
  const maxImageCount = plan?.max_image_candidate_count || 48;
  const imageTaskEntries = imageTasksForScene(tasks, activeScene?.scene_id || null);
  const videoTaskEntries = videoTasksForScene(tasks, activeScene?.scene_id || null);
  const taskToDismiss = tasks.find((task) => task.task_id === taskToDismissId) || null;

  const openImagePreview = (candidate: SceneAssetCandidate, trigger: HTMLButtonElement) => {
    const imageUrl = candidatePreviewUrl(candidate);
    if (!imageUrl) {
      return;
    }
    previewTriggerRef.current = trigger;
    setImagePreview({
      imageUrl,
      title: candidate.file_name || "AI 图片候选",
    });
  };

  const closeImagePreview = () => {
    const trigger = previewTriggerRef.current;
    setImagePreview(null);
    window.requestAnimationFrame(() => {
      if (trigger?.isConnected) {
        trigger.focus();
      }
    });
  };

  return (
    <section aria-label="脚本详情素材候选" className="assetCandidatePanel">
      {error ? (
        <p className="errorText" role="alert">
          {error}
        </p>
      ) : null}

      <div className="assetCandidateGrid">
        <section className="assetSceneRail" aria-label="分镜列表">
          <div className="assetColumnHeader">
            <h4>分镜列表</h4>
            <span>{scenes.length} 镜</span>
          </div>
          <div className="assetSceneList">
            {scenes.map((scene) => {
              const sceneSelectedCandidate = selectedCandidateForScene(candidates, scene.scene_id);
              return (
                <button
                  className={activeScene?.scene_id === scene.scene_id ? "assetSceneItem selected" : "assetSceneItem"}
                  key={scene.scene_id}
                  onClick={() => onSelectScene(scene.scene_id)}
                  type="button"
            >
                  <span>镜头 {scene.sequence}</span>
                  <strong>{scene.duration_sec} 秒</strong>
                  <em>{sceneSelectedCandidate ? "已选主素材" : "待选择"}</em>
                </button>
              );
            })}
          </div>
        </section>

        <section className="assetCandidateBrowser" aria-label="候选素材">
          <div className="assetColumnHeader">
            <h4>候选素材</h4>
            <span>{loadingCandidates ? "读取中" : `${activeSceneCandidates.length} 个候选`}</span>
          </div>

          {activeScene ? (
            <section className="assetSceneContext" aria-label={`镜头 ${activeScene.sequence} 内容`}>
              <strong>
                镜头 {String(activeScene.sequence).padStart(2, "0")} 内容 · {activeScene.duration_sec} 秒
              </strong>
              <div className="assetSceneContextColumns">
                <div>
                  <span>旁白</span>
                  <p>{activeScene.narration.trim() || "未填写旁白"}</p>
                </div>
                <div>
                  <span>画面</span>
                  <p>{activeScene.visual_description.trim() || "未填写画面"}</p>
                </div>
              </div>
            </section>
          ) : null}

          <CandidateSection layout="single" title="当前主素材">
            {selectedCandidate ? (
              <CandidateCard
                actionInProgress={actionInProgress}
                candidate={selectedCandidate}
                isSelected
                writesDisabled={writesDisabled}
                onRejectCandidate={onRejectCandidate}
                onSelectCandidate={onSelectCandidate}
              />
            ) : (
              <p className="assetEmptyText">当前分镜还没有主素材。</p>
            )}
          </CandidateSection>

          <CandidateSection layout="pair" title="旧素材候选">
            {existingCandidates.length ? (
              existingCandidates.map((candidate) => (
                <CandidateCard
                  actionInProgress={actionInProgress}
                  candidate={candidate}
                  key={candidate.candidate_id}
                  writesDisabled={writesDisabled}
                  onRejectCandidate={onRejectCandidate}
                  onSelectCandidate={onSelectCandidate}
                />
              ))
            ) : (
              <p className="assetEmptyText">没有匹配到更多旧素材。</p>
            )}
          </CandidateSection>

          <CandidateSection layout="triple" title="AI 图片候选">
            {aiImageCandidates.length ? (
              aiImageCandidates.map((candidate) => (
                <CandidateCard
                  actionInProgress={actionInProgress}
                  candidate={candidate}
                  key={candidate.candidate_id}
                  previewEnabled
                  writesDisabled={writesDisabled}
                  onOpenPreview={openImagePreview}
                  onRejectCandidate={onRejectCandidate}
                  onSelectCandidate={onSelectCandidate}
                />
              ))
            ) : (
              <p className="assetEmptyText">生成后会在这里展示多张图片候选。</p>
            )}
          </CandidateSection>
        </section>

        <aside className="assetGenerationSettings" aria-label="生成设置与任务">
          <div className="assetColumnHeader">
            <h4>生成设置与任务</h4>
            <span>{loadingPlan ? "计算中" : `上限 ${maxImageCount} 张`}</span>
          </div>

          <div className="assetSettingBlock">
            {modelSelect}
          </div>

          <div className="assetSettingBlock">
            <span className="assetSettingLabel">每分镜候选数</span>
            <div className="assetSegmentedControl" aria-label="每分镜候选数">
              {candidateCountOptions.map((count) => (
                <button
                  className={candidateCount === count ? "filterButton selected" : "filterButton"}
                  disabled={writesDisabled || actionInProgress}
                  key={count}
                  onClick={() => onCandidateCountChange(count)}
                  type="button"
                >
                  {count}
                </button>
              ))}
            </div>
          </div>

          <label className="assetReferenceToggle">
            <input
              checked={useReferenceMaterials}
              disabled={writesDisabled || actionInProgress || modelUnavailable}
              onChange={(event) => onUseReferenceMaterialsChange(event.target.checked)}
              type="checkbox"
            />
            复用旧人物/IP 素材作参考图
          </label>

          <div className="assetPlanSummary">
            <strong>
              {planSceneCount} 分镜 × {candidateCount} = {planImageCount} 张图片候选
            </strong>
            <span>单次最多 {maxImageCount} 张</span>
            {plan?.reference_material_count ? <span>{plan.reference_material_count} 个旧素材可作参考</span> : null}
            {plan?.warnings.map((warning) => (
              <span className="assetWarning" key={warning}>
                {warning}
              </span>
            ))}
          </div>

          {activeScene ? (
            <button
              className="secondaryButton"
              disabled={writesDisabled || actionInProgress || modelUnavailable}
              onClick={() => onRegenerateScene(activeScene.scene_id)}
              type="button"
            >
              单镜头重生
            </button>
          ) : null}

          <section className="assetImageTasks" aria-label="AI 图片生成任务">
            <div>
              <h5>AI 图片生成任务</h5>
            </div>
            {imageTaskEntries.length ? (
              imageTaskEntries.map((task) => (
                <div className={task.canDismiss ? "assetTaskRow failed" : "assetTaskRow"} key={task.task_id}>
                  <span>{task.label}</span>
                  <small>{task.candidateCountLabel}</small>
                  <strong>{task.statusLabel}</strong>
                  {task.resultLabel ? <small>{task.resultLabel}</small> : null}
                  {task.errorMessage ? <p className="errorText">{task.errorMessage}</p> : null}
                  {task.canDismiss ? (
                    <div className="assetTaskDismissActions">
                      <button
                        className="assetDangerTextButton"
                        disabled={writesDisabled || dismissingTaskId === task.task_id}
                        onClick={() => onRequestDismissTask(task.task_id)}
                        type="button"
                      >
                        {dismissingTaskId === task.task_id ? "正在清理" : "清理失败任务"}
                      </button>
                      <span>页面隐藏 · 审计保留</span>
                    </div>
                  ) : null}
                </div>
              ))
            ) : (
              <p className="assetEmptyText">暂无图片生成任务。</p>
            )}
          </section>

          <section className="assetVideoTasks" aria-label="AI 视频二次确认">
            <div>
              <h5>AI 视频二次确认</h5>
              <p>视频生成会产生外部费用，必须人工确认后才启动。</p>
            </div>
            {videoTaskEntries.length ? (
              videoTaskEntries.map((task) => (
                <div className="assetTaskRow" key={task.task_id}>
                  <span>{task.label}</span>
                  <strong>{task.statusLabel}</strong>
                  {task.canConfirm ? (
                    <button
                      className="secondaryButton"
                      disabled={writesDisabled || actionInProgress}
                      onClick={() => onConfirmVideoTask(task.task_id)}
                      type="button"
                    >
                      确认生成视频
                    </button>
                  ) : null}
                </div>
              ))
            ) : (
              <p className="assetEmptyText">生成素材候选后会创建待确认视频任务。</p>
            )}
          </section>
        </aside>
      </div>

      {imagePreview ? (
        <ImagePreviewDialog
          alt={imagePreview.title}
          imageUrl={imagePreview.imageUrl}
          subtitle="AI 生成图片候选"
          title={imagePreview.title}
          onClose={closeImagePreview}
        />
      ) : null}

      {taskToDismiss?.status === "failed" ? (
        <div className="assetDismissDialogBackdrop">
          <section
            aria-labelledby="asset-dismiss-dialog-title"
            aria-modal="true"
            className="assetDismissDialog"
            role="dialog"
          >
            <h4 id="asset-dismiss-dialog-title">清理失败任务？</h4>
            <p>
              该任务及其失败候选将从素材生成页面隐藏。此操作不会重新调用供应商，也不会产生额外费用。
            </p>
            <p className="assetDismissAuditNote">
              数据库继续保留任务状态、错误、候选数量和费用审计；已生成素材不受影响。
            </p>
            <div className="assetDismissDialogActions">
              <button
                className="secondaryButton"
                disabled={dismissingTaskId === taskToDismiss.task_id}
                onClick={onCancelDismissTask}
                type="button"
              >
                取消
              </button>
              <button
                className="assetDangerButton"
                disabled={dismissingTaskId === taskToDismiss.task_id}
                onClick={onConfirmDismissTask}
                type="button"
              >
                {dismissingTaskId === taskToDismiss.task_id ? "正在清理" : "确认清理"}
              </button>
            </div>
          </section>
        </div>
      ) : null}
    </section>
  );
}

function CandidateSection({
  children,
  layout,
  title,
}: {
  children: ReactNode;
  layout: "single" | "pair" | "triple";
  title: string;
}) {
  return (
    <section className="assetCandidateSection">
      <h5>{title}</h5>
      <div className={`assetCandidateCards ${layout}`}>{children}</div>
    </section>
  );
}

function CandidateCard({
  actionInProgress,
  candidate,
  isSelected = false,
  previewEnabled = false,
  writesDisabled,
  onOpenPreview,
  onRejectCandidate,
  onSelectCandidate,
}: {
  actionInProgress: boolean;
  candidate: SceneAssetCandidate;
  isSelected?: boolean;
  previewEnabled?: boolean;
  writesDisabled: boolean;
  onOpenPreview?: (candidate: SceneAssetCandidate, trigger: HTMLButtonElement) => void;
  onRejectCandidate: (sceneId: string, candidateId: string) => void;
  onSelectCandidate: (sceneId: string, candidateId: string) => void;
}) {
  const previewUrl = candidatePreviewUrl(candidate);
  const selectable = candidate.status !== "failed" && candidate.status !== "rejected";
  const rejectable = candidate.status !== "failed" && candidate.status !== "rejected";
  const showSelectAction = candidate.status === "candidate";
  const showRejectAction =
    candidate.status === "candidate" || (candidate.status === "selected" && candidate.source === "ai_generated");
  const selectLabel = candidate.source === "existing_material" ? "选择旧素材" : "选择为主素材";
  const rejectLabel = candidate.source === "existing_material" ? "排除旧素材" : "排除候选";
  const previewAvailable =
    previewEnabled && Boolean(previewUrl) && candidate.status !== "failed" && Boolean(onOpenPreview);

  return (
    <article className={isSelected ? "assetCandidateCard selected" : "assetCandidateCard"}>
      {previewAvailable && previewUrl && onOpenPreview ? (
        <button
          aria-label={`查看${candidate.file_name || "AI 图片候选"}大图`}
          className="assetPreviewFrame assetPreviewButton"
          title="查看大图"
          type="button"
          onClick={(event) => onOpenPreview(candidate, event.currentTarget)}
        >
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img alt={candidate.file_name || "素材候选预览"} src={previewUrl} />
          <span aria-hidden="true" className="assetPreviewExpandIcon">⛶</span>
        </button>
      ) : (
        <div className="assetPreviewFrame">
          {previewUrl ? (
            // eslint-disable-next-line @next/next/no-img-element
            <img alt={candidate.file_name || "素材候选预览"} src={previewUrl} />
          ) : (
            <span>{candidate.status === "failed" ? "生成失败" : "等待生成"}</span>
          )}
        </div>
      )}
      <div className="assetCandidateInfo">
        <strong>{candidate.file_name || "未生成文件"}</strong>
        <span>{assetCandidateStatusLabels[candidate.status]}</span>
        {typeof candidate.metadata.error_message === "string" ? (
          <em>{candidate.metadata.error_message}</em>
        ) : null}
      </div>
      {showSelectAction || showRejectAction ? (
        <div className="assetCandidateActions">
          {showSelectAction ? (
            <button
              className="secondaryButton"
              disabled={writesDisabled || actionInProgress || !selectable}
              onClick={() => onSelectCandidate(candidate.scene_id, candidate.candidate_id)}
              type="button"
            >
              {selectLabel}
            </button>
          ) : null}
          {showRejectAction ? (
            <button
              className="secondaryButton"
              disabled={writesDisabled || actionInProgress || !rejectable}
              onClick={() => onRejectCandidate(candidate.scene_id, candidate.candidate_id)}
              type="button"
            >
              {rejectLabel}
            </button>
          ) : null}
        </div>
      ) : null}
    </article>
  );
}

function imageTasksForScene(tasks: AssetGenerationTask[], sceneId: string | null) {
  return tasks
    .filter(
      (task) =>
        task.task_type === "image_candidates" &&
        (!task.scene_id || !sceneId || task.scene_id === sceneId),
    )
    .sort(
      (left, right) =>
        right.created_at.localeCompare(left.created_at) || right.task_id.localeCompare(left.task_id),
    )
    .slice(0, 5)
    .map((task) => {
      const generatedCount = task.result.generated_count;
      return {
        canDismiss: task.status === "failed",
        candidateCountLabel: `${task.candidate_count} 张`,
        errorMessage: task.error_message,
        label: task.scene_id ? "当前分镜图片重生" : "整批图片候选",
        resultLabel:
          typeof generatedCount === "number" ? `已生成 ${generatedCount} 张` : null,
        statusLabel: assetTaskStatusLabels[task.status],
        task_id: task.task_id,
      };
    });
}

function videoTasksForScene(
  tasks: AssetGenerationTask[],
  sceneId: string | null,
) {
  const taskEntries = new Map<
    string,
    { canConfirm: boolean; label: string; statusLabel: string; task_id: string }
  >();

  for (const task of tasks) {
    if (task.task_type !== "video_draft" && task.task_type !== "video_generation") {
      continue;
    }
    if (sceneId && task.scene_id && task.scene_id !== sceneId) {
      continue;
    }
    taskEntries.set(task.task_id, {
      canConfirm: task.task_type === "video_draft" && task.status === "draft",
      label: task.scene_id ? "当前分镜视频任务" : "脚本视频任务",
      statusLabel: assetTaskStatusLabels[task.status],
      task_id: task.task_id,
    });
  }

  return Array.from(taskEntries.values());
}
