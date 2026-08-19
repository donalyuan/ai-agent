# 当前交接

## 当前状态

- 当前分支：`main`。
- **已实现：阶段 0**：`establish-phase-zero-foundation` 已完成 35/35 tasks，`projects/episodes` 垂直切片已完成 19/19 tasks；两个 change 均已归档。
- **已实现：Assets/AssetVersions**：`implement-assets-asset-versions-slice` 已完成 20/20 OpenSpec tasks，包含领域、应用、内存/SQLAlchemy adapter、HTTP API、可逆 Alembic `0004`/`0005` 和 `0006` legacy workspace reference repair；独立 `repair-assets-object-key-contract` 以共享机器可读 corpus 统一 Schema/domain/HTTP/`0004`/`0006` 的 canonical objectKey 合同，新增拒绝 `workspace:`/`workspace:/`/`s3:`、空 `?`/`#` 分隔符与 Python `strip()` whitespace（含 newline/U+0085）。历史 migration 使用冻结本地 helper，持久化其 canonical 值；`0006` 只修复完整 workspace reference，不改写普通 legacy provider/bucket。历史 `sol_max_closure` 仍有效 fail，旧 lineage 保持 `scope_decision_required`；用户授权的 `complete-assets-asset-versions-slice-v3` replacement lineage 已由独立 `sol_xhigh` 有效审核为 pass，controller `close_check` 返回 `close_allowed=true`。相关 change 已完成并归档，replacement 结果不改写旧 lineage。
- **已定义：接口/目标架构**：`define-backend-engineering-architecture` 已完成 7/7 tasks；目标后端目录、模块所有权、分层依赖、Unit of Work/Repository/Outbox、HTTP/Temporal/Worker 边界、测试分层和阶段 0 迁移规则已同步到 Markdown 与技术实施方案 DOCX。目标架构仍按切片逐步迁移。
- **待实现：产品能力**：真实 Provider/TOS、AgentScope、FFmpeg、SSE、Outbox、完整生成/音频/媒体链路、专业剪辑、协作、移动端和平台发布仍未实现。
- 当前 Compose 环境保持运行且七类服务 healthy：Web `http://127.0.0.1:5174`、API `http://127.0.0.1:8000/v1/health/ready`。`5173` 被本项目外监听者占用，因此本项目使用 `5174`。
- API 使用 PostgreSQL JSONB 文档列；Alembic head 文件为 `0006_assets_legacy_storage_repair.py`，revision ID 为 `0006_assets_legacy_repair`（25 个 ASCII 字符，兼容默认 `alembic_version.varchar(32)`）。`0005` 为已应用 `0004` 数据库补齐 AssetVersion 项目归属组合外键和十六进制 hash 约束；`0006` 将合法 `workspace://` URI 的 helper 返回 canonical key 持久化到 `object_key`，普通 legacy provider/bucket 保持不变。Temporal 使用同一 PostgreSQL 容器中的独立数据库/用户，业务代码不查询 Temporal 表。

## 已完成验证

