## 1. 基线、owner 与契约追溯

- [x] 1.1 重新核对四份权威需求文档、现有 phase-one specs 与当前代码，建立 gap-to-task 矩阵；记录每个 operation 的 owner、输入快照、状态、幂等键和不变量。
- [x] 1.2 固化 runtime composition、既有 catalog runnable/ProviderOperationPolicy 与 operations-resilience admission、`executionRoute/workflowType/taskQueue/schemaVersion`、Provider outbound correlation、owner-specific status mapping、Storage/Assets/Export typed handoff、worker activity、migration/readiness、E2E evidence 的版本化 contracts 和稳定错误 envelope，确认默认 Mock/Local、显式 live opt-in、fail-closed/no-fallback 约束。
- [x] 1.3 为 `E2E-MVPA-001` 增加 credentialed sandbox 证据 schema，定义 catalog/policy/resource snapshot revision/hash、prerequisite、owner、`run_id + logical_operation`、observed result、failure/no-side-effect、restart/reconcile 和 artifact hash 字段，禁止 secret/完整 response/媒体 bytes。

## 2. Catalog 驱动的 runtime composition（DDD / SDD）

- [x] 2.1 扩展 `RuntimeSettings` 和 composition port，使 API、Generation Worker、Media Worker 解析显式 provider/storage/profile/renderer selection，而不是只接受 `PROVIDER_MODE=mock`。
- [x] 2.2 实现从 Provider/Profile/Model/CapabilitySnapshot catalog 解析冻结 identity 的 resolver；首次 probe 仅校验 installed/approved/MVP-A/explicit opt-in/profile/credential/timeout，live invocation 则强制 `adapterInstalled + approval=approved + successfully probed snapshot + runnable=true + featureGate=MVP-A + explicit opt-in + enabled`，并校验 project scope、catalog revision及 Provider 对 client idempotency key/remote lookup 的精确能力/协议。
- [x] 2.3 将 TextModelPort 的正式 live transport 接入 composition，读取 catalog 的 model/base URL/default parameters；调用前持久化 ProviderCall attempt/outbound correlation，按 Text Provider 能力传递 idempotency/correlation、关联 remote request id，并定义无 lookup 时保持既有 `ProviderCall.unknown` 的人工终局。
- [x] 2.4 将 `GPTImageProvider` 接入正式 live transport composition、catalog model/profile 和 credential resolver；调用前持久化 ProviderCall attempt/outbound correlation，按图片 Provider 能力传递 idempotency/correlation、关联 remote request id；ambiguous no-lookup 保持既有 `ProviderCall.unknown`，bounded retry 只重放可证明未外发的步骤，并保留 transport injection 供测试。
- [x] 2.5 将 `AgnesVideoProvider` 接入正式 live transport composition、catalog profile/capability、pre-submit durable attempt、stable outbound correlation 和 submit/reconcile port；持久化 remote video/request id，缺少 remote lookup 时关闭自动 retry/re-submit并保留 `submission_unknown` 人工处置。
- [x] 2.6 将 `TOSAdapter` 接入正式 storage profile composition，冻结 `storageProfileId/revision/snapshotHash`、private `BucketBinding` id/revision/project scope、canonical operation key、expected object facts 和 credential reference，并确保 TOS 与 Local 使用不同 adapter identity。
- [x] 2.7 复用既有 AES-256-GCM envelope、versioned keyring/Docker Secret CredentialResolver；增加 missing-master-key、rotation 和 masked-output 测试，禁止 secret 进入日志/API/evidence。
- [x] 2.8 在每次 Provider live invocation 的 ProviderCall/durable attempt 与 external submit 前复用带 revision 的 ProviderOperationPolicy concurrency/rate/quota admission；保留 429/`Retry-After` 和 quota unknown 原生事实，不跨 Provider 归一或重置为可用。
- [x] 2.9 增加 composition contract tests：首次 probe 不由 snapshot/runnable 错误阻断；live invocation 对 uninstalled/not-approved/MVP-B/disabled/non-runnable/snapshot-missing/policy 超限/quota exhausted 零 ProviderCall/外部调用；scope/expired/capability mismatch 与 live error 原样脱敏返回，且无隐式 Mock/Local fallback。

## 3. Image API 与 owner-specific operation ledgers（DDD / SDD）

