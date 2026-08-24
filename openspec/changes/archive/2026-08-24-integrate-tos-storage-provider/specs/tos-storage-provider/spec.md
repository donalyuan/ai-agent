## ADDED Requirements

### Requirement:Storage ownership and immutable object reference
系统 SHALL 将 storage profile/credential reference、bucket binding、upload session 和 immutable `StoredObject` reference 置于 storage catalog/application/infrastructure 边界，并与 assets 的 `Asset`/`AssetVersion` 分离。`StoredObject` MUST 仅含 provider/profile/bucket/canonical objectKey、project ownership、observed MIME/size/checksum/ETag 和 operation provenance，不含媒体 bytes，且不得拥有、创建、覆盖或修改 AssetVersion。

#### Scenario:verified object is handed to the owning asset application
- **WHEN** upload complete 后对象通过 observed metadata 校验
- **THEN** StoragePort 返回 immutable `StoredObjectRef`，只有 assets application 可在自己的 UoW 中以该引用 append 新 AssetVersion

#### Scenario:storage change cannot rewrite an existing version
- **WHEN** storage adapter/session 尝试变更已有 AssetVersion、其 `storageObject` 或 content hash
- **THEN** 架构/ownership test 失败，且 storage 不写 assets repository 或 version row

### Requirement:项目资产中心通用上传交接
系统 SHALL 对项目资产中心的 image/video/audio 上传使用唯一 owner 链路：Assets owner 创建 `AssetVersionReservation`，Storage owner 以 `operationKey=asset-upload:{projectId}:{assetId}:{reservationId}` 执行或恢复 create/resume/uploadPart/complete/abort/reconcile，验证后返回 immutable `StoredObjectRef`，再由 Assets owner 在自己的 UoW 幂等登记一次 AssetVersion。Storage MUST NOT 拥有或创建 AssetVersion；刷新、API/Worker 重启、retry、取消和晚到 terminal result MUST NOT 自动登记版本或替换任何 Scene/Shot/Timeline current reference。

#### Scenario:同一 reservation 恢复并登记一个版本
- **WHEN** 资产中心上传在刷新或 Worker 重启后以同一 reservation/operation key 恢复，且对象通过 MIME/size/checksum 校验
- **THEN** Storage 返回同一 verified StoredObjectRef，Assets owner 对该 reservation 至多登记一个 AssetVersion，重复请求返回同一登记结果

#### Scenario:取消后的晚到对象保持未引用
- **WHEN** reservation 已取消或 abort 已请求后 Storage 收到 late complete/terminal object
- **THEN** Storage 保留可审计的 unreferenced result 和 reconciliation diagnostic，不调用 assets repository、不登记 AssetVersion、不改变 current reference

### Requirement:Explicit private TOS configuration and credential boundary
系统 SHALL 只在 enabled `StorageProfile(adapterKey=tos)` 显式配置 region、endpoint、private bucket binding、认证 reference、timeout、项目 scope 与 presign max TTL 后创建真实 TOSAdapter。AK/SK MUST 使用 catalog/security owner 的通用 AES-256-GCM Credential 与 Docker Secret 主密钥；TOS MUST 只消费受限 resolver，不得实现、持久化或迁移第二套 credential cipher/master-key 事实。API、日志和事件 MUST 只返回掩码或 credential reference。

#### Scenario:opt-in private profile creates a TOS adapter
- **WHEN** profile 已启用、bucket 为 private、配置和 Docker Secret 主密钥齐全
- **THEN** composition 仅为该 profile 创建 TOSAdapter，并以 profile/bucket/project scope 执行请求

#### Scenario:configuration or secret is absent
- **WHEN** profile 未配置/禁用、bucket 非 private、credential 缺失、主密钥缺失或解封失败
- **THEN** profile/config 缺失返回原始 `unconfigured`，Docker Secret 主密钥缺失返回 HTTP 503 `credential_master_key_unavailable`，其他 credential failure 返回稳定 diagnostic；均不发网络请求、不改用 Local、不暴露 AK/SK，且默认 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）继续可用

