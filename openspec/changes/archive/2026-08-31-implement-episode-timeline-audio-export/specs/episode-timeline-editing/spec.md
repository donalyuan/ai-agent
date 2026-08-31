## ADDED Requirements

### Requirement:总体计划追溯和协调边界
本 change SHALL 反向追溯到 `plan-phase-one-drama-mvp-a` 的总体任务 `1.2`、直接实施任务 `4.1`--`4.3` 和共享任务 `5.1`--`5.8`。实施 MUST 以总体任务 `2.1`、`2.2`、`2.3` 的交付及已归档 AssetVersion 契约为可核验前置；总体 plan 仅协调 change 顺序、范围和验收，MUST NOT 成为运行时代码依赖。完整非目标是 MVP-B `portable` payload、Fish Audio、Groq ASR、自动字幕/自动对齐、专业 NLE、回导、发布平台和跨集自动拼接，以及接受 `profile`/`export_profile` 等 `exportProfile` 别名或让 DB/manifest/HTTP 各自维护版本源。真实 `ffmpeg`/`ffprobe` adapter 与 explicit probe 是本 change 必需交付；Mock/unconfigured 仅用于测试或诊断，绝不报告成功。9:16/16:9/1:1、1080p、30fps、H.264 `yuv420p`、AAC 48 kHz、UTF-8 SRT、四类音轨、light 引用和 ExportJob 状态已冻结；本 change MUST NOT 承担其外的职责。

#### Scenario:前置或范围不满足时保持边界
- **WHEN** 实施取证发现前置切片尚未可用，或请求把总体 plan/MVP-B 能力作为 Timeline/export runtime 依赖
- **THEN** 实施显式阻塞或拒绝该请求，记录缺失前置；不得伪造 resolver、导入总体 plan 模块或产生 portable package

#### Scenario:拒绝完整非目标职责泄漏
- **WHEN** Timeline 编辑尝试承担任一列明的非目标、接受 profile 别名、忽略已冻结的 30fps/render contract，或将 `renderer_unconfigured` 报告为成功
- **THEN** 架构依赖/契约测试失败，且不写 current Cut、TimelineVersion、Clip、审计或 Outbox

### Requirement:Episode 范围内的唯一 current Cut 与不可变 TimelineVersion
系统 SHALL 为每个 Episode 持久化恰好一个 mutable current Cut，并允许发布、命名、读取和只读比较多个不可变 `TimelineVersion`。系统 MUST NOT 创建 `TimelineDraft`，也 MUST NOT 提供第二个 mutable Cut 或创建、选择、切换 Cut 的生命周期。current Cut 与 TimelineVersion MUST 含稳定 ID、持久化 `schema_version`、HTTP DTO `schemaVersion`、revision、项目/剧集归属和审计时间；两种字段名 MUST 映射同一 canonical 版本值，不得形成两个版本源。

#### Scenario:编辑 current Cut 并发布命名版本
- **WHEN** 用户以当前 revision 编辑现有 Episode 的 current Cut，随后通过 preflight 并发布一个命名版本
- **THEN** 系统立即持久化该唯一 current Cut 的新 revision，并追加不可变 TimelineVersion；既有命名版本保持不变，且不创建或切换第二个 mutable Cut

#### Scenario:拒绝跨 episode timeline mutation
- **WHEN** 请求以其他项目或 Episode 的 ID 读取或修改 current Cut
- **THEN** 系统返回 not_found 或 forbidden/episode_mismatch，且不改变 current Cut

#### Scenario:拒绝 schema 版本来源冲突
- **WHEN** 写入或读取映射中的持久化 `schema_version`、manifest `schema_version` 与 HTTP DTO `schemaVersion` 缺失或值不一致
- **THEN** 系统返回 validation，且不写入或返回相互冲突的 Timeline/Export 版本事实

### Requirement:整数帧 clip 编辑
系统 SHALL 仅以整数帧存储 Clip 的排序、source in/out、timeline start 与 duration，并以单一静态值存储 `position`、`scale`、`opacity`。系统 MUST 支持显式排序、裁剪、拆分和 `DeleteClip`，并保留来源关系；`DeleteClip` MUST 只删除 Timeline reference，绝不删除或覆盖 AssetVersion。不得接受浮点帧、负值、零长度、越界裁剪、未定义的轨道重叠或任何 keyframe/animation payload。

#### Scenario:trim 和 split 一个 clip
- **WHEN** 用户对同集可用素材 Clip 提交合法整数帧裁剪或拆分命令
- **THEN** 系统产生满足边界的新 Clip 状态、稳定排序和递增 revision，原已发布 Version 不变

#### Scenario:拒绝无效帧范围
- **WHEN** Clip 使用非整数帧、`outFrame <= inFrame`、负值或超过素材可用帧
- **THEN** 系统返回 validation 或 `frame_out_of_bounds`，不写入部分编辑

