## 0. 总体计划追溯与边界

本 change 对应 `plan-phase-one-drama-mvp-a` 任务 `3.3`，并遵从 `5.1`、`5.2`、`5.4`、`5.5`。直接依赖已归档 AssetVersion、`implement-workflows-runs-slice` 与 `implement-provider-model-skill-catalog`；总体 plan 仅协调交付，不是运行时代码依赖。ProviderCall 的调用/费用/幂等账本仅归 catalog，RunEvent 仅归 workflows/runs；通过 `run_id`、`node_run_id`、`correlation_id` 关联。poll 原始观察可进 ProviderCall attempt metadata，归一业务状态只追加 RunEvent。完整非目标是声明所有公开 Agnes capability、隐式 live execution、同步生成、callback/webhook、text/image orchestration、raw video persistence、必要 Activity contract 之外的新 Temporal workflow、冻结最终 HTTP error envelope、billing settlement，以及重做 catalog 或 WorkflowRun/NodeRun/RunEvent 状态机与事件历史。本 change 只复用 AssetVersion/catalog 的 canonical `schema_version` 与所有者同值 HTTP `schemaVersion`，不创建独立版本事实。

## 1. 异步 Domain 与 Contract

- [x] 1.1 定义 frozen account capability snapshot、probe 前无硬编码 ID、优先考察 v2.0 稳定候选且最终只冻结一个实测通过的 image-to-video mode（排除 2.5 preview）、当前 storyboard AssetVersion/ShotSpec/duration/aspect ratio 输入 snapshot、async operation state、submit/poll/cancel/result commands、`submission_unknown` reconciliation、run linkage、retention policy/version/hold 与 stable errors，声明 ProviderCall/RunEvent 唯一所有权，并固定只复用 AssetVersion/catalog canonical `schema_version` 与同值 HTTP `schemaVersion` 的 owner-reference 合同。
- [x] 1.2 编写失败的定向 tests，覆盖 probe 前硬编码 model/mode 拒绝、probe 后非冻结 capability/2.5 preview 拒绝、storyboard AssetVersion/ShotSpec 过期或跨 scope、duration/aspect ratio 缺失/不支持、monotonic transitions、duplicate submit/cancel/poll、`submission_unknown` reconciliation、取消后未引用晚到候选、retry/cost/native usage/retention visibility、attempt metadata、无重复 RunEvent history，以及版本引用缺失、冲突或独立版本事实在调用前无写入拒绝。
- [x] 1.3 实现 framework-free state rules 与用 `run_id + logical_operation` 实现的 ProviderCall idempotency。

## 2. Persistence 与 Activity Boundary

- [x] 2.1 定义 Repository/UoW ports，并仅在需要时增加 operation state、poll-observation fingerprint 与 result-link 的 additive persistence/migration；原始 poll observation 仅进入 ProviderCall attempt metadata，不创建 event history。
- [x] 2.2 实现 post-commit、idempotent Temporal Activity adapter boundary，用于 opt-in Agnes submit/poll/cancel/result。
- [x] 2.3 实现 poll normalization、stored provider request ids、terminal precedence 与 `submission_unknown` reconciliation behavior；仅由 workflows/runs 追加归一业务 RunEvent。

## 3. Result Registration 与 Interfaces

- [x] 3.1 复用 bounded download/MIME/size/duration/dimension/checksum/StoredObjectRef 安全验证和 StoragePort，为每个 successful operation 登记一个 verified video AssetVersion；该候选前验证不是 MediaInspect derivative generation。
- [x] 3.1a 定义 `VideoTakeCandidate`、`TakeReviewDecision(accept|reject|retake)`、successor/stale/late-result 状态、candidate revision/provenance、retake 新 `logicalOperation` 与 review HTTP/contract fixtures；terminal result/storage 安全验证后只创建 immutable candidate + existing AssetVersion，candidate/pending_review 不携带 derivative readiness。accept 先校验 exact candidate/source/ShotSpec facts 并只一次交给 scenes exact current-video CAS，CAS 成功后才 MediaInspect/derivatives，最后 Timeline handoff；legacy/unknown `approve` 零 current/retake/AssetVersion/Timeline 副作用，derivative pending/failed/stale 仅阻断 Timeline/preview/export，不阻断或撤销 accepted/current。
- [x] 3.2 增加 API/worker BDD 与 adapter integration 定向 tests，覆盖 missing configuration、probe/input snapshot、failures、retries、duplicate/late polls、`submission_unknown` reconciliation、attempt metadata、无重复 RunEvent、无 raw media persistence，以及 `schema_version`/`schemaVersion` owner 同值与冲突时无 ProviderCall/RunEvent/StoragePort/AssetVersion 写入。
- [x] 3.3 保持 `Mock Provider +` 显式 Local test/offline profile 默认测试组合，并证明 Agnes 首次 connection-test/probe 仅需 installed/approved/MVP-A、explicit live opt-in/profile/credential/timeout 且成功后冻结 snapshot、不需既有 snapshot/`runnable=true`，real Agnes invocation 再要求 snapshot/`runnable=true`；未选 mode/MVP-B candidate 零外部调用且 live 失败不得切换 profile。

## 4. 验证

- [x] 4.1 运行定向 domain/application/adapter/integration/contract/BDD tests，覆盖 asynchronous failure、retry、duplicate/late poll、`submission_unknown` reconciliation、storyboard input binding、cancellation 后 late result、owner schema version 同值映射及冲突无副作用。
- [x] 4.2 运行 `openspec instructions apply --change integrate-agnes-video-provider --json`、`openspec status --change integrate-agnes-video-provider --json`、`openspec validate integrate-agnes-video-provider --strict --json`、`openspec validate --changes --strict --json`、`pnpm run check` 与 `git diff --check`。
- [x] 4.3 添加 submit preflight tests：accepted candidate/provenance/eligibility、ShotSpec、duration/aspect ratio exact snapshot；未接受、stale、foreign、hash/revision mismatch 均零 ProviderCall 和零 external submit。
- [x] 4.3a 固化 Provider/Media Worker ownership tests：Agnes 只写 transport/ProviderCall evidence，MediaInspect/DerivativePort 生成 normalized metadata、proxy、thumbnail、keyframe index、waveform records；覆盖 cancelled-late、duplicate/retry、inspection failure、source revision stale、`Mock Provider +` 显式 Local test/offline profile 与无 Timeline/Provider 越界写入。