- [x] 3.1 将 `ImageGenerationService` 接入正式 FastAPI router/dependency，复用 project scope、CAS、accepted prerequisite 和 command/outbox 约束；API 只返回 operation/candidate 状态。
- [x] 3.2 盘点现有 owner 状态与字段：ProviderCall/VideoOperation 的 durable attempt、request fingerprint、outbound correlation、remote request id/lookup outcome；UploadSession/StoredObject 的 session/unknown/reconciliation 与 verified ref；ExportJob 八态/packaging subphase 和 ExportArtifact 四态。只扩展缺失的 identity/fingerprint/diagnostic/唯一约束，禁止新建统一跨 owner 业务表或状态枚举。
- [x] 3.3 保持 owner-specific 状态映射：Text/GPT Image ProviderCall 只使用既有 `pending|succeeded|failed|unknown|cancelled`，Agnes VideoOperation 只使用既有 `pending|submitted|running|submission_unknown|succeeded|failed|cancelled`；Storage 只使用既有 UploadSession `active|completed|aborted|unknown|failed`、handoff/recovery `reconciliation_required|failed|aborted|resolved` 和 immutable StoredObjectRef；ExportJob 只使用八态与 packaging `uploading|verifying|registering` subphase，ExportArtifact 只使用 `pending|verified|failed|held`。本 change 不新增、重命名或迁移状态；增加状态值域、越权、回退和历史覆盖 contract tests。
- [x] 3.4 以 `run_id + logical_operation` 和 owner operation key 实现同意图幂等，retake/新业务意图强制新 logical operation；为 API retry、并发请求和 Worker restart 增加负向测试。
- [x] 3.5 在 ProviderCall/VideoOperation owner 内逐 Provider 实现持久 request ledger 与 owner-specific ambiguous reconciliation activity；支持 lookup 时只能用原 outbound correlation/remote request id 查询，不支持 lookup 时 Text/GPT Image ProviderCall 保持 `unknown`、Agnes VideoOperation 保持 `submission_unknown`，关闭自动 retry/re-submit并暴露人工处置，禁止假定统一 header 或补偿性重提。
- [x] 3.6 在 Storage owner 内以冻结 StorageProfile/BucketBinding snapshot 接入 TOS multipart，并只返回 immutable StoredObjectRef；image/video 必须由 Assets reservation/operation key 驱动 Assets owner exactly-once append AssetVersion/candidate，MP4/SRT/light 必须由 ExportJob 驱动 Export owner append 独立 ExportArtifact。覆盖 part retry、complete unknown、checksum/MIME/size mismatch、stale/foreign profile、registration conflict 与 response-loss，不允许 Storage/Worker 跨 owner append。
- [x] 3.7 增加 owner boundary/architecture tests，证明 runtime composition、Storage、Generation/Media Worker 不拥有或直接 append AssetVersion、Scene/Shot current、Timeline current、ProviderCall、ExportJob 或 ExportArtifact，只通过 StoredObjectRef 与 typed command/handoff 交给对应 owner。
- [x] 3.8 在 upload、paid Text/Image/Video、Media derivative/preview 和 Export command 的 UoW 前接入既有 RuntimeResourceSnapshot/CapacitySnapshot admission；probe unavailable/capability unsupported/hard limit 时在 intent、ProviderCall、UploadSession、ExportJob、AssetVersion、Outbox 前零副作用拒绝，soft limit 只记录 warning。冻结 snapshot revision/hash，Worker/restart 复核同一 operation/snapshot。

## 4. Generation Worker 与 Temporal 执行（DDD / BDD）

- [x] 4.1 为 Generation Worker 构造完整 DB/catalog/runtime composition；在 NodeRun/operation snapshot 冻结 catalog/policy/resource/capacity admission references 与 `executionRoute/workflowType/taskQueue/schemaVersion`，定义 queue readiness、workflow id 和 activity dependency 注入。
- [x] 4.2 注册 Text generation activity：消费 matching running Text NodeRun 的冻结 snapshot，调用 `TextGenerationService`，一次写入完整候选图/ProviderCall/TextReviewBatch 并进入 `waiting_review`。
- [x] 4.3 注册 Image generation activity：校验 accepted AssetBible/source/ShotSpec/BudgetGate，提交同一 logical operation，持久化 image candidate/provenance，不直接接受为 current。
- [x] 4.4 注册 Video submit/poll/cancel/result-registration/reconcile activities：校验 current catalog capability 和 confirmed BudgetGate，按同一 `run_id + logical_operation` 持久化 VideoOperation；result activity 完成 Storage/MIME/hash/size 校验后登记一个 immutable AssetVersion 与 pending_review VideoTakeCandidate，覆盖重复 cancel、晚到结果、重启和 `submission_unknown`。
- [x] 4.5 实现 Generation outbox dispatcher、稳定 Temporal workflow id 和前向 route cutover：新 Text/Image/Video intent 只进入 Generation route，direct Text HTTP 改为 command/outbox 后返回 operation；既有 Agent/direct HTTP 只 drain/reconcile 冻结 legacy operation，回滚只停用新 dispatch且不得跨 route/schema 接管。
- [x] 4.6 增加 Generation Worker activity contract/BDD tests：catalog/policy/resource admission 失败零 intent/ProviderCall/outbox/external submit，Worker 重启复用冻结 admission；Agent 与 Generation 并发、direct HTTP 不同步调用、legacy drain/rollback、duplicate dispatch、Temporal `AlreadyStarted`、late completion、foreign/stale snapshot、scope/CAS mismatch、provider failure及 remote accepted-response-loss；断言每个 logical operation 只有一个 ProviderCall/TextReviewBatch 或对应 owner fact。

