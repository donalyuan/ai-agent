## 0. 总体计划追溯与实施前置

- [x] 0.1 在开始实现前核验 `plan-phase-one-drama-mvp-a` 总体任务 `1.2`、直接实施任务 `3.4` 与共享任务 `5.1`--`5.5`；记录完整非目标为实现或拥有 Provider Adapter、提示词生成、图片/视频编辑、自动接受、跨项目复制、历史/已发布版本重写，改变 AssetVersion 的 objectKey/hash/append-only/现有 HTTP 契约，以及实现 Timeline/audio/FFmpeg/MP4/SRT/`ProjectPackage`/`exportProfile`/`portable`，并为职责泄漏编写无写入失败测试。
- [x] 0.2 定向取证总体任务 `2.1`、`2.2`、`2.3` 和已归档 AssetVersion 契约已提供所需事实；验证总体 plan 仅为协调关系，不被导入为运行时代码依赖，缺失前置或范围漂移必须显式阻塞。

## 1. Contracts and Domain

- [x] 1.1 定义只允许 image/video target 的 `AssetEditPlan`、Candidate、Decision、Impact 共享 JSON Schema 与正反 contract fixtures；以 `schema_version` 为 canonical 字段，验证 HTTP DTO `schemaVersion` 仅映射同一个值，并覆盖 story/script/audio/TimelineVersion、缺失、冲突和双独立赋值的零写入拒绝。
- [x] 1.1a 定义 Conversation、Message、Turn、用户输入/Agent 回复和 image/video-only `generateAssetEditPlanFromConversation` owner contract；覆盖 session/scope/sequence/role/correlation、刷新恢复、重复发送、pending/failed turn，以及 story/script/audio/TimelineVersion 转交对应 owner 且不创建 Plan。
- [x] 1.1b 冻结完整 image/video AssetVersion 输入 Schema：primary/base/explicit refs 均携带同项目 version ID/revision/hash；mask、图片选区/局部区域、视频/音频时间范围或 segment edit 返回 `unsupported_feature`，并以 contract/application tests 证明在 execution intent、Outbox、ProviderCall、Storage operation 和付费提交前零写入拒绝。
- [x] 1.1c 定义 AssetBible accepted `ResolvedContinuitySnapshot` 与 `ContinuityRevisionTask` owner reference Schema；Session/turn/Plan/execution 冻结 snapshot ID/revision/hash 和必要 GenerationSpec/AssetVersion refs，禁止复制/写入 entry/override 或自行解析 chain。
- [x] 1.2 实现 session/plan/candidate/decision 的领域对象、primary selection/explicit refs 隔离、稳定 target/reference ID + expected revision 的精确集合 CAS、全有或全无、旧/发布/历史只读和 stale 不变量测试。
- [x] 1.3 实现 application commands/queries、Repository/UoW/Outbox ports 与 409/read-only 错误映射测试。
- [x] 1.3a 定义并测试 `executeAssetEditPlan`/execution intent：冻结 plan/base/selection/refs、`runId + nodeRunId + logicalOperation + correlationId`、费用/能力 snapshot、request fingerprint、commit 后 Outbox、取消与 `submission_unknown` 边界；AssetEdit 只交接 intent，不直接调用 Provider。
- [x] 1.3b 接入 AssetBible resolved snapshot/task read port；覆盖 Plan generation、execute、accept 时 incomplete/stale/foreign/hash-revision mismatch 与 pending task，显示 `continuity_stale` 并证明零 Agent/Provider request、execution、Outbox、ProviderCall、Candidate/current/Timeline mutation。

## 2. Persistence and Interfaces

