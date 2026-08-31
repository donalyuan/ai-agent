## Why

阶段一 review 发现多条真实运行链路仍可造成跨项目访问、并发事实丢失、上传来源伪造和 Local workspace 隔离。文本运行也只启动 Temporal checkpoint，无法进入审核闭环；这些问题必须在阶段一能力继续使用前关闭。

## What Changes

- **BREAKING** 为 phase-one project-scoped HTTP routes 统一强制 `X-Project-Scope`，并拒绝缺失或不匹配的 scope。
- 让文本 Temporal activity 执行 `TextGenerationService.generate()` 并原子进入 `waiting_review`。
- 为 collection-backed `PhaseOneDocument` 更新增加加载 revision 的 compare-and-set，避免并发覆盖审计、outbox 和文本事实。
- 在 Compose API 与 Media Worker 间挂载同一 Local workspace volume。
- 复用 multipart operation key 前冻结并比较完整 session binding；统一使用 canonical object-key validator。
- 绑定 SourceMaterial 时校验真实 owner 当前版本；上传来源从已登记 AssetVersion 派生 hash、parse 和 validation 状态。
- 文本 Provider 调用写入并完成 catalog `ProviderCall`，保留受控 usage、能力快照和脱敏失败诊断。
- Local storage adapter 不再自行签发无引用删除证明，引用证明缺失时 fail closed。

## Capabilities

### New Capabilities
- `phase-one-runtime-integrity`: Temporal 文本执行、collection CAS、完整 ProviderCall 记录和来源事实闭环。

### Modified Capabilities
- `provider-and-storage-boundaries`: multipart session binding、canonical object key、Local workspace 共享与删除证明边界。
- `assets-http-api`: project scope header 与上传来源 owner 校验。

## Impact

- 影响 `services/api` 的 Temporal、application、SQLAlchemy、storage ports 和 HTTP routers。
- 影响 `infra/compose/compose.yaml` 的 Local workspace volume 配置。
- 需要新增安全、并发、来源绑定、ProviderCall 和 Temporal activity 回归测试；scope header 是现有未带 header 调用方的契约变更。
