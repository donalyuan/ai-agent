# 项目事实

## 当前状态

- 记录日期：2026-08-19。
- **已实现：阶段 0**：单仓库工程基线已完成；包含 React/Vite 工作台壳层、FastAPI 健康接口、PostgreSQL/Alembic 基础模型、共享 JSON Schema、Mock Provider、LocalWorkspaceAdapter、SkillRegistry/SkillRouter、Temporal 与三类 Worker 健康入口。
- **已实现：阶段 0 垂直切片**：`projects/episodes` 已完成 domain/application/Repository/UoW/SQLAlchemy migration/HTTP API 及架构、契约和 BDD/TDD 测试；业务 API 已在 Compose PostgreSQL 上验证。
- **已实现：Assets/AssetVersions 垂直切片**：素材身份与只追加版本已完成 domain/application/in-memory/SQLAlchemy/HTTP、Alembic `0004` 安全回填、`0005` 完整性 repair 与 `0006` legacy workspace reference repair（revision ID 为 25 字符的 `0006_assets_legacy_repair`）、架构与 BDD/TDD 测试；`repair-assets-object-key-contract` 以 `packages/contracts` 的机器可读 corpus 统一 Schema/domain/HTTP/`0004`/`0006` 的 canonical relative objectKey，拒绝尾随空段、Python `strip()` whitespace（含 newline/U+0085）、绝对/UNC/drive-qualified/drive-relative、无 `//` 的 RFC scheme、`?`/`#` 分隔符、反斜杠和点段。legacy migration 只解析完整 `workspace://` URI；`0004`/`0006` 均在 DDL 前以各自冻结的 Python helper 预检，并参数化持久化 helper 返回的 canonical 值，不以 SQL `trim()`/`substr()` 重解析原始值；`0006` 不改写普通 legacy provider/bucket。`AssetVersion`、`StorageObject` 和嵌套 media 深度不可变，数据库强制 AssetVersion 项目归属组合关系和 64 位十六进制 hash，只保存 storage object reference 和元数据，不保存媒体二进制。
- **已定义：接口/目标架构**：`define-backend-engineering-architecture` 与 ADR-0003 固定模块化单体、Ports/Adapters、`interfaces -> application -> domain` 依赖方向，以及后续 Outbox/Temporal/Worker 边界；目标架构不等同于全部已实现代码。
- **待实现：产品能力**：完整生成链路、专业剪辑、协作、移动端、平台发布、真实 Provider/TOS、AgentScope、FFmpeg、SSE、Outbox、音频系统和媒体 Worker。
- OpenSpec 状态：当前无活动 change；`establish-phase-zero-foundation`（35/35）、`define-backend-engineering-architecture`（7/7）、`implement-projects-episodes-slice`（19/19）、`implement-assets-asset-versions-slice`（20/20）和 `repair-assets-object-key-contract`（13/13）均已完成并归档到 `openspec/changes/archive/2026-08-19-*`。Assets 的历史 `sol_max_closure` 仍有效 fail，旧 lineage 仍为 `scope_decision_required`；不得将 replacement 结果误写成旧 lineage pass。
- Assets 旧 closure 的产品 blocker 已由独立 `repair-assets-object-key-contract` 覆盖；旧 controller 的 claims、repair records 与 max charter 仍不足以关闭旧 lineage。用户授权的 claims 超集 replacement lineage `complete-assets-asset-versions-slice-v3` 已完成全量验证，独立 `sol_xhigh` 有效 pass，controller `close_check` 返回 `close_allowed=true`；该 OpenSpec change 已通过 replacement lineage 完成收口并已归档。
- Compose PostgreSQL 曾因过长 revision ID 写入 `alembic_version.version_num` 而报 `psycopg.errors.StringDataRightTruncation`；修复后的 `0006_assets_legacy_repair` 已在真实 Compose PostgreSQL 完成 `0005 -> 0006 -> 0005 -> 0006` cycle。
- Node 使用根 `pnpm` workspace 与 `pnpm-lock.yaml`；Python API 固定 Python `>=3.12,<3.13` 并使用 `services/api/uv.lock`。
- 运行边界为 `apps/web`、`packages/contracts`、`packages/ui`、`services/api`、`workers/{agent,generation,media}` 与 `infra/compose`。
- Compose 已验证 Web、API、PostgreSQL 18、Temporal 与三类 Worker；API 和 Worker 通过同一严格 runtime composition 消费示例配置，默认只使用 Mock Provider 和 LocalWorkspaceAdapter。
- 阶段 0 仍保留健康/runtime/ports 等迁移起点；除 `projects/episodes` 与 `assets/asset-versions` 切片外，不应将目标目录或目标能力误写为已实现。

## 当前可证实目录

- `docs/agent/`：本目录，保存项目持久记忆。
- `docs/adr/`：架构决策记录。
- `docs/phase-zero-traceability.md`：R1-R8 到规格、实现和验证的追溯。
- `openspec/`：OpenSpec 配置与变更 artifacts。

## 待确认

以下信息当前没有可验证的项目文件，必须在得到证据或用户确认后再记录为事实：

- 真实 Provider、TOS、生产凭据和远程部署策略仍未实现或验证。
- 完整生成、媒体处理、专业剪辑、多人协作、手机端和发布平台仍为后续阶段范围；真实 Provider/TOS 仍未实现或验证。

## 使用规则

此文件只记录稳定且已确认的项目事实。与当前代码、测试、schema 或可执行配置冲突时，以后者为准并更新本文件。
