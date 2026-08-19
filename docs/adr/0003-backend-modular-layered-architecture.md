# ADR-0003：后端采用模块化单体与分层端口架构

- 状态：已接受
- 日期：2026-08-18

## 背景

阶段 0 已建立 FastAPI、PostgreSQL、Temporal、三类 Worker、共享 Schema 和 Provider/Storage Port，但 API 仍是用于验证边界的最小平铺结构。后续多集短剧、素材编辑、时间线和真实 Provider 会显著扩大业务复杂度，需要先固定模块所有权、依赖和事务规则。

## 决策

FastAPI 保持模块化单体，不拆全微服务。目标代码按业务模块组织，每个复杂模块分为 `domain`、`application`、`infrastructure` 和 `interfaces`；依赖方向为 `interfaces -> application -> domain`，基础设施通过 Port 向内实现并由 composition root 注入。

一个 application command 对应一个 Unit of Work。Repository 接口位于内层，实现位于基础设施；业务变更和 Outbox 事件在同一 PostgreSQL 事务提交。FastAPI 只承担 HTTP 适配，Temporal Workflow 只承担确定性编排，所有网络、数据库、TOS、AgentScope、Provider 和 FFmpeg 副作用由 Activity/Adapter 执行。SSE 从持久化事件流补发，进程内队列不作为事实源。

阶段 0 的 `app.py`、`db.py`、`domain/`、`ports/` 和 `skills/` 是迁移起点，不视为目标分层已经完成。后续按垂直功能切片迁移，旧入口只做临时兼容委派，不长期复制业务规则。

## 后果

- 后续后端 change 必须声明模块所有权、application 入口、事务和测试边界。
- 领域代码不得依赖 FastAPI、SQLAlchemy、Temporal、AgentScope、FFmpeg 或供应商 SDK。
- Worker 与 HTTP 路由复用 application service/Port，不各自实现业务规则。
- 需要增加架构依赖检查、Outbox/事务集成测试和 Temporal 确定性测试。
- 目录数量会增加，但单体部署和本地 Docker Compose 形态保持不变。