### Requirement:StorageProfile lifecycle and explicit connection test
系统 SHALL 为 `StorageProfile` 提供唯一 owner lifecycle：`POST /v1/storage-profiles`、`GET/PATCH /v1/storage-profiles/{storageProfileId}`、`POST /v1/storage-profiles/{storageProfileId}/enable`、`POST /v1/storage-profiles/{storageProfileId}/disable` 和 `POST /v1/storage-profiles/{storageProfileId}/connection-test`。Profile request/response MUST 至少包含 `storageProfileId`、`schemaVersion`、`revision`、`name`、`adapterKey=tos`、`enabled`、`bucketBindingId`、`region`、`endpoint`、`privateBucket`、`credentialRef`、`credentialStatus`、`connectTimeoutMs`、`readTimeoutMs`、`writeTimeoutMs`、`presignMaxTtlSeconds` 和 `projectScope`；credential status 只能是 `configured|unconfigured|rotating|failed|master_key_unavailable` 并带 masked summary，不得出现 secret/envelope/presigned URL。edit/enable/disable MUST 携带 `expectedRevision` 与 `If-Match`；connection-test MUST 携带 profile revision/snapshot/timeout 与 `probeCorrelationId`，不得改变 profile config revision或创建媒体对象。

#### Scenario:create or edit a storage profile
- **WHEN** 用户在 StorageProfile settings page 明确提交完整 region/endpoint/private bucket/binding/credential reference/timeout/TTL 字段
- **THEN** owner 返回新的 profile id/revision 与 masked credential status；只失效该 profile query，默认 adapter 与其他 profile 不变

#### Scenario:reject stale storage profile lifecycle mutation
- **WHEN** create/edit/enable/disable 的 `expectedRevision` 或 `If-Match` 过期，或 profile/bucket 属于其他 project scope
- **THEN** 返回 `409 storage_profile_revision_conflict`（含 expected/current revision）或 scope diagnostic，零配置、session、adapter 和 external network 写入

#### Scenario:explicitly test a configured profile
- **WHEN** 用户点击 connection-test 并确认当前 profile revision/snapshot
- **THEN** 系统只发起一次带 `probeCorrelationId` 的 owner probe，返回 `connected` 或 `unconfigured|validation|authentication|network|timeout` 脱敏状态；不隐式启用、切换 Local 或报告 capability 成功

