import dynamic from "next/dynamic";
import { useEffect, useMemo, useRef, useState } from "react";
import { ImagePreviewDialog } from "../../components/ImagePreviewDialog";
import type { Material, MaterialStatus } from "../../lib/api";
import {
  audioUsageLabels,
  audioUsageOptions,
  formatMaterialDate,
  formatMaterialFileSummary,
  getMaterialPreview,
  isAudioUploadFile,
  materialGenerationRows,
  materialSourceLabels,
  materialSourceOptions,
  materialStatusFilterOptions,
  materialStatusLabels,
  materialTypeLabels,
  materialTypeOptions,
  type MaterialFiltersState,
  type MaterialFormState,
} from "./materialModel";
import type { MaterialCanvasStageProps } from "./MaterialCanvasStage";

const MaterialCanvasStage = dynamic<MaterialCanvasStageProps>(
  () => import("./MaterialCanvasStage").then((module) => module.MaterialCanvasStage),
  { ssr: false },
);

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
  const selectedMaterialId = selectedMaterial?.material_id || null;
  const detailOpen = creatingMaterial || selectedMaterial !== null;
  const detailPreview = selectedMaterial ? getMaterialPreview(selectedMaterial) : null;
  const imagePreviewAvailable =
    selectedMaterial?.material_type === "image" && Boolean(detailPreview?.imageUrl);
  const generationRows = selectedMaterial ? materialGenerationRows(selectedMaterial) : [];
  const workIds = useMemo(
    () => uniqueReferences([
      ...materials.map((material) => material.work_id),
      filters.work_id,
    ]),
    [filters.work_id, materials],
  );
  const workVersionIds = useMemo(
    () => uniqueReferences([
      ...materials
        .filter((material) => !filters.work_id || material.work_id === filters.work_id)
        .map((material) => material.work_version_id),
      filters.work_version_id,
    ]),
    [filters.work_id, filters.work_version_id, materials],
  );

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
  }, [selectedMaterialId]);

  const closePreview = () => {
    setPreviewOpen(false);
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
          <div aria-label="素材类型筛选" className="materialSegmented materialTypeSegmented">
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
          <div className="materialFilterGrid">
            <label className="compactField">
              声音用途
              <select
                aria-label="声音用途筛选"
                value={filters.audio_usage}
                onChange={(event) => onFilterChange({
                  ...filters,
                  audio_usage: event.target.value as MaterialFiltersState["audio_usage"],
                })}
              >
                {audioUsageOptions.map((option) => (
                  <option key={option.value} value={option.value}>{option.label}</option>
                ))}
              </select>
            </label>
            <label className="compactField">
              生成来源
              <select
                aria-label="生成来源筛选"
                value={filters.source}
                onChange={(event) => {
                  const source = event.target.value as MaterialFiltersState["source"];
                  onFilterChange({
                    ...filters,
                    source,
                    work_id: source === "work_generation" ? filters.work_id : "",
                    work_version_id: source === "work_generation" ? filters.work_version_id : "",
                  });
                }}
              >
                {materialSourceOptions.map((option) => (
                  <option key={option.value} value={option.value}>{option.label}</option>
                ))}
              </select>
            </label>
          </div>
          {filters.source === "work_generation" || filters.work_id || filters.work_version_id ? (
            <div className="materialFilterGrid materialWorkFilters">
              <label className="compactField">
                来源作品
                <select
                  aria-label="来源作品筛选"
                  title={filters.work_id || undefined}
                  value={filters.work_id}
                  onChange={(event) => onFilterChange({
                    ...filters,
                    work_id: event.target.value,
                    work_version_id: "",
                  })}
                >
                  <option value="">全部作品</option>
                  {workIds.map((workId) => (
                    <option key={workId} value={workId}>作品 {shortReference(workId)}</option>
                  ))}
                </select>
              </label>
              <label className="compactField">
                来源版本
                <select
                  aria-label="来源版本筛选"
                  title={filters.work_version_id || undefined}
                  value={filters.work_version_id}
                  onChange={(event) => onFilterChange({
                    ...filters,
                    work_version_id: event.target.value,
                  })}
                >
                  <option value="">全部版本</option>
                  {workVersionIds.map((versionId) => (
                    <option key={versionId} value={versionId}>版本 {shortReference(versionId)}</option>
                  ))}
                </select>
              </label>
            </div>
          ) : null}
          <div aria-label="素材状态筛选" className="materialSegmented materialStatusSegmented">
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
                    <small>{materialListSummary(material)}</small>
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
                    {materialDetailStatusSummary(selectedMaterial)}
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
              selectedMaterial.material_type === "audio" ? (
                <div className="materialDetailPreview materialAudioPreview">
                  <div>
                    <strong>{selectedMaterial.file_name}</strong>
                    <span>{formatMaterialFileSummary(selectedMaterial)}</span>
                  </div>
                  <audio controls preload="none" src={selectedMaterial.file_url}>
                    浏览器不支持音频播放。
                  </audio>
                </div>
              ) : imagePreviewAvailable ? (
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
              showAudioUsage={creatingMaterial && isAudioUploadFile(uploadFile)}
              onFormChange={onFormChange}
            />

            {selectedMaterial ? (
              <div className="materialSystemInfo">
                <strong>系统文件信息</strong>
                <span>{formatMaterialFileSummary(selectedMaterial)}</span>
                <small>上传后自动生成并保持只读</small>
              </div>
            ) : null}

            {selectedMaterial?.generation && generationRows.length > 0 ? (
              <section aria-label="生成来源详情" className="materialGenerationInfo">
                <header>
                  <h3>生成来源</h3>
                  <span>只读</span>
                </header>
                <dl>
                  {generationRows.map((row) => (
                    <div key={`${row.label}-${row.value}`}>
                      <dt>{row.label}</dt>
                      <dd className={row.mono ? "mono" : undefined} title={row.value}>{row.value}</dd>
                    </div>
                  ))}
                </dl>
                <p className="materialCredentialStatus">凭据未记录</p>
              </section>
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
        <ImagePreviewDialog
          alt={selectedMaterial.file_name}
          imageUrl={detailPreview.imageUrl}
          subtitle={formatMaterialFileSummary(selectedMaterial)}
          title={selectedMaterial.file_name}
          onClose={closePreview}
        />
      ) : null}
    </section>
  );
}

