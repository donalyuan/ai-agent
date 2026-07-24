## Context

Novex 当前由 Rust API、PostgreSQL 领域数据、Python 视频 Worker、Admin 和 Video Agent 前端组成。`crates/novex-agent` 已实现业务 Adapter Registry、ExecutionContext 与 Run 生命周期，但 Tool、Memory、Eval crate 仍是边界占位，通用流式 Tool Loop、steering、会话树和上下文压缩尚未实现。

产品长期定位现已确认为本地单用户个人 AI 工作台，后续除视频外还会承载编程、知识研究和其他个人工作流。Pi 0.81.1 已提供稳定的 Agent Harness、Tool Loop、SQLite Session Storage 和 Node Execution Environment，适合作为通用执行引擎。现有视频领域状态、模型后台和付费任务安全规则仍是仓库事实，不能由 Pi 会话替代。

## Goals / Non-Goals

**Goals:**

- 交付一个可通过 HTTP/SSE 使用、重启后会话可恢复的本地 Pi Agent Runtime。
- 让 Runtime 按 `model_id` 读取 PostgreSQL `ai_models`，支持已存在的 OpenAI Responses 与 Chat Completions 文本配置。
- 提供 read/write/edit/bash 本地工具、流式运行事件、steer/follow-up/abort/compact/fork 控制面。
- 明确 Pi Session、正式长期 Memory 和领域状态三者的所有权。
- 保持现有视频 Agent API、Rust Adapter、Run/Step、模型选择和视频任务行为不变。

**Non-Goals:**

- 本 change 不迁移现有 video-agent 页面和对话 API到 Pi Runtime。
- 不建设多租户、RBAC、插件市场、远程托管或对外客户交付能力。
- 不让 Pi SQLite 保存脚本、素材、作品、发布记录或正式长期 Memory。
- 不引入 `pi-coding-agent` CLI/TUI 作为通用工作台 UI。
- 不执行真实视频生成、平台发布或其他可能产生费用的验证。

## Decisions

### 1. Pi Runtime 是独立 Node.js 服务

新增 `services/agent-runtime`，直接依赖精确版本的 `@earendil-works/pi-agent-core`、`@earendil-works/pi-ai` 与 `@earendil-works/pi-storage-sqlite-node`。Node.js 24 提供正式 `node:sqlite` 支持；服务通过顶层 Compose 运行，并暴露本地 HTTP/SSE API。

选择独立服务而非把 Pi 塞入 Next.js，是因为 Agent 运行需要长连接、取消、进程级会话协调和稳定数据卷。选择 SDK/Harness 而非启动 `pi --mode rpc` 子进程，是为了使用结构化 Session Repo、模型配置和 Tool Policy API，避免每会话一个 CLI 进程。

### 2. PostgreSQL 模型注册表保持唯一事实源

Runtime 使用只读 `ModelConfigRepository` 按请求 `model_id` 查询启用的 `text` 模型，映射为 Pi `Model` 与动态 Provider。凭据只在内存中传给 Pi provider，不出现在 HTTP 响应、日志、SQLite metadata、消息或事件中。

服务只支持仓库已经正式支持的 `openai_responses` 和 `openai_chat_completions`。未提交模型、模型停用、类型/协议不匹配均稳定失败，不回退环境变量或 Pi 内建目录。模型非敏感快照写入 Session 自定义 entry，便于本地回放。

备选方案是让 Runtime 调用 Rust 通用 LLM HTTP endpoint，但当前 `novex-model::LLMClient` 只返回业务字符串，不能保真传递 Pi Tool Call 和流式事件；本 change 不再建设一套中转协议。

### 3. Pi SQLite 只拥有 Agent Session

SQLite 数据库持久化 Pi Session Tree、活动 leaf、消息、工具调用结果、model change、compaction 和 branch summary。PostgreSQL 继续拥有项目、脚本、素材、作品、模型和发布等领域事实。正式长期 Memory 后续使用独立策略存储，Pi compaction summary 不自动升级为 Memory。

每个会话 metadata 保存 `model_id`、工作目录、工具 profile 和创建来源，但不保存凭据。服务重启后通过 Session Repo 重新打开会话并按当前请求重新解析启用模型；历史模型快照不被覆盖。

### 4. Runtime API 使用命令端点和 SSE 事件

