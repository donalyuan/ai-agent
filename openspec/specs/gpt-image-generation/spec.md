# gpt-image-generation Specification

## Purpose
TBD - created by archiving change integrate-gpt-image-provider. Update Purpose after archive.
## Requirements
### Requirement:总体计划追溯和所有权边界
本 capability SHALL 反向追溯 `plan-phase-one-drama-mvp-a` 任务 `3.2`，并直接依赖 AssetVersion、`implement-workflows-runs-slice` 与 `implement-provider-model-skill-catalog`。总体计划仅协调交付，不构成运行时代码依赖。ProviderCall 的唯一调用/费用/幂等持久化账本归 catalog；RunEvent 只归 workflows/runs；双方只能以 `run_id`、`node_run_id`、`correlation_id` 关联。完整非目标是文本或视频生成、硬编码 model name/endpoint、隐式 live network call、数据库直接保存 image bytes/base64、跨项目 reference-image reuse、图片后期制作、billing settlement，以及拥有或实现 catalog、WorkflowRun/NodeRun/RunEvent 状态机或事件历史；本 capability MUST NOT 承担这些职责。

#### Scenario:图片结果关联运行而不重复记事件
- **当** 图片 operation 成功、失败或被重试
- **则** catalog 更新唯一 ProviderCall，workflows/runs 可追加自己的 RunEvent，图片 change 不创建第二份事件历史

#### Scenario:拒绝非目标职责泄漏
- **当** 图片 change 尝试承担任一列明的非目标或把总体计划作为运行时依赖
- **则** 架构依赖/契约测试失败，且不发起外部调用、不写 ProviderCall、RunEvent、StoragePort 或 AssetVersion

### Requirement:复用所有者的 canonical Schema 版本
系统 SHALL 只复用 AssetVersion 与 catalog 所拥有的 canonical `schema_version`。HTTP `schemaVersion` MUST 仅由对应所有者映射同一个值；图片 adapter/application MUST NOT 创建、持久化或推导独立的 Provider 专用版本事实。

#### Scenario:使用一致的 AssetVersion 与 catalog 版本引用
- **当** 图片 operation 接收已解析且同项目的 AssetVersion、model 与 capability snapshot 引用
- **则** 引用中的 HTTP `schemaVersion` 与所有者 canonical `schema_version` 相同，本 change 只保存稳定 owner id/revision/hash

#### Scenario:版本引用缺失或冲突时无副作用拒绝
- **当** 输入引用缺少必需版本、同时携带冲突的 `schema_version`/`schemaVersion`，或实现尝试创建独立版本事实
- **则** 系统在 Provider 调用和 UoW 前返回稳定 validation error，且不写 ProviderCall、RunEvent、StoragePort 或 AssetVersion

### Requirement:由 catalog 选择 ImageGenerationPort operations
系统 SHALL 只能通过 `ImageGenerationPort` 使用 enabled configured Provider/Profile/Model 和 frozen CapabilitySnapshot 暴露 `generate` 与 `edit`。包括 `gpt-image-2` 在内的 model identifiers MUST 由 catalog configuration 选择，业务 flow 不得硬编码。

#### Scenario:使用已启用且已配置的 model 生成
- **当** 一个 run 以 enabled selected snapshot 所接受的 parameters 请求 `generate`
- **则** application 调用选定 ImageGenerationPort operation，并记录 catalog selection provenance

#### Scenario:拒绝未配置或不支持的图片 operation
- **当** profile/model 缺失或禁用、snapshot 缺少 operation 或 parameters 违反其 schema
- **则** Command 返回 explicit diagnostic result，且没有 external request、AssetVersion 或 implicit real Provider fallback

### Requirement:编辑引用明确且项目安全
系统 SHALL 要求每个 edit source/reference image 标识一个由请求 project 拥有、且被 selected capability 允许的既有不可变 AssetVersion。Command MUST NOT 修改 reference version，也不得接受 unowned/cross-project reference。

#### Scenario:编辑已拥有的 reference image
- **当** `edit` 提供 project-owned image AssetVersion 与 valid parameters
- **则** adapter 收到 resolved reference，结果按新 version 处理

#### Scenario:拒绝无效 reference
- **当** edit reference 缺失、不是 image、跨项目或与 capability snapshot 不兼容
- **则** Provider invocation 前校验失败，且不写入新 version

### Requirement:生成输入冻结 accepted AssetBible resolved snapshot
每个 generate/edit operation SHALL 在 intent 与 ProviderCall 前，从 AssetBible owner 读取并冻结目标 project/episode/scene/shot 的 accepted `ResolvedContinuitySnapshot` ID、revision、canonical hash、resolved entry references、GenerationSpec refs 和 reference AssetVersion refs。图片 application/adapter MUST NOT 复制 entry/override 内容、重新解析 override chain 或写 AssetBible。snapshot 为 incomplete、stale、foreign、hash/revision mismatch，或目标存在 pending `ContinuityRevisionTask` 时，operation MUST 在 ProviderCall、外部请求、StoragePort 与 AssetVersion 写入前失败。

#### Scenario:使用已接受且完整的连续性快照
- **当** 当前 target 的 accepted resolved snapshot 完整、同项目且 ID/revision/hash 与 owner facts 匹配，也没有 pending continuity task
- **则** operation 冻结精确 snapshot/reference provenance 后才可进入 capability、费用与 Provider 调用门

