# 主题组选题评审设计

## 背景

内容策略模块已经具备选题池、历史生成、补充批次、软删除和从已确认选题生成脚本的闭环。当前问题不在于缺少选题，而是同一主题组内选题数量变多后，运营人员难以判断哪些应优先进入脚本、哪些只是备选、哪些应淘汰。

用户已确认本轮优化聚焦“选题太多不好筛”，并同时覆盖：

- 选题重复或角度相似，难以判断该保留哪个。
- 分数和理由不足以支撑决策。
- 风险点不明显，例如太泛、不可脚本化、账号不匹配或合规风险。
- 同一主题下缺少“优先、备选、淘汰”的明确分层。
- 历史生成页和当前选题池需要同步展示同一套评审分层。

## DDD

新增 `TopicReviewSnapshot` 概念，表示某个主题组的一次 AI 评审结果。主题组由原始 `topic_generation_batches.id` 作为 `root_batch_id` 标识，包含原始批次和关联补充批次下所有未软删除选题。

`TopicReviewSnapshot` 不拥有选题生命周期，不改变 `ContentTopic.status`。`idea -> approved -> scripted -> archived` 仍由人工确认、脚本生成和人工归档推进。AI 评审只提供决策辅助。

评审结果以主题组为单位保存，包含：

- `review_summary`：主题组整体判断。
- `topic_reviews`：每条选题的分层、理由、风险和重复关系。
- `priority`：`priority`、`backup`、`reject`。
- `risk_flags`：`too_generic`、`duplicate`、`hard_to_script`、`off_positioning`、`compliance_risk`。
- `similar_topic_ids`：同主题组内疑似相似选题引用。

## BDD

运营人员进入“历史生成”，选择一个主题组后，可以点击“评审当前主题组”。系统读取该主题组下原始批次和补充批次的可见选题，调用 AI 输出分层评审，并保存最新快照。

评审完成后，中列不再只显示普通选题列表，而是按“优先推荐、可备选、建议淘汰、疑似重复”组织。运营人员可以在同一视图中确认选题、归档选题、移除未成稿选题，或从已确认选题进入脚本生成。

运营人员切换到“当前选题池”时，如果当前选题池处于某个主题组过滤上下文，也展示同一份最新评审快照。若当前选题池处于“查看全部选题”，不展示主题组评审分层，避免跨主题混评。

第一版支持手动点击评审。自动评审只预留开关和接口语义，不默认启用；后续可在生成或补充成功后自动触发。

## SDD

新增后端模型建议：

- `topic_review_snapshots`
  - `id`
  - `project_id`
  - `root_batch_id`
  - `source_run_id`
  - `status`: `running`、`succeeded`、`failed`
  - `review_summary`
  - `result JSONB`
  - `error_message`
  - `metadata JSONB`
  - `created_at`
  - `updated_at`

新增或扩展接口建议：

- `POST /api/topic-groups/:root_batch_id/reviews`
- `GET /api/topic-groups/:root_batch_id/reviews/latest`

后端约束：

- 只能评审当前项目下存在且可管理的主题组。
- 评审输入只包含未软删除选题。
- 评审输出引用的 `topic_id` 必须属于同一主题组。
- LLM 输出非法时整次失败，不写入成功快照，不修改任何选题。
- 评审结果不得自动更新 `content_topics.status`。

前端结构：

- 页面编排层统一持有 `activeTopicBatchId/rootBatchId` 和 `reviewSnapshot`。
- `ContentStrategyPage` 与 `TopicHistoryPage` 共享评审分层选题列表组件。
- 历史生成页和当前选题池都可触发手动评审。
- 无评审快照或全部选题模式下回退到现有普通列表。

## TDD

后端测试：

- migration 创建 `topic_review_snapshots` 和必要索引。
- repository 能创建、读取最新评审快照，并按项目隔离。
- 评审只读取同主题组未软删除选题，自动归并补充批次。
- 评审输出引用组外选题、缺字段、非法 priority 或非法 risk flag 时失败。
- 评审失败不写成功快照、不改变 `ContentTopic.status`。

前端测试：

- 历史生成页可触发主题组评审并展示分层。
- 当前选题池在同一主题组过滤下展示同一快照。
- 全部选题模式不展示主题组评审。
- 评审结果中的优先、备选、淘汰和重复关系可见。
- 评审展示不影响确认、归档、移除和生成脚本动作。

## 非目标

- 不实现热点源、竞品抓取或 `viral_videos` 接入。
- 不实现完整 `content_strategies` 策略编辑。
- 不让 AI 自动确认、归档或删除选题。
- 不覆盖移动端适配。
- 不改变历史生成三列结构和补充批次语义。
