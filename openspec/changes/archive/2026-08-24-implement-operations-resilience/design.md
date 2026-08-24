## Context

MVP-A 的测试/离线运行使用 `Mock Provider +` 显式选择的 Local test/offline profile（adapter identity=`local_workspace`），同时由 API、Agent/Generation/Media Worker、PostgreSQL、TOS object reference 和 ExportArtifact 共同产生临时文件及可恢复事实。容量不足、Worker 重启、数据库恢复或对象 checksum/ETag 不一致必须可诊断地拒绝；本 change 不得创建、启用、停用或切换 profile，也不得由任一 TOS adapter 私自决定跨系统行为。

## Goals / Non-Goals

**Goals:**

- 统一 Local workspace 与 Worker temporary/derivative 的 soft/hard threshold snapshot、阻断点和稳定诊断。
- 统一 API/Worker 的 CPU、内存和磁盘/容量 probe，形成带来源、时间和配置 revision 的 `RuntimeResourceSnapshot`；无法取得或不满足 minimum capability 时 fail closed，不用主机猜测值冒充通过。
- 定义数据库 backup metadata、object manifest/reference、Compose 配置、Docker Secret 主密钥和对象存储凭据的手工备份/恢复 runbook。
- 通过一次 checksum/ETag 演练证明恢复后的对象引用与数据库 manifest 一致，并保留失败证据。
- 为阈值拒绝、零副作用、重启恢复、runbook 和演练输出提供 owner contract 与 E2E evidence。

**Non-Goals:**

- 不实现自动化备份、周期调度、恢复 UI、portable 工程包回导或生产 SLA。
- 不拥有 `StorageProfile`、Bucket、CredentialResolver、TOS API、multipart/presign/delete 或任何第二套 `StoragePort`。
- 不绕过 provider/catalog/workflows/timeline/export 的 aggregate owner；本 change 只协调跨边界保护和运维证据。

## Decisions

### 1. Resilience coordinator owns policy, not storage transport

新增 `OperationsResilienceCoordinator` 读取 API/Worker CPU、内存、Local、数据库和 object manifest 的只读状态，输出 `RuntimeResourceSnapshot`、`CapacitySnapshot`、`OperationAdmission` 和诊断。TOS adapter 只报告 object metadata/ETag/checksum 与原始错误；soft threshold 产生 warning/admission degraded，hard threshold 对 upload/new generation/preview/export 等新写入返回稳定 `resource_capacity_hard_limit`，不得静默清理或切换 adapter。probe unavailable 使用 `resource_probe_unavailable`，minimum capability 不满足使用 `resource_capability_unsupported`。

### 2. Threshold semantics are explicit and side-effect free

所有 admission command 在创建 ProviderCall、UploadSession、ExportJob、AssetVersion 或 Outbox 前读取带时间和配置 revision 的 resource/capacity snapshot。soft threshold 不改变既有运行，只产生可查询诊断；CPU/内存 minimum capability 不满足或 hard threshold 拒绝新的付费/写入/预览/导出操作，返回 observed value, limit, scope 和 correlation，失败不创建部分 intent。cleanup 只能由后续显式运维动作执行，不能由 probe 自动删除用户对象；任何 cleanup/GC 也不得删除、覆盖或压缩 RunEvent、AcceptDecision、CapabilitySnapshot、脱敏 ProviderCall 摘要或仍被引用的 AssetVersion。

### 3. Manual runbook is the recovery source of truth

runbook 按 PostgreSQL logical backup/restore、object manifest/reference inventory、Compose configuration、Docker Secret keyring 和 object-storage credential reference 分项记录前置检查、备份 fingerprint、恢复顺序、权限、失败保留和回滚。凭据只记录 reference/status，不写 secret。恢复必须先恢复数据库 metadata，再验证对象引用 checksum/ETag，最后解除 admission；任一步失败保持 blocked/diagnostic。

### 4. One checksum/ETag drill is an artifact, not a success flag

演练使用脱敏 fixture/显式 test bucket，保存 source fingerprint、expected/observed checksum、ETag、manifest revision、restore correlation、时间和 operator UUID。mismatch、foreign scope、缺对象或 ETag drift 均为失败，不修改生产 current reference，不报告恢复成功。演练可重复但必须以 operation key 幂等。

## Risks / Trade-offs

- [不同 worker 的磁盘视图短暂不一致] -> admission 使用带 timestamp/config revision 的 snapshot，保留原始观测并在下一次 command 重新读取。
- [硬阈值阻断正在运行的任务] -> 只拒绝新 admission；既有 Run/Export 保留 owner 状态并记录 capacity diagnostic，不伪造取消或成功。
- [恢复顺序造成数据库与对象 manifest 暂时不一致] -> 恢复阶段保持 blocked，checksum/ETag gate 通过后才解除 admission。
- [TOS adapter 被迫承担跨边界策略] -> contract/architecture tests 断言 adapter 只返回 transport/object facts，resilience coordinator 才产生 policy decision。

## Migration Plan

1. 先定义 runtime-resource/capacity/backup/restore/drill schemas、stable diagnostics 和 `Mock Provider +` 显式 Local test/offline profile fixtures。
2. 增加 API/worker admission read path、backup metadata repository 与 runbook 文档；不改写既有 AssetVersion、ExportArtifact 或 TOS profile。
3. 接入 Compose/local probes、可重复 checksum/ETag drill 和 E2E evidence；真实 TOS 仅在显式 profile/probe 中作为对象 metadata source。
4. 回滚只移除新增 coordinator/admission/metadata 表和未消费 drill records；备份产物、原始诊断及长期 no-GC 事实按 retention 保留。

## Open Questions

- 实施前确认最新 PRD 给出的 soft/hard threshold 默认值、单位和配置 owner；未确认时不得填写运行时默认值。
- 实施前确认数据库备份工具、对象存储 manifest 格式和演练环境授权；缺失时保持 `unconfigured`，不能报告完成。

## DDD / BDD / SDD / TDD

- **DDD**：`OperationsResilienceCoordinator` 只拥有跨边界 admission policy、resource/capacity snapshot 与演练证据；Storage、Run、ProviderCall、AssetVersion、ExportArtifact 和长期审计事实仍由各自 aggregate owner 持有。
- **BDD**：用户在 soft threshold 看到 warning，在 capability/hard threshold 或恢复 mismatch 时看到稳定拒绝；重启/reconcile 不重复收费或写入，runbook 不完整时保持 blocked。
- **SDD**：共享 contracts 固定 `RuntimeResourceSnapshot`、`CapacitySnapshot`、`OperationAdmission`、stable diagnostics、versioned runbook metadata 与 checksum/ETag drill artifact，TOS adapter 只提供 transport/object facts。
- **TDD**：先覆盖 probe unavailable、minimum unsupported、soft/hard threshold、restart/unknown、no-GC、runbook 缺失与 checksum/ETag mismatch，再实现 coordinator、admission、repository、adapter 和 E2E evidence。
