## 1. OpenSpec 与原型门禁

- [x] 1.1 确认 `improve-topic-history-management` 的 proposal、design 和 specs 均通过 OpenSpec 校验。
- [x] 1.2 更新 `docs/prototypes/video-agent/video-agent.pen`，加入内容策略历史生成独立列表页。
- [x] 1.3 原型覆盖内容策略主视图入口、历史生成列表、批次详情、未生成脚本选题删除、已生成脚本选题不可删除，并体现“历史生成”入口位于“当前选题池”上方。
- [x] 1.4 获得用户明确确认口令后再进入正式编码。

## 2. 数据库与仓储

- [x] 2.1 新增 migration，为 `content_topics` 增加 `deleted_at` 和默认查询所需索引。
- [x] 2.2 扩展 `TopicRepository` contract，支持软删除选题。
- [x] 2.3 后端查询选题池时默认排除 `deleted_at IS NOT NULL`。
- [x] 2.4 后端统计选题状态时默认排除软删除选题。
- [x] 2.5 生成批次历史的 `topic_count` 只统计未软删除选题。
- [x] 2.6 软删除前校验选题状态和 `scripts.topic_id` 引用，已生成脚本或已被脚本引用时拒绝删除。

## 3. 后端 API

- [x] 3.1 新增 `DELETE /api/topics/:topic_id`，成功软删除返回稳定响应。
- [x] 3.2 删除已生成脚本选题时返回明确错误且不修改数据。
- [x] 3.3 已软删除选题不能进入 `prepare-script`。
- [x] 3.4 覆盖选题不存在、重复删除、已生成脚本不可删除和项目隔离等错误路径。

## 4. 前端 API 与状态模型

- [x] 4.1 更新 `apps/video-agent/app/lib/api.ts`，新增删除选题 API client 和测试。
- [x] 4.2 更新内容策略状态刷新逻辑，删除后刷新选题列表、统计和历史批次。
- [x] 4.3 将历史生成视图状态与当前选题池状态隔离，避免切换页面时丢失项目上下文。

## 5. 前端页面实现

- [x] 5.1 新增 `TopicHistoryPage.tsx` 页面级组件。
- [x] 5.2 内容策略主视图增加进入历史生成列表页的入口。
- [x] 5.3 历史生成页实现批次列表、批次摘要、批次详情和批次内选题列表。
- [x] 5.4 未生成脚本选题提供删除确认与成功后移除。
- [x] 5.5 已生成脚本选题不展示删除按钮，并展示不可删除状态说明。
- [x] 5.6 保持内容策略主视图的新增、编辑、确认、归档和生成脚本能力不回退。
- [x] 5.7 将历史生成和当前选题池作为内容策略左侧二级菜单展示，移除主内容区里的二级菜单按钮组。
- [x] 5.8 历史生成页选择批次时同步当前选题池批次过滤，确保返回当前选题池后展示同一批次选题。

## 6. 验证

- [x] 6.1 运行后端 repository 和 topic routes 相关测试。
- [x] 6.2 运行前端 API、内容策略和历史生成相关 Vitest。
- [x] 6.3 运行视频工作台 E2E，覆盖历史生成入口和删除限制。
- [x] 6.4 运行 `cargo fmt`、`cargo clippy`、前端 lint 和 build。
- [x] 6.5 运行 `openspec instructions apply --change "improve-topic-history-management" --json` 并确认任务进度与实际一致。
