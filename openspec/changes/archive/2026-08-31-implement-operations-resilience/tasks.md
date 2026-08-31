## 1. Contract and ownership

- [x] 1.1 在实施前核验总体 `plan-phase-one-drama-mvp-a` 的 19-change DAG、TOS/StoragePort、workflows、timeline/export artifact、local observability 和阶段 0 runtime facts；确认本 change 不是 TOS adapter 或 telemetry 的替代 owner。
- [x] 1.2 定义 `RuntimeResourceSnapshot`、`CapacitySnapshot`、CPU/memory minimum capability、soft/hard threshold、`OperationAdmission`、stable diagnostic、operation key 和 config revision 的共享 Schema/正反 fixtures。
- [x] 1.3 定义显式 Local test/offline profile（adapter identity=`local_workspace`）下的 Local workspace、Worker temporary/derivative、数据库 backup metadata、object manifest/reference、Compose config、Docker Secret keyring 与 credential reference 交接 DTO；禁止 secret/token 持久化和 profile fallback。

## 2. Capacity protection

- [x] 2.0 实现 API/Worker CPU capability/available concurrency、memory available/limit 与 disk/capacity 的只读 probe/aggregation；覆盖 source/capturedAt/config revision/stale/unavailable，且读取零 RunEvent/ProviderCall/UploadSession/AssetVersion/ExportJob/cleanup mutation。
- [x] 2.1 实现 Local/Worker/DB/object manifest 的只读 capacity probe 与 snapshot aggregation，覆盖 timestamp、scope、observed usage、limit、revision 和原始 probe error。
- [x] 2.2 定义并测试 CPU/memory capability refusal、soft threshold warning、hard threshold `resource_capacity_hard_limit` admission refusal；拒绝 upload/new paid generation/media derivative/preview/export 前的所有 intent/ProviderCall/UploadSession/ExportJob/AssetVersion/Outbox 写入。
- [x] 2.3 覆盖 threshold 期间既有 Run/Export 的保留、API/Worker restart、unknown admission reconcile、冻结 Adapter/Profile 选择和 no-fallback/no-auto-cleanup 负例；同时验证 cleanup/GC 不删除、覆盖或压缩 RunEvent、AcceptDecision、CapabilitySnapshot、脱敏 ProviderCall 摘要和仍被引用的 AssetVersion，只有明确无引用临时对象可清理。

## 3. Manual recovery runbook

- [x] 3.1 编写版本化手工备份/恢复 runbook，分项覆盖 PostgreSQL、object manifest/reference、Compose 配置、Docker Secret 主密钥和对象存储 credential reference 的前置检查、fingerprint、权限、恢复顺序和回滚。
- [x] 3.2 定义 backup/restore metadata、blocked/failed/succeeded 状态与 operator UUID/correlation 审计；缺 artifact、权限或 manifest revision 时保持 blocked 且不解除 admission。

## 4. Checksum/ETag drill and verification

- [x] 4.1 实现显式演练环境的 checksum/ETag restore drill，按同一 operation key 幂等保存 expected/observed checksum、ETag、manifest revision、数据库 reference 和 restore evidence。
- [x] 4.2 添加 checksum/ETag exact-match pass、missing/mismatch/foreign/missing-object/revision-drift fail fixtures；失败不得写 current reference、ExportArtifact 或成功恢复状态。
- [x] 4.3 添加 TOS adapter ownership architecture/contract tests，证明 adapter 不拥有阈值、全局 admission、runbook 或 restore drill。

## 5. E2E and strict closure

- [x] 5.1 将 resilience stage 接入 `E2E-MVPA-001`，逐项记录 soft/hard threshold、拒绝诊断、重启恢复、runbook artifact、checksum/ETag pass/fail 和 no-side-effect evidence。
- [x] 5.2 运行本 change 的 domain/application/adapter/BDD/TDD、`openspec instructions apply --change "implement-operations-resilience" --json`、strict validation、全量 strict validation、unchecked task scan 和 `git diff --check`；全部通过前保持任务未勾选。

## 6. 审查一致性修复

- [x] 6.1 将所有依赖数据库的业务 HTTP dependency 在无 `DATABASE_URL` 的受支持启动模式下统一抛出 `DatabaseUnavailableError`，由全局 handler 返回稳定 503 包络；覆盖 text/video generation、catalog、scenes 及同构入口，禁止未处理 `RuntimeError` 变成 500。
