# agent-runtime-kernel Specification

## Purpose
定义可注册的 Agent 执行内核、Adapter 契约、执行上下文、运行生命周期和启动期依赖校验。
## Requirements
### Requirement: Agent Kernel 必须通过注册表分派业务 Adapter

系统 SHALL 通过 `AgentRegistry` 按稳定 `AgentKey` 解析 `AgentAdapter`，Agent Kernel SHALL NOT 包含针对选题、脚本、声音、作品或未来业务 Agent 的类型分支。

#### Scenario: 已注册 Agent 被执行
- **GIVEN** Bootstrap 已注册某个 `AgentKey` 对应的 Adapter
- **WHEN** Coordinator 收到该类型会话的有效调用
- **THEN** 系统 SHALL 从 Registry 解析唯一 Adapter
- **AND** 系统 SHALL 把调用委派给该 Adapter

#### Scenario: 未注册 Agent 被拒绝
- **GIVEN** 会话引用未注册的 `AgentKey`
- **WHEN** Coordinator 处理该会话消息
- **THEN** 系统 SHALL 返回稳定的 unsupported Agent 错误
- **AND** 系统 SHALL NOT 调用任意其他 Adapter
- **AND** 系统 SHALL NOT 伪造 Assistant 回复

#### Scenario: 新增 Adapter 不修改 Kernel 分派
- **GIVEN** 开发者实现一个满足契约的新 Adapter
- **WHEN** Bootstrap 注册该 Adapter
- **THEN** Coordinator SHALL 能按其 `AgentKey` 调用该 Adapter
- **AND** Agent Kernel 源码 SHALL NOT 增加该业务类型的专属分支

### Requirement: Agent Adapter 必须拥有业务输入和依赖边界

系统 SHALL 让每个 Agent Adapter 解析并校验自己的业务 payload，并 SHALL 只持有该业务能力所需的 Repository 或 Application Service；Agent Kernel SHALL NOT 持有具体业务 Repository。

#### Scenario: Adapter 校验类型化业务输入
- **GIVEN** 通用调用 envelope 携带某 Agent 的业务 payload
- **WHEN** 对应 Adapter 开始执行
- **THEN** Adapter SHALL 先把 payload 解码为自身强类型输入并完成校验
- **AND** 未知字段、缺失字段或非法值 SHALL 返回稳定校验错误
- **AND** 业务逻辑 SHALL NOT 直接散读未校验 JSON 字段

#### Scenario: Kernel 不暴露业务专属字段
- **WHEN** 开发者查看公共 Agent 调用和 ExecutionContext 契约
- **THEN** 契约 SHALL NOT 包含 `supplement_of_batch_id`、声音编辑快照或其他单一业务 Agent 专属字段
- **AND** 这些参数 SHALL 只存在于对应 Adapter 的业务 payload 类型中

### Requirement: Coordinator 必须统一管理单轮运行生命周期

系统 SHALL 由 `AgentRunCoordinator` 统一执行会话加载、用户消息保存、Run 创建、Adapter 调用、Assistant 消息保存和 Run 终态收尾，并 SHALL 保证同一 Run 只进入一个终态。

#### Scenario: Agent 调用成功
- **GIVEN** 有效会话、已注册 Adapter 和可用模型执行上下文
- **WHEN** Adapter 返回成功 `AgentOutcome`
- **THEN** Coordinator SHALL 保存用户消息和一条 Assistant 消息
- **AND** Coordinator SHALL 把 Run 结束为 `succeeded`
- **AND** Run 输出 SHALL 关联实际 Assistant 消息

#### Scenario: Adapter 执行失败
- **GIVEN** Coordinator 已创建 running Run
- **WHEN** Adapter 返回业务、模型或持久化错误
- **THEN** Coordinator SHALL 把该 Run 结束为 `failed`
- **AND** Run SHALL 保存原始错误语义
- **AND** 系统 SHALL NOT 保存伪造的 Assistant 成功消息

