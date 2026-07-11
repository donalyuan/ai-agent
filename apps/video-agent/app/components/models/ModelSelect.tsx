import type { ModelOption } from "../../lib/api";
import { modelSelectionUnavailable } from "./modelSelection";

type ModelSelectProps = {
  id: string;
  label: string;
  loading: boolean;
  models: ModelOption[];
  value: string;
  disabled?: boolean;
  error?: string;
  onChange: (modelId: string) => void;
};

export function ModelSelect({
  id,
  label,
  loading,
  models,
  value,
  disabled = false,
  error = "",
  onChange,
}: ModelSelectProps) {
  const unavailable = !loading && modelSelectionUnavailable(value, models);
  const selectedModelMissing = Boolean(value) && !models.some((model) => model.model_id === value);
  const statusMessage = error
    ? error
    : loading
      ? "正在读取可用模型"
      : selectedModelMissing
        ? "原选择已停用或删除，请刷新后重新选择"
        : !models.length
          ? "暂无可用模型，请先在后台启用"
          : "";

  return (
    <div className="modelSelectField">
      <label htmlFor={id}>{label}</label>
      <select
        aria-describedby={statusMessage ? `${id}-status` : undefined}
        disabled={disabled || loading || !models.length}
        id={id}
        onChange={(event) => onChange(event.target.value)}
        value={selectedModelMissing ? "" : value}
      >
        {loading ? <option value="">读取中</option> : null}
        {!loading && !models.length ? <option value="">暂无可用模型</option> : null}
        {selectedModelMissing ? <option value="">原选择不可用</option> : null}
        {models.map((model) => (
          <option key={model.model_id} value={model.model_id}>
            {model.display_name} · {model.provider_name} · {model.upstream_model}
            {model.is_default ? "（默认）" : ""}
          </option>
        ))}
      </select>
      {statusMessage ? (
        <span className={unavailable || error ? "modelSelectStatus error" : "modelSelectStatus"} id={`${id}-status`}>
          {statusMessage}
        </span>
      ) : null}
    </div>
  );
}