#### Scenario:在付费调用前拒绝过期或不完整快照
- **当** snapshot incomplete、stale、foreign、hash/revision 不匹配，或 target 有 pending `ContinuityRevisionTask`
- **则** 系统返回稳定 continuity diagnostic，且不写 intent、ProviderCall、RunEvent、StoragePort 或 AssetVersion，也不使用客户端缓存内容继续生成

### Requirement:GPT Image 请求边界
系统 SHALL 将每个 GPT Image operation 限制为至多 8 个 reference，reference 总量至多 32 MiB，图像最大边长 8192；输入/reference 的 observed 格式 MUST 仅为 PNG、JPEG 或 WebP，edit mask MUST 仅为 PNG。URL source MUST 命中配置 allowlist，MUST NOT 跟随 redirect，并 MUST 在解析和连接边界拒绝 loopback、private、link-local、reserved/unspecified/multicast 与 metadata service 地址。未配置 allowlist、超出任一限制、格式不符、redirect 或 DNS 解析到禁用地址 MUST 在 Provider invocation 前失败。

#### Scenario:reject bounded-reference violations
- **当** 请求携带第 9 个 reference、超过 32 MiB、边长超过 8192、输入不是 PNG/JPEG/WebP、mask 不是 PNG，或 URL 不在 allowlist
- **则** 系统返回 validation/unconfigured，且不写 ProviderCall、RunEvent、StoragePort 或 AssetVersion

#### Scenario:reject redirect and internal network targets
- **当** allowlisted URL 返回 3xx、host 解析/重解析到 loopback/private/link-local/reserved/metadata 地址，或请求目标是已知 metadata service
- **则** fetcher 在读取媒体和任何 StoragePort 写入前返回安全 validation，绝不跟随 redirect、访问目标或把响应登记为 AssetVersion

### Requirement:存储前验证返回媒体
系统 SHALL 将 Provider URL 或 base64 image response 归一为有界 temporary input，并在 StoragePort 写入前验证 source policy、decoded MIME、byte size、dimensions 与 SHA-256 checksum。数据库持久化只能包含 StoragePort reference 与 metadata，MUST NOT 保存 image bytes/base64。

#### Scenario:持久化已验证的图片结果
- **当** 返回 payload 满足 URL/base64 policy 及所有 observed media checks
- **则** StoragePort 持久化 canonical object reference，application 追加一个关联 run 和 ProviderCall 的新 image AssetVersion

#### Scenario:拒绝 malformed 或不匹配媒体
- **当** URL policy 失败、base64 无效、MIME 与 observed bytes 不同、超限、dimensions 无效或 checksum 验证失败
- **则** 调用可审计为 failed，且 StoragePort 和 AssetVersion 都不持久化结果

### Requirement:retry、cost 与默认执行显式可见
系统 SHALL 以 `run_id + logical_operation` 作为 image work 的 idempotency key，记录 request outcome/usage/cost source，并保持 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）为默认测试组合。真实 GPT Image call MUST 要求 explicit enabled opt-in profile，不能仅因请求了 model name 而发生。

#### Scenario:重试已完成的图片 operation
- **当** 同一 run 和 logical operation 在 successful registration 后重试
- **则** 系统返回记录的 result AssetVersion，且不发起第二个 chargeable request

#### Scenario:没有真实 profile 时运行
- **当** 没有配置 opt-in real image profile
- **则** tests 使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），或返回 explicit unconfigured state，且没有 network request

### Requirement:Image success is an unreferenced candidate
成功结果 MUST first register an immutable unreferenced AssetVersion candidate with candidateId, id/revision/hash and project/episode/target provenance.

#### Scenario:Success has no implicit storyboard acceptance
- **WHEN** GPT Image result 被持久化
- **THEN** 它不是 current storyboard reference，且在精确 eligibility CAS acceptance 前不执行 video submission。

### Requirement:GPT Image runnable feature gate 与 result version 唯一 append
GPT Image 首次 connection-test/probe SHALL 仅在 `adapterInstalled=true`、catalog `approval=approved`、`featureGate=MVP-A`、explicit live opt-in、已选 profile、可解析 credential 与 timeout 齐备时执行，成功后冻结 capability snapshot，MUST NOT 要求既有 snapshot 或 `runnable=true`；explicit live invocation SHALL 额外要求该成功 snapshot 与 `runnable=true`。MVP-B candidate 可展示/保存但 MUST 零外部调用，默认测试 MUST 使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），运行开始后 Adapter/Profile MUST 冻结。verified Provider terminal success SHALL 是 result AssetVersion 唯一 append 时点，retry/reconcile MUST 返回同一 version/candidate。后续 AssetEdit accept 只可追加 AcceptDecision/audit 和同一 version 的 scenes exact eligibility CAS，MUST NOT 复制 bytes/object/ref 或 append 第二 AssetVersion；reject/stale/foreign accept 零 AssetVersion/current/Timeline mutation。

#### Scenario:未 runnable 的 image operation 不产生 result version
- **WHEN** GPT Image operation 未通过 feature gate，或既有 candidate 被接受/拒绝
- **THEN** 前者零网络/零 AssetVersion；后者仅复用 terminal success 的同一 version 并遵循 exact CAS
