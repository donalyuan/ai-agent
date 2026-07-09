import dynamic from "next/dynamic";
import { useEffect, useRef, useState, type Ref } from "react";
import type { Material, MaterialStatus, MaterialStatusFilter, MaterialType } from "../../lib/api";
import {
  formatMaterialDate,
  getMaterialPreview,
  materialStatusFilterOptions,
  materialStatusLabels,
  materialTypeLabels,
  materialTypeOptions,
  type MaterialFormState,
} from "./materialModel";
import type { MaterialCanvasStageProps } from "./MaterialCanvasStage";

const MaterialCanvasStage = dynamic<MaterialCanvasStageProps>(
  () => import("./MaterialCanvasStage").then((module) => module.MaterialCanvasStage),
  { ssr: false },
);

type MaterialFiltersState = {
  material_type: MaterialType | "all";
  status: MaterialStatusFilter;
  q: string;
  tag: string;
};

type MaterialLibraryPageProps = {
  materials: Material[];
  selectedMaterial: Material | null;
  loading: boolean;
  error: string;
  actionError: string;
  saving: boolean;
  creatingMaterial: boolean;
  filters: MaterialFiltersState;
  form: MaterialFormState;
  onFilterChange: (filters: MaterialFiltersState) => void;
  onSelectMaterial: (materialId: string) => void;
  onNewMaterial: () => void;
  onFormChange: (form: MaterialFormState) => void;
  onSaveMaterial: () => void;
  onUpdateStatus: (status: MaterialStatus) => void;
};

export function MaterialLibraryPage({
  materials,
  selectedMaterial,
  loading,
  error,
  actionError,
  saving,
  creatingMaterial,
  filters,
  form,
  onFilterChange,
  onSelectMaterial,
  onNewMaterial,
  onFormChange,
  onSaveMaterial,
  onUpdateStatus,
}: MaterialLibraryPageProps) {
  const workspaceRef = useRef<HTMLDivElement | null>(null);
  const materialNameInputRef = useRef<HTMLInputElement | null>(null);
  const [canvasSize, setCanvasSize] = useState({ width: 1, height: 1 });
  const selectedMaterialId = selectedMaterial?.material_id || null;

  useEffect(() => {
    const workspaceElement = workspaceRef.current;
    if (!workspaceElement) {
      return;
    }

    const syncSize = () => {
      const rect = workspaceElement.getBoundingClientRect();
      setCanvasSize({
        width: Math.max(1, Math.round(rect.width)),
        height: Math.max(1, Math.round(rect.height)),
      });
    };

    syncSize();
    const resizeObserver =
      typeof ResizeObserver === "undefined" ? null : new ResizeObserver(syncSize);
    resizeObserver?.observe(workspaceElement);
    window.addEventListener("resize", syncSize);
    return () => {
      resizeObserver?.disconnect();
      window.removeEventListener("resize", syncSize);
    };
  }, []);

  useEffect(() => {
    if (creatingMaterial) {
      materialNameInputRef.current?.focus();
    }
  }, [creatingMaterial]);

  return (
    <section className="materialLibraryPage">
      <header className="materialLibraryHeader">
        <div>
          <p className="sectionKicker">素材管理</p>
          <h1>素材库</h1>
        </div>
        <button className="primaryButton" type="button" onClick={onNewMaterial}>
          新增素材
        </button>
      </header>

      <div ref={workspaceRef} className="materialCanvasWorkspace">
        <aside aria-label="素材资产浮层" className="materialAssetRail">
          <div className="materialFloatingHeader">
            <div>
              <h2>资产栏</h2>
            </div>
            <span>{materials.length} 条</span>
          </div>

          <label className="compactField">
            关键词
            <input
              value={filters.q}
              onChange={(event) => onFilterChange({ ...filters, q: event.target.value })}
              placeholder="名称或 URL"
            />
          </label>
          <label className="compactField">
            标签筛选
            <input
              value={filters.tag}
              onChange={(event) => onFilterChange({ ...filters, tag: event.target.value })}
              placeholder="输入单个标签"
            />
          </label>

          <div aria-label="素材类型筛选" className="materialSegmented">
            {materialTypeOptions.map((option) => (
              <button
                key={option.value}
                className={filters.material_type === option.value ? "selected" : ""}
                type="button"
                onClick={() => onFilterChange({ ...filters, material_type: option.value })}
              >
                {option.label}
              </button>
            ))}
          </div>
          <div aria-label="素材状态筛选" className="materialSegmented">
            {materialStatusFilterOptions.map((option) => (
              <button
                key={option.value}
                className={filters.status === option.value ? "selected" : ""}
                type="button"
                onClick={() => onFilterChange({ ...filters, status: option.value })}
              >
                {option.label}
              </button>
            ))}
          </div>

          {loading ? <p className="helperText">正在读取素材...</p> : null}
          {error ? <p className="formError">{error}</p> : null}
          {!loading && materials.length === 0 ? (
            <div className="materialRailEmpty">
              <strong>素材列表为空</strong>
              <span>先登记一个已有素材 URL，再在画布里查看节点。</span>
            </div>
          ) : null}
          <div className="materialAssetList">
            {materials.map((material) => {
              const preview = getMaterialPreview(material);
              return (
                <button
                  key={material.material_id}
                  className={`materialAssetItem ${
                    selectedMaterialId === material.material_id ? "selected" : ""
                  }`}
                  type="button"
                  onClick={() => onSelectMaterial(material.material_id)}
                >
                  <span className="materialAssetPreview">
                    {preview.imageUrl ? (
                      // eslint-disable-next-line @next/next/no-img-element
                      <img alt="" src={preview.imageUrl} />
                    ) : (
                      <span>{preview.label}</span>
                    )}
                  </span>
                  <span>
                    <strong>{material.file_name}</strong>
                    <small>
                      {materialTypeLabels[material.material_type]} ·{" "}
                      {materialStatusLabels[material.status]} · {formatMaterialDate(material.updated_at)}
                    </small>
                  </span>
                </button>
              );
            })}
          </div>
        </aside>

        <section aria-label="素材画布" className="materialCanvas">
          <MaterialCanvasStage
            height={canvasSize.height}
            materials={materials}
            selectedMaterialId={selectedMaterialId}
            width={canvasSize.width}
            onSelectMaterial={onSelectMaterial}
          />
          {!loading && materials.length === 0 ? (
            <div className="materialCanvasEmpty">
              <strong>{creatingMaterial ? "正在登记新素材" : "还没有素材"}</strong>
              <span>
                {creatingMaterial
                  ? "填写右侧表单后保存到当前账号素材库。"
                  : "画布会根据素材库列表自动派生节点。"}
              </span>
              <button className="primaryButton" type="button" onClick={onNewMaterial}>
                {creatingMaterial ? "重新开始" : "开始登记"}
              </button>
            </div>
          ) : null}
          <div aria-label="画布工具栏" className="materialCanvasToolbar">
            <button type="button">添加</button>
            <button type="button">缩小</button>
            <button type="button">放大</button>
            <button type="button">居中</button>
            <button type="button">网格</button>
          </div>
        </section>

        <aside
          aria-label="素材详情浮层"
          className={`materialDetailPanel ${creatingMaterial ? "creating" : ""}`}
        >
          <div className="materialFloatingHeader">
            <div>
              <h2>{selectedMaterial ? selectedMaterial.file_name : creatingMaterial ? "新增素材" : "素材详情"}</h2>
            </div>
            {selectedMaterial ? (
              <span>{materialStatusLabels[selectedMaterial.status]}</span>
            ) : creatingMaterial ? (
              <span>新建</span>
            ) : null}
          </div>
          {actionError ? <p className="formError">{actionError}</p> : null}
          {creatingMaterial ? (
            <p className="materialDraftHint">正在登记新素材，保存后会生成画布节点。</p>
          ) : null}
          <MaterialForm form={form} nameInputRef={materialNameInputRef} onFormChange={onFormChange} />
          <div className="materialDetailActions">
            <button className="primaryButton" disabled={saving} type="button" onClick={onSaveMaterial}>
              {saving ? "保存中..." : "保存素材"}
            </button>
            {selectedMaterial ? (
              selectedMaterial.status === "active" ? (
                <button
                  className="secondaryButton danger"
                  disabled={saving}
                  type="button"
                  onClick={() => onUpdateStatus("archived")}
                >
                  归档素材
                </button>
              ) : (
                <button
                  className="secondaryButton"
                  disabled={saving}
                  type="button"
                  onClick={() => onUpdateStatus("active")}
                >
                  恢复素材
                </button>
              )
            ) : null}
          </div>
        </aside>
      </div>
    </section>
  );
}

