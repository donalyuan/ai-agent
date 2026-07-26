## ADDED Requirements

### Requirement: Pi Runtime 必须通过公开 Context hook 执行受治理装配

Pi Novex wrapper SHALL 通过 Pi 官方公开 context hook 获取实际分支消息，并把消息、steer/follow-up、compaction summary 与 Tool request/result 转换为原子 ContextCandidate；Compiler 输出 SHALL 通过同一公开 hook 返回 Harness。

#### Scenario: Pi 执行普通 Turn 或 Tool Loop
- **WHEN** Harness 为当前活动分支构建 provider context
- **THEN** wrapper SHALL 按固定 Session binding 运行 Context Compiler
- **AND** Tool request/result SHALL 保持配对且不可拆分
- **AND** Harness SHALL 使用 Compiler 返回的最终消息而不是未治理原始列表
- **AND** SSE、唯一终态和 Session Tree 语义 SHALL 保持不变

#### Scenario: Pi 执行 compaction 或 branch summary
- **WHEN** Harness 调用 compaction 或分支摘要模型
- **THEN** Runtime SHALL 使用对应 node 的精确 Context Policy 和独立 ContextSnapshot/ModelCall
- **AND** 生成摘要 SHALL 继续只是有损 Session Context
- **AND** SHALL NOT 自动写入正式长期 Memory

#### Scenario: 检查 Pi 公共边界
- **WHEN** CI 检查 Context Compiler 集成
- **THEN** 实现 SHALL 只使用公开 hook、Models、Session 和 Tool API
- **AND** SHALL NOT 修改 Pi 源码、访问私有表/字段、monkey patch 或复制 Context/Tool Loop

