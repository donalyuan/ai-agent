## Why

素材生成页当前只能在候选卡片内查看裁剪缩略图，操作者无法在选择主素材前检查 AI 生成图片的完整细节。项目已有素材库大图预览交互，应在素材生成页复用同一行为，避免因缩略图信息不足而误选候选。

## What Changes

- 允许操作者点击具有有效图片预览 URL 的 AI 图片候选缩略图打开大图预览。
- 大图预览支持关闭按钮、Escape、点击遮罩关闭、50%-200% 缩放和关闭后的焦点恢复。
- 失败、等待生成或没有图片预览 URL 的候选保持不可预览。
- “选择为主素材”和“排除候选”继续作为独立操作，不因预览行为改变。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `script-to-asset-generation`: 增加素材生成页 AI 图片候选的大图预览要求与验收场景。

## Impact

- 前端：`apps/video-agent/app/pages/script-creation/AssetCandidatePanel.tsx`、相关样式和测试。
- E2E：素材生成候选操作流程增加大图预览验证。
- API、数据库、worker、模型调用和外部费用均不受影响。
