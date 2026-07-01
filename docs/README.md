# Novex Docs

本目录承载 Novex AI Agent 基座的架构、实施计划、需求文档、项目记忆和交付手册。

当前长期架构基准见根目录 [`ARCHITECTURE.md`](../ARCHITECTURE.md)。后续新增模块、服务或业务应用时，默认先判断其归属：

- 控制面 API：`backend/`
- 管理后台：`admin/`
- 业务应用：`apps/*`
- 可复用 Rust AI 基座能力：`crates/*`
- Python sidecar / runtime：`services/*`
- 部署与环境：`infra/`
- 客户交付模板：`templates/`

## 文档入口

- [项目记忆](memory/README.md)：长期偏好、稳定规则、历史决策和跨会话背景
- [需求文档](requirements/README.md)：video-agent 完整需求、MVP 边界和数据库设计
- [OpenSpec 工作区](../openspec/)：规格、变更和归档入口，保留在仓库根目录以兼容 OpenSpec CLI