#### Scenario:surface disabled or master-key-unavailable profile
- **WHEN** disabled profile 被请求新 upload/presign，或真实 profile 缺少 Docker Secret master key
- **THEN** 新 operation 返回原始 disabled/unconfigured diagnostic 或 HTTP 503 `credential_master_key_unavailable`，不创建对象/AssetVersion、不 fallback；`Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）仍可用

### Requirement:Canonical project-scoped object key
系统 SHALL 只接受已归档 objectKey contract 认可的 canonical relative POSIX key，并在 TOS 写入前验证 project namespace、profile bucket scope 和 owner-provided asset/version reservation。输出 key SHOULD 由 owner 分配为 `projects/{project_id}/assets/{asset_id}/{version_id}/original.{ext}`；若 owner 提供其他 canonical key，系统 MUST 原样使用且不得重命名或 URI-normalize。任何 objectKey MUST 拒绝绝对路径、URI、query/fragment、反斜杠、空段和 traversal。

#### Scenario:write to a canonical owner key
- **WHEN** project-owned asset/version reservation 提供合法 canonical key 和允许 MIME 对应 extension
- **THEN** TOSAdapter 在该项目/profile bucket 内写入相同 key，返回的 reference 不含本机绝对路径或 URI key

#### Scenario:reject unsafe or cross-project key
- **WHEN** key 非 canonical、包含 URI/absolute/traversal，或 project/asset/version/profile scope 不一致
- **THEN** 系统在创建 TOS session 前返回 validation/forbidden，且不创建对象、session 或 AssetVersion

### Requirement:Multipart lifecycle is recoverable and idempotent
StoragePort SHALL 提供 `createMultipart`、`resumeMultipart`、`uploadPart`、`completeMultipart` 和 `abortMultipart`。每个逻辑 upload MUST 使用稳定 `operationKey`，每个 session MUST 绑定 project/profile/bucket/key/operation。重复 create/resume MUST 返回同一活动 session；重复 complete 仅在相同 ordered part manifest 下返回相同 object reference；part 或 manifest 冲突 MUST 显式失败，状态未知 MUST 先 reconciliation。

#### Scenario:resume and complete a multipart upload once
- **WHEN** 同一 operationKey 在 timeout 后恢复，并以已登记 receipt 的相同 ordered parts complete
- **THEN** 系统复用 session 或确认 terminal object，返回同一 `StoredObjectRef`，不创建第二对象

#### Scenario:reject conflicting part or duplicate completion
- **WHEN** 相同 session/part number 使用不同 checksum，或已 complete session 使用不同 manifest 再 complete
- **THEN** 系统返回 `multipart_part_conflict` 或 `multipart_complete_conflict`，不覆盖 part、不重复 complete

### Requirement:Verified workspace transfer and object inspection
StoragePort SHALL 提供 `stat`、`uploadFromWorkspace` 和 `downloadToWorkspace`，并在 upload/complete 后以 observed MIME、size、SHA-256 checksum 和可用 ETag 校验对象。ETag MUST 作为独立 storage metadata，MUST NOT 替代 checksum 或 AssetVersion content hash；不符对象不得交给 assets application 登记。

#### Scenario:workspace result passes validation before registration
- **WHEN** GPT Image、Agnes 或 FFmpeg 将受控 workspace 文件交给 uploadFromWorkspace，且 observed MIME/size/checksum 与 intent 一致
- **THEN** StoragePort 返回 verified immutable reference，消费者可请求 assets owner append version，数据库不保存 bytes

#### Scenario:observed media is inconsistent
- **WHEN** declared/observed MIME、size 或 checksum 不一致，或 workspace path 越出受控 root
- **THEN** 系统返回 `media_validation_failed` 或 workspace validation error，不登记 AssetVersion，也不把 ETag 当作成功替代

### Requirement:Project-scoped short presigned access
StoragePort SHALL 提供 `presignRead` 和 `presignWrite`。每个 URL MUST 绑定单个项目、profile/bucket、object/key、最小 read 或 write 动作和不超过 profile max TTL 的正 expiry；系统 MUST 不授予 bucket list、跨项目、delete 或长期访问权限。

#### Scenario:issue a bounded read URL
- **WHEN** 同项目已验证 object 请求正数且不超过 profile max 的 read TTL
- **THEN** 系统返回只读、单对象、可审计的短期 `PresignedAccess`，并记录 expiry/operation provenance 而不持久化 URL 作为业务事实

#### Scenario:reject expired or over-scoped access
- **WHEN** TTL 已过期/超上限、actor project 不匹配，或请求 list/delete/另一个 object 的权限
- **THEN** 系统返回明确 authorization/expiry validation error，且不发放 URL 或扩大权限

### Requirement:Explicit failures and no adapter fallback
默认测试 SHALL 使用 `Mock Provider +` 显式选择的 Local test/offline profile（adapter identity=`local_workspace`）；真实 TOS SHALL 仅在显式 enabled profile 选择。TOS timeout、connection failure 和 safe remote 5xx MUST 返回带 operation/session correlation 的 retryable diagnostic；未配置、credential error、validation、authorization 和 terminal conflict MUST 保持其原始可诊断类别。系统 MUST NOT 因 TOS 失败或 Local 失败自动切换另一个 adapter 或报告成功；运行开始后 Adapter/Profile MUST 保持冻结。

#### Scenario:default test runs without TOS account
- **WHEN** 默认测试环境没有 TOS profile、凭据或主密钥
- **THEN** `Mock Provider +` 显式 Local test/offline profile contract tests 运行，明确请求 TOS 时返回 `unconfigured`，没有 live request

#### Scenario:timeout remains retryable and visible
- **WHEN** explicit TOS profile 的请求发生 timeout 或连接中断
- **THEN** 系统保留安全原始错误、operation/session key 和 retryable state，先允许 reconciliation，绝不回退 Local 或新建无关对象

### Requirement:Reference-protected deletion and retention
系统 SHALL 在 TOS delete 前通过 storage reference proof 检查 AssetVersion、Run、Timeline 与 Package/manifest 引用；任一引用存在或无法证明无引用 MUST 返回 `object_in_use` 并不调用 TOS delete。业务 `StoredObject` SHALL 随引用保留；成功 workspace 临时文件保留 24 小时、失败临时文件保留 7 天；MVP-A MUST 不自动 GC 已登记候选，TOS bucket MUST 不用 TTL 自动删除业务对象。

#### Scenario:delete an unreferenced object with proof
- **WHEN** 管理操作给出项目范围的 no-reference proof，且 object 不被任何 AssetVersion/Run/Timeline/Package 引用
- **THEN** 系统执行一次受审计 delete，并按 retention policy 清理可清理的本地临时文件

#### Scenario:reject deletion of referenced or unprovable object
- **WHEN** object 被引用、reference index 不可用或无法完整证明
- **THEN** 系统返回 `object_in_use` 或 proof diagnostic，不调用 TOS delete，也不以 bucket TTL 代替

### Requirement:Consumer aggregation boundaries
GPT Image、Agnes 和 FFmpeg/timeline SHALL 只通过 StoragePort 的 workspace/multipart/read contracts 消费已验证对象。它们 MUST NOT 直接调用 TOS SDK，MUST NOT 拥有 ProviderCall、RunEvent、ExportJob 或 AssetVersion，且跨边界只传 `assetVersionId`、expected revision/hash 或 `StoredObjectRef`，不得传 nested renamed AssetVersion copies。

#### Scenario:media consumer registers through the owner
- **WHEN** 任一消费者得到 verified `StoredObjectRef`
- **THEN** 它将引用交给 assets application，并由 ProviderCall/RunEvent/ExportJob 各自拥有者记录自己的事实

#### Scenario:consumer attempts ownership leakage
- **WHEN** GPT Image、Agnes 或 FFmpeg 直接创建 TOS SDK client、修改 AssetVersion 或创建 ProviderCall/RunEvent/ExportJob
- **THEN** architecture test 失败，且不发生越界写入

### Requirement:SourceMaterial upload handoff to AssetVersion and binding
系统 SHALL 使用唯一 `sourceMaterialUploadKey=source-material-upload:{projectId}:{sourceMaterialId}:{sourceMaterialRevision}` 串联 Text owner 的 `SourceMaterialUploadIntent`、Storage owner 的 verified `StoredObjectRef`、Assets owner 的一次性 `AssetVersion` registration 与 Text owner 的 source revision binding。Storage MUST 只接受 `creationMode=adaptation`、`inputMode=uploaded_file` 的 intent；intent MUST 冻结 `materialType=novel|synopsis|existing_script`、project scope、SourceMaterial immutable revision/contentHash 和 asset/version reservation。Storage MUST 在创建 UploadSession、StoredObject、AssetVersion 或任何 storage mutation 前拒绝 original、inline_text、invalid enum、foreign scope 或 stale revision。Storage MUST 只签发包含 `projectId`、`storageProfileId/profileRevision`、`bucketId`、canonical `objectKey`、`uploadSessionId`、`operationKey`、`status=verified`、observed MIME/size/SHA-256 checksum/ETag、`verifiedAt` 和 asset/version reservation 的 `VerifiedStoredObjectHandoff`；Assets MUST 在自己的 UoW 以同一 key、expected asset revision/hash append，重复同 fingerprint 返回既有 AssetVersion，冲突返回 `asset_registration_conflict`；Text MUST 以 SourceMaterial revision CAS 保存 `assetVersionId/revision/contentHash`。`run_id + logical_operation`（如存在）只能映射此 key。adaptation 的 inline_text 直接由 text owner 保存 revision/contentHash、parse/validation 和 binding snapshot，不进入此 contract。

#### Scenario:complete an uploaded SourceMaterial handoff
- **WHEN** Text owner 提交 upload intent，Storage 完成并验证对象，Assets registration 与 SourceMaterial binding 使用同一 project/source revision/key 且均无冲突
- **THEN** 系统可读取 upload session/operation、verified handoff、AssetVersion id/revision/contentHash/storage reference 和 SourceMaterial bound snapshot；CreativeBrief/TextRun 才能冻结该 exact source，inline_text 路径不创建 storage/AssetVersion

#### Scenario:拒绝非 uploaded adaptation intent
- **WHEN** intent 为 original、inline_text、无效 material/input enum、foreign project scope 或 stale SourceMaterial revision
- **THEN** Storage 在 UploadSession、StoredObject、AssetVersion 或其他 storage mutation 前返回稳定 validation/conflict diagnostic；inline_text 仍由 text owner 独立保存其 immutable source snapshot

#### Scenario:recover unknown or failed registration without re-upload
- **WHEN** storage timeout/5xx 状态未知，或 verified object 已产生但 AssetVersion append 失败
- **THEN** 系统先按同一 operation/session key reconcile；相同 key 重试至多得到同一 StoredObjectRef/AssetVersion，失败对象保持未引用并带原始 diagnostic，不重新上传、不创建第二版本、不绑定 SourceMaterial

#### Scenario:reject invalid verification or stale binding
- **WHEN** MIME/size/checksum/authorization 校验失败，或 AssetVersion reservation/SourceMaterial revision foreign/stale/conflict
- **THEN** 返回 `media_validation_failed`、scope/`asset_registration_conflict` 或 binding conflict，零后续 binding/paid Run 写入；已登记但无法绑定的版本保持可审计且不得隐式换源

## DDD / BDD / SDD / TDD

- **DDD**：requirements 明确 storage control plane/object/session 与 assets/consumer 聚合的不可越界所有权。
- **BDD**：每项需求均含可观察的正反场景，覆盖未配置、主密钥、跨项目、非法 key、MIME/hash/size、part conflict、duplicate complete、presign、timeout、引用 delete 与 fallback。
- **SDD**：定义 profile/config、Port/DTO、DB ownership、adapter/security/dependencies、兼容性与 TOS probe 限制。
- **TDD**：以 `Mock Provider +` 显式 Local test/offline profile 提供默认正反 contract tests；SDK/version 和 live profile 为实施前 probe，不作为已验证事实。

## Current / Defined / Todo

- **Current**：只有 Local/explicit-unconfigured TOS，尚无真实 session/profile/credential/object lifecycle 实现。
- **Defined**：本 spec 的 TOS storage capability 及其安全、保留和消费者边界。
- **Todo**：实施 tables/ports/adapters/composition/tests，完成受控 SDK/account probe 与集成验收。

## Dependencies and Acceptance Commands

依赖 AssetVersion/objectKey 归档契约及 phase 1 catalog/workflows/timeline/export 的 owner-reference 合同；不依赖它们作为 runtime module。验收命令为定向 storage tests、`openspec instructions apply --change integrate-tos-storage-provider --json`、`openspec status --change integrate-tos-storage-provider --json`、`openspec validate integrate-tos-storage-provider --strict --json`、`openspec validate --changes --strict --json`、`pnpm run check` 与 `git diff --check`；live TOS probe 只在明确 profile 下追加。

### Requirement:Audio upload handoff boundary
系统 SHALL 将 Local/TOS upload 完成后的 verified `StoredObjectRef` 交给 Assets owner append audio AssetVersion；Storage MUST 不创建 AssetVersion、Timeline Clip 或 SoundCue。任一 upload/verify/authorization/license failure MUST 返回诊断，且不产生 cue/clip 或伪成功。

#### Scenario:hand off verified background music
- **WHEN** 用户的 project-scoped audio upload 完成校验
- **THEN** storage 只返回 verified StoredObjectRef，后续 Assets append 和用户选择仍为独立明确步骤

### Requirement:Operations resilience ownership boundary
系统 SHALL 将磁盘 soft/hard threshold、全局 admission、Worker/API capacity diagnostics、手工 backup/restore runbook 与 checksum/ETag restore drill 归 `implement-operations-resilience`；TOS adapter 只返回 transport/object metadata facts，不拥有跨系统策略。

#### Scenario:reject capacity or restore policy in TOS adapter
- **WHEN** TOS adapter 试图依据磁盘阈值拒绝 upload/generation/export、自动执行跨系统 backup/restore，或在数据库 manifest 未由 resilience coordinator 验证时报告恢复成功
- **THEN** architecture/contract test 失败，adapter 只保留原始 transport facts，且不创建 intent、ProviderCall、UploadSession、ExportJob、AssetVersion 或成功恢复状态

### Requirement:2 GiB 素材链路验收
系统 SHALL 将 `2_147_483_648` bytes 作为阶段一实际媒体上传链路的验收样本下限，而非平台最大值。创建 UploadSession 前 MUST 读取冻结 StorageProfile multipart/object-size capability 和 operations-resilience capacity admission。支持时 SHALL 对不入库源码仓库的有效媒体 fixture 完成 streaming multipart、至少一次中断恢复、complete/stat/size/SHA-256/ETag/MIME verification、单一 AssetVersion registration 和 Media Worker inspection/proxy；默认快速测试可用逻辑 size/manifest fake，但不得替代阶段退出的一次 actual-byte evidence。

#### Scenario:完成实际 2 GiB 可恢复媒体链路
- **WHEN** explicit Local/TOS test profile 支持该大小且容量 admission 通过，上传在至少一个 part 后中断并重启恢复
- **THEN** 系统复用同一 reservation/session/operation，验证完整对象，只登记一个 AssetVersion 并生成匹配 source fingerprint 的 inspection/proxy evidence；报告记录 actual bytes、part manifest、checksum 和 profile capability revision

#### Scenario:大小或空间不支持时前置拒绝
- **WHEN** profile object/part limit、part count 或本地可用容量无法支持 2 GiB fixture
- **THEN** 系统在创建 UploadSession、part 或 workspace file 前返回 `storage_object_size_unsupported` 或 resource admission diagnostic，不截断、拆分为多个版本、切换 adapter 或报告链路通过

### Requirement:ExportArtifact storage handoff
Timeline/export owner SHALL 为 MP4、SRT 和 light 分别提供稳定 export operation key、job/artifact identity、StorageProfile revision、declared MIME/size/checksum。StoragePort MUST 执行 upload/reconcile/stat/verification 并只返回 verified StoredObjectRef；Timeline/export owner 才可 append ExportArtifact 和推进 ExportJob。unknown/retry MUST 复用相同 key/fingerprint，MUST NOT 重新渲染、创建第二对象/Artifact、推进伪成功或 fallback。

#### Scenario:上传并交接导出产物
- **WHEN** export owner 为已渲染产物提交合法 intent，Storage 完成上传和 stat/checksum/MIME/size 验证
- **THEN** Storage 返回同一 verified object fact，export owner 可登记一个对应 Artifact；Storage 本身不写 ExportJob/Artifact/RunEvent

#### Scenario:导出上传未知时先 reconcile
- **WHEN** upload complete 响应丢失、timeout 或 worker 重启
- **THEN** Storage 以相同 operation/session/fingerprint 查询并返回既有结果或 unknown diagnostic，不重复 upload/object、不要求 rerender、不切换 profile
