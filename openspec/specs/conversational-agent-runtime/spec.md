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

### Requirement: 通用 Agent Runtime 必须支持 topic Agent adapter

通用对话 Agent Runtime SHALL 支持 `topic` adapter，使内容策略页可以通过对话消息触发选题生成、持久化选题和记录生成批次。

#### Scenario: topic Agent 生成普通选题批次

- **GIVEN** 已存在一个 `topic` Agent 会话
- **AND** 操作者消息包含非空补充要求
- **WHEN** Runtime 处理该消息
- **THEN** Runtime SHALL 创建 `agent_runs` 和 `agent_steps`
- **AND** Runtime SHALL 创建一个没有 `supplement_of_batch_id` 的 `topic_generation_batches`
- **AND** Runtime SHALL 调用 LLM 生成结构化选题
- **AND** Runtime SHALL 将候选写入 `content_topics`
- **AND** Runtime SHALL 保存 assistant 消息
- **AND** assistant 消息 metadata SHALL 包含 `batch_id`、`created_topic_ids`、`topic_count` 和 `status`

#### Scenario: topic Agent 生成补充批次

- **GIVEN** 已存在一个 `topic` Agent 会话
- **AND** 操作者消息包含非空补充要求
- **AND** 请求 metadata 包含同项目下可补充的 `supplement_of_batch_id`
- **AND** 原始批次及其补充批次下存在未软删除选题
- **WHEN** Runtime 处理该消息
- **THEN** Runtime SHALL 创建新的 `topic_generation_batches`
- **AND** 新批次 SHALL 记录 `supplement_of_batch_id`
- **AND** Runtime SHALL 在调用 LLM 时注入原始批次 prompt、同主题组已有选题和当前会话历史消息摘要
- **AND** Runtime SHALL 要求 LLM 基于同一主题继续扩展并避免重复已有选题
- **AND** Runtime SHALL 将候选写入 `content_topics`
- **AND** 新候选的 `batch_id` SHALL 指向新补充批次
- **AND** assistant 消息 metadata SHALL 包含新批次 `batch_id`、`supplement_of_batch_id`、`created_topic_ids`、`topic_count` 和 `status`

#### Scenario: topic Agent 补充目标不可用

- **GIVEN** 请求 metadata 包含不存在、跨项目、失败或空批次的 `supplement_of_batch_id`
- **WHEN** Runtime 处理该消息
- **THEN** Runtime SHALL 拒绝生成
- **AND** Runtime SHALL NOT 创建新的 `topic_generation_batches`
- **AND** Runtime SHALL NOT 写入新的 `content_topics`
- **AND** Runtime SHALL 返回明确错误

### Requirement: 每轮 Agent 模型调用必须显式选择模型

通用 Agent Runtime SHALL 保留每轮可能调用模型的消息携带 `model_id` 的既有 API 字段，并 SHALL 在 Conversation 第一次模型调用前原子建立固定 `model_id + behavior_fingerprint` binding；后续轮次和全部内部步骤不得静默切换模型行为。

#### Scenario: script Agent 首轮固定选中模型

- **GIVEN** 已存在尚未绑定文本模型的 script Agent 会话
- **WHEN** 操作者发送会触发模型调用的消息并提交启用文本模型 ID
- **THEN** Runtime SHALL 在供应商请求前原子保存该 model_id 与 behavior_fingerprint binding
- **AND** SHALL 使用该模型完成本轮意图判断、脚本生成或分镜修改
- **AND** `agent_runs` SHALL 保存模型引用并关联每次调用的 ModelCall

#### Scenario: topic Agent 内部步骤继承模型

- **GIVEN** 已存在已绑定文本模型的 topic Agent 会话
- **WHEN** 操作者发送生成或补充消息并提交相同 model_id
- **THEN** Runtime SHALL 让候选生成、质量闸门、最多一次重写和同模型重试使用固定 binding
- **AND** 每个模型步骤和重试 SHALL 创建独立 ModelCall
- **AND** Runtime SHALL NOT 自动切换到其他模型

#### Scenario: 下一轮提交不同模型

- **GIVEN** 会话已固定模型 A 及其 behavior_fingerprint
- **WHEN** 操作者下一轮在既有 `model_id` 字段提交模型 B
- **THEN** Runtime SHALL 返回稳定 `model_rebind_required`
- **AND** SHALL NOT 调用模型 A、模型 B 或其他供应商
- **AND** 操作者 SHALL 通过显式 rebind/fork 创建新的绑定后再执行