## 5. Media Worker、renderer 与 storage activity（DDD / BDD）

- [x] 5.1 为 Media Worker 构造完整 DB/catalog/runtime composition，消费冻结 resource/capacity admission references，保留现有 export workflow/八态/packaging subphase 并注册 inspect/derivative/render/storage activities。
- [x] 5.2 在 Media outbox/activity 前实现 discriminator admission：`uploaded_source|asset_center` 只校验 verified StoredObjectRef、同 scope AssetVersion id/revision/content hash/provenance 与技术输入；`generated_candidate` 再冻结 candidate/review decision、Scenes exact current CAS、AssetVersion id/revision/hash、provenance 与 project/episode/scene/shot scope。两条路径均从 verified StoredObjectRef 有界流式读取并记录 canonical MediaInspection。
- [x] 5.3 实现 proxy/thumbnail/keyframe-index/waveform Derivative activity：普通上传/source/audio 素材可走通用 inspection/derivative；generated candidate 只消费通过 accepted-current admission 的来源。每种 derivative 独立 schema/version/reference/retention/license/hold，验证输出后才标记 `ready`；Timeline/Render/Export handoff 继续拒绝未 accepted-current 素材。
- [x] 5.4 实现 Media render activity：从冻结 TimelineVersion/RenderPlan 和 capability snapshot 生成结构化 FFmpeg 参数，校验输入/output MIME/hash/size/duration/loudness，不执行 shell 片段。
- [x] 5.5 实现 bounded MP4/SRT/light upload、verify 和 StoredObjectRef handoff；只有 Export owner 可为同一 ExportJob exactly-once append 三类独立 ExportArtifact，unknown 先 reconcile，不重渲染已验证 artifact，Media/Storage 不直接登记 owner fact。
- [x] 5.6 将 accepted-current admission 后的 MediaInspection/Derivative/Render/Storage dispatch 接入持久 outbox 和稳定 workflow id，覆盖 Media Worker restart、临时目录清理和 no-GC 引用保护。
- [x] 5.7 增加 Media Worker owner/BDD tests：resource probe/capacity hard-limit 在 intent/outbox 前零副作用且重启复用 snapshot；pending/rejected/retake/stale/foreign/current mismatch 均零 Media outbox/MediaInspection/Derivative/Timeline/Export mutation；另覆盖 ffprobe claimed-vs-observed mismatch、derivative failure、Storage session unknown 与 Export packaging reconcile，证明 accepted current 和 owner 状态值域不被污染。

## 6. FFmpeg/ffprobe、Compose、Secret 与 readiness（SDD / TDD）

- [x] 6.1 为 Media Worker 实现显式 ffmpeg/ffprobe probe，逐项记录 binary version、H.264、AAC、`yuv420p`、MP4 mux/demux/container capability；区分 `renderer_unconfigured` 与 `renderer_capability_unsupported`。
- [x] 6.2 更新 API/Generation/Media 镜像和依赖配置，确保标准 Media runtime 安装并可显式提供 `FFMPEG_PATH`/`FFPROBE_PATH`；默认 Compose 的 renderer probe 应 ready，刻意移除 binary 的负向 fixture 才返回 `renderer_unconfigured`，不得伪造 renderer success。
- [x] 6.3 更新 `infra/compose/compose.yaml` 与 `.env.example`：显式注入 DATABASE/Temporal/workspace/profile 配置、Media renderer paths、Docker Secret reference、TOS endpoint/bucket/region placeholder 和 API/Media shared volume。
- [x] 6.4 增加一次性 migration service 或等价显式部署命令，执行 `alembic upgrade head` 并输出 revision；禁止 healthcheck 隐式改 schema。
- [x] 6.5 将 API/Generation/Media readiness 扩展为 database + migration head + catalog bootstrap + resource/capacity probe + queue + workspace + selected capability checks；readiness 只读且不能替代 command admission，`SELECT 1` 不能单独报告 ready。
- [x] 6.6 增加 Compose config/build/up/readiness tests：默认无真实 secret、localhost-only、七服务依赖、共享 volume、旧 head/缺 secret/缺 binary 的 fail-closed 诊断。

