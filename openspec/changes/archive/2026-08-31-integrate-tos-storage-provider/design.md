## Context

阶段 0 的 `StoragePort` 只有本地实现的基础对象、multipart、presign 与 workspace 转存操作；`TOSAdapter` 会抛出 `AdapterNotConfiguredError`，示例 runtime 通过显式 `local_workspace` profile 选择 Local。已归档的 AssetVersion 切片只保存 `storageProvider`、bucket、canonical relative `objectKey`、MIME、size、checksum、可选 ETag/region 和媒体 metadata，且 `AssetVersion` 及其嵌套 `StorageObject` 不可变。`schema_version` 与 HTTP 映射是 assets 所有者的既有契约。

阶段 1 的 GPT Image、Agnes、FFmpeg/timeline 只消费 `StoragePort`：它们必须先在受控 workspace 校验外部输出，再得到一个可登记的引用。ProviderCall 归 catalog，RunEvent 归 workflows/runs，ExportJob 归 exports；本 change 不创建这些聚合或事件历史。现有 canonical objectKey contract 已拒绝 URI、绝对路径、盘符、反斜杠、query/fragment、空段及 traversal，不能重写或放宽。

## Goals / Non-Goals

**Goals:**

- 定义显式 enabled TOS profile 下真实、私有、可审计的持久媒体存储；离线/测试只使用显式选择的 `Local test/offline profile`（adapter identity=`local_workspace`）。
- 定义 storage catalog/profile、bucket binding、immutable `StoredObject` 引用和可恢复 upload session 的职责、数据最小集与引用边界。
- 定义项目资产中心统一的 `AssetVersionReservation -> Storage operation -> Verified StoredObjectRef -> AssetVersion registration` 交接和取消/晚到结果边界。
- 定义 multipart、短期 presign、workspace transfer、`stat`、对象校验、幂等、保留与安全删除的统一 `StoragePort` 合同。
- 复用 catalog/security owner 已固定的 AES-256-GCM/Docker Secret `CredentialResolver`、掩码 API、缺主密钥 HTTP 503 和 `Mock Provider +` 显式 Local test/offline profile 可用性；TOS 只拥有 adapter 消费与显式未配置/retryable 失败。

**Non-Goals:**

- 不在本 change 编写业务代码、安装/锁定 SDK、访问 TOS、读取凭据、执行 live probe 或修改依赖。
- 不拥有或改写 AssetVersion、ProviderCall、RunEvent、ExportJob、Timeline、Package、用户/项目聚合及其 HTTP `schema_version`。
- 不拥有磁盘 soft/hard threshold、全局 admission、Worker/API capacity policy、数据库备份/恢复 runbook 或 checksum/ETag restore drill；这些职责属于独立 `implement-operations-resilience` coordinator，TOS 只返回 transport/object metadata facts。
- 不在 PostgreSQL 保存媒体 bytes、base64、完整 AK/SK、presigned URL 长期值或宿主绝对路径；不添加公共 bucket、匿名访问或隐式 Local/TOS fallback。
- 不自动 GC MVP-A 已登记候选，不把 TOS Bucket TTL 当作业务对象清理机制，不重定义已归档 objectKey 合同。

## Decisions

### 1. 领域所有权与跨边界引用

| 事实 | 所有者 | 不拥有的事实 | 跨边界形态 |
| --- | --- | --- | --- |
| `StorageProfile`、credential reference、enabled state | `providers` catalog 的 storage 配置子域 | 明文凭据、AssetVersion、run/export 状态 | `storageProfileId`、profile revision、masked summary |
| `BucketBinding`、endpoint/region、private policy | storage infrastructure | project/asset 内容与版本号 | `bucketId`、不可变 profile snapshot |
| `UploadSession`、part receipt、operation key、expiry | storage application/infrastructure | AssetVersion registration、ProviderCall/RunEvent | `uploadSessionId`、operation key、`StoredObjectRef` |
| `StoredObject` | storage application 的不可变引用事实 | AssetVersion 生命周期与引用替换 | provider/bucket/objectKey/observed metadata/checksum/ETag 的 `StoredObjectRef` |
| `Asset`、`AssetVersion`、version number/content hash | assets | TOS session、bucket profile、remote delete | `assetVersionId`、expected revision/hash，或 `StoredObjectRef` |

