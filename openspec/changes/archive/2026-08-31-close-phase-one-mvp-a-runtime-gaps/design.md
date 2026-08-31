## Context

阶段一的领域 owner、Schema、Alembic 表、Mock/Local contract 和局部 Temporal 入口已经存在，但当前运行时仍有四个可观察断点：`runtime.py` 只能装配 Mock provider；GPT Image、Agnes Video 和 TOS 仅有可注入 transport，没有 live composition；Generation Worker 没有 Text/Image/Video activity，Media Worker 只注册 export；标准 Compose 没有把 FFmpeg/ffprobe、Docker Secret、显式 migration/head readiness 与 credentialed sandbox 闭合。现有 E2E 主要验证导航和 fixture 投影，不能证明文本审核之后的真实媒体链路或 API/Worker 重启恢复。

本设计服务于四份需求文档的 MVP-A 退出门。它必须与既有 owner 规格兼容：Project/Episode、Run/NodeRun、TextReview、Scene/Shot、Asset/AssetVersion、Storage、Media、Timeline/Export 各自保留事实和 CAS；本 change 只拥有跨服务 composition、versioned operation identity/handoff contract、运行环境和退出证据编排，不创建第二套跨 owner 业务事实源。

### 权威追溯表

| 文档 | 必须观察的阶段一结果 | 当前代码/配置证据与本设计的收口点 |
| --- | --- | --- |
| `docs/video-agent-product-requirements.docx` | 创建 -> 文本审核 -> 图片/视频审核 -> MediaInspect -> Timeline -> MP4/SRT/light，且可恢复。 | 既有 `E2E-MVPA-001` 记录了离线/投影门，但没有 credentialed 全链路和 API/Worker restart；由 `phase-one-runtime-acceptance` 收口。 |
| `docs/video-agent-technical-architecture.md` | 领域 owner 通过 port、outbox、Temporal activity 和 typed handoff 连接，API 不执行外部副作用。 | `runtime.py`、Generation queue 入口和 Media export-only activity 暴露 composition/activity 断点；由 runtime-composition 与 worker-media-runtime 收口。 |
| `docs/video-agent-technical-implementation.docx` | catalog 解析、live adapter、FFmpeg/ffprobe、Secret、migration/readiness 和运行记录必须可执行。 | Provider/TOS 目前仅 transport injection，Compose readiness 仍偏数据库连通；由 composition 与 Compose/migration tasks 收口。 |
| `docs/video-agent-integration-notes.md` | 显式 profile/probe、unknown reconciliation、multipart/export retry 和不泄露凭据的集成证据。 | 默认无凭据事实只能得到 `unconfigured`；这不是账号缺陷，而是需在凭据存在时仍可运行的 wiring 与 evidence 缺口。 |

## Goals / Non-Goals

**Goals:**

- 以 DB catalog 的 Provider/Profile/Model/CapabilitySnapshot 完成显式 live Text、GPT Image、Agnes Video、TOS 运行时装配，并保持 Mock/Local 默认路径。
- 在所有 live invocation 前复用既有 catalog runnable/feature gate、ProviderOperationPolicy concurrency/rate/quota admission 与 operations-resilience resource/capacity admission，不创建第二套准入事实。
- 将外部副作用限制在可重试的 Temporal activity：Generation Worker 负责 Text/Image/Video，Media Worker 负责 inspect/derivative/render/storage。
- 以 `run_id + logical_operation`、各 owner 已有 request ledger、Text/GPT Image `ProviderCall.unknown`、Agnes `VideoOperation.submission_unknown` reconciliation 和 upload operation key 保证 retry/restart 幂等。
- 让 FFmpeg/ffprobe capability、Docker Secret、TOS 配置、Alembic head 和 readiness 成为可观察前置条件；任一缺失在成功副作用前 fail closed。
- 以不含真实密钥的默认验证和隔离 credentialed sandbox 验证两条路径，生成完整的 MVP-A 创建至导出证据矩阵。

**Non-Goals:**

