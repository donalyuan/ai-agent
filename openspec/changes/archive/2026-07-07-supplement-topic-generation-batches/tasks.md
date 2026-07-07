## 1. OpenSpec 与原型门禁

- [x] 1.1 确认 `supplement-topic-generation-batches` 的 proposal、design 和 specs 通过 OpenSpec 校验。
- [x] 1.2 更新 `docs/prototypes/video-agent/video-agent.pen`，加入历史批次详情中的补充选题入口。
- [x] 1.3 原型覆盖原始批次、补充批次列表、补充表单、补充成功后选中补充批次、补充失败提示。
- [x] 1.4 获得用户明确确认口令后再进入正式编码。

## 2. 数据库与仓储

- [x] 2.1 新增递增 migration，为 `topic_generation_batches` 增加 `supplement_of_batch_id`、注释和索引。
- [x] 2.2 扩展 `TopicGenerationBatch` 和响应模型，返回补充关系字段。
- [x] 2.3 扩展 `TopicRepository` contract，支持创建补充批次和查询补充批次。
- [x] 2.4 查询历史批次时保持项目隔离，并分别统计原始批次和补充批次的未软删除选题数。
- [x] 2.5 发起补充前校验目标批次存在、同项目、成功且仍有可见选题。

## 3. 后端 Agent 与 API

- [x] 3.1 扩展 topic Agent 消息请求，支持传入 `supplement_of_batch_id`。
- [x] 3.2 创建补充批次时写入 `supplement_of_batch_id`，并保持原批次不可变。
- [x] 3.3 生成的新选题必须关联补充批次本身，而不是原始批次。
- [x] 3.4 Agent 回复 metadata 包含新批次 `batch_id`、`supplement_of_batch_id`、`created_topic_ids` 和 `topic_count`。
- [x] 3.5 覆盖不存在批次、跨项目批次、失败批次、空批次的错误路径。
- [x] 3.6 补充生成调用 LLM 前注入原始批次 prompt、同主题组已有选题和当前会话历史消息摘要。
- [x] 3.7 补充生成 prompt 明确要求基于同一主题扩展，并避免重复已有选题。

## 4. 前端 API 与状态模型

- [x] 4.1 更新 `apps/video-agent/app/lib/api.ts` 的请求与响应类型，支持补充批次字段。
- [x] 4.2 更新内容策略状态刷新逻辑，补充成功后刷新批次列表、选题列表和统计。
- [x] 4.3 补充成功后选中新创建的补充批次，避免仍停留在原始批次导致误判。

## 5. 前端页面实现

- [x] 5.1 更新 `TopicHistoryPage.tsx`，在批次详情中增加“补充选题”入口。
- [x] 5.2 增加补充选题表单，复用现有选题 Agent 的补充要求和数量规则。
- [x] 5.3 展示原始批次关联的补充批次列表，并支持切换查看补充批次选题。
- [x] 5.4 补充生成进行中禁用重复提交，并展示失败原因。
- [x] 5.5 保持现有历史生成的移除选题、不可删除提示和返回当前选题池能力不回退。
- [x] 5.6 按已确认原型将历史生成页调整为批次、当前主题选题、补充操作三列布局。
- [x] 5.7 将历史生成页查看口径调整为主题组聚合，原始批次和补充批次选题同屏展示并标识生成来源。
- [x] 5.8 左侧历史生成列表只展示原始主题组，补充批次不得作为独立主题入口展示。
- [x] 5.9 补充成功后保持选中原始主题组，并在右侧关联补充批次区域展示新批次。

## 6. 验证

- [x] 6.1 运行后端 topic repository、topic routes 和 topic agent runtime 相关测试。
- [x] 6.2 运行前端 API、内容策略和历史生成相关 Vitest。
- [x] 6.3 运行视频工作台 E2E，覆盖历史批次补充入口和补充后切换。
- [x] 6.4 运行 `cargo fmt`、`cargo clippy`、前端 lint 和 build。
- [x] 6.5 运行 `openspec instructions apply --change "supplement-topic-generation-batches" --json` 并确认任务进度与实际一致。
- [x] 6.6 运行后端 topic Agent runtime 相关测试，覆盖补充生成上下文注入。
- [x] 6.7 重新运行 `openspec instructions apply --change "supplement-topic-generation-batches" --json` 并确认任务进度与实际一致。
- [x] 6.8 运行前端历史生成页相关测试，覆盖补充批次不作为独立主题入口。
- [x] 6.9 重新运行 `openspec validate --all` 和 `openspec instructions apply --change "supplement-topic-generation-batches" --json`。
