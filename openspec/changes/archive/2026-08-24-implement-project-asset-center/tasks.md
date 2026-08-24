## 0. 阶段一追溯与所有权取证

- [x] 0.1 在实现前核验已归档 Assets/AssetVersion/objectKey contracts，以及 StorageProfile/multipart、MediaInspection/Derivative、Scene/Shot、AssetEdit、Timeline/Export 的实际代码、Schema、Alembic head 和 owner API；缺失依赖保持 unavailable，不从总体 plan 推断运行时字段。
- [x] 0.2 建立 architecture tests，证明 Assets 只拥有 Asset metadata/AssetVersionReservation/AssetVersion，Storage 只拥有 session/object，Media Worker 只拥有 inspection/derivative，usage 只聚合 owner queries；禁止跨模块表写入、媒体 bytes、objectKey/presigned URL 泄漏和真实 Provider/TOS 隐式调用。
- [x] 0.3 记录完整非目标：物理删除/GC、文件夹/收藏、批量改标签、语义搜索、图片遮罩/选区、时间范围 Agent 编辑、独立音频审核、自动语义/视觉质检、统一审核中心和任何 owner 事实复制。

## 1. Assets Domain 与共享 Contracts

- [x] 1.1 先写失败测试，再扩展 `AssetCatalogMetadata` Schema/domain：sourceType、可选 catalogRole、有界 tags、authorization/license 字段、canonical `schema_version` 到 HTTP `schemaVersion` 单源映射、stable errors 和 Asset expectedRevision CAS。
- [x] 1.2 定义 `AssetVersionReservation`、fingerprint、`operationKey=asset-upload:{projectId}:{assetId}:{reservationId}`、`reserved|registered|cancelled|failed` 状态与只登记一次不变量；覆盖 foreign/stale/cancelled/metadata mismatch/duplicate registration。
- [x] 1.3 定义 cursor-paginated Asset catalog、bounded filter、processing summary、AssetVersion history、MediaProjection、AssetUsageProjection 与 unavailable/partial diagnostic 的共享 Zod/JSON Schema 正反 fixtures。
- [x] 1.4 编写 domain/contract tests，证明 Asset metadata 更新只产生新 Asset revision/audit，不修改 AssetVersion/StoredObject/历史 Timeline/Export provenance；版本、nested media 和 storage reference 继续 immutable。

## 2. Persistence、Application 与 HTTP

- [x] 2.1 新增可逆 additive Alembic/SQLAlchemy mappings，为旧 Asset 回填 `sourceType=imported`、空 tags、`authorizationStatus=unknown`，并增加 metadata constraints、reservation unique fingerprint/operation/state 和 migration round-trip tests；不改写既有 AssetVersion。
- [x] 2.2 扩展 Assets Repository/UoW ports 与 in-memory/SQLAlchemy adapters，实现 metadata CAS、reservation create/read/cancel/reconcile、单次 AssetVersion registration 和审计/Outbox；所有失败保持零部分写入。
- [x] 2.3 实现稳定 cursor `(updatedAt,id)`、kind/catalogRole/tag/sourceType/authorizationStatus/processingStatus 过滤、版本历史和 owner-safe project authorization queries；非法 cursor/filter、foreign scope 和数据库不可用返回稳定 422/404/403/503。
- [x] 2.4 增加 create/list/get/patch Asset、reservation、usage/media projection HTTP contract/BDD；响应只含安全摘要和 owner IDs/revisions/hashes，不含 bytes、base64、plaintext credential、objectKey/workspace URI 或持久 presigned URL。
- [x] 2.5 添加 `schema_version`/`schemaVersion` 缺失、冲突或双独立赋值的 UoW 前拒绝测试，证明零 Asset/reservation/session/version/audit/Outbox 写入。

## 3. Storage 上传恢复与注册交接

- [x] 3.1 将 reservation 交接到 Storage owner 的 create/resume/upload/complete/abort/reconcile commands，冻结 StorageProfile revision、project/bucket/key、declared MIME/size/checksum 与同一 operation key；不得直接导入 TOS SDK 或 fallback Local。
- [x] 3.2 先写 multipart/restart tests，再实现刷新、API/Worker 重启、timeout/submission-unknown 后同 session/reservation 恢复；same manifest 返回同一 StoredObjectRef/AssetVersion，conflict 保留原始 diagnostic。
- [x] 3.2a 定义 2 GiB explicit acceptance harness 与 UI states：先校验 profile part/object limits 和 resource capacity，默认快测使用逻辑 size/manifest fake，阶段退出以不入仓库的实际有效媒体执行 streaming multipart、中断/重启恢复、verification、单一 AssetVersion、inspection/proxy；记录 actual bytes/profile revision，unsupported 在 reservation/session/file read 前拒绝。
- [x] 3.3 覆盖 cancel/complete race、late object、registration failure、checksum/MIME/size mismatch、authorization/license failure 和 duplicate complete；取消或晚到结果不 append AssetVersion、不改变 Scene/Shot/Timeline current。
- [x] 3.4 以 `Mock Provider +` 显式 Local test/offline profile完成默认上传 contract；真实 TOS 只在已批准 StorageProfile、Docker Secret 和 explicit probe 下追加证据，不作为默认验收前置。

## 4. Media Projection、试听与使用位置

