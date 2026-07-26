# agent-definition-registry Specification

## Purpose
TBD - created by archiving change establish-versioned-agent-prompt-execution. Update Purpose after archive.
## Requirements
### Requirement: 仓库级 Definition Registry 必须是 Agent 与 Prompt 的唯一治理事实源

系统 SHALL 在仓库级版本化 Registry 中保存结构化 `AgentDefinition`、`PromptDefinition`、独立模板和统一 schema；Rust 与 TypeScript Runtime SHALL 只读加载同一事实源，数据库 SHALL NOT 覆盖运行定义。

#### Scenario: 两个 Runtime 加载相同发布定义
- **WHEN** Rust backend 与 Pi Runtime 启动并加载同一发布版本
- **THEN** 两个 loader SHALL 对相同 definition key/version 计算相同 canonical digest
- **AND** 每个 Agent SHALL 只有一个明确的 executor owner
- **AND** 数据库中的发布记录 SHALL 只保存 key、version、digest、状态和发布时间等不可变证据
- **AND** 数据库 SHALL 分离不可变 Definition 内容证据与按 registry digest 保存的完整生命周期 manifest 快照

#### Scenario: Registry 存在非法定义
- **WHEN** Registry 包含未知字段、重复 key/version、缺失模板、非法跨 owner 引用、无效输出 Schema 或 digest 不一致
- **THEN** 对应服务 SHALL 拒绝完成启动或发布校验
- **AND** 系统 SHALL NOT 使用旧缓存、数据库正文或内建默认 Prompt 继续执行

#### Scenario: 已发布版本被原地修改
- **GIVEN** 某 definition key/version 已存在不可变发布记录
- **WHEN** 新发布使用相同 key/version 但 canonical digest 不同
- **THEN** 发布校验 SHALL 失败
- **AND** 变更 SHALL 通过新的版本号表达

#### Scenario: 发布仅切换生命周期状态
- **GIVEN** 某 definition key/version 的内容已经发布
- **WHEN** 新代码发布只把其状态在 active、supported 或 revoked 之间切换
- **THEN** Definition 内容 digest SHALL 保持不变
- **AND** registry digest SHALL 反映新的生命周期 manifest
- **AND** 新 registry digest SHALL 追加不可变 manifest 快照而不得覆盖先前状态证据
- **AND** 历史 Session binding SHALL NOT 因状态字段变化被判定为内容漂移

### Requirement: AgentDefinition 必须声明稳定执行合同

每个 `AgentDefinition` SHALL 声明稳定 `agent_key`、版本、executor owner、角色、目标、约束、模型能力要求、允许 Tool/profile 和各 LLM 节点引用的精确 `PromptDefinition` 版本，且 SHALL NOT 复制模型凭据或部署配置。

#### Scenario: Agent Definition 通过静态校验
- **WHEN** loader 校验一个有效 AgentDefinition
- **THEN** 每个声明的 LLM node SHALL 引用存在且 owner 兼容的 PromptDefinition 精确版本
- **AND** 模型要求 SHALL 只描述文本、Tool Calling、结构化输出、视觉、reasoning 和最小 context window 等能力
- **AND** Definition SHALL NOT 包含 API Key、Authorization 或数据库模型部署正文

#### Scenario: 执行未声明节点或 Tool
- **WHEN** Runtime 请求执行 Definition 未声明的 node key、Tool 或 profile
- **THEN** 系统 SHALL 返回稳定 definition contract 错误
- **AND** SHALL NOT 编译 Prompt、调用模型或执行 Tool

### Requirement: PromptCompiler 必须固定 System 与 User 信任边界

`PromptCompiler` SHALL 只从精确版本 PromptDefinition 和结构化输入编译模型消息；System 层 SHALL 仅包含平台规则、Agent 角色与约束、节点职责、输出契约和 Tool/能力边界，动态 Context 与用户输入 SHALL 进入 User 层并携带信任等级和来源。

#### Scenario: 编译包含动态 Context 的 Prompt
- **WHEN** 编译输入包含 `confirmed_fact`、`reference`、`user_instruction`、`steer`、`follow-up` 或 `candidate` 片段
- **THEN** Compiler SHALL 把每个片段作为带稳定 ID、来源和信任等级的 User 层内容
- **AND** SHALL NOT 把动态字符串替换进 System 模板
- **AND** 输出 SHALL 包含实际 System/User 逻辑消息、输出 Schema、Tool Schema 和 definition digest

#### Scenario: 编译输入不满足定义
- **WHEN** 编译输入缺失必填变量、包含未知变量、类型或大小非法、引用无效版本或违反输出/Tool 合同
- **THEN** Compiler SHALL 返回稳定编译错误
- **AND** SHALL NOT 用空值、旧 Prompt、默认 system prompt 或字符串拼接降级
- **AND** SHALL NOT 创建可执行 ModelCall 或发起模型请求

#### Scenario: output schema 包含请求级强类型约束
- **WHEN** 既有节点的 JSON Schema 数量或枚举约束由请求中的已声明变量决定
- **THEN** PromptDefinition SHALL 以完整值占位符声明该 schema 参数
- **AND** Compiler SHALL 校验变量类型后以原始 JSON 类型替换占位符
- **AND** SHALL NOT 允许未声明、未解析或通过字符串拼接生成的 schema 参数

### Requirement: Definition 生命周期必须控制可执行版本

Agent/Prompt 版本 SHALL 使用 `candidate`、`active`、`supported`、`revoked` 生命周期；普通执行只可创建于 active 版本，既有 Session 可继续使用 supported 版本，revoked 版本 SHALL 被阻断。