`StoredObject` 不含媒体 bytes，不可变，也不调用 assets repository。完成上传后，assets application 接收一个已完成且已验证的 `StoredObjectRef`，在自己的 UoW 内决定是否 append `AssetVersion`；storage 不预先登记、覆盖或改写任何版本。消费者只传 owner ID/revision/hash 或该引用，不能传嵌套的重命名 AssetVersion 副本。

### 1a. 项目资产中心通用上传交接

项目资产中心的任意 image/video/audio 上传都必须先由 Assets owner 创建 `AssetVersionReservation`，冻结 project/asset/reservation ID、expected Asset revision、declared MIME/size/checksum、StorageProfile snapshot 和 canonical key。Storage operation key 固定为 `asset-upload:{projectId}:{assetId}:{reservationId}`；create/resume/uploadPart/complete/abort/reconcile 必须复用同一 key，不得另造通用上传 key。

Storage 只拥有 `UploadSession`、part receipt、remote object 状态和 verified `StoredObjectRef`。验证成功后，Assets owner 才可在自己的 UoW 以 reservation/fingerprint/expected revision 幂等 append 一次 AssetVersion，并把 reservation 转为 `registered`。页面刷新、API/Worker 重启、retry、取消或晚到 terminal result 都不能由 Storage 自动登记版本；已取消 reservation 的晚到对象保留为可审计的 unreferenced result，后续只能由显式 reconcile/人工处置流程处理，不能替换 Scene/Shot/Timeline current reference。

### 2. 显式 profile、私有 Bucket 和密钥边界

`StorageProfile` 必须明确保存/解析 `adapterKey=tos`、enabled、region、endpoint、bucket binding、认证引用、connect/read/write timeout、presign max TTL 与项目允许范围；bucket policy 必须为 private。endpoint/region/bucket 不是业务代码常量，profile 缺失、禁用、指向非私有 bucket 或配置不完整均在 composition/application 边界返回明确诊断。

AK/SK 作为 catalog/security owner 的通用 `Credential` AES-256-GCM 密文，带 profile-bound AAD；主密钥只由 Docker Secret 提供。`TOSAdapter` 只能消费该 owner 提供的受限 `CredentialResolver`，不得实现或持久化第二套 cipher/master-key 状态。HTTP、OpenAPI、日志、RunEvent、ProviderCall、异常文本和 `StoredObjectRef` 只能出现 credential reference 或掩码。缺少主密钥时显式 TOS/live operation 按通用合同返回 HTTP 503 `credential_master_key_unavailable`；选择 Local、无法解封和 TOS authentication failure 都不是 fallback 条件，`Mock Provider +` 显式 Local test/offline profile 不依赖主密钥并继续可用。

替代方案是把 AK/SK 直接放进环境变量或在 API 解密后传给 worker；两者扩大泄露面且与既有 AES-256-GCM/Docker Secret 决策冲突，故拒绝。

### 2a. StorageProfile lifecycle 与 connection test

`StorageProfile` 是 catalog storage-config 子域的唯一生命周期 owner；`BucketBinding` 是 storage infrastructure 的配置事实，但只能通过 profile owner command 被引用。canonical owner API 为 `POST /v1/storage-profiles`、`GET/PATCH /v1/storage-profiles/{storageProfileId}`、`POST .../{storageProfileId}/enable`、`POST .../{storageProfileId}/disable` 和 `POST .../{storageProfileId}/connection-test`。所有资源请求必须带 `projectId` 或明确 `projectScope=system|project`，HTTP/command DTO 使用同一 camelCase 映射。

