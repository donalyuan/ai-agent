# local-agent-session-persistence Specification

## Purpose
TBD - created by archiving change integrate-pi-personal-agent-foundation. Update Purpose after archive.
## Requirements
### Requirement: Agent 会话必须使用 Pi SQLite Session Storage 持久化
Runtime SHALL 使用 Pi 官方 SQLite Session Repo 保存会话 metadata、树形 entries、活动 leaf 和物化索引，并 SHALL 使用稳定本地数据路径。

#### Scenario: 服务重启后恢复会话
- **GIVEN** 会话已保存消息和工具结果
- **WHEN** Runtime 进程重启并重新打开同一 SQLite 数据库
- **THEN** 会话 SHALL 可按原 `session_id` 打开
- **AND** 活动分支、消息顺序和工具结果 SHALL 保持不变

#### Scenario: SQLite 路径不可写
- **WHEN** Runtime 启动时无法创建或迁移 SQLite 数据库
- **THEN** 就绪检查 SHALL 返回失败
- **AND** Runtime SHALL NOT 退化为内存会话

### Requirement: Session entries 必须支持增量读取和树形导航
Runtime SHALL 提供按稳定 entry sequence 增量读取 entries、查看活动 leaf、移动分支和创建 fork 的能力。

#### Scenario: 客户端断线后增量恢复
- **GIVEN** 客户端已记录最后一个 entry sequence
- **WHEN** 客户端重新请求该 sequence 之后的 entries
- **THEN** Runtime SHALL 只返回新增 entries
- **AND** 返回顺序 SHALL 与持久化顺序一致

#### Scenario: 切换活动分支
- **GIVEN** 会话树中存在目标 entry
- **WHEN** 操作者把活动 leaf 移动到该 entry
- **THEN** 后续 Context SHALL 从该分支构建
- **AND** 其他分支历史 SHALL 保留

### Requirement: Context 压缩不得成为正式长期 Memory
Runtime SHALL 将 Pi compaction 和 branch summary 作为有损 Context 记录保存，完整原始 entries SHALL 继续保留，压缩摘要 SHALL NOT 自动写入正式 Memory。

#### Scenario: 长会话触发压缩
- **WHEN** 会话达到配置的 Context 阈值或操作者手动 compact
- **THEN** Runtime SHALL 追加 compaction entry
- **AND** 后续模型 Context SHALL 使用摘要与保留的近期消息
- **AND** 原始会话历史 SHALL 仍可读取和分支

#### Scenario: 模型摘要包含未确认推断
- **WHEN** compaction summary 出现新的偏好、事实或结论
- **THEN** Runtime SHALL 只把它作为当前 Session Context
- **AND** SHALL NOT 自动升级为用户、项目或领域长期 Memory

### Requirement: 会话持久化不得泄露模型凭据
Session metadata、entries、模型快照、错误与统计信息 SHALL NOT 保存 API Key、API Secret、Authorization Header 或带敏感查询参数的 URL。

#### Scenario: 保存模型快照
- **WHEN** Runtime 为会话或运行记录实际模型配置
- **THEN** 快照 SHALL 包含 model id、供应商、协议、请求根地址、上游模型、推理等级和超时
- **AND** SHALL NOT 包含任何凭据字段

### Requirement: Pi Session 必须持久化不可变 Definition 与模型行为绑定

Pi SQLite SHALL 为每个可执行 Session 保存 agent key/version、Prompt 精确版本与 digest、tool profile、model_id、behavior_fingerprint、binding 状态和迁移来源，并 SHALL 在普通执行期间禁止原地覆盖。

#### Scenario: 重启后恢复 Session binding
- **GIVEN** Session 已完成 Definition 与模型行为绑定
- **WHEN** Runtime 重启并重新打开同一 SQLite 数据库
- **THEN** Session SHALL 恢复完全相同的 binding
- **AND** 后续执行 SHALL 先验证当前 Registry 和模型配置与 binding 兼容
- **AND** SHALL NOT 重新选择当前 active 版本替换历史版本

#### Scenario: 普通 fork 继承绑定
- **WHEN** 操作者从某 entry 创建普通 Session fork
- **THEN** 新 Session SHALL 继承源 Session 的 Agent/Prompt/model binding
- **AND** SHALL 记录 parent_session_id 与源 binding digest
- **AND** 源 Session metadata SHALL 保持不变

#### Scenario: 原地修改 binding
- **WHEN** 代码或请求尝试在既有 Session 上覆盖 Agent/Prompt 版本、model_id 或 behavior_fingerprint
- **THEN** persistence layer SHALL 拒绝该写入
- **AND** 变更 SHALL 使用显式 rebind fork 创建新 Session

### Requirement: Pi ModelCall 必须使用 namespaced SQLite 持久化

Pi Runtime SHALL 在同一稳定 SQLite 数据库中使用 Novex namespaced 表保存 ModelCall 与 Session/entry 关联，并 SHALL NOT 修改 Pi 上游私有表语义或把 Pi 审计双写到 PostgreSQL。

#### Scenario: 保存 Pi ModelCall
- **WHEN** Pi Harness 准备执行普通 Turn、Tool Loop 后续 Turn、compaction 或 branch summarization 的模型请求
- **THEN** Runtime SHALL 在 namespaced 审计表中保存 prepared ModelCall
- **AND** 记录 SHALL 关联 session_id、相关 entry/run、phase 和固定 binding
- **AND** Pi Session Tree SHALL 只保存稳定 model_call_id 关联或摘要 entry

#### Scenario: SQLite 审计写入失败
- **WHEN** namespaced ModelCall 表不可写或事务失败
- **THEN** Runtime SHALL 终止对应模型请求
- **AND** Session SHALL NOT 追加伪造的 Assistant、compaction 或 branch summary 成功 entry
- **AND** Runtime SHALL NOT 退化为仅日志审计

