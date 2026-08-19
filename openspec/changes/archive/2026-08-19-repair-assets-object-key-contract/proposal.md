## Why

当前 `AssetVersion.storageObject.objectKey` 在共享 JSON Schema、领域校验和 `0004`/`0006` legacy migration 中没有完全采用同一条路径安全规则。尾随空段、纯空白值和 drive-relative `C:relative.mp4` 可能被部分边界接受，导致客户端契约、HTTP/domain 行为和迁移结果不一致。旧 change 的 Sol/max closure 已确认这是产品 blocker，因此需要一个独立、可审计的替代 change。

## What Changes

- **BREAKING** 收紧 `objectKey` 合同：拒绝绝对路径、UNC、drive-qualified 和 drive-relative 路径、RFC scheme（包括无 `//` 的 `workspace:`/`s3:` 形式）、query/fragment 分隔符、反斜杠、`.`/`..` 段、空段、尾随 `/` 和纯空白值。
- 让领域校验、共享 JSON Schema、`0004` legacy upgrade 和 `0006` repair 使用一致的规范化与拒绝规则，并以机器可读 corpus 锁定 ECMAScript 与 Python `strip()` 的 whitespace 集合。
- 为 Schema、domain、HTTP/application 和 migration 增加反例回归测试，并验证合法 `workspace://` legacy URI 仍可读、可降级、可再次升级。
- 同步 OpenSpec tasks、阶段追溯和项目记忆，明确旧 change 仍为 `scope_decision_required`，新 change 是独立修复 lineage。

## Capabilities

### New Capabilities

- `asset-object-key-contract`: 定义跨 Schema、domain、HTTP 和 migration 的 objectKey 路径安全合同。

### Modified Capabilities

<!-- 现有全局 capability 没有独立 source spec；新增 capability 承载修复后的跨边界契约。 -->

## Impact

- 共享契约：`packages/contracts/schemas/asset-version.schema.json` 与 `packages/contracts/tests/contracts.test.mjs`。
- 后端 domain/application/HTTP：`services/api/src/video_agent_api/domain/assets.py` 及相关测试。
- Alembic：`0004_assets_asset_versions_slice.py`、`0006_assets_legacy_storage_repair.py` 及 migration 回归测试；迁移逐行持久化 Python helper 返回的 canonical key，不在 SQL 中重新解析原始 reference。
- 记录与验收：本 change 的 OpenSpec artifacts、`docs/phase-zero-traceability.md`、`docs/agent/PROJECT.md`、`docs/agent/HANDOFF.md`。
- 不新增依赖、不调用真实 Provider/TOS、不改动 `projects/episodes` 或完整视频产品链路。