Profile create/edit 表单与 DTO 至少冻结：`storageProfileId`、`schemaVersion`、`revision`、`name`、`adapterKey=tos`、`enabled`、`bucketBindingId`、`region`、`endpoint`、`privateBucket=true`、`credentialRef`、`credentialStatus`、`connectTimeoutMs`、`readTimeoutMs`、`writeTimeoutMs`、`presignMaxTtlSeconds`、`projectScope`。响应只允许 `credentialStatus=configured|unconfigured|rotating|failed|master_key_unavailable`、掩码 hint、rotation state、last connection-test status/time 和 correlation；不得出现 secret、ciphertext、nonce、authTag、key 或 presigned URL。edit/enable/disable 携带 `expectedRevision` 与 `If-Match`，stale 返回 `409 storage_profile_revision_conflict`（含 expected/current revision，零配置写入）；disabled profile 禁止新 upload/presign/connection test 以外的 TOS operation，不得 fallback，历史 session 仅按其 frozen profile snapshot reconcile/complete 或返回明确 terminal state。

`connection-test` 是显式、单 profile、带 profile revision/snapshot/timeout 的 owner command，使用稳定 `probeCorrelationId`，不改变 profile config revision，不创建媒体对象或 AssetVersion；成功返回 `connected` 与脱敏 observed endpoint/region/bucket policy，失败返回 `unconfigured|validation|authentication|network|timeout` 分类和安全 provider code。缺 Docker Secret 主密钥返回 503 `credential_master_key_unavailable`，任何失败均不得切换 `local_workspace`。UI、TOS adapter 和 catalog/security tests 必须消费同一 DTO/error matrix。

### 3. objectKey 和项目隔离

所有 TOS key 先通过已归档 canonical relative objectKey helper；不得接受 URI、绝对路径、宿主路径、`..`、query/fragment 或客户端任意 bucket。assets owner 为输出预分配稳定 `assetVersionId`，但直到 upload complete 后才 append version；推荐 canonical key 为 `projects/{project_id}/assets/{asset_id}/{version_id}/original.{ext}`。`ext` 由受验证 MIME 的允许映射或 assets owner 的 canonical key 得出，绝不从未经校验的路径拼接。

若已有 owner 传入 canonical key，storage 只验证其 canonical form、项目 namespace 和该 profile 的 bucket scope，不重命名、不补全、不尝试 URI canonicalization。profile/bucket/object 请求必须携带 `projectId`；adapter 将 prefix/scope 与 object key 比较，跨项目、未知 asset/version reservation 或 scope 不一致均在远程调用前失败。

### 4. StoragePort v2 及兼容路线

新 contract 用有界 DTO 而不是无结构位置参数：`StorageWriteIntent(projectId, profileId, objectKey, operationKey, expectedMime, expectedSize, expectedChecksum)`、`UploadSessionRef`、`PartReceipt`、`StoredObjectRef`、`PresignedAccess` 和 `DeleteProof`。实现必须至少提供：

1. `createMultipart(intent)`、`resumeMultipart(sessionId, operationKey)`、`uploadPart(sessionId, partNumber, workspacePartRef, expectedPartChecksum)`、`completeMultipart(sessionId, orderedParts, operationKey)`、`abortMultipart(sessionId, operationKey)`；
2. `presignRead(objectRef, actorProjectId, ttl)`、`presignWrite(intent, ttl)`、`stat(objectRef, actorProjectId)`；
3. `uploadFromWorkspace(workspaceRef, intent)`、`downloadToWorkspace(objectRef, workspaceRef)`、`delete(objectRef, deleteProof)`。

每个方法校验项目归属和最小权限；read/write presign 绑定单一 object/key、operation 与短 TTL，不授予 list/bucket-wide/delete 能力。为阶段 0 兼容，现有 `put/get` 可仅由 Local 测试兼容层实现，生产媒体 application 必须迁到 workspace/multipart contract；`TOSAdapter` 不通过 `put/get` 绕过验证。Local 与 TOS 对同一 v2 语义给出同类成功/可诊断失败，不允许 runtime 根据异常切换 adapter。

### 5. 上传状态、幂等与完整性

`operationKey` 对一次逻辑 upload 稳定，`uploadSessionId` 与 profile/key/project/operation 的绑定持久可恢复。重复 create/resume 返回同一活跃 session；同一 part number 仅能以同一 part checksum 重放，内容不同返回 `multipart_part_conflict`；重复 complete 在完全相同 manifest 下返回同一 `StoredObjectRef`，不同 manifest 返回 `multipart_complete_conflict`，不二次创建对象。abort 对已 abort 是幂等，对 complete 后返回明确 terminal state。

