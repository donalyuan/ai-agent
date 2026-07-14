import dynamic from "next/dynamic";
import { useEffect, useRef, useState } from "react";
import type { Material, MaterialStatus, MaterialStatusFilter, MaterialType } from "../../lib/api";
import {
  formatMaterialDate,
  formatMaterialFileSummary,
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
  uploadFile: File | null;
  filters: MaterialFiltersState;
  form: MaterialFormState;
  onFilterChange: (filters: MaterialFiltersState) => void;
  onCloseDetail: () => void;
  onSelectMaterial: (materialId: string) => void;
  onNewMaterial: () => void;
  onFormChange: (form: MaterialFormState) => void;
  onUploadFileChange: (file: File | null) => void;
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
  uploadFile,
  filters,
  form,
  onFilterChange,
  onCloseDetail,
  onSelectMaterial,
  onNewMaterial,
  onFormChange,
  onUploadFileChange,
  onSaveMaterial,
  onUpdateStatus,
}: MaterialLibraryPageProps) {
  const workspaceRef = useRef<HTMLDivElement | null>(null);
  const previewTriggerRef = useRef<HTMLButtonElement | null>(null);
  const [canvasSize, setCanvasSize] = useState({ width: 1, height: 1 });
  const [previewOpen, setPreviewOpen] = useState(false);
  const [previewZoom, setPreviewZoom] = useState(100);
  const selectedMaterialId = selectedMaterial?.material_id || null;
  const detailOpen = creatingMaterial || selectedMaterial !== null;
  const detailPreview = selectedMaterial ? getMaterialPreview(selectedMaterial) : null;
  const imagePreviewAvailable =
    selectedMaterial?.material_type === "image" && Boolean(detailPreview?.imageUrl);

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
    setPreviewOpen(false);
    setPreviewZoom(100);
  }, [selectedMaterialId]);

  useEffect(() => {
    if (!previewOpen) {
      return;
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closePreview();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [previewOpen]);

  const closePreview = () => {
    setPreviewOpen(false);
    setPreviewZoom(100);
    window.requestAnimationFrame(() => previewTriggerRef.current?.focus());
  };

  return (
    <section className="materialLibraryPage">
      <header className="materialLibraryHeader">
        <div>
          <p className="sectionKicker">素材管理</p>
          <h1>素材库</h1>
        </div>
        <button className="primaryButton" type="button" onClick={onNewMaterial}>
          上传素材
        </button>
      </header>

      <div
        ref={workspaceRef}
        className={`materialCanvasWorkspace ${detailOpen ? "detailOpen" : ""}`}
      >
        <aside aria-label="素材资产浮层" className="materialAssetRail">
          <div className="materialFloatingHeader">
            <h2>资产栏</h2>
            <span>{materials.length} 条</span>
          </div>
          <label className="compactField">
            关键词
            <input
              value={filters.q}
              onChange={(event) => onFilterChange({ ...filters, q: event.target.value })}
              placeholder="素材名称"
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
              <span>上传图片、视频、音频或字幕后会自动生成画布节点。</span>
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
                      {materialTypeLabels[material.material_type]} · {materialStatusLabels[material.status]} ·{" "}
                      {formatMaterialDate(material.updated_at)}
                    </small>
                  </span>
                </button>
              );
            })}
          </div>
        </aside>

        <section aria-label="素材画布" className="materialCanvas">
          <MaterialCanvasStage
            detailOpen={detailOpen}
            height={canvasSize.height}
            materials={materials}
            selectedMaterialId={selectedMaterialId}
            width={canvasSize.width}
            onSelectMaterial={onSelectMaterial}
          />
          {!loading && materials.length === 0 ? (
            <div className="materialCanvasEmpty">
              <strong>{creatingMaterial ? "正在上传素材" : "还没有素材"}</strong>
              <span>
                {creatingMaterial
                  ? "选择文件后确认名称和标签即可保存。"
                  : "画布会根据素材库列表自动派生节点。"}
              </span>
              <button className="primaryButton" type="button" onClick={onNewMaterial}>
                {creatingMaterial ? "重新选择" : "选择文件"}
              </button>
            </div>
          ) : null}
          <div aria-label="画布工具栏" className="materialCanvasToolbar">
            <button type="button" onClick={onNewMaterial}>添加</button>
            <button type="button">缩小</button>
            <button type="button">放大</button>
            <button type="button">居中</button>
            <button type="button">网格</button>
          </div>
        </section>

        {detailOpen ? (
          <aside
            aria-label="素材详情浮层"
            className={`materialDetailPanel ${creatingMaterial ? "creating" : ""}`}
          >
            <div className="materialFloatingHeader materialDetailHeader">
              <div>
                <h2>{creatingMaterial ? "上传素材" : "素材详情"}</h2>
                {selectedMaterial ? (
                  <span className="materialDetailStatus">
                    {materialTypeLabels[selectedMaterial.material_type]} · {materialStatusLabels[selectedMaterial.status]}
                  </span>
                ) : null}
              </div>
              <button
                aria-label="关闭素材详情"
                className="materialDetailClose"
                type="button"
                onClick={onCloseDetail}
              >
                ×
              </button>
            </div>

            {selectedMaterial && detailPreview ? (
              imagePreviewAvailable ? (
                <button
                  ref={previewTriggerRef}
                  aria-label={`查看${selectedMaterial.file_name}大图`}
                  className="materialDetailPreview materialDetailPreviewButton"
                  type="button"
                  onClick={() => setPreviewOpen(true)}
                >
                  {/* eslint-disable-next-line @next/next/no-img-element */}
                  <img alt="" src={detailPreview.imageUrl || ""} />
                </button>
              ) : (
                <div className="materialDetailPreview">
                  {detailPreview.imageUrl ? (
                    // eslint-disable-next-line @next/next/no-img-element
                    <img alt="" src={detailPreview.imageUrl} />
                  ) : (
                    <strong>{detailPreview.label}</strong>
                  )}
                </div>
              )
            ) : null}

            {actionError ? <p className="formError">{actionError}</p> : null}
            {creatingMaterial ? (
              <label className="materialUploadField">
                素材文件
                <input
                  accept=".jpg,.jpeg,.png,.webp,.gif,.mp4,.mov,.webm,.mp3,.wav,.m4a,.ogg,.srt,.vtt,.ass,.ssa"
                  type="file"
                  onChange={(event) => onUploadFileChange(event.target.files?.[0] || null)}
                />
                {uploadFile ? (
                  <span className="materialSelectedFile">
                    <strong>{uploadFile.name}</strong>
                    <small>{formatSelectedFile(uploadFile)}</small>
                  </span>
                ) : null}
              </label>
            ) : null}

            <MaterialForm
              creatingMaterial={creatingMaterial}
              form={form}
              onFormChange={onFormChange}
            />

            {selectedMaterial ? (
              <div className="materialSystemInfo">
                <strong>系统文件信息</strong>
                <span>{formatMaterialFileSummary(selectedMaterial)}</span>
                <small>上传后自动生成并保持只读</small>
              </div>
            ) : null}

            <div className="materialDetailActions">
              <button className="primaryButton" disabled={saving} type="button" onClick={onSaveMaterial}>
                {saving ? "保存中..." : creatingMaterial ? "上传并保存" : "保存修改"}
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
        ) : null}
      </div>

      {previewOpen && selectedMaterial && detailPreview?.imageUrl ? (
        <div
          className="materialImageLightbox"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) {
              closePreview();
            }
          }}
        >
          <div aria-label="图片大图预览" aria-modal="true" className="materialImageDialog" role="dialog">
            <header>
              <div>
                <strong>{selectedMaterial.file_name}</strong>
                <span>{formatMaterialFileSummary(selectedMaterial)}</span>
              </div>
              <button aria-label="关闭大图预览" type="button" onClick={closePreview}>×</button>
            </header>
            <div className="materialImageViewport">
              {/* eslint-disable-next-line @next/next/no-img-element */}
              <img
                alt={selectedMaterial.file_name}
                src={detailPreview.imageUrl}
                style={{ transform: `scale(${previewZoom / 100})` }}
              />
            </div>
            <div aria-label="大图缩放" className="materialImageZoomControls">
              <button
                aria-label="缩小图片"
                disabled={previewZoom <= 50}
                type="button"
                onClick={() => setPreviewZoom((current) => Math.max(50, current - 25))}
              >
                −
              </button>
              <strong>{previewZoom}%</strong>
              <button
                aria-label="放大图片"
                disabled={previewZoom >= 200}
                type="button"
                onClick={() => setPreviewZoom((current) => Math.min(200, current + 25))}
              >
                +
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </section>
  );
}

function MaterialForm({
  creatingMaterial,
  form,
  onFormChange,
}: {
  creatingMaterial: boolean;
  form: MaterialFormState;
  onFormChange: (form: MaterialFormState) => void;
}) {
  return (
    <div className="materialForm">
      <label>
        素材名称
        <input
          aria-label="素材名称"
          value={form.file_name}
          onChange={(event) => onFormChange({ ...form, file_name: event.target.value })}
        />
      </label>
      <label>
        {creatingMaterial ? "标签（选填）" : "标签"}
        <input
          aria-label={creatingMaterial ? "标签（选填）" : "标签"}
          value={form.tags_text}
          onChange={(event) => onFormChange({ ...form, tags_text: event.target.value })}
        />
      </label>
    </div>
  );
}

function formatSelectedFile(file: File) {
  const type = file.type || "待识别";
  const size = file.size < 1024 * 1024
    ? `${(file.size / 1024).toFixed(1)} KB`
    : `${(file.size / (1024 * 1024)).toFixed(1)} MB`;
  return `${type} · ${size}`;
}