API 提供健康检查、会话创建/列表/详情/删除/分支、entries 增量读取，以及 prompt、steer、follow-up、abort、compact 命令。`prompt` 返回 SSE：先发送 `run_started`，再转发 Pi Harness 事件，最后发送 `run_completed` 或脱敏后的 `run_failed`。

同一会话同一时间只允许一个活动 run；进程内 Session Coordinator 管理 Harness 与 AbortController。重启后不存在伪造的 running 状态，客户端通过持久化 entries 恢复上下文。

### 5. 本地工具是显式 profile，不是领域写入通道

`workspace` profile 启用 Pi 的 read/write/edit/bash，执行目录默认为配置的 workspace root；`chat` profile不启用本地执行工具。HTTP 请求不能任意注入工具实现。

Pi `0.81.1` 的 npm 发布包包含公开 `AgentTool`、`ExecutionEnv` 和 `NodeExecutionEnv`，但未包含同版本源码仓库中的四个 Harness tool factory。Runtime 因此基于这些公开接口提供同名 `read/write/edit/bash` 适配器，并以合同测试锁定 profile、参数和执行语义；不引入 `pi-coding-agent` CLI/TUI，也不访问未导出的包内路径。升级 Pi 时必须复核并优先切回上游公开 factory。

视频生成、发布、删除正式领域数据等能力未来必须作为调用 Rust Backend 的类型化领域 Tool，并继续执行各领域既有确认、幂等和资源限制；不得通过 bash 绕过正式工作台状态规则。

### 6. Rust Kernel 与 Pi Harness 分层共存

现有 Rust Kernel 暂时维持视频业务 Adapter 与 Run/Step 外部行为。新的通用 Agent 工作台统一走 Pi Runtime，不在 Rust crate 中再实现同类 Turn/Tool Loop。后续迁移某个领域 Agent 时，先建立独立 change，把其业务能力暴露成类型化 Tool，再删除对应重复执行路径，不保留长期双写。

### 7. 产品方向与文档拆分

通用 memory 记录 local-first 个人 AI 工作台定位、Pi Runtime 和多领域应用边界；Video Agent 的 `ProductionOrchestrator`、角色和 ProductionState 留在视频主题 memory。`ARCHITECTURE.md` 删除未确认的客户交付/多租户必选目标，但保留模块边界、模型唯一来源、运行快照和外部付费动作安全规则。

## Risks / Trade-offs

- [引入 Node.js Agent Runtime，偏离原 Rust-only 执行内核] → 明确服务边界，Rust 继续拥有领域与任务，Pi 只拥有通用 turn/tool/session；避免双实现。
- [Pi 上游快速迭代产生破坏性变更] → npm 精确锁定 `0.81.1`、提交 lockfile、添加合同测试，升级必须独立 change。
- [SQLite 与 PostgreSQL 双存储增加备份范围] → 固定 SQLite 数据目录与命名卷，文档明确两类数据所有权，不跨库事务双写。
- [本地 bash/write 造成误操作] → 工具 profile 显式选择、工作目录固定、结构化事件完整保留；付费和正式发布不作为通用 bash Tool 暴露。
- [模型凭据从 PostgreSQL 进入 Node 进程] → 仅进程内使用，统一敏感字段过滤，测试确保 HTTP/日志/SQLite 不出现凭据。
- [服务中断时 SSE 客户端丢失尾部事件] → 消息和工具结果先由 Pi Session Storage 持久化，客户端可用 entries 游标恢复。

## Migration Plan

1. 新增 Runtime 服务、依赖锁、SQLite 数据目录、单元/集成测试和 Compose 健康检查。
2. 在 fake provider 下验证 Tool Loop、SSE、取消、分支与重启恢复；真实模型只做配置解析合同测试，不产生外部费用。
3. 更新架构、memory、环境示例和运行说明，将 Pi Runtime 标记为新工作台的统一入口。
4. 保持现有 Video Agent 全部路由不变；后续按领域建立迁移 change。

回滚时停止并移除 Agent Runtime Compose 服务即可，现有 Rust/视频行为不受影响；SQLite 数据卷保留，恢复服务后仍可读取。

## Open Questions

无。当前 change 已确认 local-first 单用户定位、Pi 作为通用执行引擎及视频领域暂不迁移的边界。
