# project-asset-center Specification

## Purpose
TBD - created by archiving change implement-project-asset-center. Update Purpose after archive.
## Requirements
### Requirement: 项目级资产目录与筛选
系统 SHALL 在 `/projects/:projectId/assets` 提供项目归属明确、cursor 分页的资产目录，显示 Asset/当前 AssetVersion 的 stable ID、revision、kind、catalogRole、sourceType、tags、authorization/license、processing status、版本数量和缩略图/波形 readiness。过滤 MUST 至少支持 kind、catalogRole、tag、sourceType、authorizationStatus 和 processingStatus，并以 `(updatedAt,id)` 作为稳定排序 tie-breaker。页面加载、分页和筛选 MUST 为只读，MUST NOT 创建上传、版本、ProviderCall、RunEvent 或派生任务。

#### Scenario: 浏览和筛选项目资产
- **WHEN** 用户打开同项目资产中心并组合类型、标签、来源、授权和处理状态过滤
- **THEN** UI 只显示匹配且有权读取的项目资产、稳定分页 cursor 和 owner revision，不因读取产生任何业务 mutation

#### Scenario: 拒绝 foreign 项目资产
- **WHEN** 路由、cursor、filter 或 Asset/AssetVersion 属于其他项目或不可授权
- **THEN** owner 返回稳定 not-found/forbidden/validation diagnostic，UI 不显示名称、objectKey、usage 或预览 grant

### Requirement: 可恢复且可取消的上传闭环
系统 SHALL 以 Assets owner 的 `AssetVersionReservation` 和 Storage owner 的 `operationKey=asset-upload:{projectId}:{assetId}:{reservationId}` 串联 create/resume/complete/abort/reconcile。刷新、API/Worker 重启或 timeout 后 MUST 复用同一 reservation/session；verified StoredObjectRef MUST 由 Assets owner 幂等 append 一次 AssetVersion。取消或 late completion MUST NOT 自动 append 或替换 current reference。

#### Scenario: 刷新后续传并只登记一个版本
- **WHEN** multipart 上传中断或状态未知后，用户以同一 reservation/operation 恢复并完成相同 part manifest
- **THEN** 系统复用 UploadSession，校验对象，并返回同一 AssetVersion；不创建第二对象、reservation 或版本

#### Scenario: 取消上传并处理晚到结果
- **WHEN** 用户明确取消活动上传，随后 Storage 返回 late terminal object
- **THEN** reservation/session 显示 cancelled 或 reconciled-unreferenced，晚到对象保持未引用且可审计，不 append AssetVersion、不替换 Scene/Shot/Timeline 引用

### Requirement: 2 GiB 媒体上传与代理链路证据
资产中心 SHALL 在 explicit acceptance mode 以 `2_147_483_648` bytes 作为实际媒体 fixture 下限，先显示并验证 StorageProfile object/part limits 和 operations-resilience capacity admission，再使用同一 reservation/session/operation 完成 streaming multipart、一次中断/刷新或 Worker 重启恢复、complete/stat/checksum/MIME verification、单一 AssetVersion registration 和 Media Worker inspection/proxy。2 GiB MUST 只表示验收规模，不表示平台 maximum。默认快速 tests MAY 使用逻辑 size/part manifest fake，但阶段退出 MUST 包含一次 actual-byte evidence，fixture MUST NOT 提交仓库。

#### Scenario: 2 GiB 上传恢复后只登记一个版本
- **WHEN** explicit Local/TOS test profile 和容量支持实际 2 GiB fixture，上传中断后以同一 reservation/session 恢复完成
- **THEN** UI 显示 actual bytes、part progress、verification、唯一 AssetVersion 和 matching proxy readiness，且没有第二对象、版本或 operation

#### Scenario: 不支持 2 GiB 时前置失败
- **WHEN** profile object/part limit、part count 或 workspace capacity 不满足 fixture
- **THEN** UI 在 reservation/session/part/workspace 写入前显示 owner limit/admission diagnostic，不读取完整文件、不截断/拆分、不切换 adapter、不伪造通过

### Requirement: 版本、授权和元数据 CAS
资产中心 SHALL 显示全部不可变 AssetVersion 及 checksum、MIME、size、duration/encoding、storage/inspection provenance 的安全摘要。Asset 目录元数据修改 MUST 提交 `expectedRevision` 并产生新 Asset revision/audit；MUST NOT 修改历史 AssetVersion、StoredObject、Timeline/Export 中已冻结的授权或 provenance。

#### Scenario: 更新标签和授权元数据
- **WHEN** 用户以当前 expectedRevision 修改单个 Asset 的 tags、catalogRole 或 authorization/license metadata
- **THEN** Assets owner 原子保存新 revision 和审计，列表刷新为 owner state，全部 AssetVersion 内容和历史导出引用保持不变

