# conversational-agent-runtime Specification

## Purpose
TBD - created by archiving change conversational-agent-runtime. Update Purpose after archive.
## Requirements
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

### Requirement: 脚本 Agent 对话必须支持从无脚本状态生成新脚本

系统 SHALL 允许操作者创建未绑定具体脚本的 `script` Agent 会话，并通过该会话生成新脚本。该能力 SHALL 复用通用 Agent Runtime、会话消息、运行记录和脚本生成领域服务，而不得新增独立聊天接口或前端专用伪流程。

#### Scenario: 创建未绑定脚本的脚本 Agent 会话

- **GIVEN** 数据库中存在一个项目
- **WHEN** 操作者提交 `agent_type=script` 和 `project_id` 创建会话，且不传 `subject_type` / `subject_id`
- **THEN** 系统 SHALL 创建 `agent_conversations` 记录
- **AND** 会话 SHALL 绑定该项目
- **AND** 会话 SHALL 暂不绑定具体脚本
- **AND** 会话状态 SHALL 为 `active`

#### Scenario: 通过未绑定会话生成脚本

- **GIVEN** 已存在一个未绑定脚本的 `script` Agent 会话
- **AND** 操作者消息包含可识别的 `topic`、`style` 和 `scene_count`
- **WHEN** 操作者发送“帮我生成一个关于 ChatGPT 工作流的 6 镜知识科普脚本”
- **THEN** Runtime SHALL 保存用户消息
- **AND** Runtime SHALL 调用脚本 Agent adapter 的生成路径
- **AND** 系统 SHALL 生成并保存一个结构化脚本和有序分镜
- **AND** 系统 SHALL 将当前会话绑定到新脚本
- **AND** Agent 回复 SHALL 说明脚本已生成
- **AND** Agent 回复 metadata SHALL 包含 `intent=generate_script`、`script_created=true` 和新建 `script_id`
- **AND** 本轮运行 SHALL 写入 `agent_runs` 和 `agent_steps`

#### Scenario: 缺少生成参数时追问而不创建脚本

- **GIVEN** 已存在一个未绑定脚本的 `script` Agent 会话
- **WHEN** 操作者发送“帮我生成一个脚本”但没有提供足够的选题、风格或分镜数
- **THEN** Runtime SHALL 保存用户消息
- **AND** 脚本 Agent SHALL 保存一条追问式 assistant 消息
- **AND** assistant metadata SHALL 包含 `needs_input=true` 和 `missing_fields`
- **AND** 系统 SHALL NOT 创建脚本
- **AND** 系统 SHALL NOT 将会话绑定到脚本

#### Scenario: 生成脚本失败时记录失败运行

- **GIVEN** 已存在一个未绑定脚本的 `script` Agent 会话
- **AND** 操作者消息包含完整生成参数
- **WHEN** LLM 或脚本保存失败
- **THEN** 系统 SHALL 返回稳定错误
- **AND** 系统 SHALL 记录 failed run
- **AND** 系统 SHALL NOT 伪造脚本已生成的 assistant 回复

### Requirement: 脚本 Agent 对话必须区分生成与修改意图

系统 SHALL 在同一个 `script` Agent adapter 中处理脚本生成和脚本修改意图，并根据会话是否绑定脚本决定默认行为。

#### Scenario: 已绑定脚本会话默认修改当前脚本

- **GIVEN** 已存在一个绑定脚本的 `script` Agent 会话
- **WHEN** 操作者发送“把第 2 镜改得更有冲突感”
- **THEN** 系统 SHALL 保持现有分镜修改行为
- **AND** 系统 SHALL NOT 创建新脚本

#### Scenario: 已绑定脚本会话中明确要求新建脚本

- **GIVEN** 已存在一个绑定脚本的 `script` Agent 会话
- **WHEN** 操作者明确发送“新建一个关于 AI 剪辑流程的 5 镜脚本”
- **THEN** 系统 SHALL NOT 把该请求误判为修改当前脚本
- **AND** 系统 SHALL 创建新脚本或要求前端开启新的未绑定脚本会话
- **AND** 响应 SHALL 明确返回新脚本的 `script_id` 或下一步所需操作

