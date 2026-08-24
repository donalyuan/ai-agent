# agnes-video-generation Specification

## Purpose
TBD - created by archiving change integrate-agnes-video-provider. Update Purpose after archive.
## Requirements
### Requirement:总体计划追溯与唯一所有权
本 capability SHALL 反向追溯 `plan-phase-one-drama-mvp-a` 任务 `3.3`，并直接依赖 AssetVersion、`implement-workflows-runs-slice` 与 `implement-provider-model-skill-catalog`。总体计划仅协调交付，不构成运行时代码依赖。ProviderCall 是 catalog 领域唯一的调用/费用/幂等持久化账本；RunEvent 只由 workflows/runs 所有；只允许用 `run_id`、`node_run_id`、`correlation_id` 关联。完整非目标是声明所有公开 Agnes capability、隐式 live execution、同步生成、callback/webhook、text/image orchestration、raw video persistence、必要 Activity contract 之外的新 Temporal workflow、冻结最终 HTTP error envelope、billing settlement，以及重做 catalog 或 WorkflowRun/NodeRun/RunEvent 状态机与事件历史；本 capability MUST NOT 承担这些职责。

#### Scenario:polling observation 与业务事件分离
- **当** Agnes poll 返回新的、重复或迟到 observation
- **则** 原始 observation 可作为 ProviderCall attempt metadata 保存，且只有 workflows/runs 能为归一业务状态追加 RunEvent，不产生重复事件历史

#### Scenario:拒绝非目标职责泄漏
- **当** Agnes change 尝试承担任一列明的非目标或把总体计划作为运行时依赖
- **则** 架构依赖/契约测试失败，且不发起外部调用、不写 ProviderCall、RunEvent、StoragePort 或 AssetVersion

### Requirement:复用所有者的 canonical Schema 版本
系统 SHALL 只复用 AssetVersion 与 catalog 所拥有的 canonical `schema_version`。HTTP `schemaVersion` MUST 仅由对应所有者映射同一个值；Agnes adapter/application MUST NOT 创建、持久化或推导独立的 Provider 专用版本事实。

#### Scenario:使用一致的 AssetVersion 与 catalog 版本引用
- **当** Agnes operation 接收已解析且同项目的 AssetVersion、model 与 capability snapshot 引用
- **则** 引用中的 HTTP `schemaVersion` 与所有者 canonical `schema_version` 相同，本 change 只保存稳定 owner id/revision/hash

#### Scenario:版本引用缺失或冲突时无副作用拒绝
- **当** 输入引用缺少必需版本、同时携带冲突的 `schema_version`/`schemaVersion`，或实现尝试创建独立版本事实
- **则** 系统在 Provider 调用和 UoW 前返回稳定 validation error，且不写 ProviderCall、RunEvent、StoragePort 或 AssetVersion

### Requirement:冻结已启用账号的 capability snapshot
系统 SHALL 只能通过带 enabled configured profile 与只追加 CapabilitySnapshot 的 VideoGenerationPort 提交 Agnes video work。实施 probe MUST 优先考察 v2.0 稳定候选，但 probe 前 MUST NOT 硬编码 model/mode ID；MVP-A snapshot MUST 记录实际账号已 probe 成功的单一 image-to-video mode、精确 ID、operation/parameters/limits/result/cancel semantics，并 MUST 排除 2.5 preview。系统不得承诺所有公开 Agnes capabilities。

#### Scenario:使用已启用账号 capability 提交
- **当** run 选择配置账号 snapshot 中包含的 operation 和 parameters
- **则** submission audit 绑定 immutable snapshot id，并在 external call 前校验请求

#### Scenario:拒绝不可用账号 capability
- **当** operation/parameter 不在 configured account snapshot 中，或 live profile 未经 probe 显式启用
- **则** Command 返回 explicit unavailable/unconfigured state，且不提交 real request

### Requirement:幂等异步 submit lifecycle
系统 SHALL 以 `run_id + logical_operation` 为键实现 `submit`、`poll`、`cancel`、`result` 的 asynchronous operation lifecycle。submission intent MUST 在 post-commit Activity execution 前持久化；Activity retries MUST 复用记录 intent/provider request id，且不得产生 duplicate chargeable submissions。`submission_unknown` MUST 先通过已持久化 intent、provider request id 和允许的 poll/result 查询 reconciliation；不能确认时保持 unknown。

