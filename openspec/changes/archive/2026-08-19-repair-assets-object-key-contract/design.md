## 背景

阶段 0 的 Assets/AssetVersions 切片已经提供 domain、HTTP、SQLAlchemy 和 Alembic `0004`/`0006` 实现。旧 change 的 Sol/max closure 证明基础版本、项目归属和 hash 约束已修复，但发现 `objectKey` 的边界规则仍不一致：JSON Schema 可接受部分 domain/migration 必拒绝的值，且 drive-relative path 会绕过现有盘符检查。

本 change 必须保持 `AssetVersion` 只保存 storage reference 和元数据，不保存媒体二进制；合法 `workspace://projects/...` legacy reference 仍要在迁移后可读取。默认测试只能使用本地/内存/SQLite 或现有 Compose PostgreSQL，不调用真实 Provider/TOS。

## 目标与非目标

**目标：**

- 建立单一、可复用且跨 Python/JSON Schema/SQL migration 可表达的 objectKey 合同。
- 在 domain、共享 Schema、legacy upgrade/repair 和 HTTP 回归测试中锁定相同的拒绝集合。
- 在做 DDL 之前显式解析和规范化 legacy URI；未知 scheme、非法路径和 whitespace 不得静默降级。
- 保留可逆 migration 行为，验证 `0005 -> 0006 -> 0005 -> 0006` 与合法 legacy row 的可读性。

**非目标：**

- 不改变 AssetVersion API 资源形状、版本号策略或 contentHash/checksum 语义。
- 不接入真实 Provider、TOS、FFmpeg、AgentScope、Outbox 或媒体 Worker。
- 不修复 `projects/episodes` 或其他未被本 change 直接覆盖的历史问题。

## 决策

1. **统一使用 POSIX 风格相对 object key。**
   - 接受非空、非空白、只含 `/` 分隔的相对路径；每段必须非空且不是 `.`/`..`；禁止 `\\`、前导 `/`、`//`、尾随 `/`、UNC、任何盘符形式（包括 `C:relative`）。
   - 选择该规则而不是 `pathlib.Path` 的平台相关判断，因为 API/迁移在 Linux 与 Windows 开发环境都必须得到相同结果。

2. **Schema 使用结构化正则与边界校验，domain 作为最终权威。**
   - JSON Schema 的 `pattern` 使用与 Python `str.strip()` 等价的显式 Unicode whitespace 字符集，不使用 ECMAScript `\\s`；因此 newline、U+0085 和 Python 专有 whitespace 的接受/拒绝集合可由共享 corpus 直接比对。
   - Python domain 复用同一语义，并拒绝 RFC scheme（即使没有 `//`）与 `?`/`#` 分隔符；不得通过 `normalize` 把非法输入静默改成合法输入。

3. **Legacy migration 先解析 URI，再写入新列。**
   - 只接受完整 `workspace://` URI；无 `//` 的 RFC scheme、query/fragment（包括空 `?`/`#` 分隔符）和未知 scheme 都不是普通 legacy key。去除 scheme/authority 后的 key 必须逐段通过同一规则。
   - Python helper 在 DDL 前逐行预检并返回 provider、bucket 和 canonical key；迁移只参数化持久化该返回值，绝不以 SQL `trim()`/`substr()` 再解析原始 `storage_ref`。遇到非法/未知数据时显式抛出迁移错误并回滚。

5. **共享 corpus 是跨语言契约源。**
   - `packages/contracts/tests/fixtures/object-key-contract-corpus.json` 保存可接受 key、不可接受 key、可接受 workspace reference 与不可接受 legacy reference。
   - AJV、domain、HTTP、`0004` 与 `0006` 测试均读取同一文件；测试只能在各边界增加断言，不复制或改写样本集合。

4. **数据库不保存媒体 bytes。**
   - 本 change 不引入新的二进制列或外部调用；迁移只修正 reference 与约束前置校验。

## 风险与取舍

- [风险] 收紧规则可能拒绝历史上未被读取的非法 object key → 在 migration 前显式报告首个坏行，并提供可回滚的 migration；不自动改写未知数据。
- [风险] JSON Schema regex 与 Python 规则未来可能漂移 → 由单一机器可读 corpus 驱动 AJV/domain/HTTP/`0004`/`0006` 五层参数化测试，并在 OpenSpec tasks 中设为同一验收项。
- [风险] 旧 URI 中的 Unicode whitespace 在不同数据库方言表现不同 → 逐行 Python 预检先拒绝，SQL 只处理已验证的 canonical value。

## 迁移计划

1. 先补失败测试，覆盖 Schema、domain、`0004` 和 `0006` 的空段/空白/drive-relative 反例。
2. 实现共享语义的 domain/Schema 校验和 legacy canonicalization。
3. 运行 SQLite migration cycle、PostgreSQL Compose migration cycle、HTTP smoke 与全量质量门。
4. 若升级中发现非法 legacy row，保持原数据库 revision，不写入部分约束；修正数据后重新执行。
5. downgrade 仍只删除本 change 引入的约束/转换，不删除媒体对象或业务数据。

## 待确认问题

- 当前无需新增架构选择；若生产历史数据包含未列入测试集合的自定义 scheme，必须先单独定义 provider adapter 迁移规则，不能在本 change 中猜测。
