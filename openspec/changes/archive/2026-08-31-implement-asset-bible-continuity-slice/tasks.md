## 1. 合同与领域模型（DDD / SDD）

- [x] 1.1 实施前重新核对 Project、Scene/Shot、AssetVersion、TextReview、GenerationSpec、Agent 和 Provider 当前代码/Schema/Alembic head，冻结 owner reference 与明确非目标。
- [x] 1.2 先为 AssetBible、六类 typed entry/version、relationship、assignment、resolved snapshot、impact analysis、AcceptDecision 和 ContinuityRevisionTask 编写 Draft 2020-12 Schema 正反 corpus。
- [x] 1.3 先写 stable identity/immutable version tests，再实现 AssetBible 聚合、entry/version、current map、disable/supersede 和被引用版本 no-delete 规则。
- [x] 1.4 先写 Character/Look、Location/SceneVisual 关系及 cross-project/cycle/type mismatch 失败测试，再实现 typed relationship validation。
- [x] 1.5 先写 AssetVersion/GenerationSpec reference-only tests，再实现 ID/revision/hash/用途引用；拒绝 bytes、objectKey、永久 URL、提示词正文和 metadata 复制。

## 2. Override、解析与影响分析（DDD / TDD）

- [x] 2.1 先写 project->episode->scene->shot priority、same-input same-hash 和歧义/归属失败测试，再实现 assignment domain 与 deterministic resolver。
- [x] 2.2 实现 immutable `ResolvedContinuitySnapshot`、canonical sorting/hash 和 source revision chain；ShotSpec/Agent/Run fixtures 只冻结 snapshot ID/hash。
- [x] 2.3 定义 owner query ports 和 indexed reference projection，先覆盖 owner unavailable、分页不完整、revision drift，再实现 `PreviewAssetBibleRevisionImpact` 与 stable target set hash。
- [x] 2.4 证明 impact preview 不创建 successor/stale/task/ProviderCall/媒体副作用，incomplete analysis 不可接受且保留原始 diagnostic。

## 3. 接受、任务与跨 owner handoff（DDD / BDD / TDD）

- [x] 3.1 先写完整 target set CAS、任一 stale 全批失败与 idempotent retry tests，再实现 `AcceptAssetBibleRevision` 的单 UoW successor/current/AcceptDecision/audit/Outbox。
- [x] 3.2 实现按 target 去重的 `ContinuityRevisionTask` 和 `pending|acknowledged|resolved|superseded` 状态机；禁止任务直接改写任何下游 owner。
- [x] 3.3 实现 TextReview initial AssetBible typed handoff/idempotent ack fixtures，覆盖同项目 stable IDs/hash/revision、duplicate/conflict 和全部 owner ack 前媒体门关闭。
- [x] 3.4 实现 Scene/Shot assignment/resolved reference 与 GPT Image/Agent/Run read projections；架构测试拒绝 consumer 直接写 AssetBible 或自动绑定 Provider result。
- [x] 3.5 覆盖 entry successor 后旧 ShotSpec/AssetVersion/current/Timeline 仍引用旧 snapshot、只出现 pending revision task 且零自动重生成。

## 4. Repository、迁移与 HTTP（SDD / TDD）

- [x] 4.1 定义 Repository/UoW/Outbox ports 和内存 adapter，提供 entry/version/assignment/snapshot/analysis/task commands/queries，不向 domain/application 泄漏 ORM/FastAPI。
- [x] 4.2 先写 SQLite/PostgreSQL migration fixtures，再增加 additive Alembic revision、SQLAlchemy mappings、FK/唯一/type/hash/project-scope constraints，并验证 upgrade/downgrade/upgrade。
- [x] 4.3 先写 HTTP/contract 失败测试，再实现 project-scoped camelCase entry/version/assignment/resolution/impact/accept/task APIs 和 `If-Match`/`expectedRevision` 同值校验。
- [x] 4.4 覆盖 403/409/422/503、schema alias 冲突、foreign/stale/incomplete/set mismatch 和并发 accept，证明失败零版本/current/task/audit/Outbox 写入。

## 5. 用户闭环与交付证据（BDD / TDD）

- [x] 5.1 为 Workbench/Review 增加 AssetBible entry、resolved chain、影响清单、显式接受与 revision task owner projection fixtures；UI 只读 projection 不产生 resolve/task/Provider mutation。
- [x] 5.2 将 2x2x3 Character/Look/Prop 跨集连续性场景接入 `E2E-MVPA-001`，记录 owner ack、exact target set/hash、accept/task、409 与 no-auto-regeneration evidence。
- [x] 5.3 运行 domain/application/repository/HTTP/contract/architecture/migration suites、focused E2E、全量项目质量门和 `git diff --check`，保留实际结果。
- [x] 5.4 运行 `openspec instructions apply --change implement-asset-bible-continuity-slice --json`、status 与 strict validation；仅在实现和验证真实完成后逐项勾选任务并更新项目记忆。
