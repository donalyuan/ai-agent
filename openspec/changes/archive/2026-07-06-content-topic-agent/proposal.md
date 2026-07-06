# content-topic-agent Proposal

## 背景

VEDIO-AGENT 当前脚本创作已经能生成和修改结构化脚本，但脚本前置的选题仍只是用户输入的 `topic` 文本。`projects` 表示内容项目、账号方向或内容生产边界，不是具体选题；因此现有链路无法稳定追踪“哪个选题产生了哪个脚本”，也无法沉淀待评估、已确认、已成稿和归档的选题池。

用户已确认第一版优先建设“内容策略 + 选题池 + 选题 Agent + 脚本关联”闭环，并将选题 Agent 接入现有通用 Agent Runtime。

## 目标

1. 新增独立选题实体和选题生成批次，支撑选题生命周期管理。
2. 在 `apps/video-agent` 的“内容策略”页展示项目策略摘要和选题池闭环。
3. 支持人工创建选题和选题 Agent 批量生成候选。
4. 新增 `topic` Agent adapter，复用 `agent_conversations`、`agent_messages`、`agent_runs`、`agent_steps`。
5. 让脚本生成可选关联 `topic_id`，并保存 `topic_snapshot`，保证历史可复现。
6. 从已确认选题进入脚本创作前，要求确认 `style` 和 `scene_count`。

## 非目标

1. 不实现完整 `content_strategies` 编辑和 active 策略管理。
2. 不接入 `viral_videos`、外部热点源或平台趋势抓取。
3. 不实现发布排期、内容日历或多平台分发。
4. 不做复杂用户画像编辑。
5. 不改变平台控制面归属；模型 Key、MCP、Worker 队列和系统日志仍归 `admin/` 或后端控制面。
6. 不覆盖移动端原型、移动端适配或移动端验收。

## 影响范围

- 数据库新增 `content_topics`、`topic_generation_batches`，并为 `scripts` 增加可选 `topic_id`。
- 后端新增选题 repository、API、DTO、状态校验和 `topic` Agent adapter。
- 脚本生成链路扩展可选 `topic_id`，生成成功后推进选题状态。
- `apps/video-agent` 新增“内容策略”页面能力，展示策略摘要、选题生成区、选题池和脚本确认面板。
- OpenSpec、项目记忆和测试需要同步更新。
