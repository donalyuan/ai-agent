# Proposal: 打通虚拟制作团队角色执行管道

## 概述

将 `establish-virtual-production-crew` 已搭建的骨架与现有 AI 基础设施（`PromptCompiler`、`AuditedModelExecutor`、`DefinitionRegistry`）完整打通，使 `POST /api/v1/production/productions/:id/roles/:role_key/execute` 能真实调用 AI 模型并产出结构化产物。

## 背景

上一个 change 完成了：
- 12张数据库表、完整 Repository 层
- 9个角色的 RoleDefinition YAML + Prompt 模板
- 6个 Gate 实现 + Orchestrator 骨架
- HTTP API 路由（`execute_role` 当前返回 `NOT_IMPLEMENTED`）

缺失的关键环节：`RoleExecutor.execute()` 内部没有真实的模型调用逻辑，角色的 `AgentDefinition` 尚未注册到 `DefinitionRegistry`。

## 目标

1. 为9个制作角色创建 `AgentDefinition` YAML，注册到现有 `DefinitionRegistry` 体系
2. 实现 `RoleExecutor.execute()`：从 ProductionState 读取 input artifacts → 装配 ContextCandidate → 编译 Prompt → 调用 AuditedModelExecutor → 解析/验证输出 → 写入 output artifacts + ModelCall 审计
3. `AppState` 新增 `production_orchestrator()` 方法，提供注入依赖
4. `execute_role` handler 接入真实执行逻辑
5. 集成测试覆盖 Producer 角色的端到端执行

## 非目标

- `execute_flow` 异步流程执行（顺序编排）
- Fast Lane AI 集成（仅搭好入口）
- Admin 前端制作项目管理界面
- Prompt 质量评测（EvalRun）
- 角色并行执行

## 成功标准

1. `POST /productions/:id/roles/producer/execute` 返回真实的 CreativeBrief 产物 + model_call_id
2. 生成的产物可通过 `GET /productions/:id/artifacts/creative_brief` 查询
3. 模型调用已审计，可通过 `GET /model-calls` 查看
4. 输入产物缺失时返回标准错误格式
5. `cargo test --workspace` 全量通过

## 风险

- `DefinitionRegistry` 要求完整的 `registry.json`，需要与现有构建脚本对齐
- 角色输出 schema 验证失败时的回退策略需明确（保存草稿 vs 全部失败）
- `AuditedModelExecutor` 需要 `FixedModelBinding`，要确定制作角色使用哪个 model_id

## 依赖

- 现有 `novex-agent`、`novex-ai-core` crate 接口（已稳定）
- `AppState` 中已有的 `definition_registry()`、`audited_model_executor()` 方法
- `biga-postgres` 数据库中已存在的 production crew 表