complete 或 workspace transfer 后 adapter 必须 `stat`/head 并比较 observed MIME、size、SHA-256 checksum 与可用 ETag。TOS ETag 不是多 part 的 SHA-256 替代物，作为独立 observed field 保存。任一不符时对象不可传给 assets，结果为 `media_validation_failed`；可安全中止的未登记对象保留失败诊断。网络 timeout/connection reset/5xx 产生带安全原始 provider code 的 `retryable_storage_error`，状态未知时先用 operation/session key reconcile，绝不盲目重复 complete。TOS adapter 不读取或决定跨系统容量阈值，不拒绝业务 admission，不自动清理磁盘，不创建/执行备份恢复，不比较数据库与对象 manifest 以宣称恢复成功；`implement-operations-resilience` 读取这些 facts 后自行决定 warning/refusal/blocked/pass，并承担零副作用与恢复证据。

阶段退出包含显式 2 GiB（`2_147_483_648` bytes）媒体链路，但该数字只定义验收样本下限，不声明 StorageProfile 的平台最大对象大小。测试先读取冻结 profile capability（part size/min/max、part count、object size）和 operations-resilience capacity admission；支持时必须使用不提交仓库的有效媒体 fixture 执行真实 streaming multipart、至少一次中断/进程恢复、complete、stat/size/SHA-256/ETag/MIME 验证、单一 AssetVersion registration 与 Media Worker inspection/proxy。默认单元/CI 可使用等价逻辑 size/manifest fake 验证边界，但阶段退出报告必须包含一次显式 Local 或 TOS profile 的实际 2 GiB evidence。profile 不支持或空间不足时，必须在 UploadSession/part/workspace 写入前返回 `storage_object_size_unsupported` 或 resource admission diagnostic，不降低大小、拆成多个 AssetVersion、切换 adapter 或伪报通过。

### 6. 注册、删除和保留

对象验证完成才发出可登记 `StoredObjectRef`。通用资产中心上传由 assets application 以 `AssetVersionReservation`、固定 `asset-upload:{projectId}:{assetId}:{reservationId}` operation key、expected asset revision/owner hash 接受该引用并追加版本；Provider/SourceMaterial 等专用 owner handoff 继续使用各自冻结的 operation key。版本登记失败时不得伪造成功，需保留未引用对象/operation 供受控恢复。数据库始终只存 reference 和 metadata。

SourceMaterial 文件输入采用单一 `sourceMaterialUploadKey=source-material-upload:{projectId}:{sourceMaterialId}:{sourceMaterialRevision}`，且只接受 `creationMode=adaptation`、`inputMode=uploaded_file` 的 `SourceMaterialUploadIntent`。Text owner 必须在 intent 中冻结 `materialType=novel|synopsis|existing_script`、source immutable revision/contentHash、project scope 和 `assetVersionReservationId`；Storage 在任何 UploadSession 或对象操作前拒绝 original、inline_text、无效 enum、foreign scope 或 stale revision。storage 只负责 `UploadSession`、验证和签发 `VerifiedStoredObjectHandoff`（`projectId`、`storageProfileId/profileRevision`、`bucketId`、`objectKey`、`uploadSessionId`、`operationKey`、`status=verified`、observed MIME/size/checksum/ETag、`verifiedAt`、reservation）；Assets owner 以同一 key、expected asset revision/hash 在自己的 UoW append `AssetVersion`，重复相同 fingerprint 返回原版本，冲突返回 `asset_registration_conflict`，不得再建版本；Text owner 再以 SourceMaterial revision CAS 绑定 `assetVersionId/revision/contentHash`。adaptation 的 inline_text 由 text owner 直接保存 revision/contentHash、parse/validation 与 binding snapshot，不调用 Storage。

