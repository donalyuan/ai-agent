# personal-agent-runtime Specification

## Purpose
TBD - created by archiving change integrate-pi-personal-agent-foundation. Update Purpose after archive.
## Requirements
### Requirement: 系统必须提供本地单用户 Pi Agent Runtime
系统 SHALL 提供基于 Pi Agent Harness 的本地 Runtime 服务，作为视频、编程、知识研究及后续个人工作台共享的通用 Agent 执行入口；Session SHALL 通过版本化 `agent_key` 创建并固定 Agent/Prompt 与模型行为绑定，不得接收任意 system prompt。

#### Scenario: 创建通用 Agent 会话
- **WHEN** 操作者提交有效 `agent_key`、`model_id` 和工具 profile 创建会话
- **THEN** Runtime SHALL 解析唯一 active AgentDefinition 及其 PromptDefinition 精确版本
- **AND** SHALL 校验工具 profile 和模型能力满足 Definition
- **AND** SHALL 创建持久化 Pi Session 并固定 definition digest、`model_id` 与 `behavior_fingerprint`
- **AND** 返回稳定 `session_id`、定义版本、模型非敏感快照、工具 profile 和创建时间
- **AND** 响应 SHALL NOT 包含模型凭据

#### Scenario: 未启用模型被拒绝
- **WHEN** 操作者未提交模型，或提交不存在、停用、删除、非文本、协议不支持或能力不满足 Definition 的模型
- **THEN** Runtime SHALL 返回稳定模型配置错误
- **AND** SHALL NOT 创建可执行会话
- **AND** SHALL NOT 回退环境变量、Pi 内建模型目录或其他模型

#### Scenario: 提交任意 system prompt
- **WHEN** Session 创建请求包含 `system_prompt` 或其他未声明字段
- **THEN** Runtime SHALL 返回稳定请求校验错误
- **AND** SHALL NOT 创建 Session
- **AND** SHALL NOT 将该文本降级为隐式用户消息

### Requirement: Runtime 必须流式执行 Pi Harness 事件
Runtime SHALL 通过 SSE 返回一次 prompt 的开始、Pi Harness 事件和唯一终态，并 SHALL 在同一会话中拒绝并发 prompt。

#### Scenario: Prompt 成功执行
- **GIVEN** 已存在空闲会话
- **WHEN** 操作者提交非空 prompt
- **THEN** Runtime SHALL 先发送 `run_started`
- **AND** SHALL 按实际顺序转发 message、tool 和 turn 事件
- **AND** SHALL 最终发送一次 `run_completed`
- **AND** 本轮消息与工具结果 SHALL 在返回终态前持久化

#### Scenario: 同一会话并发执行被拒绝
- **GIVEN** 会话已有活动 prompt
- **WHEN** 操作者再次提交 prompt
- **THEN** Runtime SHALL 返回 `session_busy`
- **AND** SHALL NOT 启动第二个模型请求

#### Scenario: Provider 执行失败
- **WHEN** 模型或工具执行返回错误
- **THEN** Runtime SHALL 发送一次脱敏 `run_failed`
- **AND** SHALL NOT 在事件或日志中暴露 API Key、API Secret 或完整上游响应头

### Requirement: Runtime 必须支持运行控制命令
Runtime SHALL 为活动或持久化会话提供 steer、follow-up、abort、compact 和 fork 命令，并保持 Pi Harness 的事件和会话语义。

#### Scenario: 中途纠偏
- **GIVEN** 会话正在执行模型或工具步骤
- **WHEN** 操作者提交 steer 消息
- **THEN** Runtime SHALL 将消息加入 steering queue
- **AND** 当前安全完成的工具步骤结束后 SHALL 按队列模式处理纠偏消息

#### Scenario: 取消活动运行
- **GIVEN** 会话存在活动运行
- **WHEN** 操作者请求 abort
- **THEN** Runtime SHALL 向模型和可取消工具传播取消信号
- **AND** SHALL 清空尚未执行的 steering/follow-up 消息
- **AND** 后续步骤 SHALL NOT 继续产生副作用

#### Scenario: 会话分支
- **GIVEN** 会话存在一个持久化 entry
- **WHEN** 操作者从该 entry 创建 fork
- **THEN** Runtime SHALL 创建新 `session_id`
- **AND** 原会话历史 SHALL 保持不变
- **AND** 新会话 SHALL 能从选定上下文继续执行

### Requirement: 本地执行工具必须通过显式 profile 启用
Runtime SHALL 仅允许预定义工具 profile；`workspace` profile SHALL 提供 Pi read/write/edit/bash，`chat` profile SHALL 不提供本地文件或命令工具。

#### Scenario: Workspace profile 执行本地工具
- **GIVEN** 会话使用 `workspace` profile
- **WHEN** 模型调用 read、write、edit 或 bash
- **THEN** Runtime SHALL 使用配置的工作目录执行工具
- **AND** SHALL 记录 tool start、update、result 和终态事件

