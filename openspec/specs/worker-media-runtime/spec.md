# worker-media-runtime Specification

## Purpose
TBD - created by archiving change close-phase-one-mvp-a-runtime-gaps. Update Purpose after archive.
## Requirements
### Requirement:Generation Worker activities

Generation Worker SHALL 注册 Text、Image、Video submit、poll、cancel、result-registration 和 reconcile activity；activity MUST 从冻结 Run/Node/Provider/Capability 输入执行，并通过现有 owner command/handoff 写回候选、ProviderCall、VideoOperation 或诊断。Video 的 submit/poll/cancel/result-registration 均须绑定同一 `run_id + logical_operation`，每个动作可单独重试且不得重复外部副作用。

#### Scenario:Text activity 完成审核门
- **WHEN** matching running text NodeRun 具有有效 frozen selection 且 Text activity 返回完整结构化候选图
- **THEN** 只持久化一个幂等 TextReviewBatch，NodeRun/Run 进入 `waiting_review`，且 API 不等待模型完成

#### Scenario:Image 和 Video activity 进入审核
- **WHEN** image/video operation 具有 accepted prerequisite、BudgetGate 和有效 capability snapshot
- **THEN** Image activity 产生可审核 image candidate，Video submit/reconcile 产生可审核 VideoTakeCandidate 或稳定失败诊断，不直接写 accepted current

#### Scenario:Agnes 四动作生命周期闭合
- **WHEN** Agnes VideoOperation 由 submit activity 进入 `submitted|running`
- **THEN** poll activity 按冻结 request/correlation 查询并单调保存 observation，cancel activity 对同一 logical operation 至多发出一次取消请求，result-registration activity 在 terminal result 下载并通过 Storage/MIME/hash/size 检查后登记一个 immutable AssetVersion 与一个 pending_review VideoTakeCandidate；重复 poll/cancel/result、Worker 重启和晚到结果均复用同一 owner identity，不能产生第二次提交、取消、AssetVersion 或 candidate

### Requirement:冻结执行路由与前向切换

每个 NodeRun/operation SHALL 冻结 `executionRoute`、`workflowType`、`taskQueue` 与 `schemaVersion`。切换后新 Text/Image/Video intent MUST 只派发至 Generation Worker；既有 Agent workflow 与 direct Text HTTP 路径 MUST 只 drain/reconcile 已冻结 legacy operation，MUST NOT 接收新 route/schema intent。回滚 MUST 按冻结 route 完成存量，禁止旧 Worker 接管新 operation。

#### Scenario:cutover 期间重复派发
- **WHEN** API、Agent dispatcher 与 Generation dispatcher 在切换或重启期间同时观察到同一 logical operation
- **THEN** 只有 snapshot 指定的 route 可启动稳定 workflow，新旧 dispatcher、Temporal `AlreadyStarted` 和 late completion 最终只产生一个 ProviderCall/TextReviewBatch

#### Scenario:direct HTTP 提交文本生成
- **WHEN** 新 route 生效后客户端调用文本生成 mutation
- **THEN** API 只持久化 command/owner ledger/outbox 并返回 operation，不同步 await TextGenerationService，也不向 Agent legacy queue 派发

### Requirement:Media Worker activity ownership

Media Worker SHALL 注册 MediaInspection、proxy/thumbnail/keyframe/waveform Derivative、Render 和 Storage upload/verify activity；dispatch MUST 先解析带 discriminator 的 `MediaDispatchAdmission`：`uploaded_source|asset_center` 仅要求同 scope 的 verified StoredObjectRef、AssetVersion id/revision/content hash/provenance 和可解析技术输入，可执行 inspection/通用 derivative；`generated_candidate` 额外要求 accepted candidate/review decision、Scenes owner exact current CAS、AssetVersion id/revision/hash/provenance 与 project/episode/scene/shot scope。Timeline、Render 和 Export handoff 只消费 accepted current 与 ready derivative，Media Worker 不拥有 Scene/Shot/AssetVersion/Timeline current。

