import type {
  Material,
  MaterialPayload,
  MaterialStatus,
  MaterialStatusFilter,
  MaterialType,
} from "../../lib/api";

export type MaterialFormState = {
  file_name: string;
  tags_text: string;
};

export const defaultMaterialForm: MaterialFormState = {
  file_name: "",
  tags_text: "",
};

export const materialTypeLabels: Record<MaterialType, string> = {
  video: "视频",
  image: "图片",
  audio: "音频",
  subtitle: "字幕",
};

export const materialStatusLabels: Record<MaterialStatus, string> = {
  active: "可用",
  archived: "已归档",
};

export const materialTypeOptions: Array<{ value: MaterialType | "all"; label: string }> = [
  { value: "all", label: "全部类型" },
  { value: "video", label: "视频" },
  { value: "image", label: "图片" },
  { value: "audio", label: "音频" },
  { value: "subtitle", label: "字幕" },
];

export const materialStatusFilterOptions: Array<{ value: MaterialStatusFilter; label: string }> = [
  { value: "active", label: "可用" },
  { value: "archived", label: "已归档" },
  { value: "all", label: "全部" },
];

export function materialToForm(material: Material): MaterialFormState {
  return {
    file_name: material.file_name,
    tags_text: material.tags.join(", "),
  };
}

export function materialEditPayload(
  material: Material,
  form: MaterialFormState,
): MaterialPayload {
  return {
    material_type: material.material_type,
    file_url: material.file_url,
    thumbnail_url: material.thumbnail_url,
    file_name: form.file_name.trim(),
    tags: parseMaterialTags(form.tags_text),
    metadata: material.metadata,
  };
}

export function parseMaterialTags(value: string) {
  const tags: string[] = [];
  for (const tag of value.split(/[,，\n]/)) {
    const normalized = tag.trim();
    if (normalized && !tags.includes(normalized)) {
      tags.push(normalized);
    }
  }
  return tags;
}

export function getMaterialPreview(material: Material) {
  if (material.thumbnail_url) {
    return { imageUrl: material.thumbnail_url, label: materialTypeLabels[material.material_type] };
  }
  if (material.material_type === "image") {
    return { imageUrl: material.file_url, label: materialTypeLabels.image };
  }
  return { imageUrl: null, label: materialTypeLabels[material.material_type] };
}

export function formatMaterialFileSummary(material: Material) {
  const parts = [materialTypeLabels[material.material_type]];
  const format = metadataString(material.metadata.format) || extensionFromName(material.file_name);
  if (format) {
    parts.push(format.toUpperCase());
  }
  const width = metadataNumber(material.metadata.width);
  const height = metadataNumber(material.metadata.height);
  if (width !== null && height !== null) {
    parts.push(`${width} × ${height}`);
  }
  const duration = metadataNumber(material.metadata.duration_sec);
  if (duration !== null) {
    parts.push(`${duration.toFixed(1)} 秒`);
  }
  const fileSize = metadataNumber(material.metadata.file_size_bytes);
  if (fileSize !== null) {
    parts.push(formatFileSize(fileSize));
  }
  return parts.join(" · ");
}

export function formatMaterialDate(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

function metadataString(value: unknown) {
  if (typeof value === "number") {
    return String(value);
  }
  return typeof value === "string" ? value : "";
}

function metadataNumber(value: unknown) {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === "string" && value.trim()) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

function extensionFromName(value: string) {
  const match = value.match(/\.([^.]+)$/);
  return match?.[1] || "";
}

function formatFileSize(bytes: number) {
  if (bytes < 1024) {
    return `${Math.round(bytes)} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
