# 排障记录

本文件仅记录已观察、可复现且有证据的问题；不得把推测写为根因。每条记录应包含触发条件、观察到的原始结果、已验证范围和已确认处置。

## 当前记录

### Alembic autogenerate 与历史 owner 表漂移

- **触发条件**：在 `services/api` 目录执行 `uv run alembic check`，数据库位于当前 Compose PostgreSQL head `0022_asset_center_owner`。
- **观察结果**：命令退出码为 `255`，报告历史 owner/document 表在当前 `Base.metadata` 中不存在，以及 JSON 类型、索引、外键、唯一/检查约束和可空性差异；`uv run alembic current` 仍正确返回 `0022_asset_center_owner`。
- **已验证范围**：七组 owner migration tests 共 `88 passed`；升级/降级 cycle 和 Compose PostgreSQL head 均通过。阶段一 OpenSpec 退出门要求这些可逆 migration tests，不要求 `alembic check` clean。
- **已确认处置**：不修改 `env.py` 过滤 autogenerate 差异，也不生成未经设计的漂移迁移；后续若要收敛 metadata，必须单独设计 owner schema reconciliation change。

遇到文档与当前代码、测试、schema 或可执行配置冲突时，先以高优先级事实为准，再更新受影响文档；该规则不是对根因的推断。