#### Scenario: Chat profile 拒绝本地工具
- **GIVEN** 会话使用 `chat` profile
- **WHEN** 模型尝试调用未注册的本地工具
- **THEN** Runtime SHALL NOT 执行文件或命令操作
- **AND** 工具结果 SHALL 如实表示不可用

### Requirement: Pi Runtime 不得接管领域事实和高风险业务 Gate
Pi Session SHALL 只保存 Agent 上下文和执行记录；脚本、素材、作品、正式 Memory、付费生成和发布状态 SHALL 继续由对应领域服务管理。

#### Scenario: 领域工具产生业务写入
- **WHEN** 后续领域 Tool 请求生成、发布、删除正式数据或其他受控动作
- **THEN** Tool SHALL 调用拥有该规则的领域 API
- **AND** SHALL 保留该领域的确认、幂等、资源限制和失败恢复规则
- **AND** Pi Runtime SHALL NOT 直接写对应业务表

### Requirement: Pi Runtime 必须通过组合式 wrapper 接入 Novex 执行规则

Novex SHALL 使用自有 wrapper 持有 `AgentHarness`，并 SHALL 只通过 Pi 官方公开构造参数、hook、`Models` 接口和运行控制方法接入 Prompt 编译、模型调用审计与 Tool Gate。

#### Scenario: 执行普通 Turn 和 Tool Loop
- **WHEN** Session 执行 prompt 并产生一个或多个 Tool Loop 后续 Turn
- **THEN** wrapper SHALL 通过公开 hook 为每次模型调用编译输入并建立 ModelCall
- **AND** Pi Harness SHALL 继续负责 Turn、Tool、Observation、SSE、持久化和唯一终态
- **AND** wrapper SHALL NOT 实现第二套 Tool Loop

#### Scenario: 执行 compaction 或 branch summarization
- **WHEN** Runtime 执行 compact 或带 summarize 的 tree navigation
- **THEN** 该模型调用 SHALL 使用 Session 固定的 Definition 与模型 behavior fingerprint
- **AND** 每次调用及允许重试 SHALL 生成独立 ModelCall
- **AND** summary SHALL 继续作为 Session Context 而不是正式 Memory

#### Scenario: 检查 Pi 集成边界
- **WHEN** 执行静态架构检查和 Pi 兼容性测试
- **THEN** Runtime SHALL NOT 修改或 fork Pi 源码
- **AND** SHALL NOT 继承私有实现、monkey patch、反射私有字段、导入未导出内部路径或复制 agent/tool loop
- **AND** 使用的 Pi import 与 hook SHALL 位于公开 API allowlist

### Requirement: Pi Session 继续执行必须验证固定绑定

Runtime SHALL 在每次普通 Turn、Tool Loop 后续 Turn、compaction 和 branch summarization 前验证 Session 固定的 Definition 状态、模型可用性、能力和 `behavior_fingerprint`。

#### Scenario: 仅模型凭据轮换
- **GIVEN** Session 固定的 model_id 配置只发生凭据轮换
- **WHEN** Runtime 重新解析该模型且 behavior_fingerprint 不变
- **THEN** Session SHALL 可透明继续
- **AND** 新凭据 SHALL NOT 写入 Session、ModelCall、SSE 或日志

#### Scenario: 模型行为配置改变
- **GIVEN** Session 固定的 model_id 的协议、上游模型、地址、reasoning、输出上限或其他行为 settings 已改变
- **WHEN** Runtime 计算的新 behavior_fingerprint 与 binding 不同
- **THEN** Runtime SHALL 在外部调用前返回 `model_rebind_required`
- **AND** SHALL NOT 静默更新 binding 或回退其他模型

#### Scenario: 显式升级 Session
- **WHEN** 操作者从现有 Session 显式 fork 到目标 Agent/Prompt 版本或模型 binding
- **THEN** Runtime SHALL 创建新的 session_id 和迁移事件
- **AND** 原 Session、entries、binding 和 ModelCall SHALL 保持不变

### Requirement: Pi 用户插话必须作为可审计动态输入

steer 与 follow-up SHALL 通过 Pi 公开队列语义影响后续安全执行边界，并 SHALL 被 PromptCompiler 作为 User 层动态输入记录。

#### Scenario: 运行中收到 steer
- **GIVEN** Session 正在执行模型或 Tool
- **WHEN** 操作者提交 steer 文本
- **THEN** Runtime SHALL 将文本加入 steering queue
- **AND** 受影响的后续 ModelCall SHALL 记录其类型、来源和关联 entry
- **AND** SHALL NOT 改写 System 层、Definition binding、已确认事实或正式 Memory

#### Scenario: 空闲 Session 收到 follow-up
- **WHEN** 操作者向允许 follow-up 的 Session 提交后续指令
- **THEN** Runtime SHALL 按 Pi queue mode 处理该指令
- **AND** 新调用 SHALL 使用原固定 Definition 与模型 binding
- **AND** 领域 Gate 和 Tool profile SHALL 保持有效
