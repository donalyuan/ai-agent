import type {
  ContentTopic,
  ContentTopicPayload,
  ContentTopicSource,
  ContentTopicStats,
  ContentTopicStatus,
  TopicGenerationBatchSummary,
  ScriptStyle,
} from "../../lib/api";

export type ContentStrategyView = "history" | "pool";

export const topicStatusOptions: Array<{ value: "all" | ContentTopicStatus; label: string }> = [
  { value: "all", label: "全部" },
  { value: "idea", label: "待评估" },
  { value: "approved", label: "已确认" },
  { value: "scripted", label: "已成稿" },
  { value: "archived", label: "已归档" },
];

export const topicPoolStatusFilters = topicStatusOptions.filter((option) =>
  ["all", "idea", "approved", "scripted", "archived"].includes(option.value),
);

export const topicStatusLabels: Record<ContentTopicStatus, string> = {
  idea: "待确认",
  approved: "已确认",
  scripted: "已成稿",
  archived: "已归档",
};

export const topicSourceLabels: Record<ContentTopicSource, string> = {
  manual: "人工",
  agent: "Agent",
};

const topicContentTypeLabels: Record<string, string> = {
  knowledge: "知识科普",
  story: "故事叙述",
  tutorial: "教程讲解",
};

export const topicStatusClassNames: Record<ContentTopicStatus, string> = {
  idea: "statusIdea",
  approved: "statusApproved",
  scripted: "statusScripted",
  archived: "statusArchived",
};

export const scriptStyleLabels: Record<ScriptStyle, string> = {
  knowledge: "知识科普",
  story: "故事叙述",
  tutorial: "教程讲解",
};

export const topicBatchStatusLabels = {
  running: "生成中",
  succeeded: "已生成",
  failed: "生成失败",
} satisfies Record<TopicGenerationBatchSummary["status"], string>;

export function getTopicContentTypeLabel(contentType: string) {
  return topicContentTypeLabels[contentType] ?? contentType;
}

export type TopicFormState = {
  title: string;
  angle: string;
  target_audience: string;
  hook_points: string;
  content_type: string;
  score: string;
  score_reason: string;
  tags: string;
};

export const emptyTopicStats: ContentTopicStats = {
  total: 0,
  idea: 0,
  approved: 0,
  scripted: 0,
  archived: 0,
};

export const defaultTopicForm: TopicFormState = {
  title: "",
  angle: "",
  target_audience: "",
  hook_points: "",
  content_type: "knowledge",
  score: "",
  score_reason: "",
  tags: "",
};

export function topicToForm(topic: ContentTopic): TopicFormState {
  return {
    title: topic.title,
    angle: topic.angle,
    target_audience: topic.target_audience,
    hook_points: topic.hook_points.join("\n"),
    content_type: topic.content_type,
    score: topic.score === null ? "" : String(topic.score),
    score_reason: topic.score_reason,
    tags: topic.tags.join(","),
  };
}

export function topicPayloadFromForm(form: TopicFormState): ContentTopicPayload {
  return {
    title: form.title.trim(),
    angle: form.angle.trim(),
    target_audience: form.target_audience.trim(),
    hook_points: splitLines(form.hook_points),
    content_type: form.content_type.trim(),
    score: parseOptionalScore(form.score),
    score_reason: form.score_reason.trim(),
    tags: splitTags(form.tags),
  };
}

export function adjustTopicStats(
  stats: ContentTopicStats,
  from: ContentTopicStatus,
  to: ContentTopicStatus,
): ContentTopicStats {
  if (from === to) {
    return stats;
  }
  return {
    ...stats,
    [from]: Math.max(0, stats[from] - 1),
    [to]: stats[to] + 1,
  };
}

export function sortContentTopicsByScore(topics: ContentTopic[]) {
  return [...topics].sort((left, right) => {
    if (left.score !== null && right.score !== null && left.score !== right.score) {
      return right.score - left.score;
    }
    if (left.score !== null && right.score === null) {
      return -1;
    }
    if (left.score === null && right.score !== null) {
      return 1;
    }
    const createdAtDelta = Date.parse(right.created_at) - Date.parse(left.created_at);
    if (createdAtDelta !== 0) {
      return createdAtDelta;
    }
    return right.topic_id.localeCompare(left.topic_id);
  });
}

export function formatTopicBatchTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  const hour = String(date.getHours()).padStart(2, "0");
  const minute = String(date.getMinutes()).padStart(2, "0");
  return `${month}-${day} ${hour}:${minute}`;
}

function splitLines(value: string) {
  return value
    .split(/\r?\n/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function splitTags(value: string) {
  return value
    .split(/[,，\n]/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function parseOptionalScore(value: string) {
  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }
  const score = Number(trimmed);
  return Number.isFinite(score) ? score : null;
}
