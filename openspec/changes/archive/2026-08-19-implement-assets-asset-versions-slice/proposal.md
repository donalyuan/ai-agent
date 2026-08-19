# Change：实现 Assets/AssetVersions 垂直切片

## Why

阶段 0 已固定 `Asset`、`AssetVersion` 共享 Schema 与对象存储引用边界，但 API、领域服务和持久化仍没有可调用的素材版本切片。此 change 为后续时间线、生成和媒体流程提供可审计的素材身份与追加版本事实源。

## What Changes

- 新增 framework-free 的 `Asset`/`AssetVersion` domain、稳定错误和不可变/路径边界规则。
- 新增 application commands/queries、Repository/UoW ports 与 in-memory adapter。
- 扩展 SQLAlchemy adapter，并新增可逆 `0004_assets_asset_versions_slice` migration；为已应用 `0004` 的数据库补充可逆 `0005_assets_integrity_repair`。
- 提供最小 FastAPI create/get/list asset、append/get/list version HTTP API，明确 camelCase 与 persistence snake_case alias 转换。
- 增加 DDD/BDD/SDD/TDD、架构、HTTP、并发、不可变性和 migration 回归测试，并更新追溯和项目记忆。
- 修复独立 `contentHash` 持久化、legacy checksum 完整性、版本项目归属、基础数据库约束与共享 Schema 字段对齐。
- 本轮受保护修复补齐深度不可变边界、`workspace://` legacy reference 规范化、严格安全 `objectKey` Schema/领域规则，以及可重放的 `0006` repair migration。
- 历史质量门记录 `sol_max_initial fail -> 一次 protected repair -> sol_max_closure valid fail`；用户决定保留该失败和旧 controller lineage 不可关闭事实，并以 claims 超集的 `complete-assets-asset-versions-slice-v3` replacement workflow 重新完成全量验证。该 replacement lineage 已通过独立 `sol_xhigh` gate，controller `close_check` 返回 `close_allowed=true`；本 change 由 replacement lineage 完成收口，不回写或伪造旧 lineage pass。

## Non-Goals

不实现原地版本更新、媒体二进制保存、FFmpeg、真实 Provider/TOS 调用、媒体 Worker、Outbox、SSE、完整音频系统、Scene/Shot/Timeline 或前端页面。`audio` 仅是 `Asset.kind` 的一个共享枚举值，不创建 `AudioAsset` 子类型。本 change 不删除、迁移或伪造旧 workflow-controller state，也不把 controller 的目标 v3 审计能力误写为当前实现。

## Impact

影响 `services/api/src/video_agent_api/{domain,application,adapters,interfaces}`、Alembic `0004`/`0005`、相关 tests、追溯文档和项目记忆；保持 projects/episodes、health/runtime/Mock/Local、共享 Schema 与 Compose 兼容。