#### Scenario: 固定模型行为配置变化

- **GIVEN** 会话已固定模型 A
- **WHEN** 模型 A 当前配置的 behavior_fingerprint 与 binding 不同
- **THEN** Runtime SHALL 返回稳定 `model_rebind_required`
- **AND** SHALL NOT 静默更新 binding
- **AND** 仅凭据轮换且 fingerprint 不变时 SHALL 允许继续

#### Scenario: 缺少或不可用模型

- **WHEN** 一轮消息会触发模型调用但未提交模型、模型已停用、删除、能力不兼容或类型不是文本
- **THEN** Runtime SHALL 返回稳定模型错误
- **AND** Runtime SHALL NOT 调用环境变量模型、默认硬编码模型或其他供应商

### Requirement: Rust Conversation 必须固定 Agent 与 Prompt Definition

Rust Conversation SHALL 在创建时按稳定 AgentKey 固定唯一 active AgentDefinition 及其 PromptDefinition 精确版本；后续发布 SHALL NOT 静默改变既有 Conversation 的 Definition binding。

#### Scenario: 创建已支持的 Conversation
- **WHEN** 操作者通过既有 API 创建 script、topic、sound 或 work Conversation
- **THEN** 系统 SHALL 按 agent_type 到 AgentKey 的确定性映射保存 active Definition binding
- **AND** 外部 URL、现有请求字段和响应字段 SHALL 保持不变
- **AND** 未知、candidate 或 revoked Definition SHALL 阻止创建可执行 Conversation

#### Scenario: 新版本发布后继续旧 Conversation
- **GIVEN** Conversation 已绑定版本 v1
- **AND** 仓库发布同一 Agent 的 active v2
- **WHEN** 操作者继续旧 Conversation
- **THEN** Conversation SHALL 继续使用 supported v1 及其 Prompt 精确版本
- **AND** SHALL NOT 自动迁移到 v2

#### Scenario: 显式迁移 Conversation
- **WHEN** 操作者从旧 Conversation 显式 rebind/fork 到目标 Definition 或模型行为
- **THEN** 系统 SHALL 创建新的不可变 binding 与迁移关联
- **AND** 原 Conversation 的消息、Run、ModelCall 和 binding SHALL 保持不变

### Requirement: Rust Conversation 模型绑定必须原子且可恢复

Conversation 首次模型 binding 与 prepared ModelCall SHALL 在外部请求前完成持久化；并发首轮或持久化失败 SHALL NOT 产生不确定 binding。

#### Scenario: 两个并发首轮选择不同模型
- **GIVEN** Conversation 尚未固定模型
- **WHEN** 两个并发请求分别提交模型 A 和模型 B
- **THEN** 最多一个请求 SHALL 原子建立 binding
- **AND** 另一个请求 SHALL 返回会话忙或 rebind required
- **AND** SHALL NOT 对两个模型都发起调用

#### Scenario: 首轮 binding 持久化失败
- **WHEN** Conversation binding 或 prepared ModelCall 写入失败
- **THEN** 系统 SHALL NOT 调用供应商
- **AND** SHALL NOT 保存伪造 Assistant 消息或成功 Run
- **AND** 重试后 SHALL 能从明确的未绑定或已绑定状态继续

### Requirement: Rust 历史 Conversation 必须确定性建立 v1 binding

历史 Conversation SHALL 按已知 agent_type 回填对应 v1 Definition；历史模型证据不足时 SHALL 延迟到首次新模型请求绑定，不得猜测。

#### Scenario: 历史 Conversation 已有模型快照
- **WHEN** migration 可从可信 agent_run 模型快照确定最近有效模型行为
- **THEN** 系统 SHALL 回填对应 v1 Definition 与可验证 model binding
- **AND** SHALL 记录迁移来源和 snapshot digest

#### Scenario: 历史 Conversation 无模型证据
- **WHEN** migration 无法证明历史 model_id 或 behavior_fingerprint
- **THEN** 系统 SHALL 只回填 v1 Definition binding
- **AND** model binding SHALL 在下一次有效模型请求前原子建立
- **AND** 系统 SHALL NOT 从默认模型猜测历史配置

