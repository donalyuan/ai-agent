## 1. B1 冻结 admission 基线与失败测试

- [x] 1.1 重新取证 Run/NodeRun、ProviderCall、VideoOperation、outbox、Generation/Media dispatcher 的当前字段、唯一约束和 route 行为，记录 B1-B7 与实际代码的对应关系，不修改旧 `sol_max_closure` 审计状态。
- [x] 1.2 先编写 cross-process/restart focused tests：完整 frozen identity 的同一 operation 只能被稳定 workflow ID 单次执行或 reconcile。
- [x] 1.3 先编写缺失、foreign、stale、revision/hash drift、queue/schemaVersion drift 和 legacy-new route 混用的零 outbox/activity/provider/storage/render 副作用测试。
- [x] 1.4 在最小 owner ledger/route/migration 范围内实现或收紧冻结 identity 的持久化、读取和逐项验证，并运行 1.2-1.3 的测试。

  映射：`NodeRun.admission_refs`、`ProviderCall` 与 `VideoOperation` 是 owner ledger；`GenerationOutboxDispatcher` 在启动前验证 frozen queue/schema；Text/Image/Video 只将已 claim 的 `outbound_correlation` 传给 provider，Video reconcile 同样只读取该 owner correlation。

## 2. B2-B3 Media dispatch 与 typed handoff

- [x] 2.1 先编写 Generation/Media workflow、activity、task queue 与 Compose service production reachability 的正反测试，覆盖未注册/不可达 binding 的 `not_ready` 与零副作用。
- [x] 2.2 先编写 generated candidate 的 accepted-current/provenance CAS gate 与 `uploaded_source|asset_center` 独立授权 gate 测试，覆盖 pending/rejected/stale/foreign 输入拒绝。
- [x] 2.3 实现 matching durable intent/outbox 到固定 worker/activity 的 dispatch，并限制 legacy route 为既有 operation 的 drain/reconcile；运行 2.1-2.2 的 focused tests。
- [x] 2.4 先编写 Storage -> verified StoredObjectRef -> Assets owner AssetVersion 与 Storage/Export -> Export owner ExportArtifact 的 typed handoff、response-loss、duplicate 和 direct-write 拒绝测试。
- [x] 2.5 实现/reconcile 最小 typed handoff 与 owner-only result registration，验证 MIME/size/checksum/scope/operation/revision/hash/package subphase，并运行 2.4 测试。

## 3. B4 显式 local composition

- [x] 3.1 先编写 profile/capability/credential reference/adapter/renderer/queue 完整时仅组合 matching frozen port 的本地测试。
- [x] 3.2 先编写未配置、禁用、未批准、capability 不支持、credential resolver 不可用、renderer/worker 不可达时返回脱敏 `unconfigured`/`not_ready`、零外部请求且不 fallback 的测试。
- [x] 3.3 在 runtime、worker 和 Compose 的最小范围实现显式 composition/readiness 校验；保持 `PROVIDER_MODE=mock` 与 `STORAGE_MODE=local_workspace` 只能经明确选择，并运行 3.1-3.2 测试。

## 4. B5 Text terminal 与 ambiguous 映射

- [x] 4.1 先编写 matching frozen ProviderCall terminal success 仅登记一次完整、schema-valid immutable candidate graph/TextReviewBatch handoff 的 restart/duplicate 测试。
- [x] 4.2 先编写 accepted-response-loss、缺 remote identity 或无 frozen lookup capability 时保持 `ProviderCall.unknown`、不重提且不改变媒体 gate 的测试。
- [x] 4.3 实现最小 Text owner result/reconcile 映射，复用 frozen correlation/remote identity/lookup capability，并运行 4.1-4.2 测试。

## 5. B6-B7 Image/Video frozen retry 与 reconcile