### Requirement: 历史 Pi Session 迁移必须保留原始数据和执行边界

Pi Session migration SHALL 幂等识别是否存在自定义 `system_prompt`，并 SHALL 在不删除原 entries 的前提下建立等价 binding 或只读状态。

#### Scenario: 重复迁移已绑定 Session
- **WHEN** migration 多次处理已经完成 v1 binding 的 Session
- **THEN** 系统 SHALL 返回同一 binding 和迁移结果
- **AND** SHALL NOT 重复追加迁移事件或改变 active leaf

#### Scenario: 自定义 Prompt Session 请求继续执行
- **GIVEN** 历史 Session 被标记为 custom-system-prompt read-only
- **WHEN** 操作者请求 prompt、compact 或 branch summarization
- **THEN** Runtime SHALL 返回稳定 `session_migration_required`
- **AND** SHALL NOT 再读取旧 system_prompt 作为 Harness System 内容
- **AND** entries、tree、详情和导出 SHALL 保持可读

### Requirement: 删除 Pi Session 必须同步清理其调用审计

操作者通过既有明确删除流程删除 Pi Session 时，Runtime SHALL 使用 Novex namespaced durable deletion intent、Pi 公开 `SessionRepo.delete` 和可恢复的审计清理删除该 Session 的 ModelCall；fork、rebind、撤销或回滚 SHALL NOT 触发清理。系统 SHALL NOT 为伪造跨私有表单事务而依赖 Pi 私有表、trigger、未公开 SQL 或复制其删除实现。

#### Scenario: 明确删除 Session
- **WHEN** Runtime 接受并完成某 Session 的删除请求
- **THEN** Runtime SHALL 在调用公开 Session 删除前持久化 durable deletion intent
- **AND** 该 Session 自有 ModelCall SHALL 级联删除
- **AND** 其他 fork Session 及其 ModelCall SHALL 保持不变
- **AND** 只有公开 Session 删除与 namespaced 审计清理均完成后 SHALL 返回成功

#### Scenario: Session 删除在两阶段之间中断
- **WHEN** Runtime 已持久化删除意图，但在公开 Session 删除或 namespaced 审计清理前后中断
- **THEN** 重试或启动 reconciliation SHALL 通过公开 Session 列表判断 owner 是否仍存在
- **AND** owner 已删除时 SHALL 完成 namespaced 审计清理
- **AND** owner 仍存在时 SHALL 重新执行公开删除后再清理审计
- **AND** SHALL NOT 静默遗留无 owner 的 ModelCall 或把未完成删除报告为成功

#### Scenario: 创建升级 fork
- **WHEN** 操作者 fork 到新 Definition 或模型 binding
- **THEN** 源 Session 的 ModelCall SHALL 保留
- **AND** 新 Session 的后续调用 SHALL 使用新的独立 ModelCall

### Requirement: Pi SQLite 必须持久化固定 Context binding 与 namespaced 审计记录

Pi SQLite SHALL 在 Novex namespaced 表中保存 Session 的 Context Policy/tokenizer binding、ContextSnapshot、ContextCompileAttempt 和迁移事件，并 SHALL NOT 修改或依赖 Pi 私有表 schema。

#### Scenario: 重启后恢复 Context binding
- **WHEN** Runtime 重启并重新打开可执行 Session
- **THEN** SHALL 恢复 Agent/Prompt、Context Policy、Tokenizer Profile、model_id 与 behavior_fingerprint 的完整固定 binding
- **AND** 任一版本缺失、revoked 或不兼容 SHALL 在模型请求前失败
- **AND** SHALL NOT 按当前 active 版本静默重写 binding

#### Scenario: 保存 Pi Context 记录
- **WHEN** Pi Context 编译成功或失败
- **THEN** SQLite SHALL 按统一 schema 保存不可变 Snapshot 或 CompileAttempt
- **AND** 成功 ModelCall SHALL 引用 ContextSnapshot ID/digest
- **AND** namespaced 写入失败 SHALL 阻止模型调用

### Requirement: 历史 Pi Session 必须按可证明等价性迁移 Context

历史 Pi Session 只有在 golden regression 证明 Context 选择、顺序、裁剪和最终 Prompt 等价时，才 SHALL 审计地绑定 baseline Policy；否则 SHALL 标记为只读 `context_migration_required`。

#### Scenario: 历史 Session 可等价迁移
- **WHEN** 迁移程序证明一个历史 Session 的旧装配可由 baseline Policy 等价重建
- **THEN** SHALL 幂等写入固定 Context binding 和迁移证据
- **AND** SHALL 保留原 Session Tree、entries 与既有 Definition/model binding

#### Scenario: 历史 Session 无法证明等价
- **WHEN** 缺少准确 Context、旧裁剪证据或重建结果不等价
- **THEN** Session SHALL 保持可读但拒绝继续模型调用
- **AND** 显式 fork/rebind SHALL 创建使用新 active 组合的新 Session

### Requirement: 删除 Pi Session 必须清理其 Context 证据

Pi Session 明确删除流程 SHALL 使用既有 durable deletion intent 协调 namespaced Context binding、Snapshot、CompileAttempt、ModelCall 和迁移事件清理。

#### Scenario: 删除包含 Context 记录的 Session
- **WHEN** 操作者明确删除 Session
- **THEN** 其所有 Context 原始记录 SHALL 按所有权级联清理
- **AND** 相邻 fork 的 binding、ContextSnapshot 和 ModelCall SHALL 保留
- **AND** 中断恢复 SHALL 继续使用公开 SessionRepo 与 deletion reconciliation