跨 owner handoff 的可恢复状态为 `uploading -> verified -> asset_registration_pending -> bound`，失败可为 `reconciliation_required|failed|aborted`。timeout/5xx/unknown 必须先用同一 operation/session key reconcile；MIME/size/checksum/authorization 失败直接 `failed`，不创建 AssetVersion/binding；Assets append 失败保留未引用 verified object/operation，允许相同 key 重试，不重新上传；binding stale/foreign/409 保留已登记版本但不覆盖 SourceMaterial、不换源，交由 text owner 重新读取 revision 后显式重试。original、inline_text、invalid enum/scope/revision 的 intent 是 terminal validation rejection，零 storage mutation；若 Run 触发上传，`run_id + logical_operation` 只能关联该 key，不得产生第二个 upload operation。

delete 必须先由 `StorageReferenceProofPort` 检查所有可用 owner：AssetVersion、Workflow Run、Timeline、Export Package/manifest。任一活动或历史引用存在则返回 `object_in_use`，并且不发 TOS delete；不能证明无引用时同样拒绝。MVP-A 不自动 GC 已登记候选。业务 `StoredObject` 随引用保留；成功 workspace 临时文件 24 小时、失败临时文件 7 天后可由本地受控 cleaner 删除；TOS object 不配置生命周期 TTL 来绕开引用证明。

### 7. 消费方与副作用顺序

GPT Image、Agnes 和 FFmpeg worker 仅把临时结果交给 `StoragePort`，收到 verified `StoredObjectRef` 后请求 assets owner 注册。它们不创建或更新 `AssetVersion`、ProviderCall、RunEvent、ExportJob，且不从 object reference 反序列化嵌套副本。外部 storage 调用发生在各自 intent/UoW commit 后；需要重试时复用稳定 operation key，RunEvent/ProviderCall/ExportJob 的记录仍由各所有者完成。

Timeline/export 的 MP4、SRT、light 输出沿同一边界：export owner 为每个 artifact 派生稳定 operation key 并冻结 StorageProfile revision，StoragePort 只执行 upload/reconcile/stat 与返回 verified StoredObjectRef；Timeline/export owner 再以同一 job/artifact identity 追加 ExportArtifact。upload unknown 先 reconcile，同 fingerprint 重试返回同一 object fact；Storage 不创建/推进 ExportJob 或 Artifact，也不要求重新渲染、另建对象或 fallback Local/TOS。

## Risks / Trade-offs

- [TOS SDK 与 API 版本未知] -> SDK package、精确版本、multipart/resume/abort 语义和 endpoint compatibility 均列为实施前离线文档/lockfile/live profile probe；不在文档中声称已验证。
- [timeout 后远程状态未知] -> 以 operation/session key reconcile、`stat` 和 terminal manifest 取证，未知时报告 retryable，不重复 complete。
- [预分配 version ID 被误认为已登记] -> 明确其只是 assets owner 的稳定 reservation；AssetVersion 只在验证后的 application command/UoW 中追加。
- [MIME/ETag 供应商差异] -> checksum/size/observed MIME 是注册门，ETag 单独记录，不用 ETag 推导内容 hash。
- [引用索引暂不完整] -> 不能取得所有 owner 的无引用证明即拒绝删除；MVP-A 无自动 GC。
- [Local 与 TOS 行为漂移] -> contract tests 在两种 adapter 执行相同正反场景，live TOS 仅作为明确 opt-in probe，不阻塞 `Mock Provider +` 显式 Local test/offline profile。

## Migration Plan

1. 先以 contracts/domain/application tests 定义 v2 DTO、错误和 `StorageReferenceProofPort`，保留阶段 0 Local compatibility tests。
2. 在实施前重新核验 Alembic head；只添加 storage profile/bucket/session/operation/reference 元数据所需的 additive tables、FK/unique/check constraints 和 upgrade/downgrade tests，不复制 catalog Credential ciphertext/master key，不迁移媒体 bytes 或重写 AssetVersion/objectKey。
3. 实现 Local v2 adapter 与 deterministic Mock/unconfigured TOS behavior，更新 composition 使测试 harness 显式选择 `Local test/offline profile`（adapter identity=`local_workspace`），而不是把 Local 作为 TOS 失败 fallback。
4. 在受限 adapter 边界添加经 lockfile 固定的 TOS SDK、credential resolver、TOSAdapter、multipart reconciliation、presign 和 validation；缺少 profile/master key 保持原始 error。
5. 先验证 `Mock Provider +` 显式 Local test/offline profile，再使用独立、显式 enabled 的 non-production TOS profile 完成最小 live probe；回滚为禁用 profile/adapter，不删除对象、会话、引用或审计，按保留规则处理临时文件。

