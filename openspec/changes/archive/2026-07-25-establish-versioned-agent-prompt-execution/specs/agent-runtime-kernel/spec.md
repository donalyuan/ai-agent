## MODIFIED Requirements

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

## ADDED Requirements

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
