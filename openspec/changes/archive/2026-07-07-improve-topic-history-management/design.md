## Context

内容策略页当前已经实现选题 Agent、选题池、状态筛选、历史生成批次小列表、选题详情和从选题生成脚本。历史生成批次仍嵌在选题池面板内，适合少量批次切换，但不适合作为长期管理入口。

当前数据模型中 `content_topics.status` 表达业务生命周期，包含 `idea`、`approved`、`scripted`、`archived`。`scripts.topic_id` 可关联选题，脚本生成成功后会把选题推进为 `scripted`，并保存 `topic_snapshot`。用户已确认：未生成脚本的选题可以从管理视图移除，已生成脚本的选题不可删除。

前端约束：视频工作台使用 `DESIGN.md` 的浅色、紧凑、运营后台风格；参考 `Ant Design`、`IBM Carbon` 和 `GitHub Primer` 的列表、筛选、状态标签和危险操作模式。每个功能页面尽量使用独立页面级前端文件，可复用 model/helper。

## Goals / Non-Goals

**Goals:**

- 将历史生成升级成独立列表页或独立二级视图，支持批次扫描、批次详情和批次内选题管理。
- 保持内容策略主视图轻量：生成选题、查看当前选题池、查看选题详情和进入脚本创作。
- 增加选题软删除能力，默认管理视图、统计和批次计数排除软删除选题。
- 对已生成脚本的选题强制禁止删除，避免破坏脚本与选题链路。
- 保持 `archived` 的业务归档语义，不让它承担“管理视图移除”。

**Non-Goals:**

- 不实现账号/项目管理。
- 不改变选题 Agent 生成、脚本生成和 `scripted` 自动流转规则。
- 不提供软删除恢复入口；恢复能力如需要应单独提出。
- 不展示失败批次或空批次作为可管理历史入口。
- 不覆盖移动端适配。

## Decisions

### 1. 软删除使用 `deleted_at`，不复用 `archived`

为 `content_topics` 增加 `deleted_at TIMESTAMPTZ NULL`。软删除时只写入 `deleted_at`，不改 `status`。默认选题列表、统计和生成批次 `topic_count` 都排除 `deleted_at IS NOT NULL`。

原因：`archived` 是业务状态，表示选题仍可作为历史记录被管理和筛选；“从管理视图移除”是可见性行为。复用 `archived` 会让“已归档”和“已删除”混在一起，后续统计和筛选不可解释。

备选方案：复用 `archived`。改动少，但语义不清，且不满足“移除管理视图”的直觉。

### 2. 删除权限以后端事实为准

新增 `DELETE /api/topics/:topic_id`，执行软删除。后端删除前必须读取选题并检查：

- `content_topics.deleted_at IS NULL`
- 选题状态不是 `scripted`
- 不存在 `scripts.topic_id = topic_id`

只要状态为 `scripted` 或存在脚本引用，后端 SHALL 拒绝删除并返回明确错误。前端可以隐藏不可删除选题的删除按钮，但不能只依赖前端判断。

原因：`scripts.topic_id` 是真实产物链路，后端校验可以覆盖并发、绕过前端和历史异常数据。

备选方案：依赖 `status !== scripted`。实现更简单，但如果历史脚本引用存在而状态异常，会错误允许删除。

### 3. 历史生成采用内容策略内的独立页面级组件

前端新增 `apps/video-agent/app/pages/content-strategy/TopicHistoryPage.tsx`，与现有 `ContentStrategyPage.tsx` 同级。`app/page.tsx` 只负责选择当前内容策略视图和传递状态/回调，不继续把历史页 UI 堆进主文件。

内容策略主视图提供“查看历史生成”入口；历史生成视图提供返回当前选题池入口。内容策略二级入口中，“历史生成”排在“当前选题池”上方。历史视图展示批次列表、批次摘要、状态/时间/数量、批次内选题列表和删除动作。

原因：历史生成是独立管理工作流，单独组件更符合长期页面结构约定，也便于后续增加搜索、分页或批次分析。

备选方案：继续嵌在 `ContentStrategyPage` 内。改动小，但会让内容策略页继续膨胀，批次多时管理体验差。

### 4. 批次列表只展示仍有可见选题的成功批次

`GET /api/projects/:project_id/topic-generation-batches` 继续只返回 `succeeded` 且 `topic_count > 0` 的批次，但 `topic_count` 改为只统计未软删除选题。某批次下选题全部软删除后，该批次不再出现在默认历史列表中。

原因：用户目标是管理可见历史生成，不是审计所有运行记录。失败 run 和空批次仍由 Agent Runtime/运行记录承担追溯。

备选方案：历史页展示所有成功批次，包括已被清空的批次。信息更完整，但会把不可操作的空批次留在管理页，增加噪音。

### 5. 原型先覆盖桌面运营后台关键状态

Pencil 原型更新 `docs/prototypes/video-agent/video-agent.pen`，至少包含：

- 内容策略主视图中进入历史生成的入口。
- 历史生成列表页：批次列表、筛选/统计、批次详情、批次内选题行。
- 未生成脚本选题的删除动作。
- 已生成脚本选题的不可删除状态提示。

原型只覆盖桌面端，不新增移动端版式。

## Risks / Trade-offs

- [Risk] 删除后统计与当前前端本地状态可能短暂不一致。→ 删除成功后统一刷新批次列表、选题列表和统计，避免只做本地乐观删除。
- [Risk] 历史数据可能出现 `status != scripted` 但已有脚本引用。→ 后端以 `scripts.topic_id` 存在性作为最终拒绝条件。
- [Risk] 批次所有选题软删除后历史列表消失，用户可能以为批次不存在。→ 这是管理视图语义；如后续需要审计视图，单独增加“运行记录/审计历史”。
- [Risk] 新增 `deleted_at` 后查询漏加过滤。→ repository 层统一默认过滤，并用测试覆盖列表、统计、批次计数和删除后不可见。

## Migration Plan

1. 新增 migration 为 `content_topics` 增加 `deleted_at` 和必要索引。
2. 更新 repository 查询：默认列表、统计、批次计数排除软删除。
3. 新增软删除 repository 方法和 API route。
4. 更新前端 API client、内容策略视图和历史生成页面。
5. 更新 Pencil 原型并获得用户明确开发确认后再进入正式编码。

回滚时可移除前端入口并停止调用删除 API；数据库 `deleted_at` 字段保留不影响旧查询，但如果需要完全回滚，应先确认无软删除数据依赖。