#### Scenario:检查与派生成功
- **WHEN** Media Worker 读取同项目已验证 StoredObjectRef，且 admission discriminator 对应的 scope、AssetVersion/provenance 和技术输入精确匹配并完成 bounded inspection
- **THEN** 它写入 canonical MediaInspection 和可验证 ready derivative reference，来源 fingerprint 与 AssetVersion revision/hash 精确匹配

#### Scenario:普通上传素材不要求候选审核
- **WHEN** `uploaded_source|asset_center` AssetVersion 已完成 Storage verify，且不属于 storyboard generated candidate
- **THEN** MediaInspection 与通用 proxy/thumbnail/waveform derivative 可以派发；系统不要求 ProviderCall、review decision 或 Scenes current，但 Timeline/Render/Export handoff 仍拒绝未 accepted-current 的素材

#### Scenario:未接受或过期生成候选不派发
- **WHEN** `generated_candidate` 为 pending/rejected/retake/stale/foreign，或 review/current/AssetVersion/provenance 任一不匹配
- **THEN** 系统在 Media outbox/activity 前拒绝，且 MediaInspection、Derivative、Timeline 和 Export mutation 均为零

#### Scenario:派生失败不污染 owner current
- **WHEN** inspection、derivative 或 storage upload 失败
- **THEN** 记录 owner diagnostic/失败状态，Timeline handoff 和 export 被阻断，但 accepted Scene/Shot current 与 AssetVersion 不被撤销或覆盖

### Requirement:External side effects are activity-only

Temporal workflow SHALL 只保存确定性状态、重试策略和 activity 参数；Provider SDK、TOS SDK、数据库、文件和 FFmpeg MUST 只在 activity/adapter 中执行。API command MUST 返回 operation id 而不等待媒体完成。

#### Scenario:API 创建已通过前置准入的异步 operation
- **WHEN** 用户提交 Text/Image/Video/Media/Export command，且 catalog runnable/policy 与适用 RuntimeResourceSnapshot/CapacitySnapshot admission 均通过
- **THEN** API 冻结 admission references，在 UoW 中写入 owner intent、对应 owner ledger 和 outbox 后返回 queued/pending 状态，不执行同步 Provider/TOS/FFmpeg 调用

#### Scenario:资源或 policy 准入失败时零 intent
- **WHEN** resource probe unavailable/capability unsupported、CapacitySnapshot hard limit、Provider concurrency/rate/quota admission 或完整 runnable gate 任一失败
- **THEN** 系统在 intent、ProviderCall、UploadSession、ExportJob、AssetVersion、Outbox 和外部调用前返回稳定 diagnostic，不切换 Worker/adapter 或 fallback

#### Scenario:Worker 重启继续待处理 operation
- **WHEN** API 或 Worker 在 outbox 已提交、activity 未完成时重启
- **THEN** 新 Worker 通过稳定 workflow id、冻结 resource/capacity/policy admission references 和对应 owner 的持久 ledger 继续或 reconcile 同一 operation，不创建第二个逻辑操作

### Requirement:Owner-specific ambiguous result reconciliation

Text/GPT Image ProviderCall SHALL 只使用既有 `pending|succeeded|failed|unknown|cancelled`，ambiguous submit 保持 `unknown`；Agnes VideoOperation SHALL 只使用既有 `pending|submitted|running|submission_unknown|succeeded|failed|cancelled`，ambiguous submit 保持 `submission_unknown`。二者使用预先持久化的 attempt/outbound correlation，并按冻结 capability 执行 remote lookup。Storage SHALL 只使用既有 UploadSession `active|completed|aborted|unknown|failed` 与 handoff/recovery `reconciliation_required|failed|aborted|resolved`；StoredObject 仍为 immutable verified reference。ExportJob SHALL 保持既有八态，并只在 `packaging` 的 `uploading|verifying|registering` subphase、diagnostic 与 Storage operation 中表达不确定性；ExportArtifact 仍为 `pending|verified|failed|held`。各 reconcile activity 只能查询本 owner 的 request/session/operation，不得新增、重命名、迁移或统一状态，也不得自动重新提交。

