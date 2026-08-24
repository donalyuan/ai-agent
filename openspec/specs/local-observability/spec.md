# local-observability Specification

## Purpose
TBD - created by archiving change implement-local-observability. Update Purpose after archive.
## Requirements
### Requirement:W3C Trace Context 端到端传播
系统 SHALL 使用 W3C `traceparent`/`tracestate` 传播 trace context。Web、FastAPI、Outbox、Temporal Workflow/Activity、Agent/Generation/Media Worker、Provider/Storage adapter 与 FFmpeg job MUST 形成同一 root 下可验证的 parent-child lineage；异步 handoff MUST 显式携带 context，不得依赖进程内 thread-local 或时间戳推断。对外展示的 `trace_id` MUST 为 canonical 32 位小写十六进制且不得由任意业务 payload 覆盖。

#### Scenario:一次生成和导出保持同一 trace lineage
- **WHEN** 用户从浏览器启动 Run，并经过 Outbox、Temporal、Provider、Media Worker 和 FFmpeg 完成导出
- **THEN** in-memory/OTLP evidence 显示连续 parent-child lineage，相关 owner facts 和安全错误 envelope 可用同一 trace/correlation 关联，且不存在第二套业务事件账本

#### Scenario:缺失或非法 trace header 安全处理
- **WHEN** 入口没有 `traceparent` 或提供格式非法、超长或冲突的 trace header/payload trace ID
- **THEN** 系统创建新的受信 root 并记录稳定 diagnostic，不信任非法 identity、不泄露 header 内容，且不影响合法业务请求语义

### Requirement:Telemetry 不得成为业务副作用前置
OpenTelemetry SDK、exporter、collector、viewer 或 metric sink 的初始化、队列、发送和查询失败 MUST 只产生有界 `telemetry_export_unavailable` 诊断。它们 MUST NOT 回滚 UoW、重试或重复 Provider/Storage/FFmpeg operation、改变 Run/NodeRun/ExportJob/AssetVersion/AcceptDecision 状态、切换 Adapter/Profile，或使默认业务 readiness 失败。

#### Scenario:Exporter 不可用时业务保持原语义
- **WHEN** telemetry endpoint 超时、拒绝连接、队列已满或返回错误
- **THEN** 业务 command 按原 owner contract 成功、失败或保持 unknown/reconcile，且没有重复付费、重复 Outbox/RunEvent/ProviderCall/AssetVersion；可观测性验收单独报告缺失证据

### Requirement:Secret-free 结构化日志与安全诊断
系统 SHALL 输出有界 JSON logs，至少包含 timestamp、severity、service、event、trace_id、span_id、operation、outcome 和稳定 error code，并仅按 allowlist 增加 correlation 与 owner ID/revision/hash。日志、span、metric exemplar 和 viewer link MUST NOT 包含 plaintext credential/secret/token、Prompt/SourceMaterial/剧本全文、媒体 bytes/base64、objectKey/workspace path、持久 URL、URL query、原始 Provider request/response 或未脱敏 FFmpeg stderr。Provider/FFmpeg 原始诊断脱敏后最多保留 30 天。

#### Scenario:敏感输入不会进入 telemetry
- **WHEN** 请求、Provider error、Storage URL 或 FFmpeg stderr 含 secret corpus、全文、路径、query 或媒体内容
- **THEN** logs/spans/metrics 只保留稳定 code、类型和安全摘要，secret scan 无命中且业务 owner 的长期审计事实仍按自身 retention 保存

### Requirement:阶段一低基数指标
系统 SHALL 导出可复算的 count、gauge 或 histogram，覆盖 HTTP request/error/duration、SSE active/replay、Workflow/Activity result/duration/retry/queue delay、Agent model/tool/structured repair/Skill route、Provider submit/result/queue/generation/cancel/retry/submission unknown、Storage multipart bytes/parts/resume/failure、FFmpeg queue/duration/speed/failure/output/parity、resource admission 以及 native usage/cost status。Metric labels MUST 只使用有界 route template、method、status class、operation type、owner status、catalog provider/model key、adapter key 和 error class；MUST NOT 使用 project/run/node/asset/trace/user ID、原始 URL、prompt 或其他无界值。

#### Scenario:指标可对账且不产生高基数标签
- **WHEN** Mock fixture 执行一次 Run、Provider operation、multipart resume、FFmpeg export 和一个失败分支
- **THEN** 指标 delta 与 RunEvent/ProviderCall/usage/Export owner facts 一致，label schema 无实体 ID/trace/raw URL，项目/节点成本只从 owner ledger 按 trace/correlation 聚合而非另建 metric 账本

### Requirement:本地诊断入口与可选 trace viewer
系统 SHALL 在 Run、NodeRun、ProviderCall、Upload 和 Export 的授权诊断 projection 中返回安全 trace ID。配置并通过 allowlist 校验 `traceViewerBaseUrl` 时，UI MAY 生成只含 canonical trace ID 的跳转；未配置或非法时 MUST 只提供复制 ID/本地日志检索提示，不得拼接任意 URL。默认 Compose 业务 profile MUST 不依赖 collector/viewer；可选 diagnostics profile 与 in-memory exporter SHALL 用于验收。

#### Scenario:从运行详情定位 trace
- **WHEN** 用户打开同项目 Run 或 Export 失败详情且受控 viewer 已配置
- **THEN** UI 显示 trace ID 和安全 viewer link；foreign project、非法 base URL 或未配置时不生成链接且不泄露其他项目 telemetry

### Requirement:可观测性 E2E 证据
系统 SHALL 将本 change 接入 `E2E-MVPA-001`，记录一个文本 Run、一次 image/video operation、一次 multipart upload/resume 和一次 Timeline export 的 root/child span lineage、关键结构化日志、指标 delta、owner 对账与无副作用断言。默认 fixture MUST 使用 `Mock Provider +` 显式 Local test/offline profile；真实 Provider/TOS/FFmpeg 仅使用各 owner 的 explicit probe telemetry。

#### Scenario:正反证据完整时通过
- **WHEN** 验收同时覆盖正常链路、非法 header、exporter unavailable、重试/reconcile、取消晚到结果、Provider/FFmpeg failure、脱敏和高基数拒绝
- **THEN** 报告可定位每个 trace/metric/log evidence，证明无重复业务副作用；缺少任何类别时不得报告阶段一可观测性完成
