## ADDED Requirements

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

