## Why

阶段一虽已在 Scene/Shot 中约定 AssetBible reference 和 `project -> episode -> scene -> shot` 覆盖顺序，但没有 change 拥有角色、造型、场景视觉、道具和视觉风格本身，也没有项目级修改后的影响分析与连续性修订闭环。缺少该聚合会迫使 UI、文本或 Provider 模块复制资产设定，导致跨集连续性无法审计且下游可能被静默改写。

## What Changes

- 新增项目级 `AssetBible` 聚合，以稳定 UUID 管理 Character、Look/Costume、Location/SceneVisual、Prop、VisualStyle 等条目及其不可变版本；更新只创建 successor revision，不覆盖历史。
- 固定 `project -> episode -> scene -> shot` 显式 override chain，并生成带来源 revision/hash 的 resolved continuity snapshot，拒绝循环、跨项目引用、未知条目和过期 revision。
- AssetBible entry 只引用 AssetVersion、GenerationSpec/提示词规格及 owner provenance，不保存媒体 bytes，也不复制提示词正文或 AssetVersion 元数据形成第二事实源。
- 为项目级条目修改提供只读影响分析，返回精确受影响 Episode/Scene/Shot 引用集合；用户明确接受后才创建 `ContinuityRevisionTask`、successor/stale 标记和审计记录，绝不静默重生成、替换 current 或改写历史版本。
- 增加 TextReview/Project/Scene/Shot、GPT Image、Agent context 与 AssetVersion 的 owner-safe typed handoff/projection；本 change 不直接调用 Provider，不拥有媒体生成、AssetVersion、Scene/Shot、TextReview 或 WorkflowRun。
- 补充共享 Schema、migration、Repository/UoW、camelCase HTTP、审计/Outbox、BDD/TDD 和 E2E 连续性证据。

## Capabilities

### New Capabilities

- `asset-bible-continuity`: 项目级 AssetBible、稳定条目与不可变版本、分层 override 解析、影响分析和显式 ContinuityRevisionTask。

### Modified Capabilities

- 无。

## Impact

后续实现将影响 `packages/contracts`、`services/api` 的新 AssetBible domain/application/repository/HTTP、Alembic、审计/Outbox、BDD/契约测试，以及 Workbench/Review 对只读连续性投影的消费。它通过公开 ID/revision/hash 与 Project、Scene/Shot、AssetVersion、TextReview、GPT Image 和 Agent change 集成，不向这些 owner 写入私有模型。

## 与总体计划的追溯与边界

- 本 change 是 `plan-phase-one-drama-mvp-a` 总体任务 `2.1a` 的 AssetBible/连续性 owner child；总体 change 只负责排序和验收。
- **DDD**：AssetBible owner 管理条目身份、版本、override、resolved snapshot、影响分析和 ContinuityRevisionTask；媒体、文本、镜头与运行状态仍归各自 owner。
- **BDD**：用户可以查看项目资产设定、预览一次修改影响的精确范围并显式接受；取消、stale、foreign 或冲突请求不得改变下游 current。
- **SDD**：引用使用稳定 ID、`schema_version`、revision/hash、scope 与 expectedRevision；接受采用 CAS，stale 传播只记录事实，不触发隐藏副作用。
- **TDD**：先覆盖解析优先级、循环/跨项目/过期拒绝、影响集合确定性、接受原子性/幂等性、历史不可变与零 Provider 副作用，再实现最小闭环。
