## Why

阶段 0 仅提供无网络的 `LocalWorkspaceAdapter` 和显式未配置的 `TOSAdapter` 占位，尚不能将已校验媒体安全地持久化到真实私有 TOS Bucket。阶段 1 的 GPT Image、Agnes 和 FFmpeg 只应消费同一 `StoragePort`，因此需要先定义可审计的 TOS 存储契约、凭据边界和对象生命周期，而不侵入它们各自的聚合。

## What Changes

- 新增显式 opt-in 的 TOS storage profile、bucket/object/upload-session catalog 和 `TOSAdapter` 定义；默认测试组合为 `Mock Provider +` 显式选择的 `Local test/offline profile`（adapter identity=`local_workspace`），不得隐式 fallback。
- **BREAKING（仅后续实现时对 Port 适配器）** 将 `StoragePort` 定义扩展为可恢复 multipart、短期 read/write presign、`stat`、工作区转存和受引用保护的 delete；所有实现同步遵守该契约。
- 定义 immutable `StoredObject` 引用、canonical objectKey、MIME/size/checksum/ETag 验证、项目归属、稳定 operation/session key 与 TOS retryable failure 语义。
- 冻结 2 GiB（`2_147_483_648` bytes）阶段验收链路：先以 capability/capacity admission 判断 profile/part limits 与本地工作空间，再验证实际 multipart、一次中断恢复、complete/stat/checksum、AssetVersion 登记和 Media Worker 代理派生；2 GiB 是验收规模，不是平台最大值。
- 定义私有 Bucket、region/endpoint/bucket/auth reference 配置，并复用 `implement-provider-model-skill-catalog` 的通用 AES-256-GCM/Docker Secret `CredentialResolver`；TOS 不实现第二套加密或主密钥事实，API 只返回掩码。
- 冻结 `StorageProfile` 专属设置控制面：字段表单、创建/编辑/启停、`expectedRevision`/`If-Match` 409、显式 connection test、掩码 credential status 与可诊断失败；设置 UI 只消费该 owner contract。
- 定义对象保留、临时工作区清理和删除引用证明规则；已登记候选在 MVP-A 不自动 GC。
- 冻结项目资产中心的通用上传交接：Assets owner 先创建 `AssetVersionReservation`，Storage 以 `operationKey=asset-upload:{projectId}:{assetId}:{reservationId}` 执行/恢复/取消上传并只返回 verified `StoredObjectRef`，再由 Assets owner 幂等登记一次 AssetVersion；Storage、取消后的晚到结果和页面刷新均不得自动登记版本。
- 明确 GPT Image、Agnes 与 FFmpeg 仅通过 `StoragePort` 读写已校验对象，并保持 AssetVersion、ProviderCall、RunEvent、ExportJob 的既有所有权不变。
- 明确 Timeline/export owner 的 MP4/SRT/light 在 ExportJob `packaging` 内通过稳定 export operation key 上传、校验并登记独立 ExportArtifact；Storage 只返回 session/object facts，不创建 ExportJob/Artifact，也不因失败切换 profile。
- 冻结 `SourceMaterialUploadIntent -> verified StoredObjectRef -> AssetVersion registration -> SourceMaterial binding` 的跨 change 交接、唯一 `sourceMaterialUploadKey`、重试/reconcile 和失败保留语义；TOS 不拥有 SourceMaterial 或 AssetVersion。
- 明确跨 Local、Worker、数据库和对象存储的磁盘软/硬阈值、全局 admission、手工备份/恢复 runbook 与 checksum/ETag 恢复演练由独立 `implement-operations-resilience` child 拥有；TOS adapter 只提供 transport/object metadata facts，不拒绝业务、不执行备份或报告恢复成功。

## Capabilities

### New Capabilities

- `tos-storage-provider`: 真实私有 TOS 持久媒体存储、配置/凭据、上传、校验、保留和显式故障边界。

### Modified Capabilities

- `provider-and-storage-boundaries`: 扩展既有 `StoragePort` 的 multipart、presign、workspace transfer、验证和无隐式 fallback 要求。

## Impact

预期后续实现影响 `services/api` 的 ports、runtime/composition、storage/security/persistence adapters、数据库 migration、worker workspace policy、contracts 与定向测试；可能新增经锁定版本的 TOS Python SDK 依赖，但版本、SDK API 兼容性、真实账号参数及 live profile 均为实施前 probe，不在本 change 中安装或验证。不会改写既有 AssetVersion HTTP `schema_version`、objectKey 合同、ProviderCall/RunEvent/ExportJob，数据库不保存媒体 bytes。

## DDD / BDD / SDD / TDD

- **DDD**：`StorageProfile`/BucketBinding/UploadSession/StoredObjectRef 归 storage control plane；`AssetVersion` 归 assets；`SourceMaterial` 归 text owner；设置 UI 只编排 owner command。
- **DDD**：`StorageProfile`/BucketBinding/UploadSession/StoredObjectRef 归 storage control plane；`AssetVersion` 归 assets；`SourceMaterial` 归 text owner；设置 UI 只编排 owner command；operations resilience 独立拥有 capacity/admission/runbook/drill policy，TOS adapter 不拥有这些跨边界事实。
- **BDD**：除显式 TOS 上传/恢复/短期访问/删除及其拒绝矩阵外，必须观察 profile CRUD/启停/connection test、资产中心 reservation/upload/verified ref/registration，以及 SourceMaterial upload、verified ref、AssetVersion append、binding 的逐段成功/失败/恢复结果。
- **SDD**：定义 profile lifecycle DTO、Port、handoff DTO、唯一幂等键、错误/状态/恢复、credential resolver 引用、持久化边界、adapter 依赖和兼容路线；不承诺未经 probe 的 SDK 或 live account 行为。
- **TDD**：先以 `Mock Provider +` 显式 Local test/offline profile 和 owner contract tests 固化 profile CAS、connection test、handoff/reconcile 正反行为；真实 TOS 仅在显式 profile/probe 中验证，缺失时保留 `unconfigured` 原始诊断。
- **TDD**：补充 TOS ownership negative tests：adapter 试图依据磁盘阈值拒绝、创建跨系统 backup/restore 或宣称 checksum/ETag drill 成功时必须 architecture/contract fail 且零业务写入。

