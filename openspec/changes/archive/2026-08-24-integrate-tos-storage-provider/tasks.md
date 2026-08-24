## 0. 实施前取证与边界

- [x] 0.1 在实现前重读已归档 AssetVersion、objectKey contract、现有 `StoragePort`/runtime 和阶段 1 catalog/workflows/timeline/export change；记录实际 Alembic head、consumer 入口和 owner-reference 前置，不把总体计划导入运行时代码。
- [x] 0.2 建立 storage ownership architecture tests：`StorageProfile`/credential reference、bucket/session/object 与 AssetVersion、ProviderCall、RunEvent、ExportJob 的所有者隔离；禁止 storage 或 GPT Image/Agnes/FFmpeg 直接写他方聚合，并断言 TOS adapter 不读取/决定磁盘阈值或全局 admission、不执行 backup/restore runbook/checksum-ETag drill、不把 policy/refusal/pass 写成 transport success。
- [x] 0.3 在安装或调用任何 TOS SDK 前完成离线 SDK/version/API compatibility probe，确认 package/lockfile、endpoint addressing、multipart resume/abort、checksum header/ETag 和 part 限制；缺少结论时保留 `unconfigured`，不猜测版本或发 live request。

## 1. Storage Contract 与 Domain TDD

- [x] 1.1 先增加失败的 contract/domain tests，再定义 `StorageWriteIntent`、`UploadSessionRef`、`PartReceipt`、immutable `StoredObjectRef`、`PresignedAccess`、`DeleteProof`、安全错误类别和关联字段；DTO 不含 bytes、AK/SK、绝对路径或持久 presigned URL。
- [x] 1.2 扩展 `StoragePort` 为 `createMultipart`、`resumeMultipart`、`uploadPart`、`completeMultipart`、`abortMultipart`、`presignRead`、`presignWrite`、`stat`、`uploadFromWorkspace`、`downloadToWorkspace`、`delete`，并保留受限的阶段 0 Local `put/get` 兼容路线。
- [x] 1.3 先写 object key/project scope 的负例，再复用已归档 canonical helper 和 Assets owner `AssetVersionReservation` 验证 `projects/{project_id}/assets/{asset_id}/{version_id}/original.{ext}` 或 owner canonical key；通用资产中心上传固定 `operationKey=asset-upload:{projectId}:{assetId}:{reservationId}`，覆盖 absolute/URI/traversal、跨项目、错误 bucket/profile、未知/取消/stale reservation 与 operation key mismatch。
- [x] 1.4 先写 multipart 状态机和幂等失败 tests，再实现稳定 `operationKey`、session binding、part checksum receipt、相同 manifest duplicate complete 复用、不同 part/manifest conflict、abort terminal state 和 timeout reconciliation。

## 2. Storage Catalog、Security 与 Persistence