## Open Questions

- 实施前 probe 决定 TOS Python SDK 的 package 名、锁定版本、endpoint addressing、multipart part size/最大数、checksum header 与 abort/resume API；这些现在不是已验证项目事实。
- live profile 仍缺 bucket/region/endpoint、私有 ACL 证据、账号权限、Docker Secret 主密钥来源、credential record 和最大 presign TTL；缺失时结果必须为 `unconfigured`。
- `StorageReferenceProofPort` 在 Run/Timeline/Package 尚未实现前的查询适配、失败对象人工处置权限和合规保留期需由拥有者在实施前明确；不以 TTL 或猜测删除代替。

## DDD / BDD / SDD / TDD

- **DDD**：上述所有权表把存储控制面、immutable object/session 和 assets/version 事实分开；storage 不拥有消费者聚合。
- **BDD**：规格覆盖 profile CRUD/启停/409、显式 connection-test、资产中心 reservation/upload/registration、上传/恢复/完成、SourceMaterial handoff、短期 URL、项目隔离、所有必需失败、删除引用保护和无 fallback。
- **SDD**：本设计固定 profile lifecycle DTO/error matrix、v2 Port/DTO、`asset-upload` 与 `sourceMaterialUploadKey` handoff、状态/恢复、配置/加密/DB/adapter/dependency/compatibility 路线与部署回滚边界。
- **TDD**：先为 `Mock Provider +` 显式 Local test/offline profile 写可失败的 profile CAS/connection-test/handoff contract/application tests，再实现 TOSAdapter；live account 只在明确 profile/probe 中作为补充，不是默认 oracle。

## Current / Defined / Todo

- **Current**：真实 TOS、storage catalog/session persistence 和 secret resolver 未实现；阶段 0 仅有显式 Local test/offline profile、Mock 与 TOS explicit-unconfigured，AssetVersion/objectKey 既有契约已存在。
- **Defined**：本设计的职责、接口、验证、保留、错误、迁移及消费者边界。
- **Todo**：完成前述 migration/adapter/composition/tests，核验 SDK 与真实账号输入，执行明确 live probe 和集成验收。

## Dependencies and Acceptance Commands

直接依赖已归档 AssetVersion/objectKey contract，以及阶段 1 的 catalog、workflows/runs、timeline/export consumer contracts；总体 `plan-phase-one-drama-mvp-a` 只作追溯协调，不是运行时代码依赖。实施验收依次运行对应 storage domain/application/adapter/HTTP/worker/migration/BDD tests、`openspec instructions apply --change integrate-tos-storage-provider --json`、`openspec status --change integrate-tos-storage-provider --json`、`openspec validate integrate-tos-storage-provider --strict --json`、`openspec validate --changes --strict --json`、`pnpm run check` 和 `git diff --check`；live probe 仅在显式 profile 具备时追加，并保留原始失败。

## Audio StoredObjectRef boundary

**DDD**：Storage owns upload/verified StoredObjectRef; Assets owns append; Timeline owns references。**BDD**：verify/license/upload failure 零 cue/clip。**SDD**：handoff 带 project/object/verification facts，不带 Timeline command，兼容显式 Local test/offline profile 和 TOS explicit probe。**TDD**：先写 Local/TOS verification-to-Assets handoff failures；非目标是 AssetVersion/Timeline/preview 实现。验收使用本 change 已列命令。

TOS 必须仅调用 catalog 的受限 CredentialResolver；不复制 envelope/cipher/keyring。真实 profile 缺主密钥返回 503 `credential_master_key_unavailable`，不能 fallback；`Mock Provider +` 显式 Local test/offline profile 不需要主密钥。日志、presign、StoredObjectRef 和错误不得暴露 key material、objectKey 或 workspace URI。
