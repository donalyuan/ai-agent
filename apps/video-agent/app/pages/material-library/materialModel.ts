import type {
  Material,
  MaterialPayload,
  MaterialStatus,
  MaterialStatusFilter,
  MaterialType,
} from "../../lib/api";

export type MaterialFormState = {
  file_name: string;
  material_type: MaterialType;
  file_url: string;
  thumbnail_url: string;
  tags_text: string;
  source_note: string;
  license_note: string;
  duration_sec: string;
  format: string;
  width: string;
  height: string;
  language: string;
  subtitle_format: string;
};

export const defaultMaterialForm: MaterialFormState = {
  file_name: "",
  material_type: "video",
  file_url: "",
  thumbnail_url: "",
  tags_text: "",
  source_note: "",
  license_note: "",
  duration_sec: "",
  format: "",
  width: "",
  height: "",
  language: "",
  subtitle_format: "",
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
  const metadata = material.metadata;
  return {
    file_name: material.file_name,
    material_type: material.material_type,
    file_url: material.file_url,
    thumbnail_url: material.thumbnail_url || metadataString(metadata.thumbnail_url),
    tags_text: material.tags.join(", "),
    source_note: metadataString(metadata.source_note),
    license_note: metadataString(metadata.license_note),
    duration_sec: metadataString(metadata.duration_sec),
    format: metadataString(metadata.format),
    width: metadataString(metadata.width),
    height: metadataString(metadata.height),
    language: metadataString(metadata.language),
    subtitle_format: metadataString(metadata.subtitle_format),
  };
}

export function materialPayloadFromForm(form: MaterialFormState): MaterialPayload {
  const metadata: Record<string, unknown> = {};
  setMetadataValue(metadata, "source_note", form.source_note);
  setMetadataValue(metadata, "license_note", form.license_note);
  setMetadataNumber(metadata, "duration_sec", form.duration_sec);
  setMetadataValue(metadata, "format", form.format);
  setMetadataNumber(metadata, "width", form.width);
  setMetadataNumber(metadata, "height", form.height);
  setMetadataValue(metadata, "language", form.language);
  setMetadataValue(metadata, "subtitle_format", form.subtitle_format);

  return {
    material_type: form.material_type,
    file_url: form.file_url.trim(),
    thumbnail_url: form.thumbnail_url.trim() || null,
    file_name: form.file_name.trim(),
    tags: parseTags(form.tags_text),
    metadata,
  };
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

function setMetadataValue(metadata: Record<string, unknown>, key: string, value: string) {
  const normalized = value.trim();
  if (normalized) {
    metadata[key] = normalized;
  }
}

function setMetadataNumber(metadata: Record<string, unknown>, key: string, value: string) {
  const normalized = value.trim();
  if (!normalized) {
    return;
  }
  const numberValue = Number(normalized);
  metadata[key] = Number.isFinite(numberValue) ? numberValue : normalized;
}

function parseTags(value: string) {
  const tags: string[] = [];
  for (const tag of value.split(/[,，\n]/)) {
    const normalized = tag.trim();
    if (normalized && !tags.includes(normalized)) {
      tags.push(normalized);
    }
  }
  return tags;
}
