# conversational-agent-runtime Specification Delta

## ADDED Requirements

### Requirement: 通用 Agent Runtime 必须支持 topic Agent adapter

系统 SHALL 在现有通用 Agent Runtime 中接入 `topic` adapter，使选题 Agent 复用统一会话、消息、运行记录和步骤记录。

#### Scenario: 创建 topic Agent 会话

- **GIVEN** 数据库中存在一个项目
- **WHEN** 操作者创建 `agent_type=topic` 且绑定 `project_id` 的会话
- **THEN** 系统 SHALL 创建 `agent_conversations` 记录
- **AND** 会话 SHALL 绑定该项目
- **AND** 会话状态 SHALL 为 `active`

#### Scenario: topic adapter 处理选题生成消息

- **GIVEN** 已存在一个 `topic` Agent 会话
- **AND** 操作者消息包含非空补充要求
- **WHEN** Runtime 处理该消息
- **THEN** Runtime SHALL 保存用户消息
- **AND** Runtime SHALL 读取项目定位和描述
- **AND** Runtime SHALL 调用 `topic` adapter 生成选题
- **AND** Runtime SHALL 保存 assistant 消息
- **AND** Runtime SHALL 写入 `agent_runs` 和 `agent_steps`

#### Scenario: topic adapter 记录关键步骤

- **GIVEN** `topic` adapter 正在处理一次选题生成
- **WHEN** 本次运行结束
- **THEN** 系统 SHALL 至少记录 `read_project_context`、`generate_topics` 和 `persist_topics` 三类步骤
- **AND** 失败时 SHALL 记录 failed run
