# 阶段 0 追溯

状态标记统一为：**已实现：阶段 0**（当前可执行代码和验证证据）、**已定义：接口/目标架构**（契约、边界或迁移目标，不代表全部代码已完成）、**待实现：产品能力**（明确留给后续 change 的范围）。

| 需求 | OpenSpec capability | 实现边界 | 验证 |
| --- | --- | --- | --- |
| R1 | `local-engineering-runtime` | 根 `pnpm` workspace、`apps/web`、`services/api`、`workers/*`、`infra/compose`、固定 `uv` 与 lock 一致性检查 | `pnpm install --frozen-lockfile`、无缓存 Docker build、`pyproject.toml`/`uv.lock` 漂移负例、`docker compose ... config` |
| R2 | `versioned-domain-contracts` | `packages/contracts/schemas` 与有效/无效样例 | `pnpm --filter @video-agent/contracts test`（32 项） |
| R3 | `versioned-domain-contracts` | SQLAlchemy adapter 模型、strict `WorkflowDraft` revision、Alembic `0001` 与增量 `0002` 的版本字段、默认值和约束 | PostgreSQL `0001 -> 0002 -> 0001 -> 0002`、最小版本记录事务、Alembic offline SQL、`pytest` |
| R4 / R6 | `provider-and-storage-boundaries` | 六个 Port、Mock、ProviderCatalog、RuntimeSettings composition、LocalWorkspaceAdapter、显式 TOS 未配置和边界日志 | `test_foundation.py`、`test_runtime_composition.py` |
| R5 | `skill-routing-foundation` | 严格 SkillRegistry、重复版本拒绝、SkillRouter、可选 semantic adapter | `test_foundation.py`、`test_skill_registry_validation.py` |
| R7 | `local-engineering-runtime` | Compose 七服务、API/Worker 共用 Mock/Local composition、健康检查、JSON 边界日志、无密钥默认配置 | `docker compose ... up --wait`、HTTP health 探测、runtime 正反测试 |
| R8 | `foundation-quality-gates` | 根 `check` 命令、Schema/ORM/Port/Worker/runtime/Skill 准入测试 | `pnpm run check`（contracts 32、Web 2、Python 214）、Alembic offline SQL、`git diff --check` |

## `projects/episodes` 垂直切片

| 证据层 | 已实现内容 | 验证 |
| --- | --- | --- |
| DDD | 纯 Python `Project`/`Episode` entities、稳定领域错误、状态/schema version/revision 和显式更新行为 | `services/api/tests/test_projects_episodes_domain.py` |
| Application/BDD | Command/query service、Repository/UoW Protocol、内存 adapter、父级归属、重复编号和 revision 冲突 | `services/api/tests/test_projects_episodes_application.py` |
| Persistence/SDD | SQLAlchemy adapter Repository/UoW、Episode `title` 映射、`0003_projects_episodes_slice` 回填/正数检查/项目内唯一约束 | `services/api/tests/test_projects_episodes_sqlalchemy.py`、`test_projects_episodes_migrations.py`；PostgreSQL `0002 -> 0003 -> 0002 -> 0003` 已验证；Compose 自动 migration 待后续 change |
| HTTP/契约 | 独立 camelCase DTO（`schemaVersion` 到共享 Schema `schema_version` 的显式边界映射）、项目/剧集查询与更新、`If-Match`、404/409/422/503 稳定映射 | `services/api/tests/test_projects_episodes_http.py`；Compose API HTTP 验证 |
| 架构质量门 | domain 不导入 FastAPI/SQLAlchemy，application 不导入具体 adapter，interfaces 不访问 Session | `services/api/tests/test_architecture_boundaries.py` |

## `assets/asset-versions` 垂直切片

