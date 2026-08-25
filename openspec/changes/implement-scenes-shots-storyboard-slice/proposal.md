## Why

## scenes current contract

scenes 只在 Provider terminal result candidate 经人工以 exact candidate/source/ShotSpec facts `accept` 后执行一次 exact current CAS；随后 MediaInspect/derivatives 才能进入 Timeline handoff。derivative pending/failed/stale 仅阻断 Timeline/preview/export，不能阻断或撤销 accepted/current。review DTO/action/audit event 只用 `accept|reject|retake`，legacy/unknown `approve` validation 且零 current/retake side effect。

AssetEdit `accept` 只接收已存在的同一 AssetVersion，追加 AcceptDecision/audit 和同一 version 的 scenes eligibility/current CAS；它不得复制 bytes/object/ref 或追加第二 AssetVersion。reject、stale 或 foreign accept 均零 AssetVersion/current/Timeline mutation。

阶段 0 已提供 Project/Episode、AssetVersion 及 Scene/Shot 的基础 Schema/ORM 占位，但尚不存在可审计的场景与镜头聚合、排序编辑或故事板操作。该切片建立后续文本生成、素材审阅和时间线可以引用的版本化叙事事实源。

## What Changes

- 新增 Project/Episode 下的 `Scene` 聚合及其 `Shot` 子实体，包含稳定 UUID、`schema_version`、revision、状态、显示编号和连续性引用。
- 新增只由完整 accepted `TextReviewBatch` owner handoff 驱动的 Scene/Shot 落地、读取、列表，以及 Scene 在同一 Episode 内、Shot 在同一 Scene 内的重排 Command/Query；handoff 必含 candidate/source hashes、payload hash、expected revisions 和 correlation，Project/Episode/Scene/Shot 各 owner 以 typed batch/orchestration command 幂等返回 ack 后才解除媒体门。每次写入使用一个 UoW、显式并发 revision 和同事务审计/Outbox。
- 新增项目级 StorySpec、每集 ScriptSpec、SceneSpec/版本化 ShotSpec 与 AssetBible resolved snapshot reference 的不可变引用模型；Scene/Shot 不拥有 AssetBible entry/override/resolver，只消费 `implement-asset-bible-continuity-slice` 返回的稳定 snapshot ID/revision/hash。Scene/Shot 使用稳定 ID，拒绝跨项目、跨集、旧 revision 和隐式范围。
- 明确 MVP-A 不提供 storyboard insert/copy、Scene split/merge、Shot 跨场 move 或批量编辑；这些能力留到 MVP-B，MVP-A 请求不得产生 Scene/Shot、审计或 Outbox 写入。
- 增加 Scene/Shot 排序、归属和版本约束的持久化迁移，以及 camelCase HTTP API；数据库与共享 Schema 的 `schema_version` 是唯一版本事实，HTTP DTO 的 `schemaVersion` 只映射同一值，缺失或冲突时不得写入。
- 提供同一版本化 Scene/Shot 事实的 storyboard 视图与 workflow scope 视图 API；不启动或执行 Workflow。

## Capabilities

### New Capabilities
- `scenes-shots-storyboard`: 版本化 Scene/Shot 聚合、连续性引用、编辑命令和双视图 API。

### Modified Capabilities

- 无。

## Impact

后续实现将影响 `packages/contracts`、`services/api` 的 domain/application/adapters/interfaces、Alembic、BDD/契约测试和 Web 工作台数据访问。保持既有 Project/Episode、AssetVersion、`Mock Provider +` 显式 Local test/offline profile 和 HTTP 错误语义兼容；不修改 Timeline、Provider 或 AgentScope 边界。

## 与总体计划的追溯与边界

- 本 change 落实 `plan-phase-one-drama-mvp-a` 的总体任务 **2.1**，并受共享工程任务 **5.1**、**5.3**、**5.5** 约束。
- 直接实施依赖是已归档的 Project/Episode、AssetVersion/objectKey 契约及其可验证测试；`WorkflowDraft`、`WorkflowRun`、Provider catalog 与文本 skills 均不是本 change 的实施前置条件。
- `plan-phase-one-drama-mvp-a` 是 OpenSpec 的协调、排序和验收依据，不是任何 domain、application、adapter、worker 或 HTTP 组件的运行时代码依赖。
- 本 change 的完整非目标是拥有或实现 WorkflowDraft/WorkflowVersion/WorkflowRun/NodeRun/RunEvent、Temporal、Timeline/TimelineDocument、音频/导出/媒体渲染、Provider 调用、AgentScope、文本或素材生成及前端交付，以及 storyboard insert/copy、Scene split/merge、Shot 跨场 move 或批量编辑；也不原地修改已发布版本、不以隐式“整集”替代显式 Command 范围，且不把总体协调 change 或未实现的下游能力当作运行时依赖或 fallback。workflow-scope 只暴露可调度事实，不承担运行编排。

## Timeline eligibility 投影

本 change 增加 storyboard/reference projection 的 accepted/current eligibility facts，供 Timeline owner 判断 image/video immutable AssetVersion 是否可用；它不以 `AssetVersion.status` 代替审核/accept 来源，不写 Timeline 或覆盖 AssetVersion。

## 精确接受合同

**DDD**：scenes/storyboard eligibility 是 accepted/current 的唯一 owner。**BDD**：未接受、stale、foreign、hash/revision mismatch 不可见为 current。**SDD**：接受 CAS 必含 accepted provenance、candidateId、AssetVersion id/revision/hash、target/project/episode。**TDD**：覆盖所有 mismatch 的 projection 不变与零下游引用。
