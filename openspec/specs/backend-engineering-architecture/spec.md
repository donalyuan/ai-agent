# backend-engineering-architecture Specification

## Purpose
TBD - created by archiving change define-backend-engineering-architecture. Update Purpose after archive.
## Requirements
### Requirement: 模块化分层与业务所有权
后端 SHALL 以业务模块组织 FastAPI 模块化单体。每个业务模块 SHALL 明确 domain、application、infrastructure 和 interface 责任；依赖 SHALL 指向领域内层，domain SHALL 不依赖 Web、ORM、Workflow 引擎、媒体工具或供应商 SDK。

#### Scenario: 新增后端业务能力
- **WHEN** 开发者为项目、剧集、素材、工作流、时间线、Provider、Skill、审核、用量或导出新增行为
- **THEN** 该行为归属一个明确业务模块，并通过 application handler 进入领域；架构检查能拒绝从 domain 到 FastAPI、SQLAlchemy、Temporal 或供应商 SDK 的依赖

### Requirement: 应用命令与事务边界
每个写入用例 SHALL 由一个 application command handler 协调，并以一个 Unit of Work 作为事务边界。Repository 接口 SHALL 定义在内层边界，实现 SHALL 位于 infrastructure；领域状态与待发布事件 SHALL 在同一 PostgreSQL 事务提交。

#### Scenario: 提交带领域事件的写入
- **WHEN** command handler 成功修改聚合并产生领域事件
- **THEN** 聚合修改与 Outbox 记录在同一事务提交，事务失败时二者均不生效，外部网络、对象存储和媒体副作用不在该事务内执行

### Requirement: HTTP 接口保持薄适配层
FastAPI 路由 SHALL 仅处理传输层职责并调用 application handler。路由 SHALL 不直接执行 SQLAlchemy 查询、Provider/Storage SDK、AgentScope 或 FFmpeg；领域和依赖错误 SHALL 映射为稳定、可诊断的 HTTP 错误契约。

#### Scenario: 过期 revision 更新
- **WHEN** 客户端以过期 `revision` 或 `If-Match` 提交修改
- **THEN** application 层返回冲突结果，HTTP 层映射为 `409`，并返回稳定 `error_code`、`trace_id` 和允许公开的最新版本摘要

### Requirement: Temporal 与 Worker 副作用隔离
Temporal Workflow SHALL 只执行确定性编排。所有网络、数据库、文件、Provider、AgentScope、TOS 和 FFmpeg 操作 SHALL 位于 Activity 或其调用的 adapter；Worker SHALL 复用 application service 和 Port 实现，不复制领域规则。

#### Scenario: 执行视频生成节点
- **WHEN** 已发布工作流运行到视频生成节点
- **THEN** Workflow 以稳定幂等键调度 Generation Activity，Activity 通过 `VideoGenerationPort` 执行副作用并持久化统一结果，Workflow 代码不直接导入供应商 SDK 或访问数据库

### Requirement: 持久化实时事件
运行和素材编辑事件 SHALL 先持久化并获得单调序号。SSE SHALL 支持 `Last-Event-ID` 补发；进程内队列 MAY 用于降低推送延迟，但 SHALL NOT 成为事件事实源。

#### Scenario: SSE 断线恢复
- **WHEN** 浏览器携带最后已确认事件 ID 重新连接
- **THEN** API 从持久事件流补发所有后续可见事件，API 进程重启不会导致已提交事件永久丢失

### Requirement: 集中配置和依赖注入
运行配置 SHALL 由 bootstrap 读取并在 composition root 装配。业务模块 SHALL 不直接读取环境变量、Docker Secret 或创建外部客户端；Provider、Model 和 Storage 选择 SHALL 继续由数据配置驱动。

#### Scenario: 测试替换外部实现
- **WHEN** application handler 在单元测试中运行
- **THEN** 测试可以注入 fake Repository、UnitOfWork 和 Provider/Storage Port，且无需启动 FastAPI、PostgreSQL、Temporal 或真实网络客户端

### Requirement: 分层测试与渐进迁移
后端 SHALL 提供 domain、application、adapter、integration、architecture、contract 和 BDD 测试边界。阶段 0 平铺实现 SHALL 被标记为迁移起点；后续 SHALL 按功能切片迁移，且不得长期维护两份同义业务规则。

#### Scenario: 迁移一个现有功能切片
- **WHEN** 后续 OpenSpec change 将现有功能迁入目标模块
- **THEN** 先增加失败的领域/应用/架构测试，再迁移 handler、Repository、adapter 和路由；旧入口仅兼容委派，契约和迁移变化另行声明
