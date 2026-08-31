# runtime-composition Specification

## Purpose

定义阶段一 live Provider/Storage 运行时装配、catalog 解析、凭据边界和无隐式 fallback 合同。

## ADDED Requirements

### Requirement:Catalog 驱动的完整 runtime composition

Runtime composition SHALL 从持久 Provider/Profile/Model/CapabilitySnapshot 解析 Text、GPT Image、Agnes Video 和 TOS 的 port 与冻结 identity；业务代码 MUST NOT 硬编码 model、base URL、bucket 或 region。

#### Scenario:选择完整准入的 live profile
- **WHEN** catalog selection 同 scope、enabled、credential 有效、`adapterInstalled=true`、`approval=approved`、successfully probed snapshot、`runnable=true`、`featureGate=MVP-A` 且用户已 explicit live opt-in
- **THEN** API 与对应 Worker 得到同一 provider/profile/model/capability identity 的 live port，并冻结 selection 与 admission references

#### Scenario:拒绝缺失或不匹配的 catalog
- **WHEN** selection 为 uninstalled、not-approved、MVP-B、disabled-for-run、snapshot-missing、`runnable=false`，或 profile、model、scope、credential 任一缺失/过期/不匹配
- **THEN** composition 返回稳定诊断并不创建外部请求、不切换 Mock/Local、不写成功 owner fact

### Requirement:首次 probe 与 live invocation 分离

首次显式 connection-test/probe SHALL 继续使用既有 probe gate，只要求 installed、approved、`featureGate=MVP-A`、explicit live opt-in、已选 profile、可解析 credential 和 timeout；它 MUST NOT 要求既有 snapshot 或 `runnable=true`。正式 live invocation MUST 在 ProviderCall/durable attempt 与 external submit 前额外通过带 revision 的 ProviderOperationPolicy concurrency/rate/quota admission。

#### Scenario:首次 probe 冻结能力但不触发业务调用
- **WHEN** 合格 candidate 尚无 capability snapshot 或 `runnable=false`，用户显式发起 connection-test/probe
- **THEN** 系统只执行 bounded probe 并在成功时冻结 snapshot，不创建 Run intent、ProviderCall、candidate 或业务外部调用

#### Scenario:policy 或 quota 阻断 live invocation
- **WHEN** operation 达到 maxConcurrency/rate limit、最新可信 quota 为 exhausted，或 policy/admission revision 不可验证
- **THEN** 系统在 ProviderCall/durable attempt/external submit 前返回稳定 blocked/retryable diagnostic，零外部调用且不 fallback；429/`Retry-After` 与 quota unknown 保留原生 observation

### Requirement:显式模式与 no-fallback

系统 SHALL 仅在显式 `provider_mode=mock` 或 `storage_mode=local_workspace` 时选择默认离线路径；显式 live adapter 的失败 MUST 保留 adapter 原始脱敏错误，禁止 fallback 或伪造成功。

#### Scenario:默认无凭据启动
- **WHEN** 使用 `.env.example` 启动并未选择 live profile
- **THEN** Mock Provider + `local_workspace` 可用，TOS/真实 Provider 显示 `unconfigured`，且网络请求数为零

#### Scenario:live 请求失败不降级
- **WHEN** 已选择 enabled live profile 但认证、网络、timeout 或 provider capability 失败
- **THEN** operation 进入可诊断失败/unknown 状态，不产生 Mock/Local 替代结果或成功 artifact

### Requirement:凭据解析与秘密隔离

CredentialResolver SHALL 从 Docker Secret/keyring 的 versioned reference 解密 profile-bound credential envelope；HTTP、日志、ledger 和 evidence MUST 只保存 status/masked hint/diagnostic type，不保存 secret、认证头或完整私密响应。

#### Scenario:凭据可用且不泄露
- **WHEN** live profile 引用有效 secret 并完成 probe
- **THEN** adapter 获得内存中的凭据，catalog 记录 capability/status，所有可查询输出都不含明文凭据

#### Scenario:主密钥缺失
- **WHEN** profile 引用的 keyring secret 不存在或无法解密
- **THEN** 组合失败为 `credential_master_key_unavailable` 或稳定 `unconfigured`，且零外部请求、零业务成功写入

### Requirement:跨 owner 共享的 versioned operation identity

