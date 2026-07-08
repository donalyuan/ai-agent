# topic-group-review-snapshots Design

## DDD

`ContentTopic` 继续表示具体选题，业务状态仍为 `idea -> approved -> scripted -> archived`。主题组由原始 `topic_generation_batches.id` 作为 `root_batch_id` 表示，包含原始批次和所有 `supplement_of_batch_id = root_batch_id` 的补充批次。

新增 `TopicReviewSnapshot`，表示某个主题组的一次评审快照。它只记录 AI 对组内选题的决策辅助，不拥有选题生命周期，也不得自动修改 `ContentTopic.status`。

评审结果包含：

- 主题组整体摘要。
- 每条选题的推荐层级：优先、备选、淘汰。
- 推荐或淘汰理由。
- 风险标记：太泛、疑似重复、不可脚本化、账号不匹配、合规风险。
- 同组内相似选题引用。

## BDD

运营人员进入“历史生成”并选中一个主题组后，可以点击“评审当前主题组”。系统读取该主题组下所有未软删除选题，调用 AI 输出评审结果并保存快照。

评审成功后，历史生成页的中列按分层展示选题。运营人员可以优先处理“优先推荐”，对相似选题只保留一个，对建议淘汰的选题查看原因后归档或移除。

当运营人员切回“当前选题池”且当前选题池仍处于同一主题组过滤上下文时，页面展示同一份最新评审快照。若操作者选择“查看全部选题”，页面回退为普通选题池列表，不展示主题组评审分层。

第一版默认由用户手动触发评审。后续可在选题生成或补充生成成功后自动触发，但自动触发必须复用同一评审快照模型和展示语义。

## SDD

新增表建议：

```sql
CREATE TABLE topic_review_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    root_batch_id UUID NOT NULL REFERENCES topic_generation_batches(id) ON DELETE CASCADE,
    source_run_id UUID REFERENCES agent_runs(id) ON DELETE SET NULL,
    status VARCHAR(20) NOT NULL,
    review_summary TEXT NOT NULL DEFAULT '',
    result JSONB NOT NULL DEFAULT '{}'::jsonb,
    error_message TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

建议索引：

- `(project_id, root_batch_id, created_at DESC)` 用于读取最新评审。
- `source_run_id` 非空索引用于运行追踪。
- `status` 用于排查失败评审。

新增 API：

- `POST /api/topic-groups/:root_batch_id/reviews`
- `GET /api/topic-groups/:root_batch_id/reviews/latest`

评审 LLM 输出必须是结构化 JSON，建议形态：

```json
{
  "review_summary": "本主题组更适合优先制作工具落地和真实案例方向。",
  "topic_reviews": [
    {
      "topic_id": "uuid",
      "priority": "priority",
      "reason": "账号匹配度高，脚本化路径清晰。",
      "risk_flags": ["duplicate"],
      "similar_topic_ids": ["uuid"]
    }
  ]
}
```

校验规则：

- `topic_reviews.topic_id` 必须属于当前主题组。
- `priority` 只能是 `priority`、`backup`、`reject`。
- `risk_flags` 只能使用后端允许值。
- 缺少必填字段、引用组外选题或输出无法解析时，整次评审失败。
- 失败不得写入成功快照，不得修改选题状态。

前端结构：

- 页面编排层统一加载当前主题组最新评审快照。
- `ContentStrategyPage` 和 `TopicHistoryPage` 共享评审分层列表组件。
- 历史生成页和当前选题池都提供手动评审入口。
- 全部选题模式、无主题组或无评审快照时展示现有普通列表。

## TDD

后端先补失败测试：

- migration 创建 `topic_review_snapshots`、索引和约束。
- repository 创建评审快照、读取最新评审、按项目隔离。
- 评审只读取同主题组未软删除选题，并归并补充批次。
- LLM 输出非法、引用组外选题、priority 非法、risk flag 非法时整次失败。
- 评审成功记录 run/step 和快照。
- 评审失败不改选题状态。

前端先补失败测试：

- 历史生成页触发评审后展示优先、备选、淘汰和重复关系。
- 当前选题池在同一主题组过滤下展示同一快照。
- 全部选题模式不展示评审分层。
- 评审展示不影响确认、归档、移除和生成脚本动作。

## 风险与取舍

- AI 评审可能误判。解决：评审只辅助展示，不自动改变选题状态。
- 评审结果可能滞后于选题新增或软删除。解决：展示快照生成时间，并允许手动重新评审。
- 自动评审会增加成本和等待时间。解决：第一版默认手动，自动只预留接口语义。
- 前端列表复杂度上升。解决：抽取共享评审分层列表组件，普通列表作为 fallback。

## 原型要求

进入前端实现前必须更新 `docs/prototypes/video-agent/video-agent.pen` 并获得明确确认。原型至少覆盖：

- 历史生成页中列的评审分层视图。
- 当前选题池在主题组过滤下的同步评审展示。
- 手动“评审当前主题组”入口。
- 无评审快照和全部选题模式的 fallback 状态。
