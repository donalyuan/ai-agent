## Why

四份阶段一权威文档要求 MVP-A 能在显式凭据和运行时能力齐备时完成真实的文本、图片、视频、媒体检查、时间线和导出闭环；当前实现主要证明了 Mock Provider + Local workspace 的领域与存储底座，正式 live composition、Generation/Media Worker activity、FFmpeg/ffprobe 运行条件、migration/readiness 及可恢复 E2E 仍未闭合。因此阶段一退出证据不能把 `unconfigured`、仅导航 E2E 或静态接口占位当作真实链路成功。

本 change 聚合并收口已确认的运行时缺口，保持默认离线路径安全可测，同时为显式 credentialed sandbox 提供可审计、可重试、不可隐式 fallback 的 MVP-A 退出门。

## 需求追溯

| 权威文档 | MVP-A 义务 | 当前缺口证据 |
| --- | --- | --- |
| `docs/video-agent-product-requirements.docx` | 用户必须能从项目/剧集创建、文本审核、图片/视频候选审核，经过媒体检查和 Timeline，得到 MP4/SRT/light 导出及可恢复状态。 | 现有 Playwright 证据主要是 fixture 导航；未覆盖创建至导出的 credentialed 闭环、API/Worker 中途重启和 live operation evidence。 |
| `docs/video-agent-technical-architecture.md` | Ports/Adapters、Temporal activity、API/Worker 边界、owner handoff、幂等和 fail-closed 必须在运行时组合中兑现。 | `services/api/src/video_agent_api/runtime.py` 只装配 `mock`；`workers/generation/main.py` 只提供 queue health；`workers/media/main.py` 当前只注册 export activity。 |
| `docs/video-agent-technical-implementation.docx` | Provider/Profile/Model catalog、真实 Text/Image/Video/TOS transport、FFmpeg/ffprobe、Compose secret、migration/head readiness 和重启恢复必须可执行。 | `GPTImageProvider`、`AgnesVideoProvider`、`TOSAdapter` 只有可注入 transport；Compose/镜像缺少本 change 要求的完整 renderer/secret/migration/readiness composition。 |
| `docs/video-agent-integration-notes.md` | 以显式 credentialed sandbox 验证外部连接、资源前置、submission unknown reconciliation、multipart/export retry 和证据脱敏。 | 当前默认验证明确保持 `unconfigured`/`renderer_unconfigured`；它证明安全拒绝路径，但不能证明提供凭据后 live composition 和跨 Worker recovery 可运行。 |

外部账号、许可、凭据或 binary 缺失本身不是代码缺陷；本 change 的缺陷是即使这些前置已提供，当前仍没有完整可运行的 composition、activity wiring 和退出证据门。

## What Changes

- 建立由 DB catalog 的 Provider/Profile/Model/CapabilitySnapshot 驱动的正式 runtime composition，接通 Text、GPT Image、Agnes Video 和 TOS adapter 的 live transport；live invocation 必须继承既有 `adapterInstalled + approval=approved + successfully probed snapshot + runnable=true + featureGate=MVP-A + explicit opt-in` 与 ProviderOperationPolicy concurrency/rate/quota admission，未配置、禁用、未批准、MVP-B、超限或能力不足时 fail closed，绝不切换 Mock/Local 或伪造成功。
- 将 Text/Image/Video activity 接入 Generation Worker，并以冻结的 `executionRoute/workflowType/taskQueue/schemaVersion` 将新 operation 从既有 Agent/direct-HTTP 执行路径前向切换；旧路径只 drain/reconcile 已冻结存量，API 对新意图只提交和查询持久状态，不等待模型或媒体完成。
- 将 MediaInspection、Derivative、Render、Storage activity 接入 Media Worker；Media dispatch 必须先验证 image/video candidate 已审核接受、Scenes owner exact-CAS current 及匹配的 AssetVersion/provenance，pending/rejected/stale/foreign 输入零派发。
- 补齐 API/Media 镜像的 `ffmpeg`/`ffprobe` 配置与 capability probe、Docker Secret/TOS profile wiring、共享 workspace、显式 Alembic migration 和基于 migration head 的 readiness；保留 localhost-only 与无凭据默认启动。
- 固化 `run_id + logical_operation`、owner-specific ledger、pre-submit durable attempt、按 Provider 能力外发的 idempotency/correlation、remote request lookup/no-lookup 终局、Text/GPT Image `ProviderCall.unknown`、Agnes `VideoOperation.submission_unknown`、Storage session reconciliation 与 Export `packaging` subphase 的幂等及重启恢复合同；各 owner 保持既有状态值域，不支持 remote lookup 时保持对应 owner 的 unknown 终局并要求人工处置，禁止自动重提。
- 固化 StorageProfile/BucketBinding snapshot -> UploadSession -> immutable StoredObjectRef -> Assets owner exactly-once AssetVersion/candidate，以及 Export owner -> StoredObjectRef -> ExportArtifact 的 typed handoff；Storage/Worker 不得直接写 AssetVersion 或 ExportArtifact。
- 复用既有 operations-resilience 门：上传、付费生成、媒体派生/preview 和导出 command 必须在任何 intent、ProviderCall、UploadSession、ExportJob、AssetVersion 或 Outbox 前读取 RuntimeResourceSnapshot/CapacitySnapshot；probe 不可用、能力不足或 hard limit 时零副作用拒绝。
- 增加 credentialed sandbox 阶段验收：创建项目/剧集 -> 文本 -> TextReview -> 图片候选 -> 图片接受/current -> Agnes 视频候选 -> 视频接受/current -> MediaInspect -> Timeline -> MP4/SRT/light；中途重启 API 与至少一个 Worker，并保存 catalog/policy/resource admission、每个 owner、前置条件、结果、失败和无副作用证据。
- `unconfigured` 只表示明确的外部前置缺失，不能作为 MVP-A 退出成功；没有凭据或许可时报告未配置并跳过 live 请求，但退出报告必须标记为未满足。
- **BREAKING（运行时 composition 仅对显式 live profile 生效）**：禁止从 live 失败隐式 fallback 到 Mock/Local；默认 Mock/Local 测试合同保持不变。
- 明确非目标：MVP-B workflow 图编辑/批量操作、TTS/ASR、移动端、多租户、portable/full package 与回导、发布平台、callback/webhook 和真实外部账号管理。