Provider/Storage operation SHALL 使用 project scope 与稳定 `run_id + logical_operation`（无 Run 的资源操作使用 owner operation key），并冻结适用 fingerprint；该 identity contract 可跨 owner 传递，但业务状态 MUST 仍由 ProviderCall/VideoOperation、UploadSession/StoredObject 或 ExportJob/Artifact 的既有 owner 状态机保存，MUST NOT 引入统一跨 owner 状态枚举；相同 identity 的重试 MUST 幂等。

#### Scenario:相同逻辑操作重试
- **WHEN** API、Worker 或调用方以相同 operation identity 重试已提交或已完成操作
- **THEN** 系统返回对应 owner 的既有 ledger/result 或继续 owner reconcile，不新增付费 submit、不覆盖历史 request

#### Scenario:retake 创建新意图
- **WHEN** 用户对失败/拒绝的媒体候选发起 retake
- **THEN** 系统要求新的 logical operation 和新的 request fingerprint，旧 operation 仍可审计且不被重用

### Requirement:能力 probe 冻结

Provider、TOS 和 renderer 的 live capability probe SHALL 产生带 adapter version、model/profile、时间和 capability hash 的 snapshot；执行中的 operation MUST 使用该 snapshot，不能读取当前 mutable 默认值。

#### Scenario:运行中 catalog 变更
- **WHEN** operation 已提交后 profile/model/default capability 被修改或禁用
- **THEN** 原 operation 仍按冻结 snapshot reconcile/finalize，新 operation 才使用新 snapshot

#### Scenario:未探测能力阻断副作用
- **WHEN** profile 没有满足当前 operation 的有效 capability snapshot
- **THEN** composition 在 submit/render/upload 前返回 capability error，并保持对应 owner ledger 无成功副作用

### Requirement:Provider 外部 correlation 与未知终局

Text、GPT Image 和 Agnes adapter SHALL 在外部副作用前持久化 durable attempt，并从冻结 operation identity 派生 outbound correlation；capability snapshot MUST 分别声明是否支持 client idempotency key、remote request lookup 及其协议。系统 MUST NOT 假定所有 Provider 使用相同 header；远端可能已接受但无法 lookup 时，Text/GPT Image ProviderCall MUST 保持既有 `unknown`，Agnes VideoOperation MUST 保持既有 `submission_unknown`；两者都 MUST 关闭自动 retry/re-submit并要求人工处置。本 change MUST NOT 新增、重命名或迁移 ProviderCall/VideoOperation 状态值。

#### Scenario:远端接受后响应丢失
- **WHEN** durable attempt 已写入且 Provider 接受请求，但响应或 Worker 在 remote request id 持久化前丢失
- **THEN** 支持 lookup 的 adapter 以冻结 correlation 查询并幂等关联结果；不支持 lookup 时 Text/GPT Image ProviderCall 保持 `unknown`、Agnes VideoOperation 保持 `submission_unknown`，且外部 submit/计费次数至多一次

#### Scenario:Provider 不支持外部幂等
- **WHEN** capability probe 明确报告无 client idempotency key 或无 remote lookup
- **THEN** composition 冻结该限制，ambiguous failure 不进入 Temporal 自动 retry，不伪造 request id、通用 header 或成功结果

### Requirement:TOS profile snapshot 与 typed owner handoff

TOS composition SHALL 冻结 `storageProfileId/revision/snapshotHash`、private BucketBinding id/revision/project scope、canonical operationKey 和 expected object facts。Storage SHALL 只交付 immutable StoredObjectRef；image/video 的 AssetVersion 只能由 Assets owner 以 reservation exactly-once append，MP4/SRT/light 的 ExportArtifact 只能由 Export owner append。

#### Scenario:image/video 对象完成登记
- **WHEN** Storage 完成并验证同 scope UploadSession
- **THEN** 它只返回绑定 profile/bucket/operation 的 StoredObjectRef，Assets owner 再以 reservation append 一次 AssetVersion 并交付 candidate，Storage/Worker 不直接写 AssetVersion

#### Scenario:profile 或 owner handoff 冲突
- **WHEN** profile/bucket snapshot stale/foreign、reservation mismatch 或 owner registration response-loss
- **THEN** 对应 owner 以同一 operation key reconcile 或返回冲突，不切换 Local、不创建第二个 StoredObject/AssetVersion/ExportArtifact
