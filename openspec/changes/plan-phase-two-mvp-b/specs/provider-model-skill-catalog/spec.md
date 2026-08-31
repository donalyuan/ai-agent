## MODIFIED Requirements

### Requirement:Per-operation concurrency、rate limit 与 quota snapshot
Catalog SHALL 为每个 Provider/Profile operation 持久化带 revision 的 `ProviderOperationPolicy`，覆盖阶段一和阶段二 TTS、ASR、音乐及新增视频模式，至少包含 maxConcurrency、rate window/limit、bounded admission、callback/polling/reconcile 和 429/`Retry-After` policy；并 SHALL 只追加 `ProviderQuotaSnapshot`，保存 `known|unknown|exhausted`、provider-native remaining/reset/source/capturedAt。每次 live invocation MUST 在 ProviderCall/external submit 前 admission；超限或 quota exhausted MUST 返回稳定 diagnostic，quota unknown MUST 保持 unknown 且不得 fallback。

#### Scenario:阶段二 operation 超限时不提交
- **WHEN** ASR 或 TTS operation 达到并发/速率/配额限制
- **THEN** 系统返回 policy revision 和 retry information，不创建 ProviderCall/external request

#### Scenario:callback 不具备证明时继续 reconcile
- **WHEN** 新 Provider 没有签名 callback 或幂等证明
- **THEN** 系统仅使用 poll/reconcile，unknown 状态不可重复提交