- 不实现 MVP-B workflow 图编辑、批量生成/审核、TTS/ASR、移动端、多租户、平台发布、callback/webhook。
- 不实现 portable/full package、工程回导、专业 NLE、复杂转场或新的 UI 视觉体系；前端仅补足既有页面所需的状态投影和 E2E 操作。
- 不提交或复制真实 Provider/TOS 凭据，不在默认 CI 发起外部请求，不把 live 失败降级为 Mock/Local。

## Decisions

### 1. Catalog 驱动的 runtime composition

`RuntimeSettings` 只解析模式、profile id、credential reference、workspace 和 renderer 配置；Provider/Profile/Model/CapabilitySnapshot 由 catalog repository 在 application startup 或 activity 入口解析。首次 connection-test/probe 继续使用既有较窄 probe gate：已安装、approved、`featureGate=MVP-A`、explicit live opt-in、已选 profile、可解析 credential 与显式 timeout，不要求既有 snapshot 或 `runnable=true`。正式 live invocation 则必须同时满足 `adapterInstalled`、`approval=approved`、successfully probed snapshot、`runnable=true`、`featureGate=MVP-A`、explicit opt-in 和 enabled selection；MVP-B、uninstalled、not-approved、disabled-for-run 或 snapshot-missing 均不得取得可调用 port。

通过 runnable gate 后，每次 Provider live invocation 还必须在创建 ProviderCall/durable attempt 与 external submit 前执行冻结 revision 的 `ProviderOperationPolicy` concurrency/rate/quota admission；超并发、超速率或 quota exhausted 返回稳定 blocked/retryable diagnostic，429/`Retry-After` 与 quota unknown 保留 provider-native observation，不跨 Provider 归一或重置为可用。composition 返回完整的 `TextModelPort`、`ImageGenerationPort`、`VideoGenerationPort`、`StoragePort`、`MediaInspectPort` 和 `FfmpegRenderPort`，每个 port 带冻结 identity/admission references。业务代码不得写死 model、base URL、bucket、region 或 SDK client。

live profile 必须显式 `enabled` 且通过 credential resolver 和 capability probe；缺失、禁用、过期、能力不匹配或主密钥不可用返回原始脱敏诊断（如 `unconfigured`、`credential_master_key_unavailable`、`capability_unsupported`）。只有 `PROVIDER_MODE=mock` 与 `STORAGE_MODE=local_workspace` 选择默认离线路径，其他模式不允许自动替代。

替代方案是按 provider 在业务 service 内条件分支，已拒绝：它会复制 catalog 事实并使 API、Worker、测试得到不同 composition。也不把 SDK client 放进 domain：SDK 副作用必须留在 adapter。

### 2. Owner-specific ledger 与共享幂等身份

不新增跨 owner 的统一 operation 表。Text/Image 使用现有 `ProviderCall`/`provider_call_keys`，Video 使用 `VideoOperation`，TOS 使用 `UploadSession`/`StoredObject`，Export 使用 `ExportJob`/`ExportArtifact`；只有缺失字段或唯一约束时才做 additive migration。各 owner 只复用 versioned operation identity contract，至少绑定 project scope、run id（若有）、node id（若有）、logical operation 和适用 fingerprint，不共享或新增统一顶层状态机。

Text/GPT Image 的 ProviderCall 继续使用既有 `pending|succeeded|failed|unknown|cancelled`，ambiguous submit 保持 `unknown`；Agnes 的 VideoOperation 继续使用既有 `pending|submitted|running|submission_unknown|succeeded|failed|cancelled`，ambiguous submit 保持 `submission_unknown`。Storage 只使用既有 UploadSession `active|completed|aborted|unknown|failed`、handoff/recovery `reconciliation_required|failed|aborted|resolved` 与 immutable verified StoredObjectRef；不得给 StoredObject 或其他 owner 增加 `submission_unknown`。ExportJob 顶层状态保持 `queued|preflighting|rendering|packaging|succeeded|failed|cancel_requested|cancelled`，upload/verify/register 不确定性只留在 `packaging` 的既有 subphase、稳定 diagnostic 与 Storage operation reconciliation；ExportArtifact 仍为 `pending|verified|failed|held`。本 change 不新增、重命名或迁移这些状态值；明确失败只按 owner policy 恢复同一 identity，retake/新业务意图必须新建 logical operation。

