## Context

阶段一已把业务事实分散给明确 owner：运行事件归 workflows/runs，调用与 native usage 归 catalog，资源快照归 operations resilience，媒体作业和导出归 Timeline/Media Worker。当前合同普遍携带 `correlation_id`，但没有统一规定 trace 如何跨 HTTP、Outbox、Temporal、Worker 和外部 adapter 传播，也没有一组可复算的本地指标。可观测性必须连接这些事实，但不能复制或取代它们。

本 change 对应总体 `plan-phase-one-drama-mvp-a` 的独立 child。默认验收仍使用 `Mock Provider +` 显式 Local test/offline profile；真实 Provider、TOS、AgentScope 和 FFmpeg telemetry 只来自各 owner 的 explicit probe。可观测性故障不得改变业务事务、重试、费用、审核或 current pointer。

## Goals / Non-Goals

**Goals:**

- 以 W3C `traceparent`/`tracestate` 作为跨进程传播合同，并在安全 owner 记录和错误 envelope 中提供 canonical 32 位小写十六进制 `trace_id`。
- 为 Web、API、Outbox、Temporal、三类 Worker、Provider/Storage 和 FFmpeg 建立同一 trace 的 spans、secret-free JSON logs 和低基数 metrics。
- 让 Run/NodeRun/ProviderCall/AssetEdit/Upload/Export 等详情可按 trace ID 关联本地诊断；配置受控 trace viewer 时提供安全跳转，否则提供可复制 ID。
- 以 in-memory test exporter 和可选 Compose diagnostics profile 生成可复算证据，且 telemetry backend 不成为业务 readiness 或恢复前置。

**Non-Goals:**

- 不建设生产级监控集群、长期 telemetry 仓库、告警/on-call、SLA、业务分析看板或计费结算。
- 不让 spans/logs/metrics 成为 RunEvent、ProviderCall、usage/cost、ExportJob、AssetVersion、审核决定或资源准入的事实源。
- 不把 projectId、runId、assetId、traceId、用户输入或其他无界值作为 metric label，也不记录 secret、Prompt/SourceMaterial/剧本全文、媒体 bytes、objectKey、持久 URL 或原始 Provider request/response。

## Decisions

### 1. Trace context 是关联标识，不是业务状态

入口 HTTP 接受有效 W3C Trace Context；缺失时由 API 生成新 root，非法 header 返回新的安全 trace 并记录 validation diagnostic，不信任调用方提供的任意 `trace_id` 字段。Web API client 传播当前 context；API 将 context 显式写入 Outbox message metadata 和 Temporal headers，Workflow/Activity/Worker 提取后创建 child span。Provider、Storage 和 FFmpeg adapter 只在协议允许时发送标准 trace header，并始终把 provider request/job ID 作为 span attribute 或 owner reference，而不是覆盖 trace identity。

业务实体只在其现有 owner contract 允许处保存 `trace_id`/correlation reference；不创建跨 owner telemetry 业务表。替代方案是从日志时间戳推断关联，但重试、异步排队和并行镜头会产生歧义，因此拒绝。

### 2. Instrumentation 通过 ports/wrappers 进入，失败永不改变业务结果

共享 instrumentation 封装 HTTP middleware/client、Outbox publisher/consumer、Temporal interceptor、Provider/Storage port decorator 和 FFmpeg job wrapper。domain 层不依赖 OpenTelemetry SDK。SDK/exporter 初始化、发送、超时或后端不可用只产生有界本地 diagnostic，不得回滚 UoW、重试外部付费 operation、改变 Run/Export 状态或令 readiness 失败。

测试使用确定性 in-memory span/metric exporter。Compose 可选 diagnostics profile 只接收 OTLP/HTTP，并可配置受控 `traceViewerBaseUrl`；未配置时 UI 仍显示/复制 trace ID，不生成外部链接。替代的“强制完整监控栈作为默认启动依赖”会破坏本地 MVP-A 可用性，故不采用。

### 3. 日志遵守 allowlist 与 owner 事实边界

每条 JSON log 至少含 timestamp、severity、service、event name、trace_id、span_id、correlation_id（如有）、operation、outcome 和稳定 error code；owner ID/revision/hash 仅在诊断确有需要时进入 allowlist。异常使用类型、稳定 code 和脱敏 message，不保存请求 body、凭据、URL query、媒体路径或 raw stderr；FFmpeg/Provider 原始诊断先按各 owner 规则脱敏，再最多保留 30 天。

