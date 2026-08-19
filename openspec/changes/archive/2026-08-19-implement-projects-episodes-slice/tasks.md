## 1. 领域模型与测试先行

- [x] 1.1 先写 Project/Episode 领域行为的失败测试：非空字段、状态、稳定 ID、初始 revision、父级归属和编号规则。
- [x] 1.2 实现不依赖 FastAPI/SQLAlchemy 的 Project/Episode entities、状态值对象和领域错误。
- [x] 1.3 先写 revision 更新与冲突的失败测试，再实现显式 update 行为和不可静默覆盖规则。

## 2. Application、Repository 与 Unit of Work

- [x] 2.1 先写 application command/query 的内存 adapter BDD/TDD 测试，覆盖创建、读取、列表、父级不存在和重复编号。
- [x] 2.2 定义 Project/Episode Repository 与 Unit of Work Protocol，明确一个 command 的事务边界。
- [x] 2.3 实现 application command/query services 与共享状态的 in-memory adapter，确保 domain/application 不导入 FastAPI 或 SQLAlchemy。
- [x] 2.4 补齐 `project_not_found`、`episode_not_found`、`episode_number_conflict`、`revision_conflict` 的稳定错误对象和测试。

## 3. SQLAlchemy 持久化与迁移

- [x] 3.1 先写 ORM/adapter 契约测试，覆盖 Episode `title`、父级过滤、确定性排序和并发更新。
- [x] 3.2 为 Episode ORM 增加 `title` 映射，保持既有 `display_number` 数据库列兼容，并建立 SQLAlchemy Repository/UoW adapter。
- [x] 3.3 新增 `0003_projects_episodes_slice` migration：回填 title、增加正数检查和 `(project_id, display_number)` 唯一约束，提供可逆 downgrade。
- [x] 3.4 在 SQLite 单元环境和 PostgreSQL Compose 环境验证 `upgrade head`、downgrade 与重复编号约束。

## 4. HTTP 接口与契约

- [x] 4.1 先写 Pydantic request/response 契约测试，验证 camelCase、Schema 字段、422 和 `If-Match` 解析。
- [x] 4.2 实现 projects/episodes FastAPI router、依赖注入和领域错误到 404/409/422/503 的稳定映射。
- [x] 4.3 增加注入 in-memory UoW 的 HTTP BDD 测试，覆盖创建、读取、列表、更新冲突和无数据库配置边界。
- [x] 4.4 保持 health/live、health/ready、runtime composition 和现有 Mock/Local 行为不变，并加入回归断言。

## 5. 架构质量门与项目记忆

- [x] 5.1 增加架构依赖测试：domain 不依赖 FastAPI/SQLAlchemy，application 不依赖具体 adapter，interfaces 不直接访问 Session。
- [x] 5.2 更新 `docs/phase-zero-traceability.md`，新增 projects/episodes 切片的 DDD/BDD/SDD/TDD 证据和当前非目标。
- [x] 5.3 更新 `docs/agent/PROJECT.md`、`docs/agent/HANDOFF.md` 和必要的 `DECISIONS.md`，明确阶段 0 已实现、目标架构已定义、本 change 的实际状态和下一切片。
- [x] 5.4 运行 OpenSpec apply instructions、定向测试、完整 `pnpm run check`、Alembic/Compose 验证，并同步勾选已完成 tasks。