每次收费 Provider 调用必须先持久化 durable attempt 和由冻结 operation identity 派生的 outbound correlation。adapter probe 还必须冻结该 Provider 是否支持 client idempotency key、remote request lookup 及其精确协议；支持时传递对应 key 并在取得 remote request id 后持久化关联，不支持时不得伪造通用 header。若远端可能已接受但既无 remote id 又无 lookup 能力，Text/GPT Image ProviderCall 必须稳定停在 `unknown`，Agnes VideoOperation 必须稳定停在 `submission_unknown`；两者均关闭 Temporal 自动 retry/re-submit并暴露人工处置，不能以补偿性重提追求自动成功。

替代方案是 activity 内存中的 retry 或按时间戳生成 id，已拒绝：Worker 重启会重复付费提交，且不能证明 unknown 是否产生结果。

### 3. Worker 与 API 边界

API command 先校验 scope/CAS 与适用的 catalog/policy/resource/capacity admission，再写 owner intent、对应 owner ledger/outbox 并返回 operation id；不等待 Text/Image/Video、MediaInspect、FFmpeg 或对象上传。Generation Worker 的 workflow/activity 分别执行 text generation、image generation、video submit/poll/cancel/result-registration/reconcile；Media Worker 的 workflow/activity 分别执行 inspect、proxy/thumbnail/keyframe/waveform derivative、render、bounded storage upload/verify/register。每个 activity 从冻结输入快照读取，完成后用 typed handoff 回写对应 owner；Agnes 的 poll/cancel/result-registration 不得被笼统的 reconcile 语义替代。

上述写入之前，upload、paid Text/Image/Video、Media derivative/preview 和 Export command 必须读取同 scope、未过期且带 revision/hash 的 RuntimeResourceSnapshot 与 CapacitySnapshot。probe unavailable、capability unsupported 或 hard limit 在任何 intent、ProviderCall、UploadSession、ExportJob、AssetVersion、Outbox 前零副作用拒绝；soft limit 只追加 warning/admission evidence。已接收 operation 冻结 resource/capacity admission references，Worker/restart 在外部副作用前复核同一 operation/snapshot并保持 blocked/unknown 或继续，不生成第二个 intent、submit 或 outbox。

每个 NodeRun/operation snapshot 必须冻结 `executionRoute`、`workflowType`、`taskQueue` 和 `schemaVersion`。前向切换后新 Text/Image/Video intent 只能进入 Generation route；既有 Agent `phase_one_run` 和 direct Text HTTP 路径只能 drain/reconcile 已冻结的 legacy operation，不得接收新 route/schema intent。direct HTTP mutation 改为持久 command/outbox 后返回 operation，API/Agent/Generation 并发、Temporal `AlreadyStarted` 和 late completion 由 owner 唯一键保证只产生一个 ProviderCall/TextReviewBatch。回滚只停用新 dispatch，并让各自冻结 route 的存量完成或 reconcile；旧 Worker 不得接管新 schema operation。

Media outbox admission 必须先冻结 `MediaDispatchAdmission` discriminator。`uploaded_source|asset_center` 路径只需 verified StoredObjectRef、同 scope 的 AssetVersion id/revision/content hash/provenance 和技术输入，即可执行 inspection 与通用 derivative；`generated_candidate` 路径才额外要求 accepted candidate/review decision、Scenes owner exact current CAS、AssetVersion id/revision/hash/provenance 与 project/episode/scene/shot 精确匹配。generated candidate 的 pending、rejected、retake、stale、foreign 或 current mismatch 必须在 outbox/Activity 前零副作用拒绝；无论哪条路径，Timeline/Render/Export handoff 仍只接受 accepted current 与 ready derivative。

Temporal workflow 只保存确定性状态和重试分支；网络、数据库、文件、Provider SDK、TOS SDK、FFmpeg 均在 activity。activity lease、worker heartbeat 和稳定 workflow id 允许 API 或任一 Worker 重启后由各 owner 持久 outbox/ledger 继续，不重复已确认副作用。

