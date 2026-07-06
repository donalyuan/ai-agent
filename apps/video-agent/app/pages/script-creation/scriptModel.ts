import type { ScriptDetail, ScriptStatus, ScriptSummary } from "../../lib/api";

export const statusOptions: Array<{ value: "all" | ScriptStatus; label: string }> = [
  { value: "all", label: "全部" },
  { value: "draft", label: "草稿" },
  { value: "approved", label: "已通过" },
  { value: "archived", label: "已归档" },
];

export const statusLabels: Record<ScriptStatus, string> = {
  draft: "草稿",
  approved: "已通过",
  archived: "已归档",
};

export const statusClassNames: Record<ScriptStatus, string> = {
  draft: "statusDraft",
  approved: "statusApproved",
  archived: "statusArchived",
};

export function upsertSummary(scripts: ScriptSummary[], script: ScriptDetail): ScriptSummary[] {
  const summary: ScriptSummary = {
    script_id: script.script_id,
    topic_id: script.topic_id,
    source_topic_title: script.topic_snapshot?.title || null,
    title: script.title,
    status: script.status,
    scene_count: script.scenes.length,
    parent_id: script.parent_id,
    created_at: script.created_at,
  };
  const nextScripts = scripts.filter((item) => item.script_id !== script.script_id);
  return [summary, ...nextScripts];
}

export function formatDate(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "未知时间";
  }
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}
