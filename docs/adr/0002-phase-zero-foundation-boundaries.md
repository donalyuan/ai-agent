# ADR-0002：阶段 0 工程边界与本地运行形态

- 状态：已接受
- 日期：2026-08-18

## 决策

阶段 0 使用单仓库 `pnpm` workspace 管理 Web、共享契约与最小 UI 包；Python API 使用 `uv` 与 Python 3.12。跨层领域文档以 `packages/contracts` 的 Draft 2020-12 JSON Schema 为权威来源，API 使用 camelCase Pydantic 边界消费共享样例。

本地运行使用 Docker Compose 启动 Web、API、PostgreSQL、Temporal 和三类 Worker。业务 PostgreSQL 与 Temporal 数据库/用户隔离；默认只启用 Mock Provider 与 LocalWorkspaceAdapter，不接入真实 Provider 或 TOS。

## 后果

- 后续不兼容的领域文档变更必须通过新的 OpenSpec change 扩展版本和迁移路径。
- Web/API/Worker 可用同一锁定版本库与本地 Compose 验证。
- 本阶段不提供生产部署、真实凭据或媒体生成能力。
