# Change：定义后端工程架构

## Why

现有技术方案已确定 FastAPI、Temporal、三类 Worker、Provider Port 和 PostgreSQL，但尚未定义可直接指导编码的后端目录、分层依赖、模块所有权、事务边界和测试责任。若直接进入下一阶段，API、Workflow、Activity 和 Adapter 容易重复业务规则，阶段 0 的平铺代码也缺少一致的迁移目标。

## What Changes

- 将后端目标形态确定为“模块化单体 + Ports/Adapters + 领域分层”，明确 `interfaces -> application -> domain` 的依赖方向。
- 定义业务模块所有权、模块内目录模板、共享内核边界，以及跨模块只通过应用接口、稳定 ID 或领域事件协作的规则。
- 定义 Repository、Unit of Work、Outbox、领域事件、异常映射、配置和依赖注入约束。
- 定义 FastAPI、Temporal Workflow、Activity 与三类 Worker 的调用关系和副作用边界。
- 定义单元、应用、适配器、契约、集成、架构和 BDD 测试的目录与责任。
- 明确阶段 0 当前平铺实现是迁移起点，不宣称目标分层已经实现；后续按功能切片渐进迁移。

## Capabilities

### Added Capabilities

- `backend-engineering-architecture`：后端模块、分层、依赖、事务、执行和测试的可实施约束。

### Modified Capabilities

- 无。

## Impact

本变更只更新技术架构、技术实施方案、ADR、文档入口和项目记忆，不移动当前代码、不改变 API、Schema、数据库迁移或运行行为。后续新增后端功能必须以本变更为目标架构，并在各自 OpenSpec change 中完成对应迁移和测试。
