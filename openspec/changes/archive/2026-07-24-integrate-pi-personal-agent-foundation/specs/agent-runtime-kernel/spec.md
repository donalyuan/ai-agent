## ADDED Requirements

### Requirement: Pi Harness 必须承担新的通用 Turn 和 Tool Loop
新的通用个人工作台 Agent 执行 SHALL 由 Pi Harness 负责模型流、Turn、Tool Call、Observation、steering、follow-up 和 abort；Rust Agent Kernel SHALL NOT 新增相同职责的并行实现。

#### Scenario: 新工作台执行通用 Tool Loop
- **WHEN** 新工作台创建不依赖既有视频业务 Adapter 的 Agent 会话
- **THEN** 请求 SHALL 进入 Pi Runtime
- **AND** Pi Harness SHALL 负责循环执行直到停止、取消或失败
- **AND** Rust Kernel SHALL NOT 对该请求再次调用模型

#### Scenario: 现有视频 Adapter 保持行为
- **WHEN** 现有视频工作台通过既有 Conversation API 执行脚本、选题、声音或作品 Agent
- **THEN** Rust AgentRunCoordinator 与对应 Adapter SHALL 继续执行现有流程
- **AND** HTTP、模型选择、Prompt、业务结果、Run/Step 和失败收尾 SHALL 保持不变

### Requirement: 领域 Agent 迁移不得长期保留双执行路径
后续把一个既有领域 Agent 迁移到 Pi Runtime 时，系统 SHALL 先把业务能力暴露为类型化领域 Tool，并在同一 change 中确定唯一入口和旧路径移除计划。

#### Scenario: 迁移单个领域 Agent
- **WHEN** 某领域 Agent 完成 Pi Tool Adapter 与行为回归验证
- **THEN** 该领域每个用户请求 SHALL 只由一个 Runtime 执行
- **AND** SHALL NOT 同时保存两套 Assistant 消息或发起两次供应商调用
- **AND** 未迁移领域 SHALL 不受影响