## Current / Defined / Todo

- **Current**：阶段 0 示例/测试 composition 显式选择 `LocalWorkspaceAdapter` 作为离线/测试 profile，`TOSAdapter` 仅显式报未配置；`AssetVersion` 已保存 immutable storage metadata 和 canonical objectKey。当前没有 TOS 失败后自动切换行为的合同。
- **Defined**：本 proposal 中的真实 TOS、StoragePort、保留和消费者边界。
- **Todo**：后续实现、SDK/version 与 live account probe、schema/migration、adapter/composition、`Mock Provider +` 显式 Local test/offline profile 与 explicit TOS 分层测试及集成验收。

## Dependencies and Acceptance Commands

直接依赖已归档 AssetVersion/objectKey contract，以及阶段 1 catalog、workflows/runs、timeline/export 的 owner-reference 边界；`plan-phase-one-drama-mvp-a` 仅作协调追溯，不是运行时代码依赖。实施验收运行定向 storage domain/application/adapter/runtime/HTTP/worker/migration/BDD tests、`openspec instructions apply --change integrate-tos-storage-provider --json`、`openspec status --change integrate-tos-storage-provider --json`、`openspec validate integrate-tos-storage-provider --strict --json`、`openspec validate --changes --strict --json`、`pnpm run check` 和 `git diff --check`；真实 TOS 只在明确 profile/probe 下追加，未配置必须保留原始诊断。

## StorageProfile 与 SourceMaterial 交接

`StorageProfile` 的 canonical owner 是 catalog storage-config 子域；storage infrastructure 只拥有 `BucketBinding` 和 upload/object 状态。设置页调用以下 owner commands：`POST /v1/storage-profiles`、`GET/PATCH /v1/storage-profiles/{storageProfileId}`、`POST /v1/storage-profiles/{storageProfileId}/enable`、`POST /v1/storage-profiles/{storageProfileId}/disable`、`POST /v1/storage-profiles/{storageProfileId}/connection-test`。Profile DTO 至少包含 `storageProfileId`、`schemaVersion`、`revision`、`name`、`adapterKey=tos`、`enabled`、`bucketBindingId`、`region`、`endpoint`、`privateBucket`、`credentialRef`、`credentialStatus`、`connectTimeoutMs`、`readTimeoutMs`、`writeTimeoutMs`、`presignMaxTtlSeconds` 和 `projectScope`；响应只返回 `credentialStatus=configured|unconfigured|rotating|failed|master_key_unavailable`、掩码 hint、rotation/test 状态，不返回 secret/envelope。编辑、启停和 connection-test 均携带当前 `expectedRevision`/`If-Match`；配置 mutation 冲突返回 `409 storage_profile_revision_conflict`（含 expected/current revision 且零写入），connection-test 失败按 `unconfigured|validation|authentication|network|timeout` 返回原始脱敏诊断，不隐式切换 Local。

SourceMaterial upload 的唯一跨 owner key 为 `sourceMaterialUploadKey=source-material-upload:{projectId}:{sourceMaterialId}:{sourceMaterialRevision}`，但 Storage 只接受 `creationMode=adaptation`、`inputMode=uploaded_file` 的 `SourceMaterialUploadIntent`。intent 必须冻结 `materialType=novel|synopsis|existing_script`、source immutable revision/contentHash、project scope、asset/version reservation 和该 key；original 或 inline_text intent、无效 enum/scope/revision 必须在创建 UploadSession、StoredObject、AssetVersion 或任何 storage mutation 前拒绝。Storage owner 以该 key 管理 session/complete/reconcile，只在 MIME/size/SHA-256/ETag 验证后签发 `VerifiedStoredObjectHandoff`；Assets owner 在自己的 UoW 以同一 key append 一次 `AssetVersion`，重复相同 ref 返回既有版本，fingerprint/revision 不同返回 conflict；Text owner 以 source revision CAS 原子绑定 `assetVersionId/revision/contentHash`，再进入 parse/validation。timeout/unknown 先按同一 key reconcile；AssetVersion append 失败保留 verified 但未引用对象与 operation 供重试，不重新上传；binding 409/foreign/stale 不创建第二版本、不换源，并保留原始诊断。`run_id + logical_operation`（若由 Run 触发）只能映射到该 key，不得生成第二个 upload key。

Local/TOS upload 只交付上述 verified ref；Assets owner 追加 audio AssetVersion 后用户才可选入 project-scoped library，Timeline owner 才可 add music SoundCue。任一 upload/verify/register/bind 阶段失败均不得生成 cue/clip、付费 Run 或伪成功。

## Credential 引用合同

**DDD**：TOS 只消费 catalog owner 的 CredentialResolver。**BDD**：真实 TOS 主密钥缺失返回 503 `credential_master_key_unavailable`，`Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）仍可用。**SDD**：不复制 cipher/keyring；profile/credential-bound canonical AAD 与 versioned 32-byte Docker Secret keyring 由 catalog 定义。**TDD**：覆盖 missing key、re-encrypt/rotation 恢复和无泄漏。
