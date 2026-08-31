## 1. OpenSpec 与合同

- [x] 1.0 建立对 `plan-phase-one-drama-mvp-a` 总体任务 `2.1` 及共享任务 `5.1`、`5.3`、`5.5` 的追溯测试；核验直接实施依赖仅为已归档的 Project/Episode、AssetVersion/objectKey 契约及其测试。完整非目标是拥有或实现 WorkflowDraft/WorkflowVersion/WorkflowRun/NodeRun/RunEvent、Temporal、Timeline/TimelineDocument、音频/导出/媒体渲染、Provider 调用、AgentScope、文本或素材生成及前端交付，storyboard insert/copy、Scene split/merge、Shot 跨场 move、批量编辑，以及原地修改已发布版本、隐式“整集”范围、总体协调 change 或未实现下游能力的运行时依赖/fallback；证明 workflow-scope 不创建或启动 Run。
- [x] 1.1 以 DDD/BDD/SDD/TDD 补充项目级 StorySpec、每集 ScriptSpec、稳定 Scene/Shot、版本化 SceneSpec/ShotSpec、AssetBible owner resolved snapshot reference、双视图 API 和失败场景的共享 Schema/合同测试；固定 canonical `schema_version` 到 HTTP `schemaVersion` 的同值映射，并覆盖缺失、冲突或双独立赋值时无 Scene/Shot/审计/Outbox 写入。
- [x] 1.2 为 domain/application/adapter/interfaces 建立架构依赖测试，确认不存在 Workflow、Timeline、Provider 或 AgentScope 反向依赖。

## 2. 领域与应用

- [x] 2.1 先写 Scene/Shot 聚合、排序、连续性和不可变引用的 domain 失败测试，再实现 entities、value objects 与稳定错误。
- [x] 2.2 先写完整 accepted `TextReviewBatch` handoff 的 Scene/Shot owner typed batch/orchestration apply/read、candidate/source hash、payload hash、expected revision、idempotent owner ack 与媒体门失败测试，再实现 Commands、Queries、Repository/UoW/Outbox ports；两个 reorder 只接受一个明确父 scope 的完整成员顺序，不能新增、删除或改变归属。
- [x] 2.3 覆盖跨项目/跨集/跨父 scope、重复或缺失 ID、发布/归档、旧 revision、隐式范围、无效 AssetBible 覆盖和 stale 下游不自动替换；覆盖 storyboard insert/copy、Scene split/merge、Shot 跨场 move、批量编辑不存在或返回 `unsupported_feature` 且零写入。
- [x] 2.4 接入 AssetBible resolved snapshot/task read port，覆盖 accepted/incomplete/stale/foreign/hash mismatch、ContinuityRevisionTask projection 和显式 successor 前旧 ShotSpec/current media/Timeline 不变；禁止 scenes 自行解析/复制 entry/override。

## 3. 持久化与接口

- [x] 3.1 先写 SQLAlchemy Repository 与并发顺序测试，再实现映射、锁定及同事务审计/Outbox。
- [x] 3.2 新增并验证可逆 Alembic migration，回填既有 display number，并加入归属、连续排序、外键和引用约束。
- [x] 3.3 先写 HTTP/BDD/contract 失败测试，再实现 camelCase Scene/Shot CRUD、编辑 Commands、storyboard 和 workflow-scope API；HTTP `schemaVersion` 只映射 canonical `schema_version`，不得形成第二个版本事实。

## 4. 验证

- [x] 4.1 运行定向 domain（Scene/Shot 聚合、同父 scope 排序、连续性、不可变引用）、application（accepted batch handoff typed apply/read、hash/payload/revision/owner-ack、`ReorderScenes`、`ReorderShots`、MVP-B 结构编辑拒绝）、adapter（SQLAlchemy 锁定/Outbox）、HTTP/contract/BDD（双视图、越权、`schema_version`/`schemaVersion` 同值映射及冲突无写入）、migration 测试及 SQLite/PostgreSQL migration cycle。
- [x] 4.2 运行 `openspec instructions apply --change "implement-scenes-shots-storyboard-slice" --json`、`openspec status --change "implement-scenes-shots-storyboard-slice" --json`、`openspec validate "implement-scenes-shots-storyboard-slice" --strict --json`、`openspec validate --changes --strict --json`、`pnpm run check` 与 `git diff --check -- openspec/changes/implement-scenes-shots-storyboard-slice`。

## 5. Accepted Storyboard Eligibility

- [x] 5.1 定义 storyboard/reference projection 的 accepted/current AssetVersion eligibility facts，记录 TextReview/媒体审核或 AssetEdit accept provenance、project/Episode target 与 immutable version。
- [x] 5.2 添加 unaccepted/status-only/foreign/other-Episode/non-current candidate 的拒绝 fixtures，证明 projection 不写 Timeline 或 AssetVersion。
- [x] 5.3 覆盖 GPT Image 未引用 candidate -> accepted provenance -> current storyboard reference 的精确 CAS、hash/revision mismatch 与零副作用测试；同步 proposal/design/spec 追溯。
- [x] 5.3a 定义并测试 video current eligibility read/accept/reject DTO：Provider terminal result candidate 后仅 `accept|reject|retake`，accept 只一次绑定 `VideoTakeCandidate` revision、ShotSpec/duration/aspect、candidate/provenance/AssetVersion id/revision/hash/project/episode/shot exact CAS，之后 MediaInspect/derivatives 再 Timeline handoff；legacy/unknown `approve`、reject/retake/cancelled-late/foreign/stale 保持 current 不变，derivative-not-ready 只阻断 Timeline/preview/export，且均不产生重复 AssetVersion/Timeline/Export/Provider 副作用。