### Requirement:immutable version 与 current revision 并发
系统 SHALL 从 current Cut 发布可命名的不可变 `TimelineVersion`，允许读取和只读比较多个已发布版本，并要求写命令携带 expected revision。过期 revision MUST 返回 409 且不覆盖 current Cut 或任何 version。

#### Scenario:发布 current cut
- **WHEN** current Cut 通过 timeline preflight 且请求携带当前 revision
- **THEN** 系统保存新的不可变 Version，记录其 Clip/音频/字幕快照和来源 revision

#### Scenario:拒绝 stale 编辑
- **WHEN** 两个客户端针对同一 Cut 发送编辑，其中一个 expected revision 已过期
- **THEN** 过期请求返回 `revision_conflict` 409，已有编辑保持不变

### Requirement:立即持久化与 authoritative 409 恢复
系统 SHALL 让 `TrimClip`、`SplitClip`、`ReorderClips`、`DeleteClip`、`SetClipTransform`、`SetSoundCueMix`、`SetDuckingPolicy` 与 `UpsertManualCaption` 都携带 `expectedRevision` 并在成功时于同一 UoW 立即持久化 current Cut revision、审计和 Outbox。系统 MUST NOT 提供独立自动保存、通用 undo/redo 或版本恢复；409 MUST 零部分写入并返回 authoritative current revision/state。

#### Scenario:删除并持久化 clip 编辑
- **WHEN** 用户以 current expected revision 提交 `DeleteClip` 或任一合法静态编辑 command
- **THEN** owner 立即返回新的 Cut revision 和持久化 state，且已发布 TimelineVersion 与 AssetVersion 保持不变

#### Scenario:恢复 stale 的立即编辑
- **WHEN** command 使用过期 revision 或包含 keyframe/unknown transform field
- **THEN** owner 返回 `revision_conflict` 409 或 validation、未写入任何部分编辑；客户端必须丢弃乐观状态并重新读取 authoritative Cut

### Requirement:固定 30fps 整数帧来源充分性
系统 SHALL 以 30fps 整数帧计算 Clip 和字幕时间。任一 clip 的 source range、duration 或 timeline coverage 不足时 MUST 显式失败，MUST NOT 插入黑帧、延展最后帧或静默变更 duration。

#### Scenario:拒绝不足的来源覆盖
- **WHEN** 剪辑命令或预检发现素材帧数不足以满足 published TimelineVersion
- **THEN** 系统返回 `missing_asset` 或 `frame_out_of_bounds`，不写部分编辑或导出替代画面

## DDD / BDD / SDD / TDD

- **DDD**：Episode 拥有 timeline 聚合，current cut 与 immutable version 是领域不变量。
- **BDD**：场景覆盖每集隔离、一个 current、帧编辑和并发失败。
- **SDD**：Schema/DB/API 使用 UUID、整数和约束，保留现有资产兼容边界。
- **TDD**：帧算术、状态守卫和 compare-and-set 先由 domain/application 测试覆盖。

## Current / Defined / Todo

- **Current**：仅基础 Timeline Schema/ORM 占位。
- **Defined**：上述 timeline 编辑契约。
- **Todo**：实现 ports、迁移、接口和测试。

### Requirement:符合 eligibility 的装配与引用 command
系统 SHALL 以带 `expectedRevision` 的 owner commands add/remove image/video Clip 与 add/remove `dialogue|music|ambience|effects` SoundCue。image/video 只可引用同项目、当前 selected Episode、经 TextReview/媒体审核或 AssetEdit accept 成为 current storyboard/reference 的 immutable AssetVersion；audio library 必须同项目、已 storage verify、由 Assets append、authorization/license 完整且用户通过项目资产中心 selector/query 显式选择。selector handoff 只携带 project、AssetVersion id/revision/hash、authorization summary 与 derivative fingerprint；Timeline MUST NOT 复制上传 session、目录 filter、Asset metadata revision、usage 或 MediaDerivative owner state。remove MUST 只删 Timeline reference，不删/覆盖 AssetVersion。

#### Scenario:导入并添加背景音乐
- **WHEN** Local/TOS upload 已产生 verified StoredObjectRef、Assets 已 append audio AssetVersion，且用户显式选择该 project library item
- **THEN** add music SoundCue 成功；任一前置失败不产生 cue/clip 或成功响应

#### Scenario:拒绝无效装配
- **WHEN** asset 未接受、foreign、other Episode、缺 license/authorization、重复、或 expectedRevision 过期
- **THEN** command 返回稳定 validation/conflict，且不改写 Timeline 或 AssetVersion

#### Scenario:资产中心 projection 不可用时不猜测选择
- **WHEN** selector handoff 为 stale/foreign/unauthorized、usage/media projection unavailable，或 id/revision/hash/fingerprint 不匹配
- **THEN** Timeline 返回 owner diagnostic，不创建 Clip/SoundCue、不复制资产目录状态，也不触发第二次上传或派生生成

