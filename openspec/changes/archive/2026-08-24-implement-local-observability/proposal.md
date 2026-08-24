## Why

阶段一技术方案要求从浏览器、FastAPI、Temporal、Agent/Generation/Media Worker 到 Provider、Storage 和 FFmpeg 的一次操作可以用同一 trace 定位，并能计算 API、运行、外部调用、媒体和成本指标。现有 change 虽然各自保存 RunEvent、ProviderCall、usage 和错误，但没有独立 owner 负责跨边界传播与本地可观测性验收，实施后仍可能出现“业务记录存在但无法串联”的故障定位缺口。

## What Changes

- 新增本地可观测性边界：接受或生成 W3C Trace Context，将 `trace_id`/`span_id`/correlation 贯穿 Web、API、Outbox、Temporal、Worker、Provider/Storage 和 FFmpeg adapter，并在异步边界显式传递。
- 统一 secret-free 结构化日志与稳定错误 envelope；日志只引用 owner IDs/revisions/hashes 和脱敏摘要，不复制业务事件、Prompt/SourceMaterial 全文、媒体 bytes、credential 或原始 Provider payload。
- 为阶段一导出最小指标：API 请求量/错误率/延迟、SSE 连接/补发、Workflow/Activity 结果与时长、Provider 提交/排队/生成/取消/重试、FFmpeg 队列/处理/失败/输出、Skill 路由和结构化修复，以及项目/节点 usage/cost/容量。
- 提供本地 Compose 可查询的 trace/log/metric evidence 与保留/基数约束，并把它们接入 `E2E-MVPA-001`；不建设生产监控平台、告警/on-call、分布式存储集群或业务分析看板。

## Capabilities

### New Capabilities
- `local-observability`: 阶段一跨进程 trace 传播、结构化日志、低基数指标和本地验收证据。

### Modified Capabilities

- 无。

## Impact

- 后续实现会影响 Web API client、FastAPI middleware、Outbox/Temporal headers、三类 Worker、Provider/Storage/FFmpeg adapter instrumentation，以及 Compose 的本地 telemetry 配置。
- 本 change 只观察和关联既有 owner 事实；`RunEvent`、`ProviderCall`、usage/cost、ExportJob 和资源快照仍由原领域 owner 持有，telemetry 不成为恢复、计费、审核或业务状态事实源。
- 默认测试继续使用 `Mock Provider +` 显式 Local test/offline profile；真实 Provider、TOS 和 FFmpeg 只在各 owner 的 explicit probe 中产生 telemetry。