- [x] 2.1 定义 storage profile/bucket binding/session/object operation/reference proof 的 application ports、Schema 和 masked HTTP DTO；仅允许 private bucket、显式 enabled profile、region/endpoint/bucket/auth reference、timeout、project scope 与 max presign TTL。
- [x] 2.1a 冻结 `StorageProfile` 专属 owner contract：`POST/GET/PATCH /v1/storage-profiles`、enable/disable、connection-test command/API、字段表单（adapterKey、enabled、Bucket/Region/Endpoint/private policy、credentialRef/status、timeouts、presign TTL、projectScope）、masked response、`expectedRevision`/`If-Match` 与 `409 storage_profile_revision_conflict` zero-write。
- [x] 2.1b 先写 StorageProfile lifecycle/connection-test 的 domain/application/HTTP/adapter BDD tests，覆盖 create/edit/enable/disable、disabled profile、configured success、unconfigured/private-bucket/auth/network/timeout/master-key 503、probeCorrelationId、无对象/AssetVersion写入和无 Local fallback。
- [x] 2.2 先在实际 Alembic head 上写 migration tests，再新增仅归 storage 所有的 additive profile/bucket/session/part/operation/reference metadata 表、FK、project scope、unique operation/session 和 terminal-state constraints；不迁移 bytes、不改写 AssetVersion、objectKey 或既有 HTTP `schema_version`。
- [x] 2.3 集成 catalog/security owner 的通用 AES-256-GCM `CredentialResolver`：TOS 不创建第二套 cipher/master-key 表或算法，AAD 绑定 profile，HTTP/log/event 只返回掩码；先覆盖 profile/credential 缺失、Docker Secret 主密钥缺失时 live TOS HTTP 503、`Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）继续可用、解封失败和无泄漏回归。
- [x] 2.4 实现 `StorageReferenceProofPort`，以可审计 project scope 检查 AssetVersion、Run、Timeline 与 Package/manifest 引用；任一 owner 未实现、索引不可用或存在引用时均 fail closed。

## 3. Local Compatibility 与 Runtime Composition

- [x] 3.1 先扩展 Local contract tests，再让 `LocalWorkspaceAdapter` 实现 v2 multipart/presign/stat/workspace transfer/delete proof 语义，限制受控 root，并保持 adapter identity 为 `local_workspace`。
- [x] 3.2 更新 runtime/composition 的 profile resolution：默认测试组合为 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），显式 `tos` 且 profile 完整才装配 TOSAdapter；未知 mode、disabled profile 或配置缺失必须原样失败，主密钥缺失必须为 503，不可 fallback，且运行开始后不得切换 Adapter/Profile。
- [x] 3.3 添加 runtime/architecture/BDD tests，覆盖 Local 与 TOS 无隐式 fallback、跨项目、非法 key、未配置、过期 presign、TOS timeout retryable diagnostic 与无真实网络默认路径。

## 4. TOSAdapter 与对象完整性

- [x] 4.1 在 0.3 记录的锁定 SDK/version 和显式 profile 前置满足后，实现 TOSAdapter 的私有 bucket client、最小权限 presign、multipart create/resume/upload/complete/abort、`stat`、workspace transfer 与受 proof 保护 delete；业务层不得直接导入 SDK。
- [x] 4.2 先写 adapter failures，再实现 upload/complete 后的 observed MIME、size、SHA-256 checksum、ETag 校验和安全诊断；ETag 独立保存，不得替代 checksum/content hash，失败对象不得交给 assets 登记。
- [x] 4.3 实现 timeout/connection/5xx 的 operation/session reconciliation 和 retryable error mapping；验证 retry 不重复 complete/创建对象，terminal conflict、authorization 和 validation 不被错误重试。
- [x] 4.4 增加 deterministic TOS transport fake/adapter tests，覆盖私有配置、part conflict、duplicate complete、expired/over-scoped presign、MIME/hash/size mismatch、credential failure、TOS timeout 和无 Local fallback；测试不读取真实凭据。
- [x] 4.5 定义并验证 2 GiB fixture admission：冻结 profile object/part size/count capability 与 resource capacity，默认测试用逻辑 size/manifest fake；explicit Local/TOS 阶段证据使用不入仓库的 `2_147_483_648` bytes 有效媒体完成 streaming multipart、中断/重启恢复、stat/checksum/MIME、单一 AssetVersion 和 inspection/proxy，记录 actual bytes/profile revision；不支持时在 session/part/workspace 写入前拒绝。

## 5. Asset Registration 与媒体消费者边界

- [x] 5.1 对接项目资产中心 owner contract：以 `AssetVersionReservation -> Storage operation -> verified StoredObjectRef -> AssetVersion registration` 为唯一通用上传链路，固定 `asset-upload:{projectId}:{assetId}:{reservationId}` 并在 create/resume/uploadPart/complete/abort/reconcile 中复用；只有 Assets owner 可按 reservation/fingerprint/expected revision 在自己的 UoW 登记一次版本，失败不创建或改写 AssetVersion，DB 始终不存 bytes。
- [x] 5.1a 添加刷新/API/Worker 重启、timeout、duplicate complete、registration retry、cancel/late-result 的跨 owner BDD/architecture tests；证明 Storage 不拥有 AssetVersion，重复同 fingerprint 返回同一版本，取消或晚到对象保持未引用且不改变 Scene/Shot/Timeline current。
- [x] 5.2 为 GPT Image、Agnes 和 FFmpeg/timeline consumer 写 integration/architecture tests，并迁移它们仅使用 workspace/multipart/read StoragePort 及 owner IDs/references；它们不得拥有 ProviderCall、RunEvent、ExportJob 或 AssetVersion。
- [x] 5.2a 为 Timeline/export MP4/SRT/light 定义稳定 artifact operation、upload/reconcile/stat/verification handoff tests；证明 Storage 只返回 verified StoredObjectRef，Export owner 单次登记 Artifact，unknown 不 rerender/duplicate/fallback。
- [x] 5.3 覆盖注册链路的 BDD 正反场景：已校验同项目对象可 append、跨项目/非法 key/MIME/hash/size 不符被拒绝、重复 operation 无重复版本、AssetVersion HTTP `schema_version` 与 objectKey 既有合同不变。
- [x] 5.4 冻结并实现前测试 `sourceMaterialUploadKey=source-material-upload:{projectId}:{sourceMaterialId}:{sourceMaterialRevision}` 的 typed handoff；Storage 只接受 adaptation + uploaded_file `SourceMaterialUploadIntent`，冻结 `materialType=novel|synopsis|existing_script`、source revision/contentHash、project scope、reservation 与 `run_id + logical_operation` 映射。明确 `VerifiedStoredObjectHandoff -> AssetVersion registration -> SourceMaterial binding` 的 verified 字段、expected revision/hash 与状态转移，重复同 fingerprint 返回同一版本；original/inline/invalid enum/scope/revision 零 storage mutation。
- [x] 5.5 添加跨 Storage/Assets/Text owner 的 BDD/E2E：adaptation uploaded_file upload/verify/register/bind success；inline_text 无 storage/AssetVersion；original/inline intent 与 invalid enum/scope/revision 零 storage mutation；timeout/unknown reconcile、MIME/size/checksum/credential/foreign/stale/409、AssetVersion append failure 保留未引用对象和 operation、retry 不 re-upload/不重复版本/不换源/no paid Run，并记录逐段 correlation evidence。

## 6. 保留、删除与运维失败

- [x] 6.1 实现并测试引用保护 delete：仅在 `StorageReferenceProofPort` 给出 no-reference proof 时发 TOS delete；引用中或无法证明时返回 `object_in_use`，MVP-A 不自动 GC 已登记候选。
- [x] 6.2 实现受控 workspace cleaner 与审计测试：成功临时文件保留 24 小时、失败临时文件保留 7 天；TOS business objects 无 TTL 自动删除，cleaner 不可越界删除。
- [x] 6.3 定义并测试受控恢复/人工处置记录：assets 登记失败、aborted session、未知 remote state 与失败对象保留原始诊断、operation/session correlation 和恢复状态，不报告上传成功。

## 7. 验证、Probe 与交接

- [x] 7.1 运行 storage contract/domain/application/adapter/runtime/architecture/HTTP/worker/migration/BDD 正反测试，至少覆盖未配置、凭据/主密钥缺失、跨项目、非法 key、MIME/hash/size 不符、分片冲突、重复 complete、过期 presign、TOS timeout、引用中 delete 和 Local/TOS 隐式 fallback。
- [x] 7.2 运行 `openspec instructions apply --change integrate-tos-storage-provider --json`、`openspec status --change integrate-tos-storage-provider --json`、`openspec validate integrate-tos-storage-provider --strict --json`、`openspec validate --changes --strict --json`、`pnpm run check` 和 `git diff --check`；保留原始失败，未完成实现任务不得勾选。
- [x] 7.3 仅在明确 enabled non-production TOS profile、private bucket 证据、Docker Secret、最小权限账号和批准的 SDK/version 全部具备时执行 1x1x1 live probe；验证 multipart/resume/presign/stat/validation/abort/delete proof，缺一项则报告 `unconfigured`，不阻塞 `Mock Provider +` 显式 Local test/offline profile MVP。
- [x] 7.4 将 SDK/version、live account 参数、Bucket ACL、part limits、presign max TTL、reference proof 完整性和失败对象人工处置作为实施前/后可审查证据交接；并将 resilience capacity/admission/runbook/drill 交接给 `implement-operations-resilience`，以 architecture/contract test 证明 TOS adapter 只交付 object metadata/error facts；不得把未 probe 项或跨边界 policy 结果声明为已验证。

## DDD / BDD / SDD / TDD

- **DDD**：0.x、1.x、2.x 和 5.x 先固定 storage/asset/consumer 所有权，防止对象、session、版本和运行事实混写。
- **BDD**：3.x--7.x 覆盖显式 Local test/offline profile、explicit TOS、完整上传与全部必需负例的外部可观察结果。
- **SDD**：1.x--4.x、6.x 交付 Port/Schema/DB/adapter/security/dependency/compatibility/retention 的最小实现顺序。
- **TDD**：每个实现任务先写失败的定向测试；live TOS 仅是 7.3 的显式补充，不是默认测试条件。

## Current / Defined / Todo

- **Current**：本 change implementation 与 Local/Mock/explicit-unconfigured 验收均已完成；TOS SDK/version/account/private bucket/Docker Secret/credential 前置缺失，故 live probe 结果为 `unconfigured` 且未发网络请求。
- **Defined**：proposal/design/specs 已冻结的行为、边界、失败语义、依赖和验收命令。
- **Todo**：后续只有在真实外部前置全部确认且另有明确授权时才可启用 SDK/profile/live probe；不得把当前 deterministic fake 或 Local 证据解释为 live TOS 成功。

## 8. Audio StoredObjectRef Handoff

- [x] 8.1 定义 upload -> verified StoredObjectRef -> Assets audio AssetVersion -> explicit user selection 的 handoff contract，明确 Storage/Assets/Timeline owner boundary。
- [x] 8.2 添加 upload/verify/license/authorization failure 的零 cue/clip、无伪成功测试，并保持显式 Local test/offline profile、TOS explicit probe 与运行期 Adapter/Profile 冻结。

## Dependencies and Acceptance Commands

直接依赖已归档 AssetVersion/objectKey contract；阶段 1 catalog、workflows/runs、timeline/export 仅提供 owner-reference 前置，整体 `plan-phase-one-drama-mvp-a` 仅作协调。验收命令、顺序和 live-probe 条件已列于 7.2--7.3；SDK/version 和 live account 输入未验证前必须保留 `unconfigured`。

- [x] 9.1 复用 catalog AES-256-GCM CredentialResolver，测试真实 TOS 主密钥缺失 503、`Mock Provider +` 显式 Local test/offline profile 可用、rotate/re-encrypt/legacy replacement failure 无泄漏；不得新增第二套 envelope/keyring 状态。

## 10. 审查一致性修复

- [x] 10.1 补充 StorageProfile 编辑回归测试并修复设置页 PATCH：未编辑的 `bucketBindingId` 与 `credentialRef` 必须沿用 owner projection，禁止普通字段修改静默清空绑定或凭据引用。
- [x] 10.2 在 Asset reservation/admission 前按请求的 `StorageProfile` 查询并验证 project scope、enabled、current revision、private binding 与 capability snapshot；不存在、foreign、disabled、stale 或伪造 snapshot 必须 fail closed 且零 reservation/session/object 写入。
- [x] 10.3 补充 multipart complete receipt 完整性测试并校验每个 manifest receipt 的 part number、checksum、ETag 与 size 均精确匹配已上传 receipt；相同 manifest 幂等，不同 receipt 返回 conflict 且不持久化伪造 manifest。
