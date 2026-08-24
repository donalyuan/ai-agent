# operations-resilience Specification

## Purpose
TBD - created by archiving change implement-operations-resilience. Update Purpose after archive.
## Requirements
### Requirement:CPU、内存与容量 capability 预检
系统 SHALL 从 API/Worker 的受控 probe 生成带 source、capturedAt、config revision 与原始错误的 `RuntimeResourceSnapshot`，至少覆盖 CPU capability/available concurrency、memory available/limit 和 disk/capacity。需要上传、付费生成、媒体派生、preview 或 export 的 command MUST 在创建 intent/ProviderCall/UploadSession/ExportJob/AssetVersion/Outbox 前读取适用 snapshot。probe unavailable 或低于配置 minimum MUST 分别返回 `resource_probe_unavailable` 或 `resource_capability_unsupported`，不得猜测默认值或报告成功。

#### Scenario:资源能力不足时阻断新操作
- **WHEN** CPU capability、可用内存或所需容量低于 operation minimum，或 snapshot 过期/无法取得
- **THEN** 系统返回带 observed/required/source/revision/correlation 的稳定 diagnostic，不创建任何付费/写入/preview/export 副作用，也不切换 Worker 或 adapter

#### Scenario:资源 probe 读取不产生业务 mutation
- **WHEN** health/readiness、设置页或 admission 只请求最新 resource snapshot
- **THEN** 系统只返回观测与原始错误，不创建 RunEvent、ProviderCall、UploadSession、AssetVersion、ExportJob 或 cleanup action

### Requirement:跨边界容量准入
系统 SHALL 以 Local workspace、Worker temporary/derivative、数据库和 object manifest 的带 revision `CapacitySnapshot` 计算 soft/hard threshold。soft threshold MUST 保留 warning/diagnostic；hard threshold MUST 在 upload、new paid generation、preview、new export 或需要新增 workspace bytes 的 command 创建任何 intent/ProviderCall/UploadSession/ExportJob/AssetVersion/Outbox 前拒绝。任何 cleanup/GC MUST 保护长期 `RunEvent`、`AcceptDecision`、`CapabilitySnapshot`、脱敏 `ProviderCall` 摘要和仍被引用的 `AssetVersion`。

#### Scenario:软阈值保持可观察
- **WHEN** 任一受管 workspace 达到 soft threshold
- **THEN** 新 command 返回可查询的 warning/admission state，既有运行不被静默取消，snapshot 保留 observed usage、limit、scope、config revision 和 correlation

#### Scenario:硬阈值拒绝新副作用
- **WHEN** capacity 达到 hard threshold 且用户请求上传、付费生成或导出
- **THEN** 系统返回 `storage_capacity_hard_limit` 与原始观测，不创建 intent、ProviderCall、UploadSession、ExportJob、AssetVersion 或 Outbox，也不自动清理或切换到 TOS/Local fallback

### Requirement:稳定诊断与重启恢复
系统 SHALL 为 threshold probe、Worker/API restart、unknown admission 和 blocked recovery 保存稳定 diagnostic、operation key、snapshot revision、observed usage 和 original error。重启恢复 MUST 先读取同一 operation/snapshot，并保持运行开始时冻结的 Adapter/Profile 选择，不自动改用 TOS 或 Local；同时保持拒绝或继续既有 owner 状态，不重复付费或重复写入。

#### Scenario:API 或 Worker 重启后恢复
- **WHEN** admission 在 command 提交前或执行中遇到 restart/unknown
- **THEN** recovery 先 reconcile 同一 operation key，复用既有结果或保持 blocked/unknown，不生成第二个收费提交或伪造成功

### Requirement:手工备份与恢复 runbook
系统 SHALL 提供版本化手工 runbook，分别记录 PostgreSQL backup/restore、object manifest/reference inventory、Compose configuration、Docker Secret keyring 和 object-storage credential reference 的前置检查、fingerprint、恢复顺序、权限、失败保留、回滚和 operator UUID。runbook MUST NOT 保存 secret、token 或私有凭据值，且 MUST NOT 自动执行恢复或提供恢复 UI。

#### Scenario:runbook 拒绝不完整恢复输入
- **WHEN** 任一必需 backup artifact、权限、manifest revision 或 credential reference 缺失
- **THEN** runbook 将恢复保持 blocked，保留缺失项和原始 diagnostic，不解除 admission、不修改 current reference、不报告成功

### Requirement:Checksum 与 ETag 恢复演练
系统 SHALL 执行一次可重复、幂等且使用显式演练环境的 checksum/ETag 恢复演练，比较 source/expected/observed checksum、ETag、object manifest revision、数据库 reference 和 restore correlation。演练结果 MUST 记录 success/fail、时间和稳定 operator UUID。

#### Scenario:恢复演练只在对象身份精确一致时通过
- **WHEN** 数据库 metadata 恢复后对象 checksum、ETag、manifest revision 和归属均匹配
- **THEN** 演练记录 `passed` artifact，解除演练 admission，并证明 reference 未被替换

#### Scenario:Checksum 或 ETag 不匹配阻断恢复
- **WHEN** checksum/ETag 缺失、不匹配、对象 foreign/missing 或 manifest revision drift
- **THEN** 演练记录原始 mismatch、保持 `blocked`/`failed`，不写 current reference、ExportArtifact 或成功恢复状态

### Requirement:TOS adapter 所有权边界
系统 SHALL 将 StorageProfile、Bucket、CredentialResolver、StoragePort transport/object lifecycle 继续归 `integrate-tos-storage-provider`；operations resilience 只消费其 object metadata/reference facts。TOS adapter MUST NOT own disk thresholds、全局 admission、backup runbook 或 restore drill。

#### Scenario:拒绝在 TOS adapter 内实现 resilience 逻辑
- **WHEN** TOS adapter 试图根据全局磁盘阈值拒绝业务、执行跨系统备份或报告恢复成功
- **THEN** architecture/contract test 失败，resilience coordinator 保留原始 transport facts，且不产生伪成功副作用

### Requirement:resilience 验证矩阵
系统 SHALL 为 soft/hard threshold、stable diagnostic、zero-side-effect refusal、restart recovery、manual runbook 和 checksum/ETag drill 提供 domain/application/adapter/BDD/TDD fixtures，并把结果纳入 `E2E-MVPA-001` 的 resilience stage。默认 fixture MUST 使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）；所有本 change tasks 在实现前 MUST 保持 unchecked。

#### Scenario:验收要求正向与反向证据
- **WHEN** 维护者运行 resilience 验收
- **THEN** report 同时包含 threshold pass、hard refusal、restart/reconcile、runbook evidence、checksum/ETag pass 与 mismatch failure；缺少任一项不得报告 MVP-A 完成

### Requirement:长期事实 no-GC 验收
系统 SHALL 在 resilience 验收中构造超过诊断窗口、不同 `hold`、跨 owner 引用和 restart/reconcile 的长期事实，并执行 Worker temporary/derivative cleanup、capacity maintenance 与 GC 尝试。`RunEvent`、`AcceptDecision`、`CapabilitySnapshot`、脱敏 `ProviderCall` 摘要和仍被引用的 `AssetVersion` MUST 保持可读取、append-only 且不得被删除、覆盖或静默压缩；只有明确无引用且符合策略的临时对象可清理。

#### Scenario:拒绝清理仍被引用的长期事实
- **WHEN** cleanup/GC 试图删除仍被引用的 AssetVersion 或任一长期审计/运行事实
- **THEN** 系统拒绝或跳过删除并保存稳定诊断，引用、审计、RunEvent sequence 和 owner revision 均不变
