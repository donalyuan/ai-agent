## 1. Runtime 工程与依赖

- [x] 1.1 创建 `services/agent-runtime` TypeScript 工程、Node 24 Dockerfile、精确 Pi/数据库依赖与 lockfile
- [x] 1.2 建立配置解析、结构化错误、敏感信息脱敏和进程生命周期基础模块

## 2. 模型与本地持久化

- [x] 2.1 实现 PostgreSQL `ai_models` 只读解析与 OpenAI Responses/Chat Completions Pi Provider 映射
- [x] 2.2 实现 Pi SQLite Session Repo 初始化、会话 metadata、列表、打开、删除、entries 游标和 fork
- [x] 2.3 实现不含凭据的模型快照 entry，并验证凭据不进入 Session 或 API 响应

## 3. Pi Agent Runtime

- [x] 3.1 实现 Session Coordinator、Pi Agent Harness 装配与 `chat`/`workspace` 工具 profile
- [x] 3.2 实现 prompt SSE 事件流、同会话互斥和唯一运行终态
- [x] 3.3 实现 steer、follow-up、abort、compact、tree move 与 fork 控制命令
- [x] 3.4 实现 `/health`、`/ready` 和优雅关闭，分别检查 PostgreSQL 与 SQLite

## 4. 环境与架构定位

- [x] 4.1 将 Agent Runtime、持久化数据卷、端口和依赖加入项目及顶层 Docker Compose
- [x] 4.2 更新 `.env.example`、README 和运行说明，记录本地 Runtime API、数据路径和无费用验证方式
- [x] 4.3 更新 `MEMORY.md`、主题 memory 与 `ARCHITECTURE.md`，固化本地单用户多领域工作台和 Pi/Rust/视频边界

## 5. 测试与验证

- [x] 5.1 添加模型解析、脱敏、工具 profile 和稳定错误单元测试
- [x] 5.2 添加 SQLite 重启恢复、entries 增量读取、fork 和 compaction 合同测试
- [x] 5.3 添加 fake provider 下 SSE Tool Loop、并发拒绝、steer/follow-up/abort 运行测试
- [x] 5.4 在容器中运行 Agent Runtime lint/build/test、Compose 配置检查及 Rust/视频直接相关回归测试
- [x] 5.5 重新执行 OpenSpec apply instructions，确认全部任务与实际实现一致并达到 `all_done`