#### Scenario: 新 Session 解析 active 版本
- **WHEN** 操作者使用 `agent_key` 创建新 Pi Session 或 Rust Conversation
- **THEN** 系统 SHALL 固定当时唯一 active AgentDefinition 及其 PromptDefinition 精确版本
- **AND** 后续代码发布 SHALL NOT 静默改变该绑定

#### Scenario: candidate 被普通 Session 选择
- **WHEN** 普通 Session 创建或继续执行请求引用 candidate 版本
- **THEN** 系统 SHALL 拒绝执行
- **AND** candidate SHALL 只可用于静态验证、dry-run 或显式 EvalRun

#### Scenario: 已绑定版本被撤销
- **GIVEN** Session 绑定的版本状态变为 revoked
- **WHEN** Session 尝试继续调用模型
- **THEN** 系统 SHALL 在模型请求前阻断
- **AND** 历史读取、导出和 dry-run replay SHALL 保持可用
- **AND** 操作者 SHALL 通过显式 fork 迁移到受支持版本

### Requirement: 用户插话不得改变版本化执行定义

运行中的用户插话 SHALL 作为可审计 `steer`、`follow-up` 或领域修改请求进入后续 User 层输入，并 SHALL NOT 覆盖 Definition、已确认事实、正式 Memory 或领域 Gate。

#### Scenario: 用户在 Pi 运行中纠偏
- **GIVEN** Pi Session 正在执行模型或 Tool 步骤
- **WHEN** 操作者提交 steer 消息
- **THEN** Runtime SHALL 通过 Pi 公开 steering queue 在安全边界处理该消息
- **AND** 下一次受影响的 ModelCall SHALL 记录消息类型、内容、来源和关联 entry
- **AND** Agent/Prompt binding SHALL 保持不变

#### Scenario: 插话要求绕过 Gate
- **WHEN** 用户插话要求覆盖已确认产物、写入正式 Memory 或绕过成本、权限、质量或发布 Gate
- **THEN** 系统 SHALL 按现有领域规则拒绝或转为待确认请求
- **AND** SHALL NOT 因该文本修改 System 层或 Definition

### Requirement: 历史 Session 与 Conversation 必须按可证明信息迁移

系统 SHALL 以幂等迁移为历史 Pi Session 和 Rust Conversation 建立 Definition binding；无法证明的历史 Prompt、Context 或模型调用 SHALL NOT 被伪造。

#### Scenario: 迁移无自定义 system prompt 的 Pi Session
- **WHEN** 历史 Pi Session 首次打开且未保存自定义 `system_prompt`
- **THEN** Runtime SHALL 将其可审计地绑定到行为等价的 `personal.general@1`
- **AND** SHALL 追加迁移事件并保留原 Session Tree

#### Scenario: 迁移含自定义 system prompt 的 Pi Session
- **WHEN** 历史 Pi Session 保存了自定义 `system_prompt`
- **THEN** Runtime SHALL 将该 Session 标记为只读
- **AND** 继续执行 SHALL 要求显式 fork 到选定 `agent_key`
- **AND** 操作者 SHALL 明确选择丢弃旧文本或将其降级为可见普通 user instruction
- **AND** 旧文本 SHALL NOT 再作为 System 内容执行

#### Scenario: 迁移 Rust Conversation 与不完整历史 Run
- **WHEN** 迁移程序处理已知 `agent_type` 的 Rust Conversation 和缺少准确 Prompt/Context 的历史 Run
- **THEN** Conversation SHALL 确定性绑定对应 v1 Definition
- **AND** 不完整 Run SHALL 标记为 `legacy_partial_audit`
- **AND** 系统 SHALL NOT 为其生成伪造 ModelCall 或完整 PromptSnapshot

### Requirement: Definition Registry 必须治理 Context Policy 与 Tokenizer Profile

仓库级 Definition Registry SHALL 成为 `ContextPolicyDefinition` 与 `TokenizerProfile` 内容和生命周期的唯一事实源，并 SHALL 使用与 Agent/Prompt Definition 一致的不可变版本、跨语言 loader、canonical digest 和发布证据规则。

#### Scenario: Registry 加载 Context 定义
- **WHEN** Rust 与 TypeScript loader 加载同一发布 Registry
- **THEN** 两个 Runtime SHALL 对 Policy/Profile key、version、内容和 registry 计算相同 digest
- **AND** SHALL 校验未知字段、重复版本、非法来源规则、无效 tokenizer 实现和不兼容引用
- **AND** 任一校验失败 SHALL 阻止服务 ready

#### Scenario: Context 定义被原地修改
- **GIVEN** Policy 或 Profile key/version 已发布
- **WHEN** 新代码使用相同 key/version 发布不同内容 digest
- **THEN** 发布 SHALL 失败
- **AND** 算法、排序、预算、安全余量或适用范围变化 SHALL 使用新版本

### Requirement: Agent node 必须精确引用 Context Policy

新版 `AgentDefinition` 的每个 LLM node SHALL 在 PromptDefinition 精确引用之外固定一个 owner 兼容的 `ContextPolicyDefinition` 精确版本；普通执行 SHALL NOT 在线解析其他 Policy 覆盖该引用。

#### Scenario: 新 Session 解析 Agent node
- **WHEN** Runtime 为 active AgentDefinition 创建 Session、Conversation 或非会话 Run
- **THEN** binding SHALL 固定每个 node 的 Context Policy key/version/digest
- **AND** 后续发布 SHALL NOT 静默改变既有 binding

#### Scenario: Context 版本生命周期变化
- **WHEN** Policy 从 candidate 进入 active/supported/revoked，或 Profile 版本被撤销
- **THEN** candidate SHALL 仅用于静态验证、dry-run 和 EvalRun
- **AND** supported SHALL 只允许既有 binding 继续
- **AND** revoked SHALL 在模型请求前阻断但保留历史读取与回放