日志、span event 和 metric exemplars 只引用既有 owner IDs，不复制 RunEvent sequence 或 ProviderCall payload。稳定本地用户 UUID 仅进入被授权的业务审计，默认不进入 telemetry。

### 4. 指标低基数，项目/节点成本由 owner ledger 查询

最小指标集合覆盖：HTTP count/error/duration 与 SSE active/replay；Workflow/Activity result/duration/retry/queue delay；Agent model/tool/structured-repair/route confidence；Provider submit/result/queue/generation/cancel/retry/unknown；Storage multipart bytes/parts/resume/failure；FFmpeg queue/duration/speed/failure/output bytes/parity；resource/capacity admission；usage/cost known/estimated/actual/unknown。

标签只允许 service、route template、method、status class、operation type、owner status、provider/model stable catalog key、adapter key 和 error class 等有界值。project/run/node/asset/trace ID 不作为 labels；“每项目/节点成本”由 catalog/workflows 的 owner ledger 通过 trace/correlation 查询并在验收报告聚合，避免基数失控和双账本。

### 5. 验收同时证明传播、脱敏、准确性和非阻断

`E2E-MVPA-001` 为一个文本 Run、一次图片/视频 operation、一次上传和一次导出记录 root/child span lineage、关键 logs 和 metrics delta，并与 RunEvent/ProviderCall/usage/Export owner facts 对账。负例覆盖 exporter unavailable、非法 trace header、重试/重连、取消晚到结果、`submission_unknown`、Provider/FFmpeg failure 和高基数字段拒绝；任一场景不得产生重复付费、重复业务事件或状态漂移。

## Risks / Trade-offs

- [异步 context 丢失] -> Outbox payload metadata、Temporal headers 和 consumer contract tests 同时校验 parent/child lineage。
- [telemetry 泄密] -> 中央 attribute allowlist、secret corpus tests、URL/path/raw payload 拒绝和 30 天原始诊断上限。
- [metric 基数或开销失控] -> route template/有界 enum labels、禁止实体 ID labels、批量导出、bounded queue 和 exporter timeout/drop counters。
- [观察结果被误当成业务事实] -> architecture tests 禁止 domain 依赖 telemetry package，恢复与结算只读取 owner repositories。
- [本地 exporter 不可用] -> fail-open for telemetry、业务保持运行；验收单独报告 `telemetry_export_unavailable`，不得伪造 span/metric evidence。

## Migration Plan

1. 先添加共享 trace/log/metric contracts、in-memory exporter 和 secret/cardinality fixtures，不改变现有业务 Schema。
2. 依次接入 HTTP、Outbox/Temporal、Worker 和 adapter wrappers；只在现有 owner 表确需关联的位置增加可空 trace reference，并以 additive migration 实施。
3. 增加可选 Compose diagnostics profile、受控 viewer link 和 `E2E-MVPA-001` evidence collector；默认业务 profile 不依赖 telemetry backend。
4. 回滚移除 instrumentation/exporter 配置和可空关联列；业务 owner facts、历史 trace ID 字符串和审计记录保持可读。

## Open Questions

- 实施前以当前依赖锁确认 OpenTelemetry Python/浏览器/Temporal instrumentation 的兼容版本，以及可选本地 collector image digest；未验证前不得写成已安装事实。
- 实施前确认各真实 Provider/TOS 是否允许转发 W3C headers；不允许时仅在本地 span 中记录其 request/job ID，不绕过供应商协议。

## DDD / BDD / SDD / TDD

- **DDD**：telemetry 只关联 owner 事实，不拥有业务状态或恢复/结算决策。
- **BDD**：用户从 Run/Export 诊断取得 trace ID，可关联同一次跨服务操作；exporter 失败不影响业务结果。
- **SDD**：冻结 W3C headers、allowlisted log envelope、低基数 metric 集合、可选 OTLP/trace viewer 和 30 天原始诊断边界。
- **TDD**：先写传播、parentage、脱敏、基数、metric delta 和 exporter failure tests，再接入 middleware/interceptor/decorator 与 E2E。
