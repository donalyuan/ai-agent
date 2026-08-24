# Agnes Video Provider 设计

## terminal handoff 与视频顺序

Agnes 的 selected operation 必须冻结 `adapterInstalled`、catalog `approval`、capability snapshot、`runnable`、`featureGate`；首次 connection-test/probe 只需 installed、approved、MVP-A、explicit live opt-in、已选 profile、可解析 credential 与 timeout，成功后冻结 snapshot，不要求既有 snapshot 或 `runnable=true`；explicit live invocation 还需成功 snapshot 与 `runnable=true`。未选 mode/MVP-B candidate/TTS/ASR/MiniMax H3/Seedance 2.5 不可运行且零外部调用。verified Provider terminal success 是唯一 result `AssetVersion` append 时点；retry/reconcile 只返回同一 version/candidate。

Provider terminal result candidate 后只能人工以 exact candidate/source/ShotSpec facts `accept`，再由 scenes 执行一次 exact current CAS，之后 MediaInspect/derivatives，最后 Timeline handoff。review DTO/Signal/audit action 只允许 `accept|reject|retake`；legacy/unknown `approve` validation 且零 current/retake/AssetVersion/Timeline mutation。derivative `pending|failed|stale` 只阻断 Timeline/preview/export，不阻断、撤销或降级已 accepted/current。

## 上下文

阶段 0 有 `VideoGenerationPort`、`Mock Provider +` 显式 Local test/offline profile、worker queues，但没有 business Activity。AssetVersion 只追加。catalog target 引入 frozen snapshot 与 ProviderCall audit。Agnes account capability 未被验证为等价于全部公开文档能力；验收只针对已配置账号实际启用能力的 snapshot。

## 总体计划追溯、依赖与非目标

本设计落实 `plan-phase-one-drama-mvp-a` 任务 `3.3`，并遵从共享工程任务 `5.1`、`5.2`、`5.4`、`5.5`。总体 plan 仅说明协调关系，不形成运行时代码依赖。直接依赖 AssetVersion、`implement-workflows-runs-slice` 与 `implement-provider-model-skill-catalog`；它不得替代这些 change 的领域职责。

ProviderCall 由 catalog 领域唯一持久化，作为调用、费用和幂等账本；RunEvent 由 workflows/runs 领域唯一拥有。关联仅使用 `run_id`、`node_run_id`、`correlation_id`。`poll` 的原始 observation 可追加为 ProviderCall attempt metadata，带 external request id、source fingerprint、provider timestamp 和 diagnostic payload；归一后的业务状态只触发 workflows/runs 追加 RunEvent，绝不新建或镜像第二份事件历史。

完整非目标是声称全部公开 Agnes capability、隐式 live execution、同步生成、callback/webhook、text/image orchestration、raw video persistence、必要 Activity contract 之外的新 Temporal workflow、冻结最终 HTTP error envelope、billing settlement，以及重做 catalog 或 WorkflowRun/NodeRun/RunEvent 状态机与事件历史。本 change 只复用 AssetVersion 与 catalog 已拥有的 canonical `schema_version`；HTTP `schemaVersion` 仅由所有者映射同一值，Agnes adapter/application 不创建独立的 Provider 专用版本事实。若输入引用携带缺失或冲突版本，必须在 Provider 调用和 UoW 前拒绝，且不写 ProviderCall、RunEvent、StoragePort 或 AssetVersion。

## 目标

提供确定性的 `submit`、`poll`、`cancel`、`result` commands；每个 operation 绑定冻结 account snapshot、当前 storyboard AssetVersion、对应 ShotSpec、显式 duration/aspect ratio；归一重复/迟到的 polling observations；确保 `submission_unknown` reconciliation 和 Activity retries 不重复 external submit 或 output AssetVersions；并保留 failure/cost/configuration evidence。

## 决策

- 提交在 post-commit Activity 请求 Agnes 之前，先持久化一个由 `run_id + logical_operation` 键控的 intent/ProviderCall。Activity 使用 stable idempotency key 与 provider request id；重复 delivery/retry 观察记录状态，不再发起 submit。
- 提交时持久化只追加 CapabilitySnapshot，表示配置账号已验证的单一 image-to-video mode、精确 model/mode ID、required parameters、limits 与 cancellation/result semantics。实施 probe 优先考察 v2.0 稳定候选，但 probe 前不硬编码任何 ID，并明确排除 Agnes 2.5 preview；即使 catalog 后续改变，runtime selection 仍用该 snapshot。
- `submit` 输入 snapshot 必须包含当前 storyboard 引用的 image AssetVersion ID/revision/hash、对应 ShotSpec ID/version/hash、显式 durationSeconds/durationFrames 和 aspectRatio；全部引用必须属于同一 project/episode/scene/shot，且与冻结 capability 一致。任何过期、跨范围、缺失或参数冲突在外部调用前失败。
- polling responses 先作为 ProviderCall attempt metadata 保存，随后映射到一个 monotonic operation state，并包含 external request id 和 source fingerprint；stale/duplicate/conflicting polling observations 可被忽略或仅作诊断记录，不能回退 terminal state、产生重复外部副作用或重复业务事件。
- `submission_unknown` 是独立可诊断状态；任何重试 submit 前必须以已持久化 intent、provider request id 和允许的 `poll/result` 查询 reconciliation，无法确认时保持 unknown，不创建第二个收费请求。
- `cancel` 幂等：未 terminal operation 只请求一次 cancellation；重复调用返回记录的 cancel/cancelled/terminal state。cancellation 后收到 results 时遵从已持久化 provider-timestamp/state precedence rule，绝不静默转为 success；可验证的晚到媒体只能登记为未引用 candidate，绝不成为 current AssetVersion/Shot 引用。
- successful result 经与其他媒体相同的 bounded media-validation 和 StoragePort-to-new-AssetVersion flow，然后原子记录 run linkage。validation failed 不得创建 AssetVersion。
- workflows/runs 根据归一结果只追加自身 RunEvent；此 change 不自行持久化 event history。真实 adapter 只由 explicit enabled opt-in profile/probe 选择。默认 tests 使用 deterministic Mock 与显式 Local test/offline profile（adapter identity=`local_workspace`）；未配置 live Agnes 返回 explicit error。

