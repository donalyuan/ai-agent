## 1. 合同与领域模型（DDD / SDD）

- [x] 1.1 实施前重新核对已归档 projects/episodes 代码、Schema、OpenAPI 与 Alembic head，并确认本 change 只做 additive extension；记录 catalog 不再拥有项目文本阈值、workflows/runs 继续拥有 BudgetGate/Run 状态。
- [x] 1.2 先为 `creationMode`、CreativeBrief/CreativeSettings immutable versions、source binding snapshot、StorySpec/ScriptSpec refs、typed handoff/ack 编写 Draft 2020-12 JSON Schema 正反 corpus；证明 HTTP `schemaVersion` 只映射 canonical `schema_version`。
- [x] 1.3 先写 Project/CreativeBrief/Settings domain tests，再实现稳定 ID、九个 CreativeBrief 字段、正数计数/时长、money threshold、successor version、current pointer 和 expected revision 规则。
- [x] 1.4 先写 original/adaptation 失败测试，再实现 exact `CreativeBriefSourceBindingSnapshot`；拒绝正文/StoredObject/AssetVersion 复制、wrong mode、foreign/stale/hash/status/version mismatch。
- [x] 1.5 先写 accepted handoff 全有或全无、幂等/冲突和 owner ack 边界测试，再实现 Project StorySpec、Episode/ScriptSpec reference 与稳定 Episode 集合规则。

## 2. Application、Repository 与事务（DDD / TDD）

- [x] 2.1 扩展 projects Repository/UoW ports 与内存 adapter，提供 CreativeBrief/settings/binding/version/current 和 owner projection queries，不向 domain/application 泄漏 ORM/FastAPI。
- [x] 2.2 实现 save creation mode/CreativeBrief/settings 和 bind adaptation source commands；每个 command 使用一个 UoW，并在同事务追加 audit/Outbox。
- [x] 2.3 实现 `ApplyProjectEpisodeTextHandoff` 与 `ProjectEpisodeTextHandoffAck`：完整集合预校验、一个 projects UoW、任何成员失败全回滚、相同 fingerprint 返回原 ack。
- [x] 2.4 实现 Workbench/text input/Run freeze/light manifest 所需的 current/history projection；读取只返回 owner refs/hashes，不返回 source/Story/Script 正文且零 mutation。

## 3. 持久化、迁移与 HTTP（SDD / TDD）

- [x] 3.1 先写 SQLite/PostgreSQL migration fixtures，再增加 additive Alembic revision 和 SQLAlchemy mappings，覆盖 version/pointer/binding/handoff ack 表、FK、唯一、hash、money 与项目归属约束。
- [x] 3.2 验证旧项目/剧集 upgrade 时只得到明确 unconfigured 状态，不伪造 creationMode/CreativeBrief/settings；完成 upgrade/downgrade/upgrade 和失败回滚测试。
- [x] 3.3 先写 HTTP/contract 失败测试，再实现 project-scoped camelCase create/read/list/history/update/bind/handoff endpoints、`If-Match`/`expectedRevision` 同值校验及 403/409/422/503 稳定映射。
- [x] 3.4 增加数据库并发与约束测试，证明 CreativeBrief/settings 不原地覆盖、批量 Episode handoff 无部分写入、重复 handoff 不重复 audit/Outbox。

## 4. 跨 owner 与用户场景（BDD / TDD）

- [x] 4.1 为 text owner 增加只读 validated CreativeBrief/SourceBinding input port fixtures，并证明 projects 不执行 parse、Storage、TextModel 或 Provider 调用。
- [x] 4.2 为 catalog/workflows 增加项目 threshold snapshot consumer fixtures：catalog 不持久化第二份 threshold，cost unknown/超阈值由现有 CostConfirmation/BudgetGate owner 处理。
- [x] 4.3 为 TextReview accepted handoff 增加 Project/Episode owner BDD，覆盖 2x2x3、partial/foreign/duplicate/stale、owner ack 缺失和全部 ack 前媒体门关闭。
- [x] 4.4 更新 Workbench/设置/总体 E2E fixtures，记录 original 无 source、两种 adaptation 输入、Brief/settings exact snapshot、owner ack、409 和 no-side-effect evidence。

## 5. 质量门与交付证据

- [x] 5.1 运行 domain/application/repository/HTTP/contract/architecture/migration suites、`E2E-MVPA-001` focused cases、全量项目质量门和 `git diff --check`，保留实际结果。
- [x] 5.2 运行 `openspec instructions apply --change extend-projects-episodes-creative-slice --json`、status 与 strict validation；仅在实现和验证真实完成后逐项勾选任务并更新项目记忆。
