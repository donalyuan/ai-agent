# 长期决策

| 状态 | 决策 | 说明 |
| --- | --- | --- |
| 已接受 | [ADR-0001：采用 Git 与 Markdown 项目记忆](../adr/0001-use-git-markdown-project-memory.md) | 第一阶段以版本控制文档维护项目记忆，不引入服务。 |
| 已接受 | [ADR-0002：阶段 0 工程边界与本地运行形态](../adr/0002-phase-zero-foundation-boundaries.md) | 单仓库、共享 Schema、Mock/Local 默认与 Compose 本地运行基线。 |
| 已接受 | [ADR-0003：后端采用模块化单体与分层端口架构](../adr/0003-backend-modular-layered-architecture.md) | 目标结构按业务模块分层，Command/UoW/Outbox 与 Temporal/Worker 副作用边界固定；阶段 0 代码按功能切片迁移。 |

新增长期决策时，先创建或更新 ADR，再在此添加简短索引。当前架构与可执行实现优先于本索引。
