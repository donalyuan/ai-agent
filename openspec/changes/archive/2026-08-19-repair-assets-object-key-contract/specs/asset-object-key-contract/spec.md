# asset-object-key-contract Specification

## Purpose

定义 AssetVersion objectKey 在共享契约、领域对象、HTTP 输入和 legacy migration 中一致的路径安全边界。

## ADDED Requirements

### Requirement: Canonical relative object key

系统 SHALL 接受非空、非空白的 POSIX 风格相对 objectKey；每个路径段必须非空且不是 `.` 或 `..`，分隔符只能是 `/`。系统 MUST 拒绝绝对路径、UNC、drive-qualified 或 drive-relative path、任意 RFC scheme（包括没有 `//` 的 scheme）、`?`/`#` query/fragment 分隔符、反斜杠、重复/尾随 `/`、空段、点段和纯空白值。Schema 对 whitespace 的判断 MUST 与 Python `str.strip()` 一致。

#### Scenario: 接受 canonical key
- **WHEN** objectKey is `projects/a/v1.mp4`
- **THEN** Schema、domain、HTTP/application 和 migration 原样接受该值

#### Scenario: 拒绝路径穿越与空段
- **WHEN** objectKey 包含 `../`、`./`、`//`、尾随 `/` 或 blank segment
- **THEN** 每个边界都必须在持久化前以明确的 validation 或 migration error 拒绝该值

#### Scenario: 拒绝绝对路径与 drive-relative path
- **WHEN** objectKey 是 `/tmp/a.mp4`、`\\server\\share\\a.mp4`、`C:/a.mp4` 或 `C:relative.mp4`
- **THEN** Schema、domain、HTTP/application 和 migration 拒绝该值

#### Scenario: 拒绝纯 whitespace key
- **WHEN** objectKey 为空或只包含 whitespace
- **THEN** validation 失败，且不发生 repository/database write

#### Scenario: 拒绝 opaque scheme 与空 query/fragment 分隔符
- **WHEN** objectKey 或 legacy reference 是 `workspace:projects/a/v1.mp4`、`workspace:/projects/a/v1.mp4`、`s3:bucket/a.mp4`、`projects/a?` 或 `projects/a#`
- **THEN** 每个边界都必须在持久化前拒绝该值；只有完整且受支持的 `workspace://` legacy URI 形式可进入规范化

#### Scenario: 与 Python whitespace 语义一致
- **WHEN** corpus entry 是 newline、U+0085，或其他被 Python `str.strip()` 识别但不被 ECMAScript `\\s` 识别的字符
- **THEN** AJV、domain、HTTP 和两次 migration 对该 entry 的接受或拒绝结果一致

### Requirement: Legacy workspace URI normalization

`0004` 和 `0006` SHALL 在写入 provider、bucket 与 objectKey 列之前解析受支持的 `workspace://` legacy reference。canonicalization helper MUST 返回待持久化的 provider、bucket 与 objectKey 值；SQL MUST NOT 使用 `trim()`、`substr()` 或等价方式重新解析原始 `storage_ref`。结果 key MUST 满足 canonical relative object key 要求；未知 scheme、opaque scheme、空 authority/path、query/fragment 分隔符和不安全 key MUST 显式失败，并保留 migration 前的 revision。

#### Scenario: 保留可读的 workspace legacy row
- **WHEN** legacy row 包含 `workspace://projects/a/v1.mp4`
- **THEN** migration 保存 `local_workspace`、`workspace` 和 `projects/a/v1.mp4`，且 ORM/domain 可以成功读取该行

#### Scenario: 拒绝不安全的 workspace URI
- **WHEN** legacy row 包含 `workspace://C:relative.mp4`、`workspace://projects/a/` 或 `workspace://   `
- **THEN** migration 在应用新约束前失败，且不提交部分修复的 row

### Requirement: Cross-boundary regression coverage

自动化测试 SHALL 共同从 `packages/contracts` 读取同一份 machine-readable canonical/invalid key corpus，并对照 JSON Schema、domain/application/HTTP validation 和两次 legacy migration revision 执行验证。Tests MUST 继续证明：合法 metadata response 不包含 media bytes，且不会调用真实 provider/TOS。

#### Scenario: contract corpus 保持对齐
- **WHEN** canonical/invalid corpus 在 contracts、domain 和 migration 测试中运行
- **THEN** 所有层产生一致的接受/拒绝结果，且完整 repository quality gate 通过
