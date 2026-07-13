## Why

Rust 后端已经按领域拆分 Repository，但 HTTP API、DTO 和 Agent 编排仍集中在少数巨型文件中，造成职责混杂、反向依赖和过大的回归范围。现在需要在继续扩展业务前建立稳定的后端内部模块边界，并在不改变外部行为的前提下完成结构迁移。

## What Changes

- 将 `backend` 重构为 `bootstrap`、`api`、`application`、`domain` 和 `repositories` 等明确层次。
- 按项目、选题、素材、对话、脚本、素材生成和模型管理拆分 API 路由、handler 与 DTO。
- 将脚本、选题和对话领域类型从 Agent/API 聚合文件迁入 `domain`。
- 将统一 Agent Runtime 拆分为脚本、选题生成、质量闸门和主题组评审模块，同时保留统一分派入口。
- 将 handler 中的业务编排迁入 Application Service，禁止 API 直接执行 SQL 或构造模型 Prompt。
- 更新所有仓库内调用方和测试使用新模块路径，不保留旧公共路径兼容层。
- 为模块职责、业务不变量、重试、幂等和复杂编排添加必要的 Rust 文档注释。
- 保持全部 HTTP API、数据库结构、模型调用语义和现有 OpenSpec 业务行为不变。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `novex-foundation-architecture`: 增加 Rust 后端内部的 API、Application、Domain、Agent Runtime 与 Repository 模块边界和依赖方向要求。

## Impact

- 主要影响 `backend/src/lib.rs`、`backend/src/agents/`、`backend/src/repositories/`、路由 DTO 和后端集成测试的 Rust 模块路径。
- 不改变 HTTP URL、方法、状态码、请求响应字段和错误协议。
- 不新增数据库 migration，不改变 SQL 语义和数据生命周期。
- 不改变 `crates/novex-model` 的调用协议，也不把当前业务型 Runtime 强行迁入 `crates/novex-agent`。
- 进行中的 `script-to-asset-generation` change 保持独立，其规格和任务状态不受本变更影响。
