# rank-topic-groups-for-script-production Tasks

## 1. OpenSpec 与设计

- [x] 创建 proposal、design、spec 增量和 tasks。
- [x] 运行 `openspec instructions apply --change "rank-topic-groups-for-script-production" --json`，确认 change 可识别。

## 2. 记忆与原型

- [x] 更新项目记忆，记录主题组排名目标为“选出最值得立刻生成脚本的主题组”。
- [x] 更新 `docs/prototypes/video-agent/video-agent.pen`，覆盖历史生成页主题组脚本优先级排序。
- [x] 获得用户明确原型确认后再进入前端编码。

## 3. 后端主题组排名读模型

- [x] 设计 `TopicGroupSummary` / `TopicGroupScriptPriority` domain model 和 response DTO。
- [x] 实现主题组聚合查询，按原始批次归并补充批次。
- [x] 实现最新成功评审快照读取和 `review_freshness` 判定。
- [x] 实现脚本优先级确定性计算、指标统计和推荐候选选择。
- [x] 实现 `GET /api/projects/:project_id/topic-groups?sort=script_priority|created_at`。
- [x] 保持现有批次列表、补充批次和评审 API 兼容。

## 4. 前端历史生成页

- [x] 扩展 `apps/video-agent/app/lib/api.ts` 和 API 测试。
- [x] 在页面编排层加载主题组 summary，并与现有批次列表状态协调。
- [x] 历史生成页左侧增加排序切换，默认使用脚本优先级。
- [x] 主题组卡片展示状态、分数、推荐候选数量和主要风险。
- [x] 待评审或过期评审主题组复用现有“评审当前主题组”动作。
- [x] 点击主题组后保持现有中列评审分层、补充操作和当前选题池联动。

## 5. 验证

- [x] 运行后端相关 Rust 测试。
- [x] 运行前端相关 Vitest。
- [x] 运行视频工作台 E2E。
- [x] 运行 `openspec instructions apply --change "rank-topic-groups-for-script-production" --json` 并确认任务状态与实际一致。