- [x] 4.1 实现只读 MediaInspection/Derivative query adapters，按 AssetVersion id/revision/hash 验证 proxy/thumbnail/keyframe/waveform readiness、tool/schema/version、retention/license/hold；pending/failed/stale 不签发可用 grant。
- [x] 4.2 实现短 TTL read-only preview/audio grant 与播放器 DTO，覆盖 play/pause/seek、波形惰性加载、grant expiry、foreign/unauthorized/stale source；浏览器和 server cache 不持久化 URL 或媒体 bytes。
- [x] 4.3 实现 `AssetUsageQuery` owner adapters，聚合 SourceMaterial、Scene/Shot、AssetEdit candidate/decision、Timeline Clip/SoundCue/TimelineVersion 和 Export manifest 的 exact IDs/revisions/hashes/state/deep link，不直接跨 owner 写表。
- [x] 4.4 添加 usage complete/partial/unavailable、foreign、historical/current/candidate/exported、owner timeout/schema drift tests；未知不得伪报未使用，也不得生成 delete proof 或清理对象。
- [x] 4.5 提供 Timeline selector handoff，仅发送同项目 AssetVersion id/revision/hash、target Episode 和 authorization summary；Clip/SoundCue 仍由 Timeline expectedRevision typed command 创建，失败零 Timeline/Asset mutation。

## 5. 项目资产中心 UI

- [x] 5.1 在已批准前端基础上实现 `/projects/:projectId/assets` route、`assetCenterApi`、TanStack Query keys/Zod contracts 和 Zustand 仅交互状态；禁止 localStorage 保存 owner state、upload session、URL、credential 或媒体 bytes。
- [x] 5.2 实现 loading/empty/error/unavailable、稳定分页、类型/catalogRole/tag/source/authorization/processing filters、Asset/版本详情、rights 状态和 owner revision；页面加载/筛选/切 tab 零业务 mutation。
- [x] 5.3 实现 create/resume/cancel/reconcile upload UI，显示分片、校验、StoredObject verification、AssetVersion registration 和失败阶段；刷新复用 owner reservation/session，取消 late result 不显示为已入库。
- [x] 5.4 实现缩略图/代理/关键帧/波形 readiness、音频 play/pause/seek、grant expiry/retry、版本对比和安全诊断；pending/failed/stale 派生物不可用且不改变 AssetVersion。
- [x] 5.5 实现只读 usage 列表与验证 deep links，以及到 workbench review/timeline 的 explicit selection handoff；owner unavailable 显示 partial/unavailable，不显示伪空集合。
- [x] 5.6 让 Timeline 素材箱复用资产中心 selector/query/upload 状态，不复制过滤、上传或版本事实；跨项目、未接受、未授权、stale 和 revision conflict 保持 owner 原始诊断。
- [x] 5.7 使用 shadcn/Radix、Tailwind、Lucide 完成桌面目录、筛选菜单、上传进度、版本/usage tabs、音频控件、键盘/焦点/aria 和稳定尺寸；不建立卡片嵌套或营销页面。

## 6. BDD、E2E 与严格验收

- [x] 6.1 添加 domain/application/adapter/HTTP/component/state E2E tests，覆盖上传恢复/取消、单一版本、目录过滤、metadata CAS、派生失败、试听、usage partial、foreign/authorization 和访问页面零副作用。
- [x] 6.2 将 `S08a asset center` 接入 `E2E-MVPA-001`：Local profile 上传图片/音频、一次中断恢复、目录筛选、试听、usage、Timeline selector handoff，并记录 owner prerequisites、success artifact、focused diagnostic 和 no-side-effect invariant。
- [x] 6.2a 将 actual 2 GiB upload/resume/verify/register/proxy evidence 作为独立环境门接入阶段退出报告；缺少实际 bytes、part manifest、checksum、单一版本或 capability/admission 证据时不得以逻辑 fake 报告通过。
- [x] 6.3 运行 `openspec instructions apply --change "implement-project-asset-center" --json`、`openspec status --change "implement-project-asset-center" --json`、本 change strict validation、全量 strict validation、Assets/Storage/Media/Timeline 定向 tests、Web typecheck/lint/format、`pnpm run check` 与 `git diff --check`；实现前保持全部任务未勾选。

## 7. 审查一致性修复

- [x] 7.1 增加显式 upload admission API/UI 回归：客户端必须先以 file size、part plan 与选定 StorageProfile 请求 owner capability/capacity 准入，再创建 reservation、读取或哈希文件内容；unsupported/foreign/stale/disabled profile 零文件读取和零业务写入。
- [x] 7.2 将浏览器 checksum 与 multipart 上传改为有界分片流式处理，禁止 `file.arrayBuffer()` 读取完整 2 GiB 文件；同一 reservation/session 的 part checksum、最终 checksum 与进度必须保持可恢复且可验证。

## DDD / BDD / SDD / TDD

- **DDD**：1.x/2.x 固化 Asset metadata/reservation 与 owner isolation。
- **BDD**：3.x/4.x/5.x/6.x 覆盖用户可观察上传、目录、试听和 usage 场景。
- **SDD**：Schema、DB、HTTP、query/command adapters、route/cache/security 均可追溯。
- **TDD**：每个实现任务先落失败 fixture，再完成最小行为并保留 strict/E2E 证据。