### Video take review and derivative boundary

Agnes `result` 只在 StoragePort 完成 bounded download/MIME/size/duration/dimension/checksum/StoredObjectRef 安全验证后追加一个 immutable `VideoTakeCandidate` 和既有 result `AssetVersion`，状态为 `pending_review`，并携带 source image eligibility、ShotSpec、duration/aspect snapshot、AssetVersion id/revision/hash 与 provider request id；该验证不是 `MediaInspect`，candidate 不携带 derivative readiness。人工审核由 workflows/runs owner 以 `runId + logicalOperation + candidateId + candidateRevision` 绑定的显式 command 完成：`accept` 使 candidate accepted 并调用 scenes owner 的 exact current-video eligibility CAS；仅在 CAS 成功后 Media Worker 才生成 metadata/proxy/thumbnail/keyframe/waveform；`reject` 只追加拒绝事实；`retake` 追加新的 execution intent/successor candidate，必须使用新 `logicalOperation` 和冻结的 successor input，旧 take immutable 且不得复用旧预算确认。没有 accepted current eligibility 的视频 candidate 不能被 Timeline assembly 或 `timeline.handoff` 消费。

Provider 不生成或拥有 thumbnail、keyframe index、waveform、proxy 或 canonical normalized metadata。media worker 的 `MediaInspectPort` 以 verified StoredObjectRef 为输入，生成带 source AssetVersion id/revision/hash、tool/version、derivative schema/version、retention/license/hold 的独立 derivative records；Timeline 只读消费同一 source fingerprint 且状态为 `ready` 的派生物。派生失败或 source revision 变化只标记 stale/diagnostic，不改变 ProviderCall、candidate accept 或 current reference。

## 风险与取舍

- [账号 capability 因 tenant/time 而异]：snapshot probe result，验收只针对该 snapshot，不依赖 public documentation。
- [poll 乱序/重复]：monotonic state、source fingerprint、persisted transition checks 与 idempotent event handling。
- [Activity retry double-submit/double-charge]：effect 前持久化 intent，并复用 `run_id + logical_operation`/provider request id。
- [late/malformed output]：StoragePort/AssetVersion 写入前执行 bounded media validation 和 terminal-state precedence。

## 迁移计划

仅在 ProviderCall/run data 无法表达时，增加 additive async-operation、poll-observation fingerprint 与 result-link records；ProviderCall attempt metadata 不替代也不复制 RunEvent。先部署 Mock path，启用 live profile 前要求 explicit account probe。回滚禁用 profile，并保留 audit/snapshot/results 供 reconciliation。

## 待确认

Agnes 已配置账号的候选 probe request/response、最终精确 model/mode ID、native usage 字段、retention duration 与 final HTTP error envelope 仍需以 explicit profile/probe 提供；v2.0 只是首选候选族，2.5 preview 不属于本 change 的输入。当前 storyboard AssetVersion/ShotSpec/duration/aspect ratio 绑定、保留 policy/version/hold、单一 mode 和取消晚到结果为未引用 candidate 已冻结。

## DDD / BDD / SDD / TDD

- **DDD**：异步 operation、VideoTakeCandidate/TakeReview 与 ProviderCall 账本分离；Provider、workflows、scenes、media worker、Timeline 各自拥有明确事实。
- **BDD**：覆盖 probe 前拒绝硬编码 mode、storyboard 输入绑定、submit/cancel 幂等、视频 accept/reject/retake、未经接受不能 handoff、派生物 stale 与 Timeline 拒绝。
- **SDD**：固定 Activity、submit/poll/cancel/result、review/retake command、MediaInspect/DerivativePort 和 StoragePort/AssetVersion 边界。
- **TDD**：先写状态/reconciliation/review/derivative 负例，再验证 adapter、worker、HTTP 和 BDD。

## Current / Defined / Todo

- **Current**：VideoGenerationPort、Worker health、Mock Provider、显式 Local test/offline profile 与 AssetVersion append-only 已有。
- **Defined**：单一 probe mode、异步幂等、未引用晚到候选和保留状态。
- **Todo**：完成此 change 的未勾选任务，真实账号只按 explicit probe 取证。

在创建 `ProviderCall` 或执行 external submit 前，command 必须读取 current eligibility projection，并比较 accepted provenance、candidateId、AssetVersion id/revision/hash、project/episode/shot target、ShotSpec revision/hash、duration 与 aspect-ratio snapshot；任一不精确相等即拒绝，零 ProviderCall/intent/remote request。
