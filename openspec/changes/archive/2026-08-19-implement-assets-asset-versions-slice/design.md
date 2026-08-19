# Assets/AssetVersions 设计

## DDD

`Asset` 是项目内的素材身份聚合根，拥有共享五值 kind（`image|video|audio|text|document`）、名称、状态和 revision。`AssetVersion` 是只追加的不可变事实，归属于一个 Asset，服务端在同一 Asset 内从 1 开始分配连续 `versionNumber`。新增版本不修改旧版本；数据库唯一约束 `(asset_id, version_number)` 是并发最终保护。

领域只接受抽象 `StorageObject`：provider、bucket、相对 objectKey、MIME、大小、checksum 和可选媒体 metadata；不接受 bytes/base64/绝对路径。`AssetVersion`、`StorageObject` 和嵌套 `media` 在构造后均不可变，adapter/HTTP 序列化必须复制为普通 dict 后再跨边界传输。objectKey 必须是相对路径且拒绝 `/`、盘符、UNC、反斜杠、空路径段、`.`/`..` 路径逃逸。

## BDD

- 给定存在 project，创建 asset 返回稳定 id、kind/name、初始 revision 和 draft 状态。
- 给定不存在 project，创建或查询其 asset 返回明确 404/`project_not_found`。
- 给定 asset，首次 append version 返回 `versionNumber=1`，再次 append 返回 2；列表按版本号升序。
- 给定恶意 objectKey、空 MIME、负大小、错误 hash 或 bytes 字段，请求失败 422，且不产生持久化记录。
- 给定两个并发 append，唯一约束拒绝重复版本；旧版本始终可读且无法更新。

## SDD

领域/应用不依赖 FastAPI、Pydantic 或 SQLAlchemy。HTTP DTO 使用 camelCase（`projectId`、`versionNumber`、`storageObject`、`objectKey`、`mimeType`、`sizeBytes`、`durationMs`），ORM/persistence 使用 snake_case（`project_id`、`version_number`、`storage_ref`、`metadata_json` 等）；转换函数显式存在并测试。HTTP 只保存 storage object reference 和元数据。

0004 为现有资产表补齐 `name`，为 asset version 补齐可查询的 `project_id`、`content_hash`、`mime_type`、`size_bytes`、`media_metadata` 和 object reference 兼容字段；升级可回填旧行，降级可逆。legacy `workspace://path` 在 DDL 前必须拆分为 `storage_provider=local_workspace`、`bucket=workspace` 和规范化相对 `object_key=path`，不得把 URI 原样写入 object key。旧 `storage_ref`/`metadata_json` 继续兼容阶段 0 数据。旧行若缺少真实 checksum 或 checksum 不是 64 位十六进制值，迁移必须显式失败并保留原始数据，不能用占位 hash 伪造内容完整性。

`0005_assets_integrity_repair` 为已应用 `0004` 的数据库补齐约束。`asset_versions.project_id` 既保存聚合边界，也必须在数据库中通过 `(asset_id, project_id)` 组合外键匹配 `assets` 的归属项目；应用层派生不能替代数据库约束。checksum 与 content_hash 使用跨 PostgreSQL/SQLite 可执行的 64 位十六进制检查约束。

`content_hash` 是版本内容的独立事实，`storageObject.checksum` 是对象存储校验值；两者可以相同但不得在持久化或读取时互相覆盖。`project_id` 从 Asset 归属派生并在版本表中非空保存，避免读取时丢失聚合边界。

文件 `0006_assets_legacy_storage_repair.py`（Alembic revision `0006_assets_legacy_repair`，25 个 ASCII 字符，长度不超过默认 `alembic_version.varchar(32)`）专门修复已经运行过旧 `0004`/`0005`、但把 `workspace://` URI 错写进 `object_key` 的数据库；它必须在升级前拒绝不安全引用，并满足 `0005 -> 0006 -> 0005 -> 0006` 可重复验证。

HTTP 传输字段遵守共享 JSON Schema：`schema_version` 保留 snake_case，其余资产版本对象字段使用既定 camelCase alias。

## TDD 与架构

先写失败的 domain/application/adapter/HTTP/migration 测试，再实现最小行为；最后运行定向 pytest、OpenSpec apply instructions/strict validate、`pnpm run check`、SQLite migration cycle 和可行 Compose/PostgreSQL 验证。接口层不得访问 Session，application 不导入具体 adapter，domain 不导入框架。历史质量门顺序为：`sol_max_initial` 独立复审；有效失败后冻结 blocker 并执行一次受保护 repair；全新 `sol_max_closure` 仍有效 fail。用户决定不改写该历史，也不迁移、read-through 或伪造旧 controller state。完成路径改为 claims 超集 replacement workflow `complete-assets-asset-versions-slice-v3`：该 workflow 已完成当前全量验证，并由此前未参与的新 `sol_xhigh` gate 有效审核为 pass；controller `close_check` 返回 `close_allowed=true`。OpenSpec 任务 20/20 与 replacement gate 共同构成本 change 的完成证据。
