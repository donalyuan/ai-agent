# 选题质量闸门设计

## 背景

内容策略模块已经具备选题池、历史生成、补充批次、主题组评审和脚本产出优先级排名。当前用户确认的核心问题是 `topic` Agent 生成质量不稳定，表现为选题太泛、重复、偏离账号定位、难以生成短视频脚本，以及评分虚高且理由不可信。

现有链路能通过严格 JSON Schema 保证结构正确，但质量控制发生得太晚。主题组评审是在选题入库后帮助运营筛选，不能阻止低质候选进入可用选题池。

本设计目标是增加入库前质量闸门，减少人工大面积重筛。

## DDD

新增 `TopicQualityGate`，表示选题候选入库前的质量控制规则。它不替代主题组评审，也不拥有选题生命周期。

新增 `TopicQualityEvaluation`，表示某个生成批次的一次质量评估快照。它关联 `topic_generation_batches.id`，记录通过项、淘汰项、质量原因和是否自动重写。

质量维度固定为：

- 账号匹配度：是否贴合项目定位和描述。
- 具体度：是否避免百科式、泛化标题。
- 差异化：是否避免同批或同主题组已有选题重复。
- 脚本化可行性：是否适合短视频结构化表达。
- 风险与禁区：是否存在合规风险或明显偏题。
- 评分可信度：评分与理由是否一致，是否虚高。

`ContentTopic.status` 保持 `idea -> approved -> scripted -> archived`。质量淘汰不是业务状态；淘汰候选不写入 `content_topics`，只保存在质量报告中。

## BDD

运营人员提交选题生成要求后，系统先生成候选，但不立即入库。质量闸门评估每条候选，若首轮通过率低于阈值，则带着质量问题自动重写一次。

重写后，系统再次评估候选。通过候选进入当前选题池，状态为 `idea`；淘汰候选只在质量报告中只读展示，不支持确认选题、生成脚本或归档。

Agent 回复显示本批质量结果：通过数量、淘汰数量、是否触发重写、主要淘汰原因。历史生成页展示批次质量摘要，右侧质量报告展示被淘汰候选和原因。

补充批次也经过同一质量闸门，并把同主题组已有选题作为重复检测上下文。

## SDD

新增表 `topic_quality_evaluations`：

- `id`
- `project_id`
- `batch_id`
- `source_run_id`
- `status`: `succeeded | failed`
- `pass_count`
- `reject_count`
- `rewrite_triggered`
- `result JSONB`
- `error_message`
- `created_at`
- `updated_at`

质量评估结果结构：

```json
{
  "summary": "本批次 5 条中 3 条通过，2 条因泛化和重复被淘汰。",
  "items": [
    {
      "candidate_key": "candidate-1",
      "title": "选题标题",
      "decision": "pass",
      "quality_score": 86,
      "flags": [],
      "reason": "贴合账号定位，脚本化路径清晰。"
    }
  ]
}
```

质量 flags：

- `too_generic`
- `duplicate`
- `off_positioning`
- `hard_to_script`
- `compliance_risk`
- `score_untrusted`

通过项写入 `content_topics.metadata.quality_gate`：

```json
{
  "quality_gate": {
    "evaluation_id": "uuid",
    "quality_score": 86,
    "flags": [],
    "reason": "贴合账号定位，脚本化路径清晰。"
  }
}
```

新增读取接口：

- `GET /api/topic-generation-batches/:batch_id/quality-evaluation`

第一版不新增独立手动触发质量评估接口，质量闸门只挂在选题 Agent 生成链路中。

固定阈值：

- 单条候选 `quality_score >= 70` 才可入库。
- 出现 `off_positioning` 或 `compliance_risk` 直接淘汰。
- 出现 `duplicate` 默认淘汰，除非评估明确说明可差异化补充。
- 首轮通过率 `< 60%` 触发最多一次自动重写。
- 自动重写最多 1 次。

错误处理：

- 候选生成失败：生成批次 `failed`，不写入选题。
- 质量评估失败：生成批次 `failed`，写入失败质量快照，不写入选题。
- 自动重写失败：保留第一次质量评估快照，生成批次 `failed`，不写入选题。
- 重写后仍低质：有通过项则只入库通过项；无通过项则生成批次 `failed`。

前端展示：

- Agent 消息显示“通过 N 条 / 淘汰 M 条 / 已重写 1 次”。
- 当前选题池卡片展示质量分和风险标签。
- 历史生成批次卡展示质量摘要。
- 历史生成页右侧展示质量报告，淘汰候选只读。

## TDD

后端测试：

- migration 创建 `topic_quality_evaluations`、索引和约束。
- repository 创建、读取最新质量评估，并按项目隔离。
- 选题 Agent 正常生成时完成“生成 -> 评估 -> 入库通过项 -> 写质量快照”。
- 泛化、重复、偏离定位、难脚本化、合规风险和评分虚高被识别为质量 flags。
- 首轮低质过多触发最多一次重写。
- 质量评估失败不写入任何 `content_topics`。
- 补充批次评估包含同主题组已有选题上下文。
- `GET /api/topic-generation-batches/:batch_id/quality-evaluation` 只返回同项目数据。

前端测试：

- Agent 消息展示质量结果。
- 选题卡展示质量分和风险标签。
- 历史生成页展示批次质量摘要。
- 质量报告展示淘汰候选且不提供确认、生成脚本或归档操作。

E2E：

- 生成一批选题后，页面可看到通过、淘汰和是否重写。
- 只有通过候选出现在当前选题池。
- 历史生成页可打开质量报告。

## 非目标

- 不新增完整账号管理、账号策略编辑或账号切换能力。
- 不改变 `ContentTopic.status` 生命周期。
- 不新增 `quality_rejected` 状态。
- 不让 AI 自动确认、归档、删除选题或生成脚本。
- 不接入外部热点、竞品抓取、发布数据回流或长期学习模型。
- 不覆盖移动端适配。
