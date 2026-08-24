# ADR-0006：闭合阶段一运行诊断与媒体交接

- 状态：已接受
- 日期：2026-08-23

## 背景

再次将阶段一 PRD、技术实施文档与 18 个既有 active change 反向追溯后，发现八类能力虽属于 MVP-A 或其明确边界，但没有同时落到 owner、规格场景、实施任务和阶段退出证据：指定历史版本重跑、Episode UI 状态隔离、SoundCue 完整字段、导出失败定位、导出上传登记、实际 2 GiB 素材链路、本地可观测性以及 LAN 访问的阶段归属。

## 决策

- 阶段一增加独立 `implement-local-observability` child，形成 1 个总体协调 change 与 18 个 child。该 child 以 W3C Trace Context、secret-free JSON logs、低基数 metrics 和可选本地 diagnostics profile 关联 owner 事实；telemetry 故障不影响业务 readiness、状态、重试或费用。
- `CreateRunFromHistoricalSnapshot` 只从用户明确选择的 immutable `RunInputSnapshot` 创建新 Run、新 `rerunOfRunId` 与新 logical operations；不得重启历史 Run、默认采用 current、隐式升级/rebase 或复用 failed-successor evidence。
- Workbench/Review 的 viewport、折叠、筛选、选择与 active Agent session 以 `projectId + episodeId` 隔离；恢复只保存并重新校验 owner references，不保存正文、不跨集 fallback、不重发消息或付费操作。
- `SoundCue.track` 是 PRD `cueType` 的唯一 canonical 映射，并冻结 start/duration、受限 trigger、priority、continuity refs、static gain 与 linear fades；automation/keyframes 延后 MVP-B。
- ExportJob 保持八态；`packaging` 内增加 `uploading|verifying|registering` subphase。MP4、SRT、light 分别经 Storage upload/stat/checksum/MIME/size verify 后登记，失败以受限 `ExportDiagnosticTarget` 定位，unknown 先 reconcile。
- 阶段退出必须用精确 `2_147_483_648` bytes 的实际媒体 fixture 验证 capability preflight、multipart interruption/resume、单一 AssetVersion 与 Media Worker inspection/proxy。2 GiB 不是平台最大值，logical-size fake 不能替代退出证据。
- MVP-A 默认与验收只监听 localhost/`127.0.0.1`；LAN exposure、simple password 和 reverse-proxy auth 延后 MVP-B，不得通过无认证广域监听实现。
- `E2E-MVPA-001` 新增 `S03a`、`S08b`、`S11a`，并扩展 `S01`、`S09`、`S10`，每项保留 focused failure 和 no-side-effect evidence。

## 后果

- 阶段一当前共有 19 个 active change 和 463 项全部未勾选的实施任务；数量以后以 OpenSpec 与 task 实际扫描为准。
- `implement-local-observability` 不成为业务状态、审核、恢复、计费或导出事实源；各领域 owner 仍拥有原事实。
- 本 ADR 只冻结 OpenSpec 设计、阶段边界和验收要求，不代表运行时、Schema、migration、依赖、Worker、测试或前端代码已实现。
- ADR-0005 记录的 18-change/429-task 数量是补入本 ADR 前的历史状态，当前数量与新增合同以本 ADR 和总体 OpenSpec 为准。
