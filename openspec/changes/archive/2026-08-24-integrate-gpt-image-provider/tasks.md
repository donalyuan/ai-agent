## 0. 总体计划追溯与边界

本 change 对应 `plan-phase-one-drama-mvp-a` 任务 `3.2`，并遵从 `5.1`、`5.2`、`5.4`、`5.5`。直接依赖已归档 AssetVersion、`implement-workflows-runs-slice` 与 `implement-provider-model-skill-catalog`；总体 plan 仅协调交付，不是运行时代码依赖。ProviderCall 的调用/费用/幂等账本仅归 catalog；RunEvent 仅归 workflows/runs；以 `run_id`、`node_run_id`、`correlation_id` 关联。完整非目标是文本/视频生成、硬编码 model/endpoint、隐式 live network call、数据库保存 image bytes/base64、cross-project reference、图片后期制作、billing settlement，以及拥有或实现 catalog、WorkflowRun/NodeRun/RunEvent 状态机或事件历史。本 change 只复用 AssetVersion/catalog 的 canonical `schema_version` 与所有者同值 HTTP `schemaVersion`，不创建独立版本事实。

## 1. Contract 与 Domain Tests

- [x] 1.1 定义 generate/edit commands、results、operation errors、catalog snapshot references、run linkage、request-audit fields、accepted AssetBible `ResolvedContinuitySnapshot` ID/revision/hash 与 GenerationSpec/AssetVersion owner refs，以及只复用 AssetVersion/catalog canonical `schema_version` 与同值 HTTP `schemaVersion` 的 owner-reference 合同。
- [x] 1.2 编写失败的定向 tests，覆盖 capability parameters、无 hardcoded model selection、project-safe references、AssetBible snapshot incomplete/stale/foreign/hash-revision mismatch、pending `ContinuityRevisionTask`、最多 8 个 reference/32 MiB/8192、PNG/JPEG/WebP 输入、mask 仅 PNG、allowlist、redirect 拒绝、loopback/private/link-local/reserved/metadata/DNS rebinding 拒绝、unconfigured behavior、duplicate retry keys、ProviderCall/RunEvent 所有权，以及版本引用缺失、冲突或独立版本事实在调用前无写入拒绝。
- [x] 1.2a 接入 AssetBible owner resolved snapshot/task read port；冻结 accepted snapshot provenance，禁止图片 application/adapter 解析或写 entry/override，并证明任一 continuity gate 失败时零 intent/ProviderCall/external request/StoragePort/AssetVersion。
- [x] 1.3 使用 ImageGenerationPort、catalog resolution、existing UoW 与 append-only AssetVersion rules 实现 application orchestration。

## 2. Adapter 与 Media Validation

- [x] 2.1 在 ImageGenerationPort 后实现 opt-in GPT Image adapter，包含 masked credential handling 与 normalized URL/base64 response contract。
- [x] 2.2 在 StoragePort 前实现最多 8 个 reference、32 MiB、8192、PNG/JPEG/WebP 输入、mask 仅 PNG、配置 allowlist、禁止 redirect/禁用地址与解析后复检的 bounded URL/base64 MIME、size、dimension 与 checksum validation。
- [x] 2.3 增加 adapter 定向 tests，覆盖 malformed URL/base64、MIME/hash mismatch、mask 格式、redirect、loopback/private/link-local/reserved、metadata 地址、DNS 解析变化、limits、provider failures、retry state、usage/cost audit，且不产生重复 RunEvent history。

## 3. Storage、Interfaces 与 Verification

- [x] 3.1 经 StoragePort 持久化 verified result references，并仅在 verified Provider terminal success 追加关联 ProviderCall 的 image AssetVersion，不在数据库存储 bytes；retry/reconcile 返回同一 version/candidate，后续 AssetEdit accept 仅 AcceptDecision/audit + 同一 version scenes exact CAS，零第二 append/bytes/object/ref copy。
- [x] 3.2 增加 API/worker BDD 与 architecture tests，覆盖 run association、failure visibility、`Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）测试组合、opt-in-only live call、workflows/runs 独有的 RunEvent，以及 `schema_version`/`schemaVersion` owner 同值与冲突时无 ProviderCall/RunEvent/StoragePort/AssetVersion 写入。
- [x] 3.3 运行定向 domain/application/adapter/integration/contract/BDD tests，以及 `openspec instructions apply --change integrate-gpt-image-provider --json`、`openspec status --change integrate-gpt-image-provider --json`、`openspec validate integrate-gpt-image-provider --strict --json`、`openspec validate --changes --strict --json`、`pnpm run check` 与 `git diff --check`。
- [x] 3.4 添加 image success 只登记未引用 candidate、完整 provenance/id/revision/hash/target/AssetBible snapshot handoff，以及 canonical accept 前不得更新 storyboard/current 或触发 Agnes、legacy/unknown `approve` 零副作用、reject/stale/foreign accept 零 AssetVersion/current/Timeline mutation的失败测试。