#### Scenario:重试 submit Activity
- **当** 一个 Activity 在既有 run/logical operation 上因 uncertain transport outcome 重试
- **则** 它在任何 submit 前 reconciliation recorded operation 或 provider request id，并记录单一 logical ProviderCall

#### Scenario:submission status remains unknown
- **当** transport failure 后无法通过 provider request id、poll 或 result 确认是否已受理
- **则** operation 保持 `submission_unknown` 并可诊断，不创建第二个 submit、ProviderCall 或收费请求

#### Scenario:两次取消 active operation
- **当** 调用方为同一 non-terminal operation 重复请求 cancel
- **则** 只尝试一次 external cancellation request，后续调用返回 persisted cancellation state

### Requirement:polling observations 归一
系统 SHALL 将已验证的 poll observations 归一到一个 persisted operation state、provider request id 与 source fingerprint。原始 observation 可存 ProviderCall attempt metadata；duplicate、stale 或 conflicting observations MUST NOT 回退 terminal state、创建 duplicate side effects 或重复 RunEvent。

#### Scenario:duplicate poll observation
- **当** 多次 poll 为同一 provider request 提供等价 observation
- **则** 系统至多记录一个 effective state transition，同时保留 diagnostic source evidence，且 workflows/runs 至多追加一个对应业务 RunEvent

#### Scenario:cancellation 后的 late result
- **当** recorded cancellation 或其他 terminal outcome 后 arrival result
- **则** persisted transition precedence 决定 visible terminal state，系统不静默标记为 success；通过校验的媒体仅登记为未引用 candidate，不替换 current AssetVersion/Shot 引用

### Requirement:Provider evidence retention
系统 SHALL 为 capability probe、ProviderCall attempt metadata、late-result candidate 和 native usage 保存 retention policy/version/hold 状态。保留期尚未由 profile 确认时，系统 MUST 保持诊断可见，不得删除或伪造已过期。

#### Scenario:preserve an unresolved retention policy
- **当** live profile 尚未提供 retention duration
- **则** 系统保持记录的 policy/version/hold 与 `unconfigured` 诊断，不改变 ProviderCall 或候选引用

### Requirement:Current storyboard input binding
系统 SHALL 在 Agnes `submit` 前冻结并校验当前 storyboard 引用的 image AssetVersion ID/revision/hash、对应 ShotSpec ID/version/hash、显式 durationSeconds/durationFrames 和 aspectRatio。所有输入 MUST 属于同一 project/episode/scene/shot，且 duration/aspect ratio MUST 被冻结 capability snapshot 接受；MUST NOT 使用旧分镜、未确认候选、隐式时长或项目默认画幅替代。

#### Scenario:submit with the exact current storyboard input
- **当** 当前 Shot 的 storyboard AssetVersion 与 ShotSpec revision/hash 匹配，且显式 duration/aspect ratio 合法
- **则** submission intent 冻结全部 owner reference 和参数后才允许 Agnes Activity submit

#### Scenario:reject stale or implicit video input
- **当** storyboard AssetVersion/ShotSpec 已变化、跨 scope、未确认，或 duration/aspect ratio 缺失/不受 capability 支持
- **则** 系统在 ProviderCall/external submit 前返回 validation/stale/unconfigured，不写 StoragePort 或 AssetVersion

### Requirement:验证结果登记
系统 SHALL 在 StoragePort 持久化前，通过 bounded MIME、size、适用时 duration/dimension 及 checksum checks 验证 successful Agnes result media。有效 result MUST 追加一个关联 run 和 ProviderCall 的新 video AssetVersion；数据库记录不得含 media bytes。

#### Scenario:仅一次登记有效结果
- **当** 一个 normalized terminal-success observation 有 valid media result
- **则** StoragePort 写入 canonical reference，application 仅追加一个 linked AssetVersion

#### Scenario:拒绝无效 result media
- **当** result retrieval、MIME/checksum、size 或 metadata validation 失败
- **则** operation 可审计为 failed，不追加 AssetVersion，且 retry policy 保持 explicit