#### Scenario: stale 元数据更新零覆盖
- **WHEN** expectedRevision 过期、标签/枚举无效或授权字段冲突
- **THEN** 返回 409/validation diagnostic，零 Asset、AssetVersion、usage、Storage 和 Outbox 部分写入

### Requirement: Media Worker 派生状态与安全预览
资产中心 SHALL 只消费 Media Worker 为精确 AssetVersion id/revision/hash 生成的 `MediaInspection`/`MediaDerivative`。proxy、thumbnail、keyframe index 和 waveform 仅在 status=`ready`、source fingerprint 匹配、project/authorization 有效时签发短 TTL read-only grant；pending/failed/stale MUST 显示原始诊断且不得伪装 ready。

#### Scenario: 显示 ready 缩略图和波形
- **WHEN** 同项目当前 AssetVersion 的 inspection、thumbnail 和 waveform 均 ready 且 fingerprint 匹配
- **THEN** UI 惰性读取安全预览/波形 grant，并显示工具/schema/version，不暴露 objectKey 或长期 URL

#### Scenario: 派生失败不污染版本事实
- **WHEN** derivative pending/failed/stale、hash/revision 不匹配或 grant 过期
- **THEN** 对应预览/波形/试听禁用并显示 diagnostic，AssetVersion、accepted current 和 Timeline reference 保持不变

### Requirement: 音频试听和 Timeline 选择交接
同项目 audio AssetVersion 在 authorization 有效且 owner 提供 ready 可播放 reference 时 SHALL 可 play/pause/seek，并显示时长、波形、catalogRole 和 license。将音频或媒体交给 Timeline MUST 只发送稳定 AssetVersion id/revision/hash 和目标 Episode，最终 Clip/SoundCue 创建仍由 Timeline typed command 与 expectedRevision 决定。

#### Scenario: 试听已验证音频
- **WHEN** 用户选择同项目、授权有效且 ready 的 audio AssetVersion
- **THEN** 播放器使用短 TTL read grant 试听并同步波形，不保存媒体 bytes 或 presigned URL 到 localStorage/Query persisted cache

#### Scenario: 无效选择不创建 Timeline 引用
- **WHEN** 音频 foreign、未授权、stale、derivative 未 ready或目标 Episode 不匹配
- **THEN** selector 显示 owner diagnostic，且不创建 Clip、SoundCue、TimelineVersion 或 AssetVersion

### Requirement: 精确且只读的使用位置
系统 SHALL 通过各 owner query ports 聚合 AssetVersion 的 SourceMaterial、Scene/Shot、AssetEdit candidate/decision、Timeline Clip/SoundCue、TimelineVersion 和 Export manifest 引用，返回 reference type、owner ID/revision、scope/state、source hash 与验证后的 deep link。usage projection MUST NOT 成为引用事实源；owner 不可用时 MUST 返回 unavailable/partial diagnostic，不得伪报“未使用”。

#### Scenario: 查看当前和历史使用位置
- **WHEN** 用户打开同项目 AssetVersion 的 usage view 且各 owner query 可用
- **THEN** 系统按精确 owner revision/hash 区分 current、historical、candidate 和 exported references，并提供安全 deep link

#### Scenario: usage owner 不可用时 fail closed
- **WHEN** 任一必需 owner query timeout、schema 不兼容或 revision 无法验证
- **THEN** usage view 显示 `usage_projection_unavailable` 或明确 partial 状态，不返回伪空集合、不删除对象、不签发 delete proof

### Requirement: 资产中心边界与阶段一验收
资产中心 SHALL 复用 owner contracts，不拥有或复制 UploadSession、StoredObject、MediaInspection、MediaDerivative、Scene/Shot、Timeline、Export、ProviderCall 或 RunEvent。默认测试 MUST 使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），页面读取零网络/Provider/TOS mutation；真实 TOS 只走 explicit probe。`E2E-MVPA-001` MUST 记录 `S08a asset center` 的上传恢复、目录筛选、试听、usage 和 no-side-effect 证据。

#### Scenario: 完成资产中心 focused E2E
- **WHEN** 测试在显式 Local profile 上传一张图片和一段音频、恢复一次中断、筛选目录、试听并查看 usage
- **THEN** 报告记录 owner prerequisites、同一 reservation/operation、单一 AssetVersion、ready/failed derivative、usage exact refs、focused diagnostic 和零重复/越界写入

#### Scenario: 页面访问不触发外部副作用
- **WHEN** 用户只打开资产中心、切换筛选、版本或 usage tab
- **THEN** 不创建/切换 Profile，不调用真实 TOS/Provider，不创建 UploadSession/ProviderCall/RunEvent/AssetVersion/derivative
