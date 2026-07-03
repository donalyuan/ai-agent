# conversational-agent-runtime Specification Delta

## ADDED Requirements

### Requirement: 后端必须提供通用 Agent 对话会话模型

系统 SHALL 提供可持久化的 Agent 会话与消息模型，使不同业务 Agent 能复用同一套对话上下文、资源绑定和历史消息读取能力。

#### Scenario: 创建脚本 Agent 会话

- **GIVEN** 数据库中存在项目和脚本
- **WHEN** 操作者提交 `agent_type=script`、`project_id`、`subject_type=script` 和 `subject_id=<script_id>` 创建会话
- **THEN** 系统 SHALL 创建 `agent_conversations` 记录
- **AND** 会话 SHALL 绑定该项目和脚本资源
- **AND** 会话状态 SHALL 为 `active`

#### Scenario: 保存并读取会话消息

- **GIVEN** 已存在 Agent 会话
- **WHEN** 系统保存用户消息和 Agent 回复
- **THEN** 系统 SHALL 按创建时间升序返回消息列表
- **AND** 每条消息 SHALL 包含 `message_id`、`conversation_id`、`role`、`content`、`metadata` 和 `created_at`

### Requirement: 通用 Agent Runtime 必须为 Skill/MCP/Tool 留出接口

系统 SHALL 通过通用运行时接口处理用户消息，而不是把对话逻辑直接写死在某个业务路由中。

#### Scenario: Agent adapter 处理一次用户消息

- **GIVEN** 已存在会话和历史消息
- **WHEN** 操作者发送一条用户消息
- **THEN** Runtime SHALL 先保存用户消息
- **AND** Runtime SHALL 读取会话绑定资源和历史上下文
- **AND** Runtime SHALL 调用该 `agent_type` 对应 adapter
- **AND** Runtime SHALL 保存 Agent 回复消息
- **AND** Runtime SHALL 写入 `agent_runs` 和 `agent_steps` 记录本次运行

#### Scenario: 未支持的 Agent 类型被拒绝

- **GIVEN** 会话的 `agent_type` 暂未接入 adapter
- **WHEN** 操作者向该会话发送消息
- **THEN** 系统 SHALL 返回错误
- **AND** 系统 SHALL NOT 伪造成功回复

### Requirement: 脚本 Agent 必须支持对话式修改分镜

系统 SHALL 允许操作者通过脚本 Agent 对话指定修改方向，并将 LLM 返回的结构化分镜补丁落库到当前脚本。

#### Scenario: 用户要求修改指定分镜

- **GIVEN** 脚本 Agent 会话绑定一个存在的脚本
- **AND** 该脚本包含第 3 镜
- **WHEN** 操作者发送“把第 3 镜改得更有冲突感，画面换成办公室深夜加班”
- **THEN** 系统 SHALL 调用 LLM 生成结构化分镜补丁
- **AND** 系统 SHALL 更新第 3 镜的 `narration`、`visual_description`、`emotion` 和 `duration_sec`
- **AND** Agent 回复 SHALL 说明已修改的分镜序号和修改摘要
- **AND** 回复 metadata SHALL 包含更新后的 `script_id` 和 `scene_sequence`

#### Scenario: LLM 输出无效补丁时不改动脚本

- **GIVEN** 脚本 Agent 会话绑定一个存在的脚本
- **WHEN** LLM 返回无法解析或不包含有效分镜序号的内容
- **THEN** 系统 SHALL 返回错误
- **AND** 系统 SHALL NOT 更新任何分镜内容
- **AND** 系统 SHALL 记录失败运行

### Requirement: 对话 API 必须提供稳定错误语义

系统 SHALL 对对话 API 的常见错误返回稳定结构，供前端工作台后续接入。

#### Scenario: 空消息被拒绝

- **GIVEN** 已存在会话
- **WHEN** 操作者发送空白消息
- **THEN** 系统 SHALL 返回 HTTP 400
- **AND** 响应体 SHALL 包含错误说明

#### Scenario: 会话不存在

- **WHEN** 操作者读取或发送消息到不存在的会话
- **THEN** 系统 SHALL 返回 HTTP 404
- **AND** 响应体 SHALL 包含 `conversation_id`

#### Scenario: 脚本不存在

- **WHEN** 操作者为不存在的脚本创建脚本 Agent 会话
- **THEN** 系统 SHALL 返回 HTTP 404
- **AND** 响应体 SHALL 包含 `script_id`