## Capabilities

### New Capabilities

- `runtime-composition`: 定义 catalog 驱动的 Provider/Profile/Model/CapabilitySnapshot 解析、live transport composition、凭据/配置边界、fail-closed 和统一幂等操作合同。
- `worker-media-runtime`: 定义 Generation/Media Worker activity、Temporal dispatch、FFmpeg/ffprobe probe、TOS/Local storage execution、重启恢复、上传验证和 owner handoff 边界。
- `phase-one-runtime-acceptance`: 定义 MVP-A credentialed sandbox 端到端退出矩阵、migration/readiness/resource gates、重启恢复证据和 `unconfigured` 语义。

### Modified Capabilities

无。既有领域规格的 owner、状态和安全合同保持不变；本 change 只补齐其跨服务运行时组合与阶段退出所需的实现和验收约束。

## Impact

- 运行时代码：`services/api/src/video_agent_api/runtime.py`、Provider/Storage ports 与 adapters、Generation/Media Worker dispatch/activity、Temporal workflow wiring。
- 配置与部署：`services/api/Dockerfile`、Worker 镜像/启动配置、`infra/compose/compose.yaml`、`.env.example`、Docker Secret 与 migration/readiness 脚本。
- 持久化与契约：catalog capability resolution、冻结 execution route、provider remote correlation/reconciliation、既有 owner ledger、Storage/Assets/Export typed handoff 所需的 additive Alembic/Schema 校验；不保存媒体二进制或凭据明文，不新增跨 owner 事实源。
- 测试与证据：domain/application/adapter/HTTP/worker/migration/BDD/TDD 回归、credentialed sandbox E2E、API/Worker restart recovery、resource probe 和阶段一退出报告。
- **DDD**：Provider/Storage catalog、operations-resilience admission、Run/operation ledger、Generation/Media Worker、Timeline/Export owner 的边界不合并；各 owner 状态机不统一改写，每个外部副作用由对应 activity 和 typed handoff 负责。
- **BDD**：以真实可观察链路和失败无副作用为验收；catalog/resource/policy gate 或凭据前置失败必须在规定边界零外部调用，凭据缺失只产生稳定 `unconfigured`，不构成退出成功。
- **SDD**：保留现有 camelCase HTTP/Schema、scope、CAS、catalog runnable/ProviderOperationPolicy、RuntimeResourceSnapshot/CapacitySnapshot、Storage/Export 状态值域、30fps canonical RenderPlan、StoragePort 与 no-fallback 合同；execution route、live profile、外部 correlation、typed handoff、operation key 和证据格式必须显式版本化。
- **TDD**：先测试 catalog live/policy admission、resource pre-intent zero-side-effect、route cutover/legacy drain、remote accepted-response-loss、owner-specific unknown mapping、owner handoff、审核后 Media admission 和 readiness fail-closed，再执行默认 Mock/Local 回归，最后在隔离 credentialed sandbox 执行 live E2E；真实 Provider/TOS 不进入默认 CI。