- [x] 5.1 先编写 Image 在 matching ProviderCall/correlation/capability 下先 lookup reconcile、最多一个 candidate handoff、无 lookup 时保持 `ProviderCall.unknown` 的测试。
- [x] 5.2 先编写 Video 在 matching VideoOperation/provider request/poll-cancel-result contract 下先 reconcile、无 lookup 时保持 `submission_unknown`、重复 cancel/late result 不覆盖 current/Timeline/ExportArtifact 的测试。
- [x] 5.3 实现最小 Image/Video retry/reconcile 收口，禁止自动 re-submit、generic state conversion 与 fallback，并运行 5.1-5.2 测试。

## 6. B1-B7 集成验证与诚实证据

- [x] 6.1 运行对应 API、domain/application/adapter/HTTP、Generation Worker、Media Worker、migration 与 restart focused tests；记录每条通过项、真实失败项和残余风险。
- [x] 6.2 运行项目规定的格式、lint/typecheck、OpenSpec strict、迁移 cycle、Mock/Local Compose build/up/readiness 与 `git diff --check`；任何失败先按新的受控契约分流，不勾选对应实施任务。

  RCX7 实际结果：focused API `162 passed`；Ruff、mypy（99 source files）、根 `pnpm run lint`、根 `pnpm run format:check`、`uv lock --check`、`git diff --check` 与本 change OpenSpec strict 通过（Web lint 保留 10 条既有 warning、0 error）。完整 API pytest 曾运行但最终摘要未被执行包装保留，不能写作通过。共享 Mock/Local Compose 使用当前源码 build 后，先以 current image 完成 `0028_provider_lookup_contract -> 0029_lookup_binding -> 0028_provider_lookup_contract -> 0029_lookup_binding (head)`，并确认 `provider_calls.remote_lookup_binding` 列存在；随后 `up --build -d --wait` 通过，PostgreSQL、Temporal、API、Web、Agent、Generation、Media 均 healthy；`/v1/health/ready` 返回 `{"status":"ready"}`，Generation/Media `--health` 通过。R4 activity 只在持久 binding 与 capability/operation/protocol 全匹配时 lookup；无匹配保持 `unknown/unsupported`，不 submit。
- [x] 6.3 更新本 change 的无敏感信息证据，明确 credentialed E2E、真实 Provider/TOS/付费调用和 MVP-A exit closure 为非目标，且当前结果为 `result=unconfigured`、`readiness=not_ready`、`externalAcceptCount=0`。
- [x] 6.4 在所有实际实施与验证完成后，逐项核对本清单再勾选；不得以旧 change 任务全勾选、旧 `sol_max_closure` 或本 proposal 的 apply-ready 状态替代实现完成。

当前为 `22/22`。RCX8 增量修复确认：StorageProfile/BucketBinding 与 renderer identity 仅经显式 composition 进入 API/Generation/Media/Export；`FrozenRemoteLookup`/ProviderCall binding/Generation injection 精确匹配七元组，旧 revision 不复用。旧 `sol_max_closure` 未被读取、迁移或作为关闭依据。credentialed E2E、真实 Provider/TOS/付费请求与 MVP-A exit closure 均非本 repair 结果，外部边界保持 `result=unconfigured`、`readiness=not_ready`、`externalAcceptCount=0`。

RCX10 final source binding：实现集合 fingerprint=`f1592300a5eeeef12fffbee9035f08b15148959bdd63290d1d6be8f3c4ddc6de`；`CatalogService` 删除 field-only lookup，Image `profileRevision` 在 intent 前必填，Export/Media 在 storage I/O 前校验完整 identity。验证证据与逐文件 SHA-256 见 `docs/evidence/repair-runtime-closure-contracts-integration.json`；旧测试对新 breaking contract 的 3 项失败保持可见，未恢复 fallback。

RCX6 `repair_blockers` 增量已在受保护节点完成：新增 `provider_calls.remote_lookup_binding` additive 字段并接通 SQLAlchemy round-trip；Storage/Export 使用冻结 profile identity 与 capability 校验；API/Media renderer 环境 fallback 已移除。旧 RCX4 失败审计未改写。
