## 1. Contract、依赖与安全基线

- [x] 1.1 在实施前核验当前 Web/API/三类 Worker、Outbox/Temporal、RunEvent、ProviderCall、Storage、FFmpeg 和错误 envelope 的实际代码/Schema/依赖锁；确认本 change 不拥有或复制任一业务事实。
- [x] 1.2 先定义 W3C Trace Context、canonical trace/span ID、safe log envelope、metric names/units/有界 labels、exporter failure 和 `traceViewerBaseUrl` allowlist 的共享 contracts/正反 fixtures。
- [x] 1.3 先写 architecture/security tests，禁止 domain 依赖 telemetry SDK、禁止实体 ID/raw URL/prompt 作为 metric labels，并用 secret/full-text/path/objectKey/raw Provider/FFmpeg corpus 验证 logs/spans/metrics 脱敏。
- [x] 1.4 以当前 lock 验证并固定 OpenTelemetry Web/Python/Temporal instrumentation 与 OTLP/HTTP 兼容版本；缺少兼容证据时保持任务阻塞，不使用系统全局包或未锁定依赖。

## 2. HTTP、异步与 Worker Trace

- [x] 2.1 先写缺失/合法/非法/冲突 trace header tests，再实现 Web API client 与 FastAPI middleware 的 root/child span、错误 envelope trace ID 和 request/response 安全 attributes。
- [x] 2.2 先写 parentage/restart/retry tests，再实现 Outbox metadata 与 Temporal client/Workflow/Activity interceptors 的显式 context inject/extract；证明重放、AlreadyStarted、Activity retry 和 SSE reconnect 不创建错误 root 或重复业务事件。
- [x] 2.3 为 Agent、Generation、Media Worker 建立共享 instrumentation composition，覆盖启动、queue delay、operation outcome 和 exporter unavailable；telemetry 失败不得改变 readiness、Adapter/Profile、Run/Export 或重试语义。
- [x] 2.4 为 Provider、Storage 和 FFmpeg ports 实现 decorator/wrapper spans；协议允许时传播标准 header，不允许时只关联本地 request/job ID，并拒绝原始 payload/path/stderr 泄漏。

## 3. 日志、指标与本地诊断

- [x] 3.1 实现 allowlist JSON logger 与 bounded exporter queue/drop diagnostic，字段至少含 timestamp/severity/service/event/trace/span/operation/outcome/error code；原始诊断遵守脱敏后 30 天上限。
- [x] 3.2 先写 metric delta/cardinality tests，再实现 HTTP/SSE、Workflow/Activity、Agent/Skill、Provider、Storage multipart、FFmpeg、resource admission 和 usage/cost 的 count/gauge/histogram instruments。
- [x] 3.3 实现从 owner ledger 按 trace/correlation 聚合项目/节点 usage/cost 的只读诊断 adapter；禁止在 metric labels 或 telemetry store 中建立第二份项目/节点成本账本。
- [x] 3.4 在 Run/NodeRun/ProviderCall/Upload/Export 安全 projection 中接入 trace ID，并实现受控 viewer link/copy ID；foreign scope、非法/未配置 base URL 不生成链接且零业务 mutation。
- [x] 3.5 增加 in-memory test exporter 与可选 Compose diagnostics profile/OTLP 配置；默认业务 profile 在 collector/viewer 缺失时仍健康，示例配置不包含密钥或设备绝对路径。

## 4. BDD、E2E 与严格验收

- [x] 4.1 添加 unit/contract/integration tests，覆盖 trace canonicalization、跨 HTTP/Outbox/Temporal/Worker/adapter parentage、日志脱敏、metric label schema/delta、exporter timeout/drop 和 viewer URL 安全。
- [x] 4.2 将 observability stage 接入 `E2E-MVPA-001`：记录文本 Run、image/video operation、multipart resume、Timeline export 的 span lineage/log/metric delta/owner 对账，以及非法 header、reconcile、取消晚到、Provider/FFmpeg failure 和无重复副作用证据。
- [x] 4.3 运行定向 Web/API/Worker/instrumentation tests、Compose diagnostics config/smoke、`openspec instructions apply --change "implement-local-observability" --json`、status、strict/all-change strict、`pnpm run check` 和 `git diff --check`；全部实现并验证前保持任务未勾选。

## DDD / BDD / SDD / TDD

- **DDD**：telemetry 只关联 owner 事实，业务恢复、审核、计费和 current state 仍读取 owner repositories。
- **BDD**：用户可从安全诊断取得 trace ID；telemetry backend 失败不改变业务结果。
- **SDD**：W3C headers、allowlisted logs、低基数 metrics、OTLP/Viewer 配置和 retention 边界可实现且可拒绝。
- **TDD**：传播/脱敏/基数/exporter failure tests 先于 middleware/interceptor/decorator，E2E 再对账 owner facts。
