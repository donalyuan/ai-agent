## Why

阶段一 PRD 要求用户可在项目内完成素材上传、恢复、浏览、筛选、试听、授权核对和使用位置追踪，但现有 active changes 只有 Storage multipart 后端、基础 Asset/AssetVersion API、Media Worker 派生物和 Timeline 内的最小素材箱，缺少可独立操作的项目级资产闭环。该缺口会使“上传素材并成为可追踪 Timeline 输入”仍依赖隐藏 API 或人工拼接，无法完成 MVP-A 用户验收。

## What Changes

- 扩展 `Asset` 目录元数据：来源类型、媒体分类、关键字/标签、授权/许可证状态和 revision；`AssetVersion` 继续只追加且只保存验证后的元数据与存储引用。
- 新增项目级资产中心路由与查询，支持按类型、标签、来源、授权和处理状态筛选，查看不可变版本、代理/缩略图/关键帧/波形状态、音频试听和只读使用位置。
- 将 UI 上传流程接到 Storage owner 的 create/resume/complete/abort/reconcile 与 Assets owner 的单次 append；刷新、失败恢复和取消复用同一 UploadSession/operation key，不重复上传或创建第二 AssetVersion。
- 增加 2 GiB 实际媒体链路的显式验收模式：先展示 profile/capacity preflight，再以同一 reservation/session 完成分片、中断恢复、对象校验、单一 AssetVersion 登记和代理派生；该规模是验收样本，不是产品最大值。
- 只读取 Media Worker 的 `MediaInspection`/`MediaDerivative`，不在资产中心生成、覆盖或伪造代理、缩略图、关键帧、波形和媒体信息。
- 提供 Workbench、候选审核和 Timeline 的稳定深链/选择交接；资产中心不创建 Scene/Shot/Timeline 引用，也不拥有 ProviderCall、RunEvent、ExportJob 或 StorageObject。
- 明确 MVP-A 不含媒体删除/GC、文件夹/收藏、批量改标签、语义搜索、独立音频审核、自动语义/视觉质检或统一审核中心。

## Capabilities

### New Capabilities

- `project-asset-center`: 项目级资产目录、上传恢复、筛选、试听、派生状态、版本和使用位置 UI/应用闭环。

### Modified Capabilities

- `assets-slice`: 为 Asset 目录增加来源、分类、标签和授权/许可证元数据，同时保持 AssetVersion append-only 与媒体 bytes 禁止边界。
- `assets-http-api`: 增加带项目归属、过滤、分页、CAS 元数据更新和只读 usage projection 的资产中心 API 合同。

## Impact

- 影响 `apps/web` 的项目资产中心路由、Query/Zod/state、上传和试听 UI。
- 影响 `services/api` 的 Assets domain/application/repository/HTTP、additive migration、使用位置 projection 与测试。
- 只消费 `integrate-tos-storage-provider` 的 Storage lifecycle、`implement-episode-timeline-audio-export` 的 MediaInspection/Derivative，以及既有 Scene/Shot/Timeline/Export 引用事实；不接管这些 owner。
- 默认测试使用 `Mock Provider +` 显式 Local test/offline profile；真实 TOS 仍仅通过独立 explicit probe。

## DDD / BDD / SDD / TDD

- **DDD**：Asset 管目录元数据和不可变版本；Storage 管 session/object；Media Worker 管派生物；usage 是跨 owner 的只读 projection。
- **BDD**：用户可上传/续传/取消/恢复、筛选、试听、查看处理状态和使用位置；foreign、stale、未授权、重复完成和派生失败均有可观察诊断且零越界写入。
- **SDD**：新增 additive metadata、查询/分页/过滤/usage DTO、项目路由、上传交接和 owner reference contracts；不保存媒体 bytes 或 presigned URL。
- **TDD**：先写 domain/contract/HTTP/UI/E2E 失败测试，再实现最小闭环，并把资产中心证据纳入 `E2E-MVPA-001`。

## 阶段一资产目录组件边界

资产目录和 usage 表格 MUST 使用 TanStack Table + TanStack Virtual，并复用创作工作台建立的 `shared/ui`、语义 tokens 和 Lucide 控件。Table 只负责稳定 cursor 行、筛选/排序和 usage 投影；Virtual 只负责有界 DOM，不能复制 AssetVersion、UploadSession、StoredObject 或 usage owner facts。音频 play/pause/seek 和波形仍是资产中心领域功能，不进入 `shared/ui`。

验收必须覆盖长目录/usage 虚拟化、版本与授权诊断、上传恢复状态、音频试听、owner unavailable/partial 和 Timeline selector handoff；页面读取、筛选、usage tab 和音频控件不触发真实 TOS/Provider 或其他隐式业务 mutation。
