# rank-topic-groups-for-script-production Design

## DDD

主题组仍由原始 `topic_generation_batches.id` 作为 `root_batch_id` 表示，包含原始批次和所有 `supplement_of_batch_id = root_batch_id` 的补充批次。补充批次只提供同主题上下文下的新增选题，不成为历史列表的独立排名对象。

新增主题组脚本优先级读模型，建议命名为 `TopicGroupScriptPriority`。它不是新的生命周期实体，只是基于当前可见选题、主题组批次和最新成功 `TopicReviewSnapshot` 计算出的可解释排序结果。

优先级状态建议稳定为四类：

- `ready_for_script`：已有可立刻进入脚本创作的高优先选题。
- `needs_review`：缺少成功评审，或最新评审已不能覆盖当前可见选题。
- `needs_supplement`：主题方向有潜力，但当前缺少无明显风险的脚本候选。
- `defer`：淘汰、重复、偏离定位或脚本化困难信号占主导，暂不建议推进。

AI 评审快照仍只负责给出选题层级和风险判断。脚本优先级排名由后端确定性计算，排名结果不得自动修改 `ContentTopic.status`。

## BDD

运营人员进入“历史生成”时，左侧主题组默认按脚本产出优先级排序。最上方应优先出现“建议立刻出脚本”的主题组，并在卡片中展示推荐候选数量、优先推荐数量、主要风险和排序理由。

点击高优先主题组后，中列继续展示该主题组的评审分层。优先推荐区域中适合立刻出脚本的候选应更容易被识别，运营可以从这些候选进入脚本生成确认面板。

当某主题组没有评审快照，或快照覆盖的选题 ID 与当前可见选题不一致时，历史列表不得把它排入“建议立刻出脚本”。页面应提示“待评审”或“需重新评审”，并引导操作者触发现有“评审当前主题组”动作。

运营仍可以切换为按时间排序，用于查看最近生成和补充记录。排序切换不改变当前项目、当前选中的主题组、选题状态或脚本生成链路。

## SDD

建议新增主题组 summary API，避免继续让前端从批次列表临时拼装排名：

- `GET /api/projects/:project_id/topic-groups?sort=script_priority|created_at`

返回结构建议：

```json
{
  "topic_groups": [
    {
      "root_batch_id": "uuid",
      "project_id": "uuid",
      "prompt": "如何使用 AI 生成视频并获利",
      "created_at": "2026-07-08T00:00:00Z",
      "topic_count": 15,
      "supplement_batch_count": 2,
      "latest_review_snapshot_id": "uuid",
      "review_freshness": "fresh",
      "script_priority": {
        "status": "ready_for_script",
        "score": 86,
        "reason": "存在 3 个无明显风险的优先推荐选题，脚本化路径清晰。",
        "metrics": {
          "priority_count": 4,
          "backup_count": 5,
          "reject_count": 6,
          "duplicate_count": 1,
          "hard_to_script_count": 0,
          "off_positioning_count": 0,
          "ready_candidate_count": 3,
          "high_score_topic_count": 4
        },
        "recommended_topic_ids": ["uuid"]
      }
    }
  ]
}
```

`review_freshness` 建议取值：

- `fresh`：最新成功评审覆盖当前主题组全部未软删除选题。
- `missing`：不存在成功评审。
- `stale`：成功评审存在，但评审结果与当前未软删除选题集合不一致。

脚本优先级计算规则第一版采用确定性规则：

- 只有 `review_freshness = fresh` 的主题组才允许进入 `ready_for_script`。
- `ready_candidate_count` 来自优先推荐选题，且不得包含 `duplicate`、`hard_to_script`、`off_positioning`、`too_generic`、`compliance_risk` 风险。
- `high_score_topic_count` 默认统计 `score >= 80` 的未软删除选题。
- `reject_count`、`duplicate_count`、`hard_to_script_count`、`off_positioning_count` 和 `compliance_risk` 对分数产生惩罚。
- `recommended_topic_ids` 最多返回 3 个候选，必须属于当前主题组且未软删除，并优先选择 `ready_candidate` 中评分更高的选题。

建议公式保持可测试、可解释：

```text
score =
  ready_candidate_count * 22
+ priority_count * 8
+ high_score_topic_count * 5
+ backup_count * 2
- reject_count * 6
- duplicate_count * 5
- hard_to_script_count * 10
- off_positioning_count * 10
- compliance_risk_count * 15
```

最终分数 clamp 到 `0..100`。若 `review_freshness != fresh`，分数返回 `null`，状态必须为 `needs_review`，不得参与高优先排序。

前端历史生成页：

- 左侧列表顶部增加排序切换：`脚本优先`、`按时间`。
- 默认使用 `脚本优先`。
- 主题组卡片展示状态文案、分数、推荐候选数量和主要风险。
- `needs_review` 和 `stale` 主题组展示“评审当前主题组”引导，但复用已有手动评审动作。
- 当前选中主题组、补充操作和当前选题池联动逻辑保持不变。

## TDD

后端先补失败测试：

- 主题组 summary 按原始批次聚合原始批次和补充批次，不把补充批次作为独立排名对象。
- 有新鲜评审快照时，按脚本优先级计算 `ready_for_script`、分数、指标和推荐候选。
- 无评审快照时返回 `needs_review`，不得排入高优先结果。
- 评审快照缺少当前可见选题或包含已软删除选题时返回 `stale` 或 `needs_review`。
- 风险标记对分数产生稳定惩罚，`hard_to_script`、`off_positioning`、`compliance_risk` 不得进入 ready candidates。
- 排名计算不得修改 `content_topics`、`topic_generation_batches` 或 `topic_review_snapshots`。
- API 只返回当前项目主题组，不返回其他项目数据。

前端先补失败测试：

- 历史生成页默认展示“脚本优先”排序。
- `ready_for_script` 主题组展示在待评审和暂缓主题组之前。
- 主题组卡片展示分数、推荐候选数量和主要风险。
- 切换到按时间排序后按原始批次时间排序。
- 缺少评审或评审过期的主题组展示待评审/需重新评审，不显示为可立刻出脚本。
- 点击主题组后，中列仍展示该主题组评审分层，生成脚本入口保持可用。

## 风险与取舍

- 排名依赖评审快照质量。解决：未评审或过期评审不进入高优先，并保留手动重新评审。
- 确定性公式可能不够精细。解决：第一版先可解释、可测试，后续再通过数据反馈调权重。
- 历史列表信息密度会上升。解决：卡片只展示关键指标，详细原因仍放在主题组评审分层中。
- 新 API 与旧批次 API 可能短期并存。解决：主题组排名只服务历史生成页；补充批次审计和操作仍可复用现有批次数据。

## 原型要求

进入前端实现前必须更新 `docs/prototypes/video-agent/video-agent.pen` 并获得明确确认。原型至少覆盖：

- 历史生成页左侧主题组脚本优先级排序。
- 主题组卡片的状态、分数、推荐候选和风险摘要。
- 排序切换：脚本优先 / 按时间。
- 待评审、需重新评审、建议立刻出脚本、需补充和暂缓状态。
