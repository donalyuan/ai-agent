## Context

阶段 0 已实现 project-scoped `Asset`、append-only `AssetVersion`、metadata-only StorageObject reference 和基础 HTTP。阶段一其他 active changes 另行定义 StorageProfile/TOS multipart、Provider 结果入库、Media Worker inspection/derivatives、Scene/Shot accepted current、Timeline/Export 引用和其余四个业务 UI，但没有项目级资产目录或上传到使用位置的用户闭环；本 change 补成第五个前端/业务闭环。

本 change 是阶段一第五个 UI/业务闭环，并直接依赖已归档 Assets 契约、`integrate-tos-storage-provider`、`implement-episode-timeline-audio-export` 及引用 AssetVersion 的各 owner。依赖只通过 commands/queries/IDs/revisions 交接；总体协调 change 仍不是运行时代码依赖。

## Goals / Non-Goals

**Goals:**

- 为单个项目提供 `/projects/:projectId/assets`，覆盖上传/续传/取消/恢复、目录浏览、筛选、版本、授权、派生状态、音频试听和使用位置。
- 扩展 Asset 目录元数据，同时保持 AssetVersion、StoredObject、MediaInspection 和 MediaDerivative 的既有所有权与不可变边界。
- 让刷新、API/Worker 重启、multipart retry 和 AssetVersion registration retry 复用同一 operation/reservation，不重复对象、版本或费用。
- 将资产中心作为 Workbench、候选审核和 Timeline 的稳定选择/深链入口，并纳入 `E2E-MVPA-001`。

**Non-Goals:**

- 不拥有 StorageProfile、UploadSession、StoredObject、MediaInspection、MediaDerivative、Scene/Shot、Timeline、Export、ProviderCall 或 RunEvent。
- 不在数据库、Query cache、浏览器 store 或日志保存媒体 bytes、base64、plaintext credential 或长期 presigned URL。
- 不实现物理删除/GC、文件夹、收藏、批量改标签、语义搜索、图片遮罩/选区编辑、时间范围 Agent 编辑、独立音频审核、自动语义/视觉质检或统一审核中心。
- 不把 Local 当作 TOS 失败 fallback，不在默认测试发真实网络或执行真实 Provider。

## Decisions

### 1. 聚合所有权保持分离

Assets owner 扩展 `AssetCatalogMetadata` 和 `AssetVersionReservation`；Storage owner 继续唯一拥有 UploadSession/part receipts/StoredObject；Media Worker 继续唯一拥有 `MediaInspection`/`MediaDerivative`；各业务 owner 继续拥有对 AssetVersion 的真实引用。资产中心 application/UI 只调用这些 owner 的 typed commands/queries并聚合只读 projection。

替代方案是新建一个包含 Asset、Storage、Derivative 和 usage 的“大素材聚合”，会复制版本、对象和处理状态并破坏现有 append-only/owner contract，因此拒绝。

### 2. Asset 元数据可 CAS 修改，AssetVersion 永远不可变

`AssetCatalogMetadata` 包含 `sourceType=user_upload|provider_generated|source_material|imported`、可选 `catalogRole=character|location|prop|storyboard|video_take|dialogue|music|ambience|effects|other`、有界 tags、`authorizationStatus=unknown|declared|verified|restricted|expired`、可选 copyright/license label/reference。元数据变更使用 Asset `expectedRevision`，成功产生新 revision 和审计；它不得改写任何 AssetVersion、StoredObject 或历史 usage。

选择 Asset-level 元数据而不是复制到每个版本，可满足目录筛选并避免版本间重复。版本/manifest 仍必须保存其交付时冻结的 authorization/license provenance，不能只依赖后来可变的目录标签。

### 3. 上传使用 Assets reservation 与 Storage operation 双重幂等

Assets owner 创建 `AssetVersionReservation`，冻结 project/asset/reservation ID、expected asset revision、declared kind/MIME/size/checksum、StorageProfile snapshot 和 canonical upload key。Storage operation key 固定为 `asset-upload:{projectId}:{assetId}:{reservationId}`。create/resume/complete/abort/reconcile 均复用该 key；verified `StoredObjectRef` 只可由 Assets owner对同一 reservation append 一次 AssetVersion。

取消先请求 Storage abort，再把未登记 reservation 标记 cancelled；若 terminal object 晚到，保留未引用对象和诊断，绝不自动 append。状态未知先 reconcile，不能创建第二 reservation。浏览器不自行推断成功。

上传前 UI 必须读取 Storage owner 的 object/multipart limits 和 operations-resilience capacity admission。阶段退出用不提交仓库的有效媒体 fixture 验证一次 2 GiB（`2_147_483_648` bytes）链路：分片流式上传、至少一次中断/刷新或 Worker 重启、相同 session 恢复、完整校验、单一 AssetVersion 登记和 inspection/proxy readiness。默认快速测试可使用逻辑 size/part manifest fake，但不得作为 actual-byte 退出证据。若 profile 或空间不支持，UI 在创建 reservation/session 或读取文件内容前显示原始 limit/admission diagnostic；不得截断、拆成多个 AssetVersion、换 profile 或把 2 GiB 误写成平台最大值。

### 4. 派生物和试听只消费 ready reference

