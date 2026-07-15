import type {
  AudioUsage,
  Material,
  MaterialPayload,
  MaterialSource,
  MaterialStatus,
  MaterialStatusFilter,
  MaterialType,
} from "../../lib/api";

export type MaterialFormState = {
  file_name: string;
  tags_text: string;
  audio_usage: AudioUsage | "";
};

export const defaultMaterialForm: MaterialFormState = {
  file_name: "",
  tags_text: "",
  audio_usage: "",
};

export type MaterialFiltersState = {
  material_type: MaterialType | "all";
  status: MaterialStatusFilter;
  q: string;
  tag: string;
  audio_usage: AudioUsage | "all";
  source: MaterialSource | "all";
  work_id: string;
  work_version_id: string;
};

export const defaultMaterialFilters: MaterialFiltersState = {
  material_type: "all",
  status: "active",
  q: "",
  tag: "",
  audio_usage: "all",
  source: "all",
  work_id: "",
  work_version_id: "",
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

export const audioUsageLabels: Record<AudioUsage, string> = {
  tts: "TTS 配音",
  bgm: "背景音乐",
  ambient: "环境音",
  action_sfx: "动作音效",
  mixed: "混合音频",
  other: "其他",
};

export const materialSourceLabels: Record<MaterialSource, string> = {
  user_upload: "用户上传",
  ai_generated: "AI 生成",
  work_generation: "作品生成",
};

export const audioUsageOptions: Array<{ value: AudioUsage | "all"; label: string }> = [
  { value: "all", label: "全部用途" },
  ...Object.entries(audioUsageLabels).map(([value, label]) => ({
    value: value as AudioUsage,
    label,
  })),
];

export const materialSourceOptions: Array<{ value: MaterialSource | "all"; label: string }> = [
  { value: "all", label: "全部来源" },
  ...Object.entries(materialSourceLabels).map(([value, label]) => ({
    value: value as MaterialSource,
    label,
  })),
];

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
    audio_usage: material.audio_usage || "",
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

export type MaterialGenerationRow = {
  label: string;
  value: string;
  mono?: boolean;
};

export function materialGenerationRows(material: Material): MaterialGenerationRow[] {
  const generation = material.generation;
  if (!generation) {
    return [];
  }

  const rows: MaterialGenerationRow[] = [];
  const workId = textValue(generation.work_id) || material.work_id || "";
  const workVersionId = textValue(generation.work_version_id) || material.work_version_id || "";
  if (workId || workVersionId) {
    rows.push({
      label: "作品 / 版本",
      value: `${workId || "未记录"} / ${workVersionId || "未记录"}`,
      mono: true,
    });
  }

  const runId = textValue(generation.generation_run_id);
  const stepId = textValue(generation.generation_step_id);
  if (runId || stepId) {
    rows.push({
      label: "run / step",
      value: `${runId || "未记录"} / ${stepId || "未记录"}`,
      mono: true,
    });
  }

  const model = firstRecordText(generation.model_snapshot, [
    "display_name",
    "name",
    "model_name",
    "upstream_model",
    "model",
  ]);
  if (model) {
    rows.push({ label: "模型", value: model });
  }

  const voice = firstRecordText(generation.voice_snapshot, [
    "speaker_name",
    "voice_name",
    "display_name",
    "speaker_id",
    "voice_id",
  ]);
  if (voice) {
    rows.push({ label: "音色", value: voice });
  }

  const voiceParameters = formatVoiceParameters(generation.voice_snapshot);
  if (voiceParameters) {
    rows.push({ label: "声音参数", value: voiceParameters });
  }

  const textSummary = firstRecordText(generation.prompt_snapshot, [
    "text_summary",
    "content_summary",
    "summary",
  ]);
  if (textSummary) {
    rows.push({ label: "文本摘要", value: textSummary });
  }

  const language =
    firstRecordText(generation.voice_snapshot, ["language", "locale"]) ||
    firstRecordText(generation.timeline_snapshot, ["language", "locale"]) ||
    textValue(material.metadata.language);
  const duration = numberValue(generation.duration_sec) ?? numberValue(material.metadata.duration_sec);
  if (language || duration !== null) {
    rows.push({
      label: "语言 / 时长",
      value: [language, duration === null ? "" : `${duration.toFixed(1)} 秒`]
        .filter(Boolean)
        .join(" · "),
    });
  }

  const alignmentSource = textValue(generation.alignment_source);
  if (alignmentSource) {
    rows.push({
      label: "对齐来源",
      value: alignmentSource === "tts_timestamp" ? "TTS 时间戳" : alignmentSource === "asr" ? "ASR" : alignmentSource,
    });
  }

  const sourceAudioMaterialId = textValue(generation.source_audio_material_id);
  if (sourceAudioMaterialId) {
    rows.push({ label: "来源音频", value: sourceAudioMaterialId, mono: true });
  }

  const subtitleFormat = textValue(generation.subtitle_format) || textValue(material.metadata.subtitle_format);
  const timelineVersion = firstRecordText(generation.timeline_snapshot, ["version", "timeline_version"]);
  if (subtitleFormat || timelineVersion) {
    rows.push({
      label: "字幕 / 时间轴",
      value: [subtitleFormat.toUpperCase(), timelineVersion].filter(Boolean).join(" · "),
    });
  }

  const requestTraceId = textValue(generation.request_trace_id);
  if (requestTraceId) {
    rows.push({ label: "request trace", value: requestTraceId, mono: true });
  }

  return rows;
}

export function isAudioUploadFile(file: File | null) {
  if (!file) {
    return false;
  }
  return file.type.toLowerCase().startsWith("audio/") || /\.(mp3|wav|m4a|ogg)$/i.test(file.name);
}

function formatVoiceParameters(snapshot: Record<string, unknown> | undefined) {
  if (!snapshot) {
    return "";
  }
  const values = [
    labeledValue("情绪", snapshot.emotion),
    labeledValue("语速", snapshot.speed),
    labeledValue("音调", snapshot.pitch),
    labeledValue("音量", snapshot.volume),
  ].filter(Boolean);
  return values.join(" · ");
}

function labeledValue(label: string, value: unknown) {
  const normalized = textValue(value);
  return normalized ? `${label} ${normalized}` : "";
}

function firstRecordText(record: Record<string, unknown> | undefined, keys: string[]) {
  if (!record) {
    return "";
  }
  for (const key of keys) {
    const value = textValue(record[key]);
    if (value) {
      return value;
    }
  }
  return "";
}

function textValue(value: unknown) {
  if (typeof value === "string") {
    return value.trim();
  }
  if (typeof value === "number" && Number.isFinite(value)) {
    return String(value);
  }
  return "";
}

function numberValue(value: unknown) {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === "string" && value.trim()) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
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