### Requirement:Video take review, rejection and retake
系统 SHALL 在 bounded download/MIME/size/duration/dimension/checksum/StoredObjectRef 安全验证后，将 video result 登记为 immutable `VideoTakeCandidate` 和既有 result AssetVersion，初始状态为 `pending_review`，并绑定 candidate revision、source image eligibility、ShotSpec/duration/aspect snapshot、result AssetVersion id/revision/hash；该验证不是 `MediaInspect` derivative generation，candidate 不携带 derivative readiness。`accept|reject|retake` 是显式、可审计的 review decisions：accept 只能经 scenes owner exact CAS 成为 current video reference，且仅在该 CAS 成功后才由 Media Worker 生成 metadata/proxy/thumbnail/keyframe/waveform；reject 保留结果但不得改变 current；retake MUST 创建新的 execution intent、successor candidate 和新的 `logicalOperation`，旧 take 不得覆盖或复用旧预算确认。未经 accept 的 candidate MUST NOT 进入 Timeline 或 `timeline.handoff`。

#### Scenario:accept a verified video take
- **WHEN** 用户以当前 candidate revision 明确 accept，且 source/ShotSpec/result facts 仍匹配
- **THEN** workflows/runs 记录 review decision，scenes owner 以 candidate/provenance/AssetVersion id/revision/hash/project/episode/shot exact CAS 建立 current video eligibility；MediaInspect/derivatives 随后执行，只有 Timeline/preview/export 等待 ready derivative

#### Scenario:reject or retake without mutating current
- **WHEN** 用户 reject 当前 take，或提交带 successor input 的 retake
- **THEN** reject 只追加 rejected audit；retake 追加新 execution/logical operation，旧 candidate immutable/stale，原 current storyboard/video reference 与 Timeline 保持不变

#### Scenario:block unaccepted video handoff
- **WHEN** video candidate 仍 pending_review/rejected/stale/foreign，或 derivative readiness/hash/revision 不匹配
- **THEN** `timeline.handoff` 与 Timeline assembly 返回可诊断 gate failure，且不写 Clip、ExportJob、ProviderCall 或 remote submit

### Requirement:Provider and media-derivative ownership
Agnes Provider MUST 只拥有 capability probe/snapshot、submit/poll/cancel/result transport、ProviderCall attempt/native usage evidence；MUST NOT 生成 thumbnail、keyframe index、waveform、proxy、canonical normalized metadata 或 Timeline/current facts。media worker 的 `MediaInspectPort`/`MediaDerivativePort` SHALL 以 verified StoredObjectRef 生成独立、可失效的 metadata/derivative records，并绑定 source AssetVersion id/revision/hash、tool/version、derivative schema/version 与 retention/license/hold。

#### Scenario:derive media facts in the worker
- **WHEN** Media worker 接收已验证 video StoredObjectRef
- **THEN** worker 生成 canonical observed metadata、proxy、thumbnail、keyframe index、waveform 的独立 records；ProviderCall、AssetVersion、Storyboard current 与 Timeline 不被越界修改

#### Scenario:stale or failed derivative is visible
- **WHEN** ffprobe/ffmpeg 检查失败、source hash/revision 变化或 derivative output 不符合 bounds
- **THEN** derivative 状态为 failed/stale 并保留原始诊断，不能报告 ready 或允许 Timeline handoff、preview、export；已通过 exact candidate/source/ShotSpec facts 校验并由 scenes current CAS 接受的 candidate/current 保持不变，不能因该下游状态被阻断、撤销或降级

### Requirement:Mock 默认与 opt-in 真实调用
系统 SHALL 保持 deterministic Mock Provider 和显式 Local test/offline profile（adapter identity=`local_workspace`）为默认测试执行组合。真实 Agnes request MUST 要求 explicit enabled opt-in profile、frozen account capability probe 与 adapter-side credential resolution；missing configuration、Provider failure、retry 和 cost/usage fields 必须可观察，且不得切换到 Local/其他 profile。

#### Scenario:没有 live Agnes configuration 的测试
- **当** integration tests 在没有 enabled live profile 时运行
- **则** 它们使用 `Mock Provider +` 显式 Local test/offline profile 或收到 explicit unconfigured behavior，且不发起 network request

### Requirement:Agnes 需要精确的已接受 storyboard 输入
在 ProviderCall 持久化或 external submit 前，Agnes MUST 校验 accepted candidate/provenance/eligibility projection，以及精确的 AssetVersion id/revision/hash、ShotSpec、duration 和 aspect-ratio snapshot。

#### Scenario:Preflight mismatch has zero external side effect
- **WHEN** 任一必需输入未接受、stale、foreign 或不匹配
- **THEN** 在 ProviderCall 持久化或 provider submission 前拒绝 request。