替代方案是让 API 直接调用 Provider 或让一个 Worker 拥有所有业务活动，已拒绝：会阻塞 HTTP、混淆 owner、扩大凭据暴露面并破坏已有 Media/Export 归属。

### 4. Renderer、Storage 和配置边界

Media Worker 启动前执行显式 `ffmpeg`/`ffprobe` probe，记录版本、H.264 decoder/encoder、AAC decoder/encoder、`yuv420p`、MP4 mux/demux/container 等 capability snapshot；缺失路径为 `renderer_unconfigured`，能力不足为 `renderer_capability_unsupported`。RenderPlan 仅从冻结 TimelineVersion 编译，命令参数为结构化白名单，输入/output 通过流式读取、MIME/hash/size/duration 校验，不能执行 shell 片段。

TOS profile 使用既有 AES-256-GCM credential envelope、版本化 Docker Secret/keyring 和 profile-bound AAD；compose 只注入 credential reference/secret mount，不把 secret 放进日志、HTTP 或 evidence。TOS multipart、MP4/SRT/light export upload 均通过 `StoragePort`，Local 与 TOS 的 operation contract 相同但 adapter identity 不同。

Storage runtime 输入冻结 `storageProfileId/revision/snapshotHash`、private BucketBinding id/revision/scope、canonical operationKey 和 expected object facts。image/video 输出只允许按 `Assets reservation -> Storage UploadSession/reconcile -> immutable StoredObjectRef -> Assets owner append AssetVersion exactly once -> candidate` 交接；MP4/SRT/light 只允许按 `ExportJob intent -> Storage session/ref -> Export owner append ExportArtifact` 交接。Storage/Generation/Media Worker 均不得直接 append AssetVersion 或 ExportArtifact；stale/foreign profile、registration conflict 和 response-loss 必须由对应 owner reconcile。

### 5. Migration、readiness 和 Compose

新增表和索引只使用 additive Alembic revision；部署顺序为数据库可用 -> 显式 `alembic upgrade head`（或等价的一次性 migration service）-> API/Worker readiness。API readiness 除数据库连接外必须验证 migration head、catalog bootstrap、resource/capacity probe 和所选运行模式的 capability state；缺少 live 凭据或资源 probe 可以报告 `unconfigured`/resource diagnostic，但不能报告完整 MVP-A ready。Compose healthcheck 不执行隐式 migration，也不能把 `SELECT 1` 当作 schema ready。

`api`、`generation`、`media` 共享必要的 `DATABASE_URL`、Temporal 地址和 local workspace；`media` 明确注入 `FFMPEG_PATH`/`FFPROBE_PATH`，live TOS profile 通过显式 env/secret 绑定。默认 `.env.example` 仍无真实值，默认启动可用 Mock/Local。

### 6. 验收证据和安全分层

默认回归证明无网络、无费用、无 secret 泄露和 no-fallback；credentialed sandbox 使用专用 profile、账号、bucket 和 allowlist，不将凭据或 response body 写入仓库。E2E 固定经过 catalog runnable/policy admission、resource/capacity pre-intent admission、创建项目/剧集、文本生成、TextReview、图片候选及接受/current、Agnes 视频候选及接受/current、MediaInspect、Timeline、MP4/SRT/light，并在 route cutover、remote accepted-response-loss、multipart/export 阶段重启 API 与至少一个 Worker。每个阶段保存 policy/snapshot revision、owner、前置条件、operation id、execution route、remote correlation/lookup outcome、observed state、failure/no-side-effect 和 artifact hash。

缺少凭据、许可、ffmpeg capability 或远程账号时，测试输出稳定 `unconfigured`/`renderer_unconfigured` 和原始脱敏错误；这只能证明拒绝路径，退出报告为 `not_ready`，不能转为 MVP-A success。

## Risks / Trade-offs