#### Scenario:Provider 未知提交最终可查询
- **WHEN** transport timeout 发生且无法判断 provider 是否接受 request
- **THEN** Text/GPT Image ProviderCall 保持 `unknown`，Agnes VideoOperation 保持 `submission_unknown`，并分别记录 request fingerprint/outbound correlation；支持 lookup 时查询后按各自既有状态值幂等 finalize，不支持时关闭自动 retry/re-submit并保持人工可诊断，外部接受次数至多一次

#### Scenario:Storage 或 Export packaging 结果不确定
- **WHEN** multipart complete、export upload、verify 或 register 响应丢失
- **THEN** Storage 以原 session/operation 进入既有 unknown/reconciliation 状态，ExportJob 保持 `packaging` 及原 subphase/diagnostic，ExportArtifact 保持既有状态；owner reconcile 后再推进，不创建 `submission_unknown` 顶层状态、不重复对象/Artifact 或重渲染

#### Scenario:明确失败可重试
- **WHEN** transport 返回明确未提交的 retryable error
- **THEN** owner 按同一 logical operation 重试或保留可诊断失败，重试不产生第二个 owner fact/StoredObject registration

### Requirement:FFmpeg/ffprobe capability gate

Media Worker SHALL 在真实 render 前显式 probe `ffmpeg`/`ffprobe` binary version、H.264 decoder/encoder、AAC decoder/encoder、`yuv420p` 和 MP4 mux/demux/container；缺失或不支持 MUST 返回 `renderer_unconfigured`/`renderer_capability_unsupported`。

#### Scenario:renderer capability 可用
- **WHEN** probe 通过且冻结 snapshot 与 Timeline RenderPlan 匹配
- **THEN** Render activity 使用结构化白名单参数生成 30fps canonical MP4，并对输入/output MIME/hash/size/duration 执行验证

#### Scenario:renderer 未配置
- **WHEN** `FFMPEG_PATH`/`FFPROBE_PATH` 缺失、不可执行或 probe 失败
- **THEN** Preview/Export 在产生成功 artifact 前被阻断，状态为 `renderer_unconfigured` 或 capability error，不伪造 MP4/SRT/light 成功

### Requirement:TOS multipart and export upload idempotency

所有 TOS asset/export upload SHALL 通过 StoragePort 使用冻结 StorageProfile/BucketBinding snapshot、canonical object key、project scope、expected size/checksum/MIME 和唯一 operation key；part、complete、stat、verify MUST 可重放并禁止跨 scope 复用。Storage 只返回 immutable StoredObjectRef；AssetVersion 与 ExportArtifact 分别由 Assets/Export owner exactly-once append。

#### Scenario:multipart 中断后恢复
- **WHEN** upload 在任意 part 后中断并重启 API/Worker
- **THEN** resume/reconcile 复用同一 session，校验已完成 parts，complete 后只生成一个 StoredObjectRef，并由 Assets owner reservation append 一个 AssetVersion

#### Scenario:export 半成功恢复
- **WHEN** MP4/SRT/light 某一输出 upload 或 register 结果 unknown
- **THEN** worker 先 reconcile 该 Storage operation，Export owner 再逐 artifact append/reconcile；未全部完成前 ExportJob 不进入 `succeeded`，且不重新渲染已验证输出

### Requirement:Worker readiness and shared workspace

Generation/Media Worker readiness SHALL 检查 Temporal queue、数据库/migration head、catalog composition 和必要 workspace/renderer capability；API/Media local mode MUST mount the same configured workspace volume。

#### Scenario:head 或 workspace 不满足
- **WHEN** worker 连接到旧 migration head、无法读取共享 workspace 或所需 live capability 缺失
- **THEN** readiness 为 not ready，不认领业务 operation，并返回可诊断 prerequisite

#### Scenario:API 上传供 Media Worker 消费
- **WHEN** API 通过 Local workspace 写入 verified object reference
- **THEN** Media Worker 从同一 volume 读取相同 bytes/checksum，且只写 owner-approved derivative reference
