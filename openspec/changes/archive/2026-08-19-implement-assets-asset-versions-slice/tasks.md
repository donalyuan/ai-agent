## 1. OpenSpec 与领域 TDD

- [x] 1.1 补齐 proposal/design/specs，明确 DDD/BDD/SDD/TDD、alias 边界和非目标。
- [x] 1.2 先写 Asset/AssetVersion 领域失败测试，再实现纯 Python entities/errors/value validation。
- [x] 1.3 先写 application command/query 失败测试，再实现 Repository/UoW ports 与 in-memory adapter。

## 2. 持久化与迁移

- [x] 2.1 先写 SQLAlchemy adapter/并发/不可变性失败测试，再实现 repository/UoW 映射。
- [x] 2.2 新增可逆 `0004_assets_asset_versions_slice`，补齐 name、storage 元数据列和唯一/检查约束。
- [x] 2.3 先写 SQLite migration cycle 与旧数据回填测试，再验证 upgrade/downgrade。

## 3. HTTP 与架构

- [x] 3.1 先写 HTTP BDD/契约失败测试，再实现 create/get/list asset 与 append/get/list version 路由。
- [x] 3.2 验证 camelCase HTTP 与 snake_case persistence alias、错误映射、无数据库 503 和 health/runtime 兼容。
- [x] 3.3 增加架构依赖测试，确认 domain/application/interfaces 边界和无媒体二进制。

## 4. 追溯与质量门

- [x] 4.1 更新 `docs/phase-zero-traceability.md`、`docs/agent/PROJECT.md`、`docs/agent/HANDOFF.md`。
- [x] 4.2 运行 OpenSpec apply/strict validate、定向 pytest、`pnpm run check`、SQLite migration cycle 和可行 Compose/PostgreSQL 验证。

## 5. 质量修复与复审

- [x] 5.1 先写独立 contentHash、legacy checksum 和版本 project_id 回归测试，再补齐 ORM 映射与 migration 完整性约束。
- [x] 5.2 先写 HTTP get/list/404/503/共享 Schema 契约测试，再对齐 transport alias 和错误边界。
- [x] 5.3 补充 SQLAlchemy 唯一版本冲突与 migration 旧数据回填 cycle，运行定向检查。
- [x] 5.4 运行全量质量门、Compose/PostgreSQL smoke，并完成本轮闭环前验证；Sol/max closure 仍由 5.6 独立执行。
- [x] 5.5 根据 Sol/xhigh blocker 先写回归测试，再补齐 AssetVersion 项目归属组合约束、hash 十六进制约束和 OpenAPI 路径参数契约。
- [x] 5.6 历史 `sol_max_closure` 已执行并有效 fail，进入 `scope_decision_required`；用户已决定保留旧 lineage 不可关闭事实，不勾画为 pass、不得自动修复或重开旧 controller。
- [x] 5.7 根据 `sol_max_initial` 有效 fail 执行一次受保护 repair：关闭 B1 和标准 `workspace://` 场景，增加 `0006` repair 并收紧 B3；closure 发现 B3 仍有未覆盖反例，B4 仍为旧 controller 审计限制。
- [x] 5.8 修复 PostgreSQL 默认 `alembic_version.varchar(32)` 与 `0006` revision 长度不兼容，并以 SQLite migration cycle 回归锁定 compact revision。

> 历史记录：`sol_max_closure` 曾发现 `assets/a/`、空白 objectKey 和 `C:relative.mp4` 未在 Schema/domain/migration 中一致拒绝；`repair-assets-object-key-contract` 已独立覆盖这些产品 blocker。旧 controller 同时缺少完整 claims、repair/max charter，旧 lineage 仍不可关闭。用户已决定建立 claims 超集 replacement workflow，自动链不重开旧 controller。

- [x] 5.9 已建立 claims 超集 replacement workflow `complete-assets-asset-versions-slice-v3`，完成当前 OpenSpec/全量质量门、Compose PostgreSQL migration cycle、HTTP smoke 与无真实 Provider/TOS 扫描；此前未参与的独立 `sol_xhigh` gate 已有效 pass，controller `close_check` 返回 `close_allowed=true`。本 change 由 replacement lineage 完成收口，旧 lineage 不被改写。
