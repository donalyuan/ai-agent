# topic-group-review-snapshots Tasks

## 1. OpenSpec 与设计

- [x] 创建 proposal、design、spec 增量和 tasks。
- [x] 运行 `openspec instructions apply --change "topic-group-review-snapshots" --json`，确认 change 可识别。

## 2. 记忆与原型

- [x] 更新项目记忆，记录主题组评审快照、双页面同步展示和 AI 不自动改状态的确认边界。
- [x] 更新 `docs/prototypes/video-agent/video-agent.pen`，覆盖历史生成页和当前选题池的评审分层视图。
- [x] 获得用户明确原型确认后再进入前端编码。

## 3. 数据库与仓储

- [x] 新增 migration：`topic_review_snapshots`。
- [x] 添加项目、主题组、时间、run 追踪索引。
- [x] 实现评审快照 domain model 和 DTO。
- [x] 实现 repository contract tests。
- [x] 实现 Postgres repository。

## 4. 后端评审 API 与 Agent

- [x] 实现 `POST /api/topic-groups/:root_batch_id/reviews`。
- [x] 实现 `GET /api/topic-groups/:root_batch_id/reviews/latest`。
- [x] 复用主题组查询逻辑，读取原始批次和补充批次的可见选题。
- [x] 构造主题组评审 prompt。
- [x] 解析并校验 LLM 结构化评审输出。
- [x] 记录 `agent_runs` 和 `agent_steps`。
- [x] 覆盖非法 JSON、缺字段、非法 priority、非法 risk flag、引用组外选题和 LLM 失败。
- [x] 确认评审成功和失败都不修改 `ContentTopic.status`。

## 5. 前端实现

- [x] 扩展 `apps/video-agent/app/lib/api.ts` 和测试。
- [x] 在页面编排层维护当前主题组评审快照。
- [x] 抽取共享评审分层选题列表组件。
- [x] 历史生成页增加手动评审入口和分层展示。
- [x] 当前选题池在主题组过滤下同步展示同一评审快照。
- [x] 全部选题模式回退普通列表。
- [x] 保持确认、归档、移除和生成脚本动作可用。

## 6. 验证

- [x] 运行后端相关 Rust 测试。
- [x] 运行前端相关 Vitest。
- [x] 运行视频工作台 E2E。
- [x] 运行 `openspec instructions apply --change "topic-group-review-snapshots" --json` 并确认任务状态与实际一致。