## 7. Credentialed sandbox E2E 与退出门（BDD / TDD）

- [x] 7.1 建立显式 credentialed sandbox profile/allowlist/secret injection harness；默认 CI 仅使用 Mock/Local，live test 缺少前置时输出 `unconfigured` 并标记 `not_ready`。
- [x] 7.2 实现固定闭环 E2E：完整 catalog runnable/ProviderOperationPolicy 与 resource/capacity admission -> 创建 Project/Episode -> Text -> TextReview -> GPT Image candidate -> image review/accept -> Scenes image exact-CAS current -> Agnes submit/poll/result（并覆盖 cancel 分支）-> Video candidate -> video review/accept -> Scenes video exact-CAS current -> MediaInspect/Derivative -> Timeline -> MP4/SRT/light；另以普通 uploaded/source/audio AssetVersion 验证 S08a/S08b inspection/proxy 不要求 review/current，而 Timeline/Export 仍拒绝未 accepted-current。每步断言 admission revision/hash、owner、scope、CAS、review/current revision/hash、StoredObjectRef/AssetVersion/ExportArtifact handoff、artifact/provenance 和稳定 operation identity。
- [x] 7.3 在 legacy Agent 与 Generation Worker 同时运行时验证新 intent 只进入冻结 Generation route、legacy 只 drain；在已提交 Generation operation 后重启 API，在未完成/unknown Media 或 multipart/export operation 中重启至少一个 Worker，断言 workflow 与 owner ledger resume/reconcile、无重复 submit、无重复 AssetVersion/ExportArtifact。
- [x] 7.4 增加 focused failure scenarios：首次 probe 与 live runnable gate 分离；uninstalled/not-approved/MVP-B/non-runnable/policy 超限/quota exhausted 零 ProviderCall/external submit；resource probe unavailable/capability unsupported/hard limit 零 intent/ledger/outbox；按 Provider 能力注入远端已接受后响应/Worker 丢失，验证 correlation lookup，或 no-lookup 时 Text/GPT Image `ProviderCall.unknown` / Agnes `VideoOperation.submission_unknown` 人工终局且外部接受/计费至多一次；同时覆盖 Agnes duplicate cancel、poll observation 重复/迟到、result registration response-loss 与 late result precedence、Storage session/Export packaging unknown、缺凭据、provider/TOS timeout、renderer capability missing、generated candidate TextReview 未通过或 pending/rejected/stale/foreign/current mismatch 零 Media dispatch、普通 uploaded/source/audio inspection/proxy 正向不回归、derivative 未 ready、checksum mismatch 和 scope violation。
- [x] 7.5 生成 `E2E-MVPA-001` 版本化报告和脱敏摘要，区分 `ready`、`not_ready`、`unconfigured`、`renderer_unconfigured` 与业务 `failed`，禁止未配置路径计入退出成功。
- [x] 7.6 增加证据 secret scan、catalog/ProviderOperationPolicy/RuntimeResourceSnapshot/CapacitySnapshot identity/revision/hash/admission、execution route、outbound correlation/lookup outcome、external accept count、StorageProfile/BucketBinding snapshot、owner-specific state/subphase、owner handoff、operation/artifact hash、restart/reconcile replay 和 no-GC retention 验证；报告不能包含认证头、token、完整 response 或媒体 bytes。

## 8. 验证、回归与交付门

- [x] 8.1 先运行 domain/application/adapter contract tests，再运行 API/router/worker/migration/Compose readiness tests；失败时保留原始错误和受影响 operation，不静默重试。
- [x] 8.2 运行默认 Mock/Local 全量质量门，验证无外部网络、无付费调用、no-fallback、完整 catalog/resource admission、scope/CAS/幂等和 Provider/Storage/Export 既有状态机不回归。
- [x] 8.3 在显式 credentialed sandbox 运行 provider/TOS/renderer probes 与完整 E2E；缺任一外部前置时只报告稳定 `unconfigured`，不得宣称 MVP-A ready。
- [x] 8.4 执行 `openspec validate close-phase-one-mvp-a-runtime-gaps --strict --json`、项目实际 strict validate、`pnpm run check`、Compose config/build/up/readiness、迁移 cycle 与 `git diff --check`，记录命令和实际输出。
- [x] 8.5 对照 DDD owner、BDD 场景、SDD contract、TDD 结果更新 `docs/evidence/E2E-MVPA-001.json` 和项目交接；确认不存在 secret、真实请求残留或范围外 MVP-B 实现。
