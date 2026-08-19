## 1. 先补契约与领域测试

- [x] 1.1 在 `packages/contracts` 创建机器可读 objectKey corpus，先让 AJV 对 canonical keys、RFC opaque scheme、空 `?`/`#` 分隔符和 Python whitespace 反例失败。
- [x] 1.2 让 domain 与 HTTP 测试共同读取 corpus，断言所有不可接受 key 在 repository/database write 前失败。
- [x] 1.3 让 `0004` 与 `0006` migration 测试共同读取 corpus，覆盖合法 `workspace://` canonicalization、unsafe legacy URI、rollback/pre-revision preservation 与 helper 返回值持久化。

## 2. 实现共享路径契约

- [x] 2.1 收紧 `packages/contracts/schemas/asset-version.schema.json`，以显式 Python `strip()` whitespace 字符集表达 objectKey 规则，且不接受 domain 拒绝的值。
- [x] 2.2 更新 domain 与 migration canonicalization helper，确定性拒绝 blank、RFC scheme、query/fragment、dot、separator、absolute、UNC、drive-qualified 与 drive-relative key。
- [x] 2.3 让 `0004`/`0006` 参数化写入 helper 返回的 canonical provider、bucket 与 objectKey，不再用 SQL `trim()`/`substr()` 解析原始值；保持 HTTP/application DTO metadata-only。

## 3. 修复 legacy migration

- [x] 3.1 重构 `0004_assets_asset_versions_slice.py` 的 legacy 解析：在 DDL/数据写入前校验并规范化 workspace reference，并显式拒绝不安全或未知值。
- [x] 3.2 重构 `0006_assets_legacy_storage_repair.py`，采用与 `0004` 相同的规范化语义，并覆盖 whitespace 与 drive-relative 检查。
- [x] 3.3 验证 SQLite downgrade/upgrade 和 Compose PostgreSQL `0005 -> 0006 -> 0005 -> 0006`；保留合法行，并在 malformed row 存在时不产生部分提交。

## 4. 证据、文档与质量门

- [x] 4.1 运行 shared-corpus 的 contracts/domain/HTTP/`0004`/`0006` 定向测试，修复全部回归，且不调用真实 Provider/TOS。
- [x] 4.2 运行 `pnpm run check`、Ruff、format、mypy、OpenSpec strict validation、`git diff --check`、Compose PostgreSQL cycle 和 HTTP smoke。
- [x] 4.3 更新 `docs/phase-zero-traceability.md`、`docs/agent/PROJECT.md` 和 `docs/agent/HANDOFF.md`，加入 `已实现：阶段 0` / `已定义：接口/目标架构` / `待实现：产品能力` 状态标记、旧 change 的 `scope_decision_required` 状态及本 change 的验证证据。
- [x] 4.4 写入 protected-repair result artifact，记录实际 diff、验证、风险、未解决问题、控制器 `LEGACY_STATE_MIGRATION_REQUIRED` 证据，并明确暂不启动 review。