- [x] 2.1 新增可逆 Alembic 与 SQLAlchemy 表、外键、索引、canonical `schema_version`、并发约束和 migration tests，不改写 AssetVersion；证明数据库、共享 Schema 与 DTO 不形成多个版本源。
- [x] 2.2 实现 Repository adapter、impact/stale resolver 和 transaction/Outbox 集成测试。
- [x] 2.3 添加 additive HTTP API、请求/响应 Schema 和错误 envelope contract tests，响应不含媒体 bytes；HTTP `schemaVersion` 必须从 canonical `schema_version` 映射，冲突时 validation 失败且无写入。
- [x] 2.3a 实现/测试 execution status/reconcile 与 Provider result handoff：对 verified immutable AssetVersion 幂等登记一个 `AssetEditCandidate`，区分 failed/unknown/pending_review/rejected/accepted/stale，提供 candidate compare、reject/accept DTO 与稳定错误；覆盖重试不重复收费、取消晚到不改变 current、Provider/AssetEdit candidate 类型混用零写入。

## 3. Acceptance and Verification

- [x] 3.1 编写 BDD：Schema 无效、`schema_version`/`schemaVersion` 缺失或冲突、conversation message/turn sequence、用户输入与 Agent 回复恢复、从完成 turn 生成 image/video Plan、accepted AssetBible snapshot 成功及 continuity stale/pending task 拒绝、story/script/audio/TimelineVersion 零写入拒绝、完整 AssetVersion refs 成功、mask/选区/局部区域/时间范围 `unsupported_feature` 且调用前零副作用、候选保留原版、primary selection/explicit refs 切换无泄漏、显式镜头/场/集/勾选精确 CAS 接受、隐式范围拒绝、跨项目拒绝。
- [x] 3.2 编写并发/只读/409 BDD：基础版本变化、任一 target revision 冲突时全有或全无、旧/已发布/历史只读和重复接受。
- [x] 3.3 执行 domain/application/adapter/HTTP/BDD 的正反定向测试，包含版本字段单一来源、映射冲突无写入、前置缺失和范围漂移拒绝；记录未配置 Provider/媒体依赖及原始错误。
- [x] 3.4 依次运行 `openspec status --change "implement-agent-asset-edit-review" --json`、`openspec instructions apply --change "implement-agent-asset-edit-review" --json`、`openspec validate implement-agent-asset-edit-review --strict --json`、`openspec validate --changes --strict --json`、`pnpm run check` 和 `git diff --check`；全部实现 task 完成前不得勾选本验收项。

## DDD / BDD / SDD / TDD

- **DDD**：1.x 固化聚合与不变量。
- **BDD**：3.x 验证用户可观察行为。
- **SDD**：1.1、2.x 落实 Schema、API、DB、依赖、兼容性和失败契约。
- **TDD**：每项实现先添加失败测试，再完成最小实现。

## Current / Defined / Todo

- **Current**：任务均未实施。
- **Defined**：任务顺序和验收命令已冻结。
- **Todo**：完成所有未勾选任务后才可将此 change 标记为实现完成。

## 4. Accepted Version Handoff

- [x] 4.1 定义 AssetEdit accept 输出复用 verified Provider terminal success 已 append 的同一 immutable AssetVersion 与 accepted/current storyboard-reference eligibility handoff contract；retry/reconcile 返回同一 version/candidate，accept 只追加 AcceptDecision/audit 与 scenes exact CAS，不复制 bytes/object/ref 或追加第二 version；AcceptDecision 与仍被其引用的 AssetVersion 长期 no-GC。
- [x] 4.2 添加 canonical `accept|reject|retake`（legacy/unknown `approve` validation）以及 accept 不创建/删除 Timeline Clip/SoundCue、不追加 AssetVersion、foreign/stale scope rejection、Assets append-only compatibility 和超过诊断窗口/不同 hold/restart/reconcile 后 cleanup/容量维护/GC 拒绝删除、覆盖或静默压缩 AcceptDecision 及仍被引用 AssetVersion 的 tests。
- [x] 4.3 添加 accepted provenance/candidate/id/revision/hash/target 精确 CAS contract，覆盖 status-only、stale、foreign、hash/revision mismatch 不改 current。