#### Scenario: Run 不得重复收尾
- **GIVEN** 某次调用已进入成功或失败收尾路径
- **WHEN** Coordinator 退出本轮调用
- **THEN** 系统 SHALL 只尝试一次 Run 终态转换
- **AND** 系统 SHALL NOT 先标记成功后再标记失败，或先标记失败后再标记成功

### Requirement: ExecutionContext 必须提供受控的共同执行能力

系统 SHALL 通过 `AgentExecutionContext` 向 Adapter 提供本轮 conversation、project、subject、run、固定 Definition/模型 binding、受审计模型执行引用和 Step Recorder，且 SHALL NOT 暴露裸模型客户端或其他 Agent 的业务 Repository。

#### Scenario: Adapter 记录业务步骤
- **GIVEN** Adapter 正在处理一次已创建 Run 的调用
- **WHEN** Adapter 记录读取资源、模型调用或业务写入步骤
- **THEN** Step Recorder SHALL 把步骤关联到当前 Run
- **AND** 步骤顺序 SHALL 稳定且满足数据库唯一约束
- **AND** 现有 Agent 的 step type 和相对顺序 SHALL 保持不变

#### Scenario: Adapter 使用本轮模型绑定
- **GIVEN** Coordinator 已为本轮解析 Definition 和模型执行 binding
- **WHEN** Adapter 执行一个或多个模型步骤
- **THEN** 所有步骤 SHALL 使用本轮相同的 model_id 与 behavior_fingerprint
- **AND** 每个模型步骤和重试 SHALL 通过受审计执行引用创建独立 ModelCall
- **AND** 系统 SHALL 保持禁止自动跨模型切换的规则

#### Scenario: Adapter 尝试绕过审计入口
- **WHEN** 生产 Adapter 尝试直接获取或调用裸 `LLMClient`
- **THEN** Bootstrap 类型与架构检查 SHALL 阻止该依赖
- **AND** Adapter SHALL 只能提交 node key 和结构化编译输入给受审计执行引用

### Requirement: Agent 执行必须使用唯一应用门面和通用 Run 生命周期

系统 SHALL 让会话型 Agent 通过唯一 backend 执行门面调用 `AgentRunCoordinator`，并 SHALL 让不产生 Conversation 消息的 Agent 任务通过通用 Run 生命周期协调器完成创建和终态收尾。

#### Scenario: ConversationService 调用唯一执行门面
- **GIVEN** `ConversationService` 已解析模型和业务 payload
- **WHEN** 它提交一次会话型 Agent 调用
- **THEN** 系统 SHALL 通过 `AgentExecutor` 执行 Coordinator、错误下转和 Domain 映射
- **AND** `ConversationService` SHALL NOT 重复组装 Coordinator 或 Domain 转换流程

#### Scenario: 非会话 Agent 任务成功
- **GIVEN** 脚本生成、项目策略草稿或主题组评审准备执行
- **WHEN** 业务操作成功
- **THEN** 通用 Run 生命周期协调器 SHALL 创建一次 running Run 并完成一次 succeeded 收尾
- **AND** Run 输出 SHALL 由对应业务操作的结果生成

#### Scenario: 非会话 Agent 任务失败
- **GIVEN** 通用 Run 生命周期协调器已经创建 running Run
- **WHEN** 业务操作失败
- **THEN** 协调器 SHALL 尝试一次 failed 收尾并保留原始业务错误
- **AND** 系统 SHALL NOT 用失败收尾产生的次生错误覆盖原始业务错误

### Requirement: Adapter 注册必须在启动期校验

系统 SHALL 在 Bootstrap 组装阶段完成 Adapter 注册和必需依赖校验，不得把可预知的配置缺失延迟到用户请求期。

