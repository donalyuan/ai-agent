# personal-agent-runtime Specification

## Purpose
TBD - created by archiving change integrate-pi-personal-agent-foundation. Update Purpose after archive.
## Requirements
### Requirement: 系统必须提供本地单用户 Pi Agent Runtime
系统 SHALL 提供基于 Pi Agent Harness 的本地 Runtime 服务，作为视频、编程、知识研究及后续个人工作台共享的通用 Agent 执行入口。

#### Scenario: 创建通用 Agent 会话
- **WHEN** 操作者提交有效 `model_id`、工具 profile 和可选 system prompt 创建会话
- **THEN** Runtime SHALL 创建持久化 Pi Session
- **AND** 返回稳定 `session_id`、模型非敏感快照、工具 profile 和创建时间
- **AND** 响应 SHALL NOT 包含模型凭据

#### Scenario: 未启用模型被拒绝
- **WHEN** 操作者未提交模型，或提交不存在、停用、删除、非文本或协议不支持的模型
- **THEN** Runtime SHALL 返回稳定模型配置错误
- **AND** SHALL NOT 创建可执行会话
- **AND** SHALL NOT 回退环境变量或 Pi 内建模型目录

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
