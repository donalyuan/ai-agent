# content-topic-agent Design

## DDD

`Project` 继续表示内容项目、账号方向或内容生产边界，不承担具体选题生命周期。

新增 `ContentTopic` 表示具体选题，归属一个项目。选题字段包括标题、角度、目标受众、看点、内容类型、评分、评分理由、标签、来源和状态。状态流转为 `idea -> approved -> scripted -> archived`：

- `idea -> approved` 由人工确认。
- `approved -> scripted` 由脚本生成成功自动推进。
- `idea/approved/scripted -> archived` 由人工归档。
- `archived` 不允许生成脚本。

新增 `TopicGenerationBatch` 表示一次选题 Agent 批量生成记录。Agent 生成结果自动入库为 `idea`，并记录批次，方便按批次查看、清理和追溯。

`Script` 可选关联 `topic_id`。生成脚本时同时保存 `scripts.content.topic_snapshot`，避免选题后续编辑影响历史脚本理解。

`topic` Agent adapter 是通用 Agent Runtime 的业务 adapter，不新增孤立聊天逻辑。

## BDD

运营人员进入“内容策略”页后，选择项目并看到项目定位、描述、选题统计和选题池。无选题时，页面引导手动新增或使用 Agent 生成。

运营人员可手动新增选题。新增后状态为 `idea`，来源为 `manual`。

运营人员可输入补充要求，例如“本周 AI 工具方向，生成 8 个选题”。系统创建或复用 `topic` Agent 会话，读取项目上下文，调用 LLM 生成结构化候选，创建生成批次，并把候选自动写入选题池。生成结果状态为 `idea`，来源为 `agent`。

内容策略页展示历史生成批次。历史生成只展示 `succeeded` 且有实际选题的批次，失败批次和空批次不作为可切换入口出现。存在生成批次时，页面默认按最新批次展示选题；运营人员可以切换任一历史批次，也可以切回全部选题。状态筛选覆盖 `全部`、`待评估`、`已确认`、`已成稿` 和 `已归档`。

运营人员评估后确认选题，状态变为 `approved`。只有 `approved` 选题显示“生成脚本”。点击后系统弹出确认面板，展示选题快照，并让用户确认 `style` 和 `scene_count`。脚本生成成功后，选题状态自动变为 `scripted`。

生成失败时不污染已有选题池；脚本生成失败时不创建脚本、不更新选题状态。

## SDD

新增数据表：

- `content_topics`
- `topic_generation_batches`

修改数据表：

- `scripts.topic_id UUID NULL REFERENCES content_topics(id) ON DELETE SET NULL`
- `scripts.content.topic_snapshot` 在生成时写入

新增或扩展 API：

- `GET /api/projects/:project_id/topics?status=&source=&batch_id=`
- `GET /api/projects/:project_id/topic-generation-batches`
- `POST /api/projects/:project_id/topics`
- `PUT /api/topics/:topic_id`
- `PUT /api/topics/:topic_id/status`
- `POST /api/projects/:project_id/topics/generate`
- `POST /api/topics/:topic_id/prepare-script`
- `POST /api/scripts/generate` 扩展可选 `topic_id`

`content_type` 第一版以后端开放文本处理；前端提供常用快捷项但不将其写死为后端枚举。

`topic` Agent adapter 的 run steps 至少包含：

- `read_project_context`
- `generate_topics`
- `persist_topics`

LLM 输出必须是结构化 JSON 数组。任一候选字段缺失、score 越界、输出为空或无法解析时，整批失败，不做部分入库。

前端第一版启用“内容策略”页面，包含策略摘要区、选题生成区、选题池、选题详情侧栏和脚本确认面板。页面不提供完整策略编辑。

## TDD

后端先补失败测试：

- migration 创建 `content_topics`、`topic_generation_batches`、`scripts.topic_id`、索引和约束。
- repository 支持选题 CRUD、批次保存、状态更新、按项目/状态/来源/批次查询。
- API 支持手动新增、编辑、筛选、状态流转和 Agent 生成。
- `topic` Agent 对有效 LLM 输出入库并记录 run/step。
- `topic` Agent 对空输出、非法 JSON、字段缺失和 score 越界整批失败。
- 脚本生成带 `topic_id` 时校验项目归属和状态。
- 脚本生成成功后保存 `topic_snapshot` 并把选题更新为 `scripted`。

前端先补失败测试：

- 内容策略页渲染项目摘要、空状态和选题池。
- 选题状态筛选、历史批次切换、手动新增、确认、归档可见。
- Agent 生成失败只影响生成区。
- approved 选题打开脚本确认面板。
- 脚本生成成功后选题变为已成稿，并能在脚本详情看到来源选题。
