## Context

已归档的 `projects/episodes` 切片已经实现稳定 UUID、父级归属、状态、revision、Repository/UoW、SQLAlchemy 和 HTTP `If-Match`，但 Project 目前只有名称/状态，Episode 只有编号/标题/状态。阶段一的 text、workbench 和总体 change 已把 `creationMode`、CreativeBrief、项目设置/预算阈值和 accepted Project/Episode handoff 指向 projects owner，却没有 active child 实现这些事实。

本 change 对既有切片做 additive extension。SourceMaterial 的 import/parse/validation/recovery 仍由 text owner 管理，Provider/Model/Skill 默认值由 catalog 管理，Run/BudgetGate 由 workflows/runs 管理；projects 只保存其拥有的创作事实和跨 owner 精确引用。

## Goals / Non-Goals

**Goals:**

- 为 Project 增加 `creationMode`、不可变 CreativeBrief 版本、精确 adaptation source binding、项目创作设置和文本费用确认阈值。
- 以单一 projects UoW 原子落地 accepted StorySpec、Episode 顺序和每集 ScriptSpec reference，并返回可重试的 owner ack。
- 复用既有稳定 UUID、revision、Repository/UoW、camelCase HTTP 和错误 envelope，保持历史版本只读。
- 为 Workbench、text owner、workflows/runs 和 light export 提供 owner-scoped 读取投影，不复制下游事实。

**Non-Goals:**

- 不拥有或实现 SourceMaterial bytes、上传、解析、校验、TextModel、文本 candidate/TextReviewBatch、Scene/Shot、AssetBible、Provider catalog、WorkflowRun、BudgetGate、费用确认、媒体生成、Timeline 或 UI。
- 不在 Project 中保存 Provider/Profile/Model/Skill default/override，也不复制 Provider usage 或 Run 状态。
- 不原地覆盖 CreativeBrief/设置/StorySpec/ScriptSpec 历史，不以数据库 fallback 或前端草稿冒充 owner 事实。

## Decisions

### 1. 在既有 Project 聚合上增加版本指针

`Project` 增加 `creationMode`、`currentCreativeBriefVersionId`、`currentCreativeSettingsVersionId`、可空 `currentStorySpecRef` 和 revision。`CreativeBrief` 使用稳定 `creativeBriefId`，每次保存追加 `CreativeBriefVersion`，包含 `creativeBriefVersionId`、单调 version、projectId、九个 canonical 业务字段、`schema_version`、payloadHash、createdAt 和 actor UUID。Project current pointer 只有带 expected Project/Brief revision 的 command 才能移动。

这样既保持用户所见的“当前简报”，又让已启动 Run 可冻结旧版本。替代方案是把 CreativeBrief JSON 原地放进 Project；它会让历史 Run 和 source binding 无法证明输入，因此不采用。

### 2. 项目设置只定义本阶段确需的预算事实

`ProjectCreativeSettingsVersion` 是不可变版本，MVP-A 的 canonical 字段仅为 `textCostConfirmationThreshold`，其值包含非负 decimal `amount` 和 ISO 4217 `currency`；`null` 表示不设金额阈值，但 `cost=unknown` 仍由全局规则强制确认。Provider/Profile/Model/Skill defaults 和参数 override 继续由 catalog 持有。

catalog 可读取 project owner 的 threshold snapshot；它不得再次持久化第二份阈值。`CostConfirmation` 与 provider-native usage 仍由 catalog 记录，BudgetGate 和 Run 的 `waiting_review` 状态仍由 workflows/runs 记录。该拆分避免 Project 聚合吸收调用账本，也消除现有规划中 catalog 与 projects 同时拥有阈值的冲突。

### 3. adaptation binding 是 projects owner 的不可变引用快照

original 的 current CreativeBriefVersion 不得带 SourceMaterial 字段。adaptation 只有在 text owner 返回同项目、状态有效且 revision/hash 精确匹配的 binding handoff 后，projects 才追加 `CreativeBriefSourceBindingSnapshot`；字段固定为 project/source/brief IDs、source/brief revisions、source content hash、brief payload hash、parse/validation/binding status 和 binding version。

快照不复制 SourceMaterial 正文、StoredObject、AssetVersion 或 parse diagnostics。恢复必须复用相同 source/brief revision；任一字段变化创建新快照并使旧 Run 输入保持不变。替代方案是 projects 自己解析 source，会破坏 owner 边界并引入存储副作用，因此不采用。