### Requirement:绑定来源的 proxy 预览与最终 parity
系统 SHALL 将 immutable `ProxyRendition`/`PreviewManifest`/`PreviewArtifact`（或语义等价派生事实）绑定 current Cut identity/revision 或 TimelineVersion、timelineFingerprint/renderPlanHash；current Cut 变化 MUST 令 preview stale 并暂停。preview 和 FFmpeg MUST 共用 canonical RenderPlan/compiler，其 AssetVersion IDs、排序、source/timeline ranges、transform/basic audio、caption、duration/format facts 一致，且不复用 ExportJob 状态。

#### Scenario:验证 golden preview parity
- **WHEN** media adapter 运行显式 golden sample
- **THEN** 关键帧 SSIM >= 0.98、总时长容差 <= 1 frame、字幕边界和音频 onset/sync <= 1 frame；MVP-A 不承诺逐像素/逐采样一致、4K/高码率浏览器渲染、专业监看或复杂特效

### Requirement:Timeline 只消费已接受 video 与 ready derivative
Timeline SHALL consume only scenes owner 的 accepted current video eligibility plus media worker `MediaDerivative` records with matching source AssetVersion id/revision/hash and status `ready`. Unaccepted/pending/rejected/retake/stale/foreign video candidates、missing inspection, derivative failure or source fingerprint mismatch MUST block Clip/proxy/Timeline handoff before any Timeline or Export mutation.

#### Scenario:proxy 使用 media worker derivative
- **WHEN** 为 TimelineVersion 选择 accepted current video 和匹配的 ready proxy/keyframe/waveform/metadata record
- **THEN** Timeline 绑定 source fingerprint、derivative id 和 current Cut revision；它不重新生成或改写 media derivative fact

#### Scenario:来源变更使预览失效
- **WHEN** current video、AssetVersion revision/hash、derivative source fingerprint 或 current Cut revision 变化
- **THEN** preview/proxy 标记为 stale 并要求显式 refresh；不报告 export success 或 implicit current replacement
### Requirement:基础视觉转场
MVP-A MUST 只通过 canonical RenderPlan/compiler 支持 `cut` 和 `crossfade`，使用整数 30fps frame、有界 transition duration 和 Clip adjacency/overlap invariant。

#### Scenario:Renderer parity 失败阻断成功
- **WHEN** preview 和 FFmpeg compilation 与 canonical RenderPlan 偏离
- **THEN** preview/export 不报告 success；wipe、mask、auto transition、keyframe 和 audio crossfade 均 out of scope。

### Requirement:导出诊断必须可定位到 owner fact
Timeline/export preflight SHALL 为每个失败项返回 `ExportDiagnosticTarget`，其 `targetType` 仅为 `timeline|clip|caption|sound_cue|asset_version|renderer|storage|artifact`，并携带同项目 Episode/TimelineVersion 以及适用的精确 owner ID/revision、frame/fieldPath 和 owner-validated route token。调用方 MUST NOT 从 message 文本、数组位置或 display name 猜测定位；published TimelineVersion MUST 保持只读，renderer/storage 全局问题只能跳到项目设置。

#### Scenario:缺失素材或字幕越界可跳到问题位置
- **WHEN** preflight 发现 Clip/Caption/SoundCue/AssetVersion 缺失、越界、stale 或授权失败
- **THEN** diagnostic 指向精确 owner fact/revision 和安全 route token，UI 可定位并修复 current Cut；失败不创建 ExportJob/Artifact 或修改 published Version

### Requirement:packaging 包含 artifact 上传校验与登记
ExportJob.status MUST 继续只允许 `queued|preflighting|rendering|packaging|succeeded|failed|cancel_requested|cancelled`。rendering 完成后 `packaging` SHALL 公开 `uploading|verifying|registering` progress subphase，并为 MP4、SRT、light 分别使用冻结 StorageProfile 和 export operation key 执行 StoragePort upload、stat/checksum/MIME/size verification，再由 Timeline/export owner 追加独立 `ExportArtifact`。只有三个 artifact 全部 verified/registered 且 manifest refs 精确匹配时才可 `succeeded`；unknown MUST 先 reconcile，失败 MUST NOT 重渲染、重复上传、伪造 artifact 或 fallback Local/TOS。

#### Scenario:上传并登记三个独立导出产物
- **WHEN** renderer 已产生有效 MP4/SRT/light 输出，Storage upload、verification 与 owner registration 全部成功
- **THEN** Job 在 packaging 中依次报告 upload/verify/register progress，追加三个独立 ExportArtifact 后转为 succeeded，响应不暴露 objectKey 或持久 URL

#### Scenario:上传响应未知时不重复产物
- **WHEN** artifact upload/complete 响应丢失或 registration 状态未知
- **THEN** worker 以同一 operation key/stat/checksum 先 reconcile，保留 packaging/retryable diagnostic，不重渲染、不创建第二对象/Artifact、不切换 profile 或报告 succeeded
