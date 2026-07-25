## ADDED Requirements

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
