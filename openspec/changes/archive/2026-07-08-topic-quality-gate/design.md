# topic-quality-gate Design

## DDD

新增 `TopicQualityGate` 概念，表示选题候选入库前的质量控制规则。它服务于 `topic` Agent 生成链路，不拥有选题生命周期，也不替代主题组评审。

新增 `TopicQualityEvaluation`，表示某个生成批次的一次质量闸门快照。它关联 `topic_generation_batches.id`，记录本批候选的通过项、淘汰项、质量原因和是否触发自动重写。

质量维度固定为：

- 账号匹配度：是否贴合项目定位和描述。
- 具体度：是否避免百科式、泛化标题。
- 差异化：是否避免同批或同主题组已有选题重复。
- 脚本化可行性：是否适合短视频结构化表达。
- 风险与禁区：是否存在合规风险或明显偏题。
- 评分可信度：评分与理由是否一致，是否虚高。

`ContentTopic.status` 继续保持 `idea -> approved -> scripted -> archived`。质量淘汰不是业务状态；淘汰候选不写入 `content_topics`，只保存在质量评估报告中。

## BDD

运营人员提交选题生成要求后，系统先生成候选，但不立即入库。质量闸门评估每条候选，若首轮通过率低于阈值，则带着质量问题自动重写一次。

重写后，系统再次评估候选。通过的候选进入当前选题池，状态仍为 `idea`；淘汰的候选只在质量报告中只读展示，不支持确认选题、生成脚本或归档。

Agent 回复需要直接说明本批质量结果：通过数量、淘汰数量、是否触发重写，以及主要淘汰原因。历史生成页展示批次质量摘要，右侧质量报告展示被淘汰候选和原因。

补充批次也必须经过同一质量闸门，并把同主题组已有选题作为重复检测上下文。

## SDD

新增表：

```sql
CREATE TABLE topic_quality_evaluations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    batch_id UUID NOT NULL REFERENCES topic_generation_batches(id) ON DELETE CASCADE,
    source_run_id UUID REFERENCES agent_runs(id) ON DELETE SET NULL,
    status VARCHAR(20) NOT NULL,
    pass_count INT NOT NULL DEFAULT 0,
    reject_count INT NOT NULL DEFAULT 0,
    rewrite_triggered BOOLEAN NOT NULL DEFAULT FALSE,
    result JSONB NOT NULL DEFAULT '{}'::jsonb,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

建议索引：

- `(project_id, batch_id, created_at DESC, id DESC)` 读取批次最新质量报告。
- `source_run_id` 非空索引用于运行追踪。
- `status` 用于排查失败评估。

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

质量 flags 固定为：

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

接口只返回当前项目可访问批次的最新质量评估。第一版不新增独立手动触发质量评估接口，质量闸门只挂在选题 Agent 生成链路中。

阈值策略第一版固定在后端：

- 单条候选 `quality_score >= 70` 才可入库。
- 出现 `off_positioning` 或 `compliance_risk` 直接淘汰。
- 出现 `duplicate` 默认淘汰，除非评估明确说明可差异化补充。
- 首轮通过率 `< 60%` 触发最多一次自动重写。
- 自动重写最多 1 次，避免成本失控。

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

后端先补失败测试：

- migration 创建 `topic_quality_evaluations`、索引和约束。
- repository 创建、读取最新质量评估，并按项目隔离。
- 选题 Agent 正常生成时完成“生成 -> 评估 -> 入库通过项 -> 写质量快照”。
- 泛化、重复、偏离定位、难脚本化、合规风险和评分虚高被识别为质量 flags。
- 首轮低质过多触发最多一次重写。
- 质量评估失败不写入任何 `content_topics`。
- 补充批次评估包含同主题组已有选题上下文。
- `GET /api/topic-generation-batches/:batch_id/quality-evaluation` 只返回同项目数据。

前端先补失败测试：

- Agent 消息展示质量结果。
- 选题卡展示质量分和风险标签。
- 历史生成页展示批次质量摘要。
- 质量报告展示淘汰候选且不提供确认、生成脚本或归档操作。

E2E：

- 生成一批选题后，页面可看到通过、淘汰和是否重写。
- 只有通过候选出现在当前选题池。
- 历史生成页可打开质量报告。

## 风险与取舍

- 质量闸门可能误杀创意选题。解决：淘汰候选保留在质量报告中，后续可基于人工反馈优化阈值。
- 额外 LLM 评估和重写会增加成本。解决：第一版最多重写一次，且只有低通过率触发。
- 质量报告结构可能与主题组评审重复。解决：质量闸门负责入库前筛除低质候选，主题组评审负责入库后的运营分层。
- 账号策略信息不足会限制质量判断。解决：第一版使用现有 `projects.positioning` 和 `projects.description`，账号管理另起 change。

## 原型要求

进入前端实现前必须更新 `docs/prototypes/video-agent/video-agent.pen` 并获得明确确认。原型至少覆盖：

- 当前选题池选题卡的质量分和风险标签。
- Agent 生成后的质量摘要消息。
- 历史生成批次卡的质量摘要。
- 历史生成页右侧质量报告。
- 淘汰候选只读展示状态。
