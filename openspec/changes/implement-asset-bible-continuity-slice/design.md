## Context

Scene/Shot change 已定义 `project -> episode -> scene -> shot` 的 AssetBible reference/override chain，但只消费引用，并不拥有角色、造型、场景视觉、道具或视觉风格。图片生成、Agent 和 Workbench 也需要相同的 resolved continuity snapshot。当前没有聚合负责这些条目的稳定身份、不可变版本、影响分析或修改后的修订任务。

AssetBible 是项目级创作事实，不是媒体库，也不是 Provider prompt cache。本 change 在模块化单体中新增独立 domain/application/repository/HTTP 边界，并只用稳定引用与 Project、Episode/Scene/Shot、AssetVersion、TextReview、GPT Image 和 Agent 集成。

## Goals / Non-Goals

**Goals:**

- 建立项目级 AssetBible、稳定 entry 身份和不可变版本，覆盖 Character、Look/Costume、Location/SceneVisual、Prop、VisualStyle。
- 确定性解析 project/episode/scene/shot override，并冻结可验证的 resolved continuity snapshot/hash。
- 在修改 current entry 前提供精确影响分析；仅在用户显式接受后创建 successor、stale facts 和 ContinuityRevisionTask。
- 提供 owner-safe handoff/projection，使文本、镜头、图片和 Agent 消费同一连续性事实。

**Non-Goals:**

- 不生成图片/视频，不调用 Provider/AgentScope/TextModel，不拥有 GenerationSpec/提示词正文、AssetVersion、Scene/Shot、TextReviewBatch、WorkflowRun、Timeline 或媒体 bytes。
- 不自动重生成或替换受影响的 Story/Script/ShotSpec/AssetVersion/current reference，也不提供物理删除/GC。
- 不实现人物关系图、自动视觉 QC、语义搜索、多人审批或跨项目共享 AssetBible。

## Decisions

### 1. AssetBible 为项目级聚合，entry 身份与版本分离

每个 Project 最多一个稳定 `AssetBible` identity。`AssetBibleEntry` 使用稳定 UUID 和 discriminated `entryType=character|look|location|scene_visual|prop|visual_style`；内容变化追加 `AssetBibleEntryVersion`，包含 version ID、entry ID/type、projectId、单调 version、structured attributes、canonical payload hash、`schema_version`、revision、createdAt 和 actor UUID。聚合 current map 只保存 entryId -> currentVersionId。

Look 必须引用同项目 Character；SceneVisual 必须引用同项目 Location；其他关系使用 typed reference 并拒绝循环。删除在 MVP-A 表示 disable/supersede，不物理删除被历史引用的 entry/version。选择 identity/version 分离，是为了让 ShotSpec、Run 和历史媒体继续解析原版本。

### 2. 媒体与生成规格只保存引用

entry version 可保存 `referenceAssetVersionRefs[]` 和 `generationSpecRefs[]`，每项均包含 owner ID/revision/hash 与用途；不得保存媒体 bytes、objectKey、永久 URL、提示词正文或复制 AssetVersion metadata。读取时按调用方权限从相应 owner 获取投影；owner unavailable 返回 partial/unavailable，不把缺失解释为空。

这避免 AssetBible 成为第二资产库或第二提示词事实源。替代方案是嵌入完整 prompt/asset 文档，虽然读取方便，但会破坏版本与授权边界，因此不采用。

### 3. override 使用显式 assignment 和确定性优先级

`AssetBibleOverrideAssignment` 必须绑定 projectId、scopeType、scopeId、entryId、entryVersionId、expected scope/entry revision 和 assignment revision。合法 scope 依次为 project、episode、scene、shot；解析从 project 向下应用，最具体 scope 胜出，但每层来源都保留在 chain 中。

resolver 校验所有 scope 同项目、Episode/Scene/Shot 归属、entry 类型兼容和 version 未被禁用，并产出 immutable `ResolvedContinuitySnapshot`：target scope、按 entry/role 排序的完整 override chain、resolved version references、各 source revisions 和 canonical hash。ShotSpec、Agent session 和 Run 只冻结 snapshot ID/hash，不复制内容。相同输入必须产生相同排序与 hash。

### 4. 影响分析先于 current pointer 变更

`PreviewAssetBibleRevisionImpact` 接受 base entry version、candidate successor payload、expected AssetBible/entry revision 和语义 scope，建立 immutable `ContinuityImpactAnalysis`。它通过 owner query ports 收集直接或经 resolved snapshot 引用该 version 的精确 Episode/Scene/Shot IDs/revisions，并为每个 target 给出 reason、当前 snapshot/hash 和建议动作；无法证明完整性时状态为 `incomplete`，不能接受。

