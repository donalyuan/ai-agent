# GPT Image Provider 设计

## runnable gate 与单一 append

GPT Image 首次 connection-test/probe 只需 `adapterInstalled=true`、catalog `approval=approved`、`featureGate=MVP-A`、explicit live opt-in、已选 profile、可解析 credential 与 timeout，成功后冻结 capability snapshot，不要求既有 snapshot 或 `runnable=true`；explicit live invocation 再同时要求该成功 snapshot 与 `runnable=true`。MVP-B candidate 零外部调用，默认测试组合为 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），并保持 explicit live opt-in；运行开始后 Adapter/Profile 冻结。verified Provider terminal success 为唯一 result AssetVersion append 点，retry/reconcile 返回同一 version/candidate。AssetEdit accept 仅追加 AcceptDecision/audit 和同一 version 的 scenes exact eligibility CAS，严禁复制 bytes/object/ref 或第二 append；reject/stale/foreign accept 零 AssetVersion/current/Timeline mutation。

## 上下文

阶段 0 提供 `ImageGenerationPort`、deterministic Mock Provider、`LocalWorkspaceAdapter` 与只追加的 AssetVersion。Provider catalog change 提供选定 model/capability snapshot、schema validation、secret masking 和 ProviderCall 幂等。`gpt-image-2` 是 configuration candidate，不是代码级默认值。

## 总体计划追溯、依赖与非目标

本设计落实 `plan-phase-one-drama-mvp-a` 任务 `3.2`，并遵从共享工程任务 `5.1`、`5.2`、`5.4`、`5.5`。总体 plan 只描述协调关系，不构成运行时代码依赖。直接依赖 AssetVersion、`implement-workflows-runs-slice`、`implement-provider-model-skill-catalog` 和 `implement-asset-bible-continuity-slice`；未完成依赖时保持待实现状态，不能以本 change 代替其职责。

ProviderCall 的唯一持久化调用、费用与幂等账本归 catalog；本 change 只传递/记录关联键 `run_id`、`node_run_id`、`correlation_id`。RunEvent 归 workflows/runs，本 change 不创建、复制或维护 RunEvent 历史。

完整非目标：文本/视频生成、hardcoded model names/endpoints、implicit live network calls、直接在数据库保存 image bytes/base64、跨项目 reference-image reuse、image post-production、billing settlement，以及拥有或实现 catalog、WorkflowRun/NodeRun/RunEvent 状态机或事件历史。本 change 只复用 AssetVersion 与 catalog 已拥有的 canonical `schema_version`；HTTP `schemaVersion` 仅由所有者映射同一值，图片 adapter/application 不创建独立的 Provider 专用版本事实。若输入引用携带缺失或冲突版本，必须在 Provider 调用和 UoW 前拒绝，且不写 ProviderCall、RunEvent、StoragePort 或 AssetVersion。

## 目标

通过由 catalog data 选择的 adapter 支持 `generate` 和 `edit`；在持久化前校验输入与输出；经 StoragePort 创建新 AssetVersion；与 Run 关联；并显式呈现 failed/unconfigured/retry/cost states。

## 决策