function MaterialForm({
  creatingMaterial,
  form,
  showAudioUsage,
  onFormChange,
}: {
  creatingMaterial: boolean;
  form: MaterialFormState;
  showAudioUsage: boolean;
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
      {showAudioUsage ? (
        <label>
          声音用途（选填）
          <select
            aria-label="声音用途（选填）"
            value={form.audio_usage}
            onChange={(event) => onFormChange({
              ...form,
              audio_usage: event.target.value as MaterialFormState["audio_usage"],
            })}
          >
            <option value="">未分类</option>
            {audioUsageOptions
              .filter((option) => option.value !== "all")
              .map((option) => (
                <option key={option.value} value={option.value}>{option.label}</option>
              ))}
          </select>
        </label>
      ) : null}
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

function materialListSummary(material: Material) {
  const parts = [materialTypeLabels[material.material_type]];
  if (material.audio_usage) {
    parts.push(audioUsageLabels[material.audio_usage]);
  }
  if (material.source) {
    parts.push(materialSourceLabels[material.source]);
  }
  parts.push(materialStatusLabels[material.status], formatMaterialDate(material.updated_at));
  return parts.join(" · ");
}

function materialDetailStatusSummary(material: Material) {
  const parts = [materialTypeLabels[material.material_type]];
  if (material.audio_usage) {
    parts.push(audioUsageLabels[material.audio_usage]);
  }
  if (material.source) {
    parts.push(materialSourceLabels[material.source]);
  }
  parts.push(materialStatusLabels[material.status]);
  return parts.join(" · ");
}

function uniqueReferences(values: Array<string | null>) {
  return [...new Set(values.filter((value): value is string => Boolean(value)))];
}

function shortReference(value: string) {
  return value.length > 12 ? value.slice(0, 8) : value;
}