- replacement workflow 验证与关闭门均已通过：OpenSpec apply/status/strict、`pnpm run check`（Contracts 32、Web 2、Python 214）、`git diff --check`、Compose config/`up --build -d --wait` 与七服务 health 均通过。真实 PostgreSQL 已再次验证 `0005 -> 0006 -> 0005 -> 0006` 并回到 `0006_assets_legacy_repair`；HTTP smoke 为 health 200、合法 Asset/AssetVersion create 201/read 200、`workspace:projects/a/v1.wav` 与 `projects/replacement?` 均为 422，且 `contentHash`/`checksum` 独立、响应无媒体 bytes。源码和当前 Compose 日志扫描只命中本地 health probe、显式未配置 TOS adapter 与测试脱敏字段，没有真实 Provider/TOS 或付费调用。独立 `sol_xhigh` 覆盖 A1-A7 后有效 pass，claim 为 `bab28b17-a3f6-4fbb-a954-8f92c1a3f2df`；controller `close_check` 返回 `close_allowed=true` 并释放 workspace lease。
- Alembic 已在临时和 Compose PostgreSQL 验证 `0001 -> 0005`、`0005 -> 0004 -> 0005` 以及修复后的 `0005 -> 0006 -> 0005 -> 0006`；最终 head 为 `0006_assets_legacy_repair`。Compose 真实 HTTP smoke 已创建 Project/Asset/AssetVersion，确认独立 contentHash/checksum、无媒体 bytes；七类服务均 healthy。当前 Compose 不自动执行 migration，新空卷启动前仍须显式执行 `alembic upgrade head`。
- Compose API 真实 HTTP 已验证：health ready 200、项目/剧集创建成功、重复编号 409、正确 `If-Match` 更新 revision、过期 revision 409。
- API 镜像的无缓存 frozen lock 构建通过；隔离副本中的 `pyproject.toml`/`uv.lock` 漂移会在 `uv lock --check` 处失败。
- API 与三类 Worker 共用严格 runtime composition；示例配置实际选择 Mock Provider/LocalWorkspaceAdapter，未知模式显式失败，Provider/Storage 边界日志及 Skill manifest 严格准入均有正反测试。
- `docker compose --env-file .env.example -f infra/compose/compose.yaml config`、镜像 build、`up --wait`、API/Web HTTP 探测均通过。
- Assets HTTP smoke 已通过：health 200、OpenAPI 使用 `{projectId}/{assetId}/{versionId}`、创建/读取 AssetVersion 保持独立 `contentHash` 与 storage `checksum`、missing asset 404；响应无媒体 bytes。
- 范围扫描仅发现健康探针、Schema `$id` 和显式未配置 TOS 测试引用；没有真实 Provider/TOS 客户端或付费调用。
- 后端架构文档变更的 DOCX ZIP/关键段落/表格几何/a11y 结构检查通过；当前 Windows 环境没有 `soffice`，因此未完成 PNG 渲染和逐页视觉 QA。

## 待确认

- 真实 Provider、TOS、加密服务、生产密钥与远程部署仍需后续单独 OpenSpec change。
- Compose 使用开发数据库凭据，仅限本地环境；不得作为生产密钥或配置复用。
- Compose readiness 只验证数据库连通性；新空卷不会自动执行 Alembic，部署/启动脚本接管 migration 需由后续独立 change 固化。
- 旧 `sol_max_closure` 的产品 blocker 已在独立 `repair-assets-object-key-contract` 中实施：Schema、domain、HTTP 与 `0004`/`0006` 通过共享 corpus 一致拒绝 `assets/a/`、Python whitespace、`workspace:`/`s3:`、空 query/fragment delimiter 及 `C:relative.mp4`；历史 migration 不导入可演进 runtime helper，且不以 SQL `trim()` 重解析 raw value。旧 controller lineage 仍不得作为关闭依据并保持 `scope_decision_required`；该 change 的完成依据是已通过的 replacement lineage。
- `sol_max_closure` 的 controller blocker：旧 task `assets-asset-versions-slice-v2` 缺少完整 claims、repair records 和 max charter，不能声明 `close_allowed`。root 读取 audit-context 时可见空记录，而 closure reviewer 的实例返回 `LEGACY_STATE_MIGRATION_REQUIRED`；两条证据都不支持关闭。不得删除、迁移、read-through 或伪造该 state。
- 既有 `projects/episodes` OpenAPI 仍暴露 snake_case 路径参数，属于本轮 assets 之外的后续契约修复范围。
- 下一步按目标架构处理 `workflows/runs/timelines` 和 `providers/skills/usage`；每个切片先更新对应 OpenSpec、测试和项目记忆。不要一次性重构全部阶段 0 平铺代码。

此文件只保留可继续执行的当前状态；任务完成后应替换过期内容，而非追加流水账。