preview 是只读操作，不创建 successor、stale 或任务。选择先分析后接受，确保用户能在付费或大范围改动前看到实际影响；不采用保存后再扫描，因为那会使拒绝/超时留下半完成 current。

### 5. 显式接受原子创建 successor 与 ContinuityRevisionTask

`AcceptAssetBibleRevision` 必须提交 analysis ID/revision/hash、candidate payload hash、完整 resolved target reference set、expected AssetBible/entry/scope revisions、稳定 actor UUID 和语义范围。application 在一个 UoW 中重新计算/核对影响集合，追加 entry successor version、移动 AssetBible current pointer、追加 AcceptDecision/audit/Outbox，并为每个受影响 target 创建或去重 `ContinuityRevisionTask`。

任务保存 target type/ID/revision、old/new entry versions、old snapshot/hash、reason、状态 `pending|acknowledged|resolved|superseded` 和 owner handoff correlation。它只要求对应 owner 重新评估；不能直接改写 Scene/Shot/ShotSpec、Text candidate、GenerationSpec 或媒体。任一 expected revision 过期则整个接受返回 409，零 successor/pointer/task 写入。相同 fingerprint 重试返回原决定和任务集合。

### 6. 跨 owner handoff 只传精确引用和 ack

TextReview accepted handoff 可请求创建/引用初始 entry versions，但 AssetBible owner 必须以自己的 typed command 校验项目、stable IDs、hash/revision 后落地并返回 ack。Scene/Shot owner 只提交/读取 assignment 与 resolved snapshot reference；GPT Image 和 Agent 只读消费 accepted snapshot、GenerationSpec refs 和 AssetVersion refs。任何 consumer 不得直接更新 AssetBible 表或将 Provider result 自动设为 reference/current。

### 7. HTTP、持久化和测试边界

共享 Schema/DB 使用 `schema_version`，HTTP 使用同值 `schemaVersion` alias。commands 同时使用 `expectedRevision` 和 `If-Match`；跨项目、循环、类型不兼容、incomplete impact、stale set/hash 或 unknown field 在 UoW/Provider 前失败。列表与 resolved queries 支持 project/scope/type/status filter，但页面读取不触发 resolve persistence、impact task 或 Provider 调用。

### 8. DDD / BDD / SDD / TDD

- **DDD**：AssetBible owner 管理条目、版本、assignment、resolved snapshot、impact analysis 和 ContinuityRevisionTask。
- **BDD**：创作者可查看/修改设定、预览受影响集/场/镜头并显式接受；下游只出现待修订，不被自动改写。
- **SDD**：typed entries、稳定 ID、immutable version、CAS、canonical hash 和 owner reference 是唯一交换合同。
- **TDD**：先写解析/影响/接受的领域失败测试，再覆盖 repository/HTTP/migration/contract 和 E2E owner handoff。

## Risks / Trade-offs

- [影响集合依赖其他 owner read model] -> 保存 query watermark/revisions；任一 owner unavailable 或集合无法证明完整时标记 `incomplete` 并禁止接受。
- [大量 Shot 引用使同步分析变慢] -> MVP-A 使用索引化 reference projection 和分页读取，接受前仍核对 canonical set hash；不以异步最终一致替代原子决定。
- [override 组合产生循环或歧义] -> typed relationship、固定 scope priority、拓扑/循环校验和确定性排序；歧义返回 validation，不任意选取。
- [任务创建后下游长期未处理] -> 任务是显式、可查询的领域事实；媒体门按 consumer contract 阻断 stale target，但本 change 不擅自修复。
- [AssetVersion/GenerationSpec owner 暂时不可用] -> 保留 reference 与 partial diagnostic，不复制内容或把 unavailable 写成 deleted。

## Migration Plan

1. 先定义共享 schemas、reference contracts 和 domain failure tests。
2. 新建 additive Alembic revision，增加 AssetBible、entry/version、assignment、snapshot、impact analysis、revision task、audit/outbox 表及归属/唯一/hash 约束。
3. 部署 resolver、Repository/UoW、commands/queries 和 HTTP；先由 projects/text/scenes 的 fixtures 验证 handoff，不接真实 Provider。
4. 执行 SQLite/PostgreSQL upgrade/downgrade/upgrade、并发接受、完整影响集合和历史引用回归；回滚只移除新接口/表，不改写 AssetVersion 或 Scene/Shot。

## Open Questions

无。MVP-A entry types 与 override scope 已冻结；新增 entry type、跨项目共享或自动修复属于后续 change。