function MaterialForm({
  form,
  nameInputRef,
  onFormChange,
}: {
  form: MaterialFormState;
  nameInputRef: Ref<HTMLInputElement>;
  onFormChange: (form: MaterialFormState) => void;
}) {
  const updateField = (field: keyof MaterialFormState, value: string) => {
    onFormChange({ ...form, [field]: value });
  };

  return (
    <div className="materialForm">
      <label>
        素材名称
        <input
          ref={nameInputRef}
          value={form.file_name}
          onChange={(event) => updateField("file_name", event.target.value)}
        />
      </label>
      <div aria-label="素材类型选择" className="materialTypePicker">
        {(["video", "image", "audio", "subtitle"] as MaterialType[]).map((type) => (
          <button
            key={type}
            aria-label={`类型：${materialTypeLabels[type]}`}
            className={form.material_type === type ? "selected" : ""}
            type="button"
            onClick={() => updateField("material_type", type)}
          >
            {materialTypeLabels[type]}
          </button>
        ))}
      </div>
      <label>
        素材 URL
        <input value={form.file_url} onChange={(event) => updateField("file_url", event.target.value)} />
      </label>
      <label>
        缩略图 URL
        <input
          value={form.thumbnail_url}
          onChange={(event) => updateField("thumbnail_url", event.target.value)}
        />
      </label>
      <label>
        标签
        <input value={form.tags_text} onChange={(event) => updateField("tags_text", event.target.value)} />
      </label>
      <label>
        来源备注
        <textarea
          rows={2}
          value={form.source_note}
          onChange={(event) => updateField("source_note", event.target.value)}
        />
      </label>
      <label>
        授权备注
        <textarea
          rows={2}
          value={form.license_note}
          onChange={(event) => updateField("license_note", event.target.value)}
        />
      </label>
      <div className="materialFormGrid">
        <label>
          时长
          <input value={form.duration_sec} onChange={(event) => updateField("duration_sec", event.target.value)} />
        </label>
        <label>
          格式
          <input value={form.format} onChange={(event) => updateField("format", event.target.value)} />
        </label>
        <label>
          宽度
          <input value={form.width} onChange={(event) => updateField("width", event.target.value)} />
        </label>
        <label>
          高度
          <input value={form.height} onChange={(event) => updateField("height", event.target.value)} />
        </label>
        <label>
          字幕语言
          <input value={form.language} onChange={(event) => updateField("language", event.target.value)} />
        </label>
        <label>
          字幕格式
          <input
            value={form.subtitle_format}
            onChange={(event) => updateField("subtitle_format", event.target.value)}
          />
        </label>
      </div>
    </div>
  );
}