#### Scenario: 重复 AgentKey 阻止启动
- **GIVEN** 两个 Adapter 声明相同 `AgentKey`
- **WHEN** Bootstrap 构建 Agent Registry
- **THEN** Registry SHALL 返回重复注册错误
- **AND** 应用 SHALL NOT 以不确定分派状态完成启动

#### Scenario: Adapter 缺少必需依赖阻止启动
- **GIVEN** 已启用业务 Agent 缺少必需 Repository 或 Application Service
- **WHEN** Bootstrap 构造并注册该 Adapter
- **THEN** 应用 SHALL 返回明确的依赖组装错误
- **AND** Kernel SHALL NOT 使用 `Option<Repository>` 延迟到运行时失败

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

### Requirement: Rust Kernel 必须按 Definition node 执行所有生产 LLM 调用

Rust Agent 与非会话业务 Run SHALL 在调用模型前解析 executor owner 为 Rust 的 AgentDefinition 和精确 PromptDefinition node，并 SHALL 通过统一 PromptCompiler 与 AuditedModelExecutor 执行。

#### Scenario: 执行现有会话型 Agent 节点
- **WHEN** 脚本、选题、声音或作品 Adapter 请求模型生成、评审或修改
- **THEN** Adapter SHALL 提交 Definition 声明的 node key 与结构化动态输入
- **AND** AuditedModelExecutor SHALL 编译 Prompt、持久化 ModelCall 后调用 provider
- **AND** 现有输出解析、业务写入、Run/Step 和 Gate SHALL 保持原归属

#### Scenario: 执行非会话型 LLM 节点
- **WHEN** 项目策略草稿、直接脚本生成或主题组评审等操作调用模型
- **THEN** 通用 Run 生命周期 SHALL 固定 Definition 与模型 binding
- **AND** 内部全部模型步骤和重试 SHALL 通过相同受审计入口
- **AND** 每个步骤 SHALL 关联自己的 model_call_id

### Requirement: Rust 生产 LLM 节点迁移必须完整且唯一

本变更 SHALL 一次迁移仓库中全部现有 Rust 生产文本模型调用，不得只迁移示范 Agent、保留旧 Prompt builder 执行路径或允许未审计 provider 调用。

#### Scenario: 静态扫描生产调用入口
- **WHEN** CI 扫描 backend 与 crates 的生产代码
- **THEN** 裸 `LLMClient` 调用 SHALL 只存在于底层 provider、受审计 executor 或明确测试 allowlist
- **AND** 项目策略、脚本、选题生成、质量评审、重写、主题组评审、声音和作品节点 SHALL 全部存在 Definition inventory
- **AND** 任一遗漏 SHALL 阻止完成迁移

#### Scenario: 同一业务请求执行模型
- **WHEN** 任一已迁移业务请求需要一个或多个模型步骤
- **THEN** 每个步骤 SHALL 只有一个生产执行入口
- **AND** SHALL NOT 产生双模型调用、双 Assistant 消息或新旧审计双写

### Requirement: Definition 与调用审计不得改变领域 Kernel 所有权

Prompt 编译和 ModelCall 审计 SHALL 作为通用执行能力接入，Rust Kernel 与各 Adapter 的领域状态、Repository、事务、确认和 Gate 归属 SHALL 保持不变。

#### Scenario: 模型输出触发领域写入
- **WHEN** 已审计模型调用返回可解析输出
- **THEN** 对应 Adapter SHALL 继续按现有业务校验和事务写入领域数据
- **AND** ModelCall 层 SHALL NOT 直接写脚本、选题、声音、作品或发布表
- **AND** 失败输出 SHALL NOT 绕过现有失败收尾

#### Scenario: 用户输入要求绕过领域 Gate
- **WHEN** 动态用户输入或模型输出要求执行付费、发布、删除或其他受控动作
- **THEN** Kernel SHALL 继续调用拥有规则的领域 Gate
- **AND** PromptCompiler 或审计 wrapper SHALL NOT 授予额外权限
