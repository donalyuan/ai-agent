# Change: 集成 GPT Image Provider

## Provider result handoff

GPT Image 首次 connection-test/probe 仅在 `adapterInstalled=true`、catalog `approval=approved`、`featureGate=MVP-A`、explicit live opt-in、已选 profile、可解析 credential 与 timeout 齐备时可执行，成功后冻结 capability snapshot，不要求既有 snapshot 或 `runnable=true`；explicit live invocation 还需该成功 snapshot 与 `runnable=true`。MVP-B candidate 可展示/保存但零外部调用，默认测试组合为 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），并保持 explicit live opt-in；运行开始后 Adapter/Profile 冻结。verified Provider terminal success 是 result `AssetVersion` 的唯一 append 时点，retry/reconcile 返回同一 version/candidate。后续 AssetEdit `accept` 只追加 AcceptDecision/audit 和同一 version scenes exact eligibility CAS，不复制 bytes/object/ref 或追加第二 AssetVersion。

## 原因

现有 `ImageGenerationPort` 有安全的 Mock 边界，但没有 production-shaped 的图片生成/编辑流程，无法校验 capability parameters、验证返回媒体并登记新的不可变 AssetVersion。

## 变更内容

- 在 `ImageGenerationPort` 后定义 GPT Image `generate` 与 `edit` 的 application/adapter 行为。
- 在调用前校验选定 catalog capability/parameters、图片引用所有权与最多 8 个 reference、合计 32 MiB、最大边长 8192；输入图片只允许 PNG/JPEG/WebP，edit mask 只允许 PNG。
- 每次生成或编辑都必须冻结 AssetBible owner 返回的 accepted `ResolvedContinuitySnapshot` ID/revision/hash 及其 GenerationSpec/AssetVersion owner references；snapshot incomplete、stale、foreign、hash mismatch 或目标存在 pending `ContinuityRevisionTask` 时，必须在创建 ProviderCall 或外部请求前失败。
- 对 URL 输入执行严格 SSRF 边界：必须命中配置 allowlist，禁止任何重定向和 loopback、私网、链路本地、保留、metadata service 地址。
- 在 StoragePort 持久化和 AssetVersion 登记前校验 URL/base64 输出的 MIME、size 与 checksum。
- 记录 request/cost/retry outcome，同时保持 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）为默认测试组合，真实调用仅显式 opt-in。

## 能力

### 新增能力

- `gpt-image-generation`：受控的 GPT Image generate/edit、media validation、storage 与 AssetVersion registration。

### 修改能力

无。

## 总体计划追溯与边界

本 change 反向追溯到 `plan-phase-one-drama-mvp-a` 的总体任务 `3.2`，并受共享任务 `5.1`、`5.2`、`5.4`、`5.5` 约束。总体计划只协调交付顺序，不是运行时代码依赖。直接依赖已归档的 AssetVersion、`implement-workflows-runs-slice` 的 Run/RunEvent 边界、`implement-provider-model-skill-catalog` 的 catalog/冻结 snapshot/ProviderCall 账本，以及 `implement-asset-bible-continuity-slice` 的只读 resolved snapshot/continuity task projection。

ProviderCall 仍仅由 catalog 领域持久化为一次调用/费用/幂等账本；本 change 以 `run_id`、`node_run_id`、`correlation_id` 关联它。RunEvent 仅由 workflows/runs 领域追加；图片 adapter/application 不创建平行事件历史。

完整非目标包括文本或视频生成、硬编码 model name/endpoint、隐式 live network call、数据库直接保存 image bytes/base64、跨项目 reference-image reuse、图片后期制作、billing settlement，以及拥有或实现 catalog、WorkflowRun/NodeRun/RunEvent 状态机或事件历史。本 change 只复用 AssetVersion 与 catalog 已拥有的 canonical `schema_version`；HTTP `schemaVersion` 仅由所有者映射同一值，图片 adapter/application 不创建独立的 Provider 专用版本事实。

## 影响

预期实现使用现有 catalog、`ImageGenerationPort`、`StoragePort`、Asset/AssetVersion UoW rules、contracts 与 worker/application tests。文本和视频生成不在本 change 范围。

## 未引用候选合同

**DDD**：成功 image 只由 image owner 登记为未引用 AssetVersion candidate。**BDD**：它在 scenes 精确接受前不可成为 storyboard current。**SDD**：candidate 返回 immutable id/revision/hash 和 project/episode/target provenance，不直接更新 projection。**TDD**：验证成功登记与 accept 前零 reference/零 Agnes 副作用。