- Application 接收 operation command、run id、logical operation、project asset target、selected catalog snapshot、parameters 及可选且已拥有的 reference AssetVersions。catalog 在 adapter 获得 opt-in credential 前校验 operation/parameters。
- Application 还必须从 AssetBible owner 读取并冻结同一 project/episode/scene/shot target 的 accepted `ResolvedContinuitySnapshot` ID/revision/hash、resolved entry references、GenerationSpec refs 与 reference AssetVersion refs。该 payload 只保存 owner references，不复制 entry 内容；snapshot incomplete、stale、foreign、hash mismatch 或 target 存在 pending `ContinuityRevisionTask` 时，在 intent、ProviderCall 和网络请求前返回稳定 conflict/validation diagnostic。
- `generate` 与 `edit` 留在 `ImageGenerationPort`；`edit` 需要由 selected snapshot 提供明确 source/reference semantics。不引入 text-model 或 video-model adapter。
- adapter 将 URL 或 base64 payload 归一为有界 temporary stream。每次请求最多 8 个 reference、reference aggregate 不超过 32 MiB、图像最大边长 8192；输入/reference 只允许 observed PNG/JPEG/WebP，edit mask 只允许 observed PNG。validator 校验配置 URL allowlist、decoded MIME allowlist、declared/observed MIME 一致性、dimensions、byte limit 与 SHA-256，再由 StoragePort 写 canonical object reference。
- URL fetcher 不跟随 HTTP redirect；解析和连接前后均拒绝 loopback、RFC1918/private、link-local、reserved/unspecified/multicast 与 cloud metadata service 地址，DNS 解析到任一禁用地址即失败。未配置 allowlist、host/port 不匹配或 redirect 响应均在读取响应体和 Provider invocation 前拒绝。
- Asset registration 是验证后的 application command。它追加新的不可变 image AssetVersion，并在同一 UoW 记录 run/call linkage；验证失败不写 AssetVersion。
- `run_id + logical_operation` 是 retry key。retry 读取 catalog 所有的 ProviderCall state：terminal success 复用 registered version；pending/recoverable state 遵从 policy；没有 explicit、audited retry decision 不得发起新的 chargeable request。
- 工作流侧可根据结果追加归一业务 RunEvent，但只有 workflows/runs 写入该事件；图片侧只提供关联与诊断，不保存重复事件历史。

## 风险与取舍

- [URL 过期、DNS rebinding 或恶意地址]：只经 allowlisted、size-bounded validator 获取一次，禁止 redirect，并在解析/连接边界拒绝所有内网、保留和 metadata 地址。
- [Base64 expansion 耗尽内存]：限制 encoded/decoded 大小，transport 允许时 streaming。
- [Provider output metadata 不准确]：以 observed bytes/dimensions 覆盖 claimed metadata，保留 mismatch diagnostic。
- [真实账号差异]：由 capability snapshots 和 opt-in probes 管理可用性；默认 tests 使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）。
- [连续性 snapshot 在排队期间变化]：提交前重新读取 AssetBible owner revision/hash 与 continuity task；不匹配即停止，不以旧 prompt 或客户端缓存继续生成。

## 迁移计划

本 change 依赖 catalog 与 AssetVersion persistence。只在现有表无法表达时，通过 additive migration 增加 result/call-to-run linkage；先部署 Mock behavior，再在配置 capability probe 后启用 live profile。回滚禁用 profile，并保留 append-only audit/versions。

## 待确认

URL allowlist 的具体 host entries、reference-image provider fields、live-account capabilities 与 native usage fields 仍是 explicit profile/probe 输入；最多 8 个 reference、合计 32 MiB、最大边长 8192、PNG/JPEG/WebP 输入、mask 仅 PNG、禁止 redirect 与禁用地址类别已冻结，未配置 allowlist 时必须显式失败。

## DDD / BDD / SDD / TDD

- **DDD**：图片结果只追加 AssetVersion，ProviderCall/RunEvent 保持各自所有权；AssetBible 只由其 owner 解析，图片侧仅冻结引用。
- **BDD**：覆盖 project-safe edit、accepted continuity snapshot、pending task、8/32 MiB/8192/allowlist、失败、重试和 opt-in。
- **SDD**：固定 bounded validation、AssetBible/AssetVersion owner references 和无媒体 bytes 存储。
- **TDD**：先写 capability/continuity/reference/media 负例，再验证 adapter、StoragePort、HTTP 和 BDD。

## Current / Defined / Todo

- **Current**：ImageGenerationPort、`Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）与 AssetVersion append-only 已有。
- **Defined**：catalog-driven generate/edit、输入/输出限制和幂等审计。
- **Todo**：完成此 change 的未勾选任务，真实调用只在显式 profile/probe 下验证。

Image success 先以 ProviderCall/run snapshot 关联未引用 candidate AssetVersion；不得将 `AssetVersion.status` 作为接受证据。只有 scenes owner 使用 exact candidate/provenance/id/revision/hash/target CAS 接受后，consumer 才能读取 current storyboard reference。
