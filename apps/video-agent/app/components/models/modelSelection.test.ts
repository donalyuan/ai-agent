import { describe, expect, it } from "vitest";
import type { ModelOption } from "../../lib/api";
import { modelSelectionUnavailable, reconcileModelSelection } from "./modelSelection";

const defaultModel: ModelOption = {
  model_id: "11111111-1111-4111-8111-111111111111",
  display_name: "默认文本模型",
  model_type: "text",
  provider_name: "OpenAI",
  api_protocol: "openai_responses",
  upstream_model: "gpt-default",
  is_default: true,
};

const secondaryModel: ModelOption = {
  ...defaultModel,
  model_id: "22222222-2222-4222-8222-222222222222",
  display_name: "备用文本模型",
  upstream_model: "gpt-secondary",
  is_default: false,
};

describe("模型选择状态", () => {
  it("首次加载优先选择后台默认模型", () => {
    expect(reconcileModelSelection("", [secondaryModel, defaultModel])).toBe(defaultModel.model_id);
  });

  it("没有默认模型时选择第一条可用模型", () => {
    expect(reconcileModelSelection("", [secondaryModel])).toBe(secondaryModel.model_id);
  });

  it("已有选择从新选项中消失时保留旧 ID 并标记不可用", () => {
    const missingModelId = "33333333-3333-4333-8333-333333333333";

    expect(reconcileModelSelection(missingModelId, [defaultModel])).toBe(missingModelId);
    expect(modelSelectionUnavailable(missingModelId, [defaultModel])).toBe(true);
  });

  it("空列表或空选择均不可调用", () => {
    expect(modelSelectionUnavailable("", [defaultModel])).toBe(true);
    expect(modelSelectionUnavailable(defaultModel.model_id, [])).toBe(true);
  });
});
