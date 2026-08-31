## Why

旧 `close-phase-one-mvp-a-runtime-gaps` 的全部实现任务已经勾选，但其 `sol_max_closure` 审核仍以 B1-B7 指出运行时闭合证据不足。旧 lineage 不可重试、不可改写为已关闭；因此需要独立 replacement change，把可核验的跨进程冻结准入、媒体交接、显式 local composition 与 owner-specific reconcile 合同收口。

本 change 只修复 B1-B7。credentialed E2E、真实 Provider/TOS/付费请求与 MVP-A 退出关闭没有授权且前置不具备，必须继续报告 `not_ready`/`unconfigured`，不能以本 change 宣称 live ready。

## What Changes

- 为跨 API/Generation Worker/Media Worker restart 的新 operation 固化冻结 admission identity：`runId`、`nodeRunId`、`logicalOperation`、catalog/capability/resource snapshot、`executionRoute`、`workflowType`、`taskQueue` 和 `schemaVersion` 必须由 durable owner ledger 读取并验证；缺失、漂移或 legacy identity 不得前向派发或 fallback。
- 收口 Media dispatch、activity、Storage 和 Export 的 typed handoff：Generation/Media dispatcher 只消费 owner 已提交的持久 intent/outbox；Storage 只返回 verified `StoredObjectRef`；Assets/Export owner 分别且仅能登记 `AssetVersion`/`ExportArtifact`；production wiring 必须可达并在 unknown/restart 时先 reconcile。
- 定义显式本地 live composition：只有配置完整、已批准、可运行且 frozen 的 local profile 可组合对应 adapter/activity；缺失 credential reference、capability、renderer 或 queue binding 时返回原始脱敏 `unconfigured`/`not_ready`，不得转为 Mock/Local fallback 或伪造成功。
- 对 Text 的 terminal success 与 ambiguous submit 做 owner 映射：success 只能通过匹配的 Text/ProviderCall ledger 产生一次 immutable batch/candidate handoff；不确定提交保留 `ProviderCall.unknown` 并按冻结 lookup contract reconcile。
- 对 Image/Video 固化 retry/reconcile：Text/Image 保持 `ProviderCall.unknown`，Video 保持 `VideoOperation.submission_unknown`；retry 复用 frozen correlation/remote identity，不能确认时停止自动重提，late result 只能按 owner contract 登记。
- **BREAKING**：旧的未冻结或不完整 admission、未类型化 Storage/Export handoff、未配置 live composition 不再可被“宽松”路径接纳；请求改为稳定失败，且零外部副作用。

## Capabilities

### New Capabilities

- `runtime-closure-contract-repair`: 定义 B1-B7 的跨进程冻结身份、owner ledger、Media/Storage/Export typed handoff、local composition、Text/Image/Video terminal/reconcile 以及失败优先验证合同。

### Modified Capabilities

- 无。本 change 不改写既有 capability 的 owner 或状态值域，而以一个明确的 replacement runtime-closure capability 为其跨进程执行与验收补充可追溯合同。

## Impact

- 受影响范围：`services/api` 的 runtime、Generation/Media dispatch、Text/Image/Video/Storage/Export application 与 adapters；`workers/generation`、`workers/media`、`workers/runtime.py`；必要的 additive schema/migration、Compose composition 和 focused tests。
- DDD：Run/NodeRun、ProviderCall、VideoOperation、UploadSession、StoredObjectRef、AssetVersion、ExportJob/ExportArtifact 继续各自拥有状态；本 change 不引入统一 operation 状态机，也不让 worker 越权写 owner 事实。
- BDD：重启、重复投递、丢失响应、unknown、stale/foreign/incomplete admission 和无配置 local profile 都必须有可观察的稳定结果及零副作用断言。
- SDD：冻结身份、typed handoff payload、outbox/activity route、remote correlation、lookup capability、状态映射与 no-fallback 是接口合同；真实凭据、账号、TOS、远程 endpoint、付费调用和 MVP-A 退出报告不在范围。
- TDD：先写跨进程/restart、success/ambiguous、retry/reconcile、handoff/production reachability、unconfigured/no-fallback 的失败测试，再实现最小闭合；默认验证只使用 Mock/Local，本 change 的外部 E2E 证据固定为 `not_ready`。
