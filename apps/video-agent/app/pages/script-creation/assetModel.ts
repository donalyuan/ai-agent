import type {
  AssetGenerationProvider,
  AssetGenerationRequestPayload,
  AssetGenerationTask,
  Scene,
  SceneAssetCandidate,
} from "../../lib/api";

export const assetProviderLabels: Record<AssetGenerationProvider, string> = {
  "gpt-image-2": "gpt-image-2",
  jimeng: "即梦",
};

export const assetTaskStatusLabels: Record<AssetGenerationTask["status"], string> = {
  draft: "待确认",
  pending: "排队中",
  processing: "生成中",
  completed: "已完成",
  failed: "失败",
};

export const assetCandidateStatusLabels: Record<SceneAssetCandidate["status"], string> = {
  candidate: "候选",
  selected: "已选为主素材",
  rejected: "已排除",
  failed: "生成失败",
};

export function assetGenerationPayload(
  provider: AssetGenerationProvider,
  imageCandidatesPerScene: number,
  useReferenceMaterials: boolean,
): AssetGenerationRequestPayload {
  return {
    provider,
    image_candidates_per_scene: imageCandidatesPerScene,
    use_reference_materials: useReferenceMaterials,
  };
}

export function sortScenesBySequence(scenes: Scene[]) {
  return [...scenes].sort((left, right) => left.sequence - right.sequence);
}

export function candidatesForScene(candidates: SceneAssetCandidate[], sceneId: string) {
  return candidates
    .filter((candidate) => candidate.scene_id === sceneId)
    .sort((left, right) => left.rank - right.rank || left.created_at.localeCompare(right.created_at));
}

export function selectedCandidateForScene(candidates: SceneAssetCandidate[], sceneId: string) {
  return candidatesForScene(candidates, sceneId).find((candidate) => candidate.status === "selected") || null;
}

export function candidatePreviewUrl(candidate: SceneAssetCandidate) {
  return candidate.thumbnail_url || candidate.file_url || "";
}

export function mergeUpdatedCandidate(
  candidates: SceneAssetCandidate[],
  updatedCandidate: SceneAssetCandidate,
) {
  const nextCandidates = candidates.map((candidate) => {
    if (
      updatedCandidate.status === "selected" &&
      candidate.scene_id === updatedCandidate.scene_id &&
      candidate.candidate_id !== updatedCandidate.candidate_id &&
      candidate.status === "selected"
    ) {
      return { ...candidate, status: "candidate" as const };
    }

    return candidate.candidate_id === updatedCandidate.candidate_id ? updatedCandidate : candidate;
  });

  if (nextCandidates.some((candidate) => candidate.candidate_id === updatedCandidate.candidate_id)) {
    return nextCandidates;
  }

  return [...nextCandidates, updatedCandidate];
}

export function upsertAssetTask(tasks: AssetGenerationTask[], updatedTask: AssetGenerationTask) {
  const withoutTask = tasks.filter((task) => task.task_id !== updatedTask.task_id);
  return [updatedTask, ...withoutTask];
}
