## Context

`close-phase-one-mvp-a-runtime-gaps` 的任务清单已经全数完成，但旧 `sol_max_closure` 对 B1-B7 的审核没有形成可关闭结论。该历史记录必须保持原样：replacement 不能重试、迁移、read-through 或把旧任务改写为 closed。本 design 只为当前独立 change 定义修复和验证边界。

当前仓库已有 Run/NodeRun、ProviderCall、VideoOperation、UploadSession、StoredObjectRef、AssetVersion、ExportJob/ExportArtifact、outbox 与 Generation/Media Worker。问题不在于再建立一套状态，而在于这些 owner 跨进程读取 frozen admission、投递 activity、登记结果与恢复未知状态时缺少一个可测试、fail-closed 的闭合合同。

### DDD

各领域对象保持唯一状态归属：Run/NodeRun 冻结 route；Text/Image 的 `ProviderCall` 拥有 submit 观察；Video 的 `VideoOperation` 拥有异步 lifecycle；Storage 拥有 UploadSession/verified `StoredObjectRef`；Assets 与 Export 分别拥有 `AssetVersion`、`ExportArtifact`。Worker 是 activity 执行者，不拥有上述事实。

### BDD

可观察场景是：API 接受一次完整 intent 后，另一个进程只能按同一 frozen identity 执行；重启、重复 outbox、响应丢失、remote lookup 缺失、Storage/Export handoff 不完整或 local profile 未配置都稳定失败或 reconcile，且不产生第二次提交、版本或导出制品。

### SDD

所有跨边界 payload 必须含 owner id、scope、revision/hash、operation identity 与 frozen route/admission references；typed handoff 只接收 owner 定义的 immutable reference，绝不传媒体 bytes 或可重解释的裸对象键。未知状态不做 generic fallback。

### TDD

每项实现先以 API/worker 重启、cross-process、duplicate、success、ambiguous、unknown、unconfigured 和 no-side-effect 测试固定失败，再增加最小实现；Mock/Local 是唯一默认测试组合，外部 credentialed 场景只检查 `not_ready` 证据。

## Goals / Non-Goals

**Goals:**

- 完成 B1-B7：跨进程 frozen admission、Media dispatch/activity/production reachability、Storage/Export typed handoff、local explicit composition、Text terminal mapping、Image/Video retry/reconcile。
- 保持 owner 状态值域、CAS、scope、immutable version、no-fallback 和 API 不执行外部副作用的既有边界。
- 让每个修复都有 focused failure-first test、可运行 local verification 和明确的 `not_ready` 外部证据。

**Non-Goals:**

- 不执行 credentialed E2E、真实 Provider/TOS/付费 API、真实远程 probe 或账号/密钥管理。
- 不宣布 MVP-A 退出关闭、不更新旧 `sol_max_closure`、不重写其审核历史。
- 不增加统一 operation 表、不改变 ProviderCall/VideoOperation/Storage/Export 的既有状态值域，不实现 MVP-B、callback/webhook 或 UI 重构。

## Decisions

### 1. Durable owner ledger 是唯一跨进程 admission 来源

新 operation 必须在 owner transaction 中冻结 `runId`、`nodeRunId`（适用时）、`logicalOperation`、project scope、catalog/capability/resource admission reference、`executionRoute`、`workflowType`、`taskQueue`、`schemaVersion` 与 correlation policy。dispatcher/activity 只读取该 record，并逐项比对 payload；缺失、scope/hash/revision 不符、legacy route 接收新 intent 或 snapshot drift 均在任何 provider/storage/render side effect 前拒绝。

选择 durable ledger 而非 activity 参数或进程内 cache，因为后者无法证明 restart 后身份一致；也拒绝把 identity 重新从当前 catalog 推导，因为 catalog 更新不得改变已提交 operation。

### 2. 按 owner 分层的 typed handoff 收口 Media、Storage 与 Export

Generation dispatcher 只从 matching durable intent/outbox 启动固定 workflow ID；Media dispatcher 只从 accepted-current 的 generated candidate 或已授权 uploaded source 启动其允许的 activity。activity 输入和返回只使用版本化 typed DTO。Storage 的 terminal output 是 verified `StoredObjectRef`；Assets owner 才能 append 一次 AssetVersion，Export owner 才能登记一次 ExportArtifact。任何 response-loss、duplicate delivery 或 restart 先对同一 operation/revision reconcile，不能改走未类型化直接写库路径。