### 4. accepted 文本以一个 projects typed batch command 原子落地

`ApplyProjectEpisodeTextHandoff` 只接受 text owner 已 accepted 的 immutable handoff，包含 batch/handoff ID 和 revision、projectId、StorySpec candidate/source IDs/hashes、StorySpec immutable reference、按 number 排序的 Episode stable IDs 与 ScriptSpec immutable references、payloadHash、correlationId、expected Project revision、existing Episode expected revisions 和 `schema_version`。

application 在一个 projects UoW 中校验完整成员集合、项目归属、精确 hash/revision 和 accepted provenance；然后移动 Project StorySpec current reference，创建或更新相同 stable ID 的 Episode，并移动每集 ScriptSpec current reference，同时追加 audit/Outbox 与 `ProjectEpisodeHandoffAck`。任何成员冲突都回滚全部写入。相同 handoff fingerprint 重试返回原 ack；相同 handoff ID 不同 fingerprint 返回 conflict。

Project/Episode ack 只表示本 owner 已落地，不能代替 Scene/Shot 或 AssetBible owner ack。workflows/runs 只有收齐总体合同要求的全部 owner ack 后才能打开付费媒体门。

### 5. HTTP、Schema 与读取投影保持一份版本事实

数据库与共享 JSON Schema 使用 `schema_version`；HTTP 仅通过 alias 映射为 `schemaVersion`。写 API 使用 command body 的 `expectedRevision` 并要求与 `If-Match` 同值；缺失、冲突、foreign scope 返回 422/403/409 且在 UoW 前失败。读取 API 返回 current 和显式历史版本，且 source/Story/Script 只返回 owner references/hashes，不返回正文。

Workbench、text input、Run freeze 和 light manifest 读取同一 owner projection。页面加载、GET、select 或导出投影不得移动 pointer、创建 Episode、启动 Run 或写审计事件。

### 6. DDD / BDD / SDD / TDD

- **DDD**：Project 拥有 creationMode、CreativeBrief、ProjectCreativeSettings 和 StorySpec current reference；Episode 拥有 ScriptSpec current reference。
- **BDD**：用户可保存 original/adaptation 简报、恢复精确版本、修改阈值并在文本接受后看到完整 Episode 列表；冲突没有部分成功。
- **SDD**：稳定 ID、`schema_version`、revision/hash、If-Match/CAS、immutable version 和 typed handoff/ack 是边界合同。
- **TDD**：先写 domain/application 失败测试，再写 repository/HTTP/contract/migration 与跨 owner E2E fixtures。

## Risks / Trade-offs

- [阈值 owner 从既有 catalog 规划迁移会产生重复列] -> 在实施前先修改 catalog contracts，使其只引用 projects snapshot；migration/架构测试禁止 catalog 持久化 threshold。
- [批量 handoff 同时创建多个 Episode 造成部分写入] -> 单一 projects UoW、完整集合预校验和数据库事务；任何冲突回滚且无 ack/Outbox。
- [CreativeBrief 版本和 Project revision 双重并发] -> command 同时校验 Project current pointer 与目标 version/fingerprint，HTTP `If-Match` 只映射 expected Project revision。
- [source owner 暂时不可用] -> projects 不猜测状态；返回 dependency unavailable 并保持 current binding 不变。
- [旧项目缺少新字段] -> migration 使用明确的未配置状态而非伪造业务值，用户首次保存后才形成 current version。

## Migration Plan

1. 先增加共享 Schema、domain/application 失败测试和 catalog owner 泄漏测试。
2. 新建 additive Alembic revision，增加 CreativeBrief/settings/source binding/handoff ack 表与 Project/Episode nullable pointers；先回填结构性默认状态，再添加 FK/唯一/检查约束。
3. 部署 Repository/UoW、commands/queries 和 HTTP adapters；旧 Project/Episode API 保持兼容，新字段只在扩展 DTO 中出现。
4. 使用 SQLite/PostgreSQL 验证 upgrade/downgrade/upgrade、并发 409、批量回滚与幂等重试；失败时回滚新端点和新表/列，不改写旧项目/剧集事实。

## Open Questions

无。MVP-A 项目设置目前只冻结文本费用确认阈值；未来新增创作设置必须先通过独立 OpenSpec 扩展 canonical Schema。
