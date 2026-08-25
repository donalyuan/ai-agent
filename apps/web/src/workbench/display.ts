const statusLabels: Record<string, string> = {
  queued: "排队中",
  running: "处理中",
  waiting_review: "等待审核",
  succeeded: "已完成",
  failed: "失败",
  cancel_requested: "正在取消",
  cancelled: "已取消",
  pending_review: "待审核",
  accepted: "已接受",
  rejected: "已拒绝",
  building: "构建中",
  ready: "就绪",
  pending: "等待处理",
  stale: "需要刷新",
  draft: "草稿",
  published: "已发布",
  active: "启用",
  idle: "尚未开始",
};

const operationLabels: Record<string, string> = {
  "text.generate": "文本生成",
  "image.generate": "画面生成",
  "video.submit": "视频提交",
  "text.review": "文本审核",
};

const materialTypeLabels: Record<string, string> = {
  novel: "小说原文",
  synopsis: "故事梗概",
  existing_script: "已有剧本",
};

const inputModeLabels: Record<string, string> = {
  inline_text: "粘贴文本",
  uploaded_file: "已验证文件",
};

export function statusLabel(value: string | null | undefined): string {
  if (!value) return "未设置";
  return statusLabels[value] ?? "未识别状态";
}

export function operationLabel(value: string): string {
  return operationLabels[value] ?? "工作节点";
}

export function materialTypeLabel(value: string): string {
  return materialTypeLabels[value] ?? "来源材料";
}

export function inputModeLabel(value: string): string {
  return inputModeLabels[value] ?? "输入方式";
}

export function formatRevision(value: number | null | undefined): string {
  return value == null ? "版本未知" : `第 ${value} 版`;
}