选择显式 owner handoff 而非 Worker 直接写 SQL，因为后者绕过 CAS、审计与唯一性约束；production reachability 以注册的 worker/activity、task queue、Compose service binding 和 focused integration test 证明，不以 import 成功替代。

### 3. local live composition 采取显式完备配置而非隐式降级

runtime composition 必须以配置的 local profile、approved/runnable capability、credential reference、queue/activity binding 和所需 renderer/storage capability 创建 port。缺少任一项返回保留原始原因的 `unconfigured` 或 `not_ready`。`PROVIDER_MODE=mock` 与 `STORAGE_MODE=local_workspace` 只在明确选择时使用，live path 失败绝不自动替换。

选择 fail-closed 而非“为开发方便”回退，是因为 B4 要验证真实 composition 的资格边界；默认 Mock/Local 测试仍保持可重复且零网络。

### 4. terminal 与 ambiguous 状态按 owner 映射，不归一为成功

Text/Image success 必须关联 matching frozen ProviderCall，且只通过 owner 的 idempotent result command 产生一次 immutable candidate/batch；ambiguous submit 保持 `ProviderCall.unknown`。Video 的 ambiguity 保持 `VideoOperation.submission_unknown`。retry 先用 persisted remote correlation 和 frozen lookup capability reconcile；若无 remote id 或 lookup，不发第二次 submit，保留人工处置诊断。late result 只能登记为 owner 允许的 immutable、未引用候选，不能替换 current 或 export。

选择 owner-specific unknown 而非通用 retry，是为了不掩盖付费外部操作是否已被接受；也避免将 Text/Image 与 Video 的状态机混合。

### 5. 验证把可运行 local contract 与不可用外部证据分开

focused tests 覆盖 B1-B7 的正反路径，随后运行 API/worker test、lint/typecheck、OpenSpec strict、migration/Compose/readiness（在本地 Mock/Local 配置）。证据文件必须列出缺失的 `MVP_A_CREDENTIAL_SANDBOX` prerequisites、`result=unconfigured`、`readiness=not_ready` 和 `externalAcceptCount=0`。该记录是边界证据，不是 live 成功。

## Risks / Trade-offs

- [共享 schema/outbox 的 additive 变更可能与运行中的 worker 版本不兼容] → 以 schemaVersion/route freeze、明确 rollout 顺序和 restart/duplicate tests 约束；旧 route 只 drain/reconcile。
- [unknown 状态会需要人工处置] → 保留 correlation、native diagnostic、owner id 和查询入口；禁止自动重提造成重复费用。
- [本地 Mock/Local 无法证明真实远端兼容性] → 证据保持 `not_ready`，不把 mock 结果外推为 credentialed E2E。
- [Compose/readiness 仅证明本地 wiring] → 显式校验 activity/queue/worker 绑定和 migration head，并将账号、网络、TOS、二进制前置列为外部未决项。
- [并行改动可能改变目标代码事实] → 实施前重新读取代码、diff、migration head 与 tests；本 proposal 不把当前未验证实现写成已完成事实。

## Migration Plan

1. 先为 B1-B7 写失败测试，并在现有 owner ledger/route/schema 中补充最小 additive frozen fields 或约束。
2. 部署 migration 后，使 API 只为新 intent 写新冻结 identity；旧已冻结 route 只允许 drain/reconcile，不再接收新 admission。
3. 注册 Generation/Media activities 与 production task queue wiring，先在 Mock/Local Compose 中验证 restart、duplicate 和 typed handoff。
4. 启用 strict unconfigured composition checks；配置缺失时继续拒绝，不把运行流量转给其他 adapter。
5. 执行 focused tests、质量门、migration cycle、Compose readiness 和证据更新；若任一步失败，保留 owner ledger/diagnostic 并回滚应用版本或 additive migration 的调用路径，不删除 operation/candidate/artifact 审计。

## Open Questions

- credentialed sandbox 的账号、TOS 权限、allowlist、secret delivery、远端 endpoint 与 `ffmpeg`/`ffprobe` 输入尚未提供；它们不阻塞本地 B1-B7 合同实现，但阻止 live E2E 与 MVP-A exit closure。
- 生产部署的 worker rollout 编排和真实远端 retry limits 需在取得上述外部授权与输入后以单独受控 change 决定；本 change 不作假设。