- [live SDK/API 与账号能力未经仓库验证] -> adapter 以 transport contract 和显式 probe 隔离，保留 raw redacted error；实现前固定依赖版本并以 sandbox probe 作为 gate。
- [Provider 超时造成重复付费提交] -> 在 ProviderCall/VideoOperation 等既有 owner ledger 先持久化 operation identity，ProviderCall `unknown` 与 VideoOperation `submission_unknown` 只由对应 owner reconcile，logical operation 唯一约束覆盖重启和并发。
- [新旧 Agent/Generation 路径双重执行] -> operation 冻结唯一 route/queue/schema；新 intent 前向切换，legacy 只 drain，rollback 不跨 route 接管。
- [Provider 不支持幂等键或 remote lookup] -> capability snapshot 精确记录支持范围；remote acceptance 不可判定时按 owner 保持 ProviderCall `unknown` 或 VideoOperation `submission_unknown` 并进入人工处置，禁止自动重提。
- [新 composition 绕过既有 catalog/resource gate] -> 首次 probe 与 live invocation 分离；live port resolve、ProviderCall/intent/outbox 与 external submit 前分别执行既有 runnable/policy/resource admission，负向测试断言零副作用。
- [统一 unknown 状态覆盖 owner 状态机] -> 只共享 operation identity；Provider、Storage、Export 分别沿现有状态/subphase/diagnostic reconcile，不新增跨 owner 状态枚举。
- [TOS multipart 或 export upload 半成功] -> 每 part/complete/stat/register 均可幂等重放，unknown 先 reconcile，失败不切换 Local、不重渲染。
- [Worker/API 版本与 schema 不一致] -> readiness 读取 migration head 和 capability snapshot；旧 Worker 不得认领未知 activity/schema，部署顺序先 migration 再服务。
- [FFmpeg 输入或命令注入] -> 结构化参数白名单、隔离临时目录、有界流式 IO、输出验证和原始 stderr 脱敏记录。
- [credentialed E2E 污染默认 CI 或证据泄露] -> live profile 只在显式环境启用，默认 marker 禁止真实网络；evidence 只记录 ids、hash、状态和诊断类型。
- [跨 owner 责任被 runtime 聚合层吞并] -> composition 只提供 ports、共享 identity 和 adapter wiring，不拥有统一业务表；typed command/handoff 仍由现有 owner service 执行，architecture tests 检查依赖方向。

## Migration Plan

1. 先盘点各 owner operation ledger，冻结 Provider、Storage、Export 既有状态值域；只有缺失的 identity/attempt/reconcile diagnostic 字段才新增 additive schema，并运行 SQLite/PostgreSQL upgrade/downgrade/re-upgrade cycle。
2. 实现 catalog resolver、credential resolver、完整 runnable/ProviderOperationPolicy admission、operations-resilience pre-intent admission 和 live adapters wiring，并保持 Mock/Local 默认 composition；启用前执行 provider/TOS/renderer/resource probes。
3. 先冻结 route/queue/schema 并将新 intent 前向切换到 Generation，再让 legacy Agent/direct HTTP drain 存量；随后注册 Generation/Media workflow/activity，接通各 owner 持久 outbox/ledger、accepted-current admission 和 remote correlation/unknown 终局。
4. 更新镜像、Compose secret/env、显式 migration service 和 head-aware readiness；先跑默认 quality gates，再跑 credentialed sandbox。
5. 回滚时停止 live profile 和新 activity dispatch，保留 append-only owner ledgers/facts；只有 snapshot 冻结为 legacy route 的存量可由旧 worker 完成或 reconcile，旧 worker 不得接管新 schema operation，不得删除已引用对象或把失败结果改写为成功。

## Open Questions

- 各 live provider 的 SDK 版本、区域和请求字段需在实施前由显式 sandbox probe 取证；未知字段不得由设计文档猜测。
- 生产部署是由 CI/运维执行一次性 migration 还是 Compose profile 执行，需在目标环境确定；两者都必须在 API readiness 前完成并可审计。
- 真实 Agnes 是否提供可查询的 request status 端点需由账号能力确认；若不提供，必须保留 `submission_unknown`，不能自动再次 submit。
