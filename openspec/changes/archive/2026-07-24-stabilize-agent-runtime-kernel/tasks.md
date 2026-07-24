## 1. 锁定现有行为基线

- [x] 1.1 在容器内运行现有 conversation、script、topic、sound、work Agent 相关测试并记录当前通过基线
- [x] 1.2 补充失败测试，锁定现有 HTTP 请求响应、错误映射、消息 metadata、Run/Step 类型与顺序以及模型快照语义
- [x] 1.3 补充模块边界失败测试，禁止 `novex-agent` / `novex-ai-core` 依赖 `backend`

## 2. 建立通用 Kernel 契约

- [x] 2.1 先为 `AgentKey` 合法性、相等性和稳定序列化编写失败测试，再在 `novex-ai-core` 实现通用值对象
- [x] 2.2 先为 Adapter 注册、重复注册、未知 Agent 和新增测试 Adapter 无须修改分派编写失败测试，再实现 `AgentAdapter` 与 `AgentRegistry`
- [x] 2.3 先为通用 `AgentInvocation`、`AgentExecutionContext`、`AgentOutcome` 和业务 payload 隔离编写失败测试，再实现对应契约
- [x] 2.4 先为成功、Adapter 失败、Assistant 保存失败和单次终态收尾编写失败测试，再实现 `AgentRunCoordinator`
- [x] 2.5 实现 Session Store、Run Recorder、Step Recorder 和模型执行引用 ports，并用 fake 实现完成 `novex-agent` Kernel contract tests

## 3. 接入 Backend 持久化与启动组装

- [x] 3.1 为 PostgreSQL Conversation/Run/Step 实现 Kernel ports，并验证现有 Domain 对象与数据库记录转换不改变字段语义
- [x] 3.2 先为重复 `AgentKey`、非法 key 和缺失必需依赖编写 Bootstrap 失败测试，再实现 Registry 启动组装
- [x] 3.3 将现有 HTTP DTO 转换为通用 invocation envelope，并让业务专属参数只进入对应 payload
- [x] 3.4 更新 ConversationService 调用 `AgentRunCoordinator`，保持模型解析、错误到 HTTP 映射和公开 API 不变

## 4. 迁移业务 Adapter

- [x] 4.1 将脚本生成与分镜修改迁移为 `ScriptAgentAdapter`，使用强类型 payload 并保持现有 Prompt、消息和 Step 语义
- [x] 4.2 将普通选题、补充选题、质量闸门和有限重写迁移为 `TopicAgentAdapter`，保持批次、质量报告和同模型规则
- [x] 4.3 将声音推荐迁移为 `SoundAgentAdapter`，保持声音编辑快照、目录校验和不执行付费 TTS 的规则
- [x] 4.4 将作品方案与草稿修改迁移为 `WorkAgentAdapter`，保持确认 Gate、结构化 patch 和下游不调用规则
- [x] 4.5 为四个 Adapter 运行统一 contract suite，验证 key、payload 未知字段拒绝、依赖范围和错误分类

## 5. 删除旧入口与结构债务

- [x] 5.1 删除旧 `AgentRuntime` 业务分派、`Option<Repository>` 注入和公共请求中的业务专属字段
- [x] 5.2 删除旧 Runtime 公共路径和兼容导出，更新生产代码与测试到唯一新路径
- [x] 5.3 静态检查 Kernel 不包含 `topic/script/sound/work` 业务分支，基础 crates 不引用 Axum、SQLx 或 backend 模块

## 6. 全量验证与状态同步

- [x] 6.1 在容器内运行 Kernel、conversation、script、topic、sound、work 的直接相关测试并修复全部回归
- [x] 6.2 在容器内运行 `cargo fmt --check`、`cargo build --workspace` 和 `cargo test --workspace`
- [x] 6.3 验证没有新增 migration、公开 API 差异、Prompt 差异或消息/Run/Step 审计差异
- [x] 6.4 执行 `openspec instructions apply --change "stabilize-agent-runtime-kernel" --json`，确认任务进度与实际实现一致

## 7. 收敛重复执行与 Run 生命周期

- [x] 7.1 先补充失败测试，锁定通用非会话 Run 生命周期的成功、业务失败、成功收尾失败和单次终态语义
- [x] 7.2 将 `AgentExecutor` 改为唯一正式执行门面，并让 `ConversationService` 与现有集成测试复用该门面
- [x] 7.3 抽取统一的 backend Agent 错误下转 helper，并抽取 Coordinator 失败收尾 helper
- [x] 7.4 实现通用 `RunLifecycleCoordinator`，迁移脚本生成、项目策略草稿和主题组评审的直接 Run 编排
- [x] 7.5 运行相关测试、`cargo fmt --check`、`cargo build --workspace`、`cargo test --workspace` 与静态重复检查
- [x] 7.6 再次执行 OpenSpec apply instructions，确认任务进度与实际实现一致
