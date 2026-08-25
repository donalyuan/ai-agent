## Why

阶段 0 的 Project/Episode 切片只保存基础身份、状态与 revision，尚没有阶段一从 CreativeBrief 到文本生成所需的项目级创作事实，也没有 TextReview 接受结果落入 Project/Episode owner 的正式边界。若继续仅由 Workbench 或 Workflow 保存这些数据，创作输入、预算阈值和 accepted StorySpec/ScriptSpec 将出现多个事实源，无法形成可恢复、可审计的 MVP-A 闭环。

## What Changes

- 以 additive extension 扩展已归档的 Project/Episode 能力：Project 保存 `creationMode=original|adaptation`、项目创作设置、文本费用确认阈值，以及不可变、版本化的 `CreativeBriefVersion`。
- 固定 CreativeBrief 字段为 `subject`、`genre`、`audience`、`characterPremise`、`style`、`episodeDurationSeconds`、`episodeCount`、`scenesPerEpisode`、`shotsPerScene`、`schema_version` 和 revision；更新创建 successor version，不原地覆盖历史内容。
- adaptation 模式保存精确、不可变的 `CreativeBriefSourceBindingSnapshot`；original 模式不得伪造 SourceMaterial 引用。本 change 只消费 SourceMaterial owner 已验证的绑定事实，不拥有上传、解析或恢复。
- 新增 Project/Episode 对 TextReview accepted handoff 的 typed batch/orchestration command 与幂等 ack：Project owner 接收项目级 StorySpec reference，Episode owner 接收每集 ScriptSpec reference；全部使用稳定 UUID、精确 candidate/source hash、expected revision 和同一事务 CAS。
- 明确 Project 只拥有创作设置和项目级预算阈值；Provider/Profile/Model/Skill 默认绑定与参数覆盖仍由 catalog 拥有，运行时 BudgetGate、费用确认和 Run 状态仍由 workflows/runs 拥有。
- 补充共享 Schema、migration、Repository/UoW、camelCase HTTP、审计/Outbox、BDD/TDD 与 E2E owner handoff 证据，不改变既有 Project/Episode API 的已实现语义。

## Capabilities

### New Capabilities

- `projects-episodes-creative-configuration`: Project/Episode 的创作模式、CreativeBrief 版本、SourceMaterial 绑定快照、项目创作设置/预算阈值和 accepted 文本 handoff/ack。

### Modified Capabilities

- 无。

## Impact

后续实现将影响 `packages/contracts`、`services/api` 的 projects domain/application/repository/HTTP、Alembic、审计/Outbox、BDD/契约测试和 Workbench 的 owner API 消费。它依赖已归档的 Project/Episode 切片，并只引用 SourceMaterial、TextReview 和 Workflow owner 的公开合同；不拥有 SourceMaterial parse、TextModel、WorkflowRun、Provider catalog、Scene/Shot、AssetBible、媒体任务或 UI。

## 与总体计划的追溯与边界

- 本 change 是 `plan-phase-one-drama-mvp-a` 总体任务 `2.0` 的 Project 创作配置 owner child；总体 change 只协调依赖和验收，不是运行时模块。
- **DDD**：Project 是 creationMode、CreativeBrief、项目创作设置和项目预算阈值的唯一 owner；Episode 是 accepted ScriptSpec reference 的 owner。
- **BDD**：original/adaptation 均可保存、恢复并显式发起生成；stale/foreign/revision mismatch 的输入返回稳定错误且零写入，accepted handoff 重试只返回原 ack。
- **SDD**：所有版本、快照和 handoff 均带 `schema_version`、稳定 ID、revision/hash 与项目范围；HTTP 使用 expectedRevision/`If-Match`，冲突返回 409。
- **TDD**：先覆盖 original/adaptation、并发冲突、exact source binding、不可变历史、批量 handoff 原子性/幂等性和 owner 泄漏失败，再实现最小闭环。