资产中心读取匹配 AssetVersion id/revision/hash 的 `MediaInspection` 与 `MediaDerivative`，显示 `queued|running|ready|failed|stale`。缩略图、代理和波形只有在 ready 且项目归属、授权与 source fingerprint 都匹配时才可请求短 TTL read-only grant；音频试听使用 proxy/normalized audio 或 owner 允许的原始音频 read grant，不暴露 objectKey。

资产中心不触发派生生成，也不把 pending/failed/stale 显示为可用。缺少波形不阻断目录读取，但阻断对应波形/试听能力并显示原始诊断。

### 5. 使用位置是精确只读聚合，不是第二事实源

`AssetUsageQuery` 通过 owner query ports 聚合 SourceMaterial、Scene/Shot current/historical reference、AssetEdit candidate/decision、Timeline Clip/SoundCue、TimelineVersion 和 Export manifest 的精确 reference，返回 reference type、owner ID/revision、display scope、state、deep link 与 source hash。资产中心不直接跨模块写表或创建引用。

任一必需 owner query 不可用时返回 `usage_projection_unavailable`/partial diagnostic，而不是把未知误报为“未使用”。foreign project 和未授权历史引用不可见。

### 6. 前端只保存交互状态

TanStack Query 保存 owner DTO；Zustand 只保存当前项目的 filter/sort/view、选中 AssetVersion、音频播放位置和未提交上传表单。上传 session/reservation、版本、派生物和 usage 均从 server owner 恢复，不进入 localStorage。路由为 `/projects/:projectId/assets`，并提供到 workbench review/timeline 的 owner-validated deep link。

分页使用稳定 cursor，并固定排序 tie-breaker `(updatedAt,id)`；filter 至少覆盖 kind/catalogRole/tag/sourceType/authorizationStatus/processingStatus。MVP-A 不承诺全文或语义搜索。

## Shared UI、目录表格与虚拟化

资产中心复用 `shared/ui` 的 DataTable、Tabs、Tooltip、Dialog、Progress 和 Toaster，TanStack Table 定义目录/usage 的 columns、cursor、筛选和排序，TanStack Virtual 为长目录、版本历史和 usage 列表提供有界渲染。音频播放器/波形保持资产领域 adapter；不得从 `shared/ui` 导出第二个播放器或表格变体。

组件测试和 focused E2E 必须证明 2 GiB upload admission、目录筛选、usage partial/unavailable、版本诊断、音频 seek 与 Timeline handoff 的状态尺寸稳定、键盘可操作、scope 安全且读取零副作用；缺少 owner 数据时显示 unavailable，不能返回伪空集合。

### 7. 安全、保留和测试边界

所有列表、版本、上传、试听、usage 和 deep link 均逐层验证 project ownership、stable local user UUID、authorization/license、revision/hash 和短 TTL grant。访问页面或切换筛选不得创建 UploadSession、ProviderCall、RunEvent、AssetVersion 或派生任务。

默认 E2E 使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）；TOS 仅 explicit probe。资产中心不提供物理删除或自动 GC，既有 retention/no-GC 合同保持有效。

## Risks / Trade-offs

- [usage 聚合跨多个 owner，任一不可用可能产生误导] -> fail closed 为 unavailable/partial diagnostic，并以 owner revision/hash 标记每条结果，不返回伪空集合。
- [上传 complete 与取消竞态] -> reservation + operation key + terminal reconciliation；晚到对象不自动登记版本。
- [可变授权标签与已导出 provenance 不一致] -> Asset 元数据只服务当前目录；Timeline/Export 继续冻结交付时 provenance。
- [长列表和波形加载影响浏览器] -> cursor 分页、缩略图/波形惰性加载、短 TTL grant 和有界 response。
- [新 change 与 Timeline 素材箱重复] -> Timeline 素材箱复用资产中心 selector/query，不建立第二套上传、过滤或版本状态。

## Migration Plan

1. 为 Asset metadata、revision audit 和 AssetVersionReservation 增加 additive migration；旧 Asset 回填 `sourceType=imported`、空 tags、`authorizationStatus=unknown`，不改已有 AssetVersion。
2. 先实现 domain/application/query ports 和 Local deterministic fixtures，再接 HTTP 与 project asset center UI。
3. 接入 Storage multipart/reconcile、MediaDerivative read model 和 usage owner query ports；缺依赖保持 unavailable，不伪造成功。
4. 将 Timeline 素材箱改为复用共享 asset selector，并补 `E2E-MVPA-001` 的 `S08a asset center` 证据。
5. 回滚可隐藏 UI/route，但不删除新 metadata、reservation audit、AssetVersion 或 usage 引用；additive 列保持兼容。

## Open Questions

- 无阻塞产品问题。具体 HTTP path/error envelope 在实施前根据已实现 owner API 取证冻结，不得由 UI 猜测第二套合同。

## DDD / BDD / SDD / TDD

- **DDD**：Asset metadata/reservation、Storage session/object、Media derivative 和 usage owner reference 四类事实严格分离。
- **BDD**：上传、恢复、取消、筛选、试听、版本和使用位置均有成功、foreign/stale/unknown/failed 可观察路径。
- **SDD**：定义 additive DB、query/command DTO、route/cache、分页/filter、grant、安全、兼容和非目标。
- **TDD**：先写 reservation/operation、CAS、owner isolation、projection unavailable、UI route 和 E2E 失败测试，再实施最小行为。
