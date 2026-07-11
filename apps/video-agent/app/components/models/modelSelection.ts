import type { ModelOption } from "../../lib/api";

export function reconcileModelSelection(currentModelId: string, models: ModelOption[]) {
  if (currentModelId) {
    return currentModelId;
  }

  return models.find((model) => model.is_default)?.model_id || models[0]?.model_id || "";
}

export function modelSelectionUnavailable(modelId: string, models: ModelOption[]) {
  return !modelId || !models.some((model) => model.model_id === modelId);
}