| 证据层 | 已实现内容 | 验证 |
| --- | --- | --- |
| DDD | framework-free `Asset`、`AssetVersion`、`StorageObject`；五种共享 kind、相对 objectKey、hash/MIME/size/media metadata 校验；版本只追加且不可原地更新 | `services/api/tests/test_assets_domain.py` |
| BDD/Application | create/get/list asset、append/get/list version command/query；项目归属、版本号从 1 递增、旧版本可读和稳定错误 | `services/api/tests/test_assets_application.py` |
| Persistence/SDD | SQLAlchemy Repository/UoW、显式 snake_case 映射、`0004_assets_asset_versions_slice` 的冻结 legacy canonicalization、`0005_assets_integrity_repair` 项目归属/十六进制 hash 约束，以及文件 `0006_assets_legacy_storage_repair.py`（revision ID `0006_assets_legacy_repair`，25 个 ASCII 字符）对完整 `workspace://` reference 的冻结修复；migration 参数化持久化 helper 返回值、不以 SQL 重解析 raw value，普通 legacy provider/bucket 保持不变；可逆迁移、`(asset_id, version_number)` 唯一保护 | `services/api/tests/test_assets_sqlalchemy.py`、`test_assets_migrations.py`；共享 corpus、SQLite 与 Compose PostgreSQL 均验证 head、`0005 -> 0006 -> 0005 -> 0006`、安全 reference 和版本号长度；HTTP smoke 与七服务 health 通过 |
| HTTP/契约 | `/v1/projects/{projectId}/assets`、`/v1/assets/{assetId}/versions` 等 create/get/list API；camelCase DTO 与 storage object reference 约束，404/409/422/503 映射 | `services/api/tests/test_assets_http.py` |
| 架构质量门 | domain 不导入 FastAPI/SQLAlchemy，application 不导入具体 adapter，HTTP 不访问 Session；`AssetVersion`、`StorageObject` 和嵌套 media 深度不可变；不接收媒体二进制 | `services/api/tests/test_assets_domain.py`、`test_assets_application.py`、`test_architecture_boundaries.py`、定向 pytest |

当前状态：**已实现：阶段 0** 的 assets 主体代码、深度不可变、标准 `workspace://` repair，以及 `repair-assets-object-key-contract` 对 Schema/domain/HTTP/`0004`/`0006` 的 canonical objectKey 修复已落地。共享 corpus 锁定 RFC opaque scheme、空 `?`/`#`、Python `strip()` whitespace、绝对/UNC/反斜杠、点段与 drive path 的一致拒绝；`0004`/`0006` 使用冻结 helper 并直接持久化其 canonical 返回值。`implement-assets-asset-versions-slice` 的 20/20 OpenSpec tasks、Contracts 32、Assets domain/HTTP/migration 133、全量质量门 Python 214、Compose PostgreSQL cycle 与 HTTP smoke 均已执行，完整证据见当前交接。旧 `sol_max_closure` 仍有效 fail，旧 lineage 保持 `scope_decision_required`，其 workflow-controller 缺少完整 claims、repair/max charter；用户授权的 claims 超集 replacement lineage `complete-assets-asset-versions-slice-v3` 已通过独立 `sol_xhigh` gate，controller `close_check` 返回 `close_allowed=true`。该 change 已通过 replacement lineage 完成收口并已归档，本轮没有删除、迁移或伪造旧 state。**已定义：接口/目标架构** 的下一阶段仍包括 audio/workflows/runs/timelines 等切片。既有 `projects/episodes` OpenAPI 路径参数命名残余风险不在本轮范围内；完整产品链路、真实 Provider/TOS 仍是**待实现：产品能力**。

该切片只实现项目/剧集与素材身份/版本边界；Scene、Shot、Workflow、Timeline、Provider、Skill、SSE、Outbox、真实外部调用和完整产品链路仍为**待实现：产品能力**，不得从当前路由或 ORM 基础表推断为已完成。

阶段 0 不包含真实 Provider/TOS 调用、付费调用、完整生成、专业剪辑、多人/手机端、发布/TikTok、多 Agent 产品能力或 semantic 模型。
