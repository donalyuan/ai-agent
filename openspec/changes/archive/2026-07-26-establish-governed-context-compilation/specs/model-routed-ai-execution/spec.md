## ADDED Requirements

### Requirement: 文本模型必须显式绑定版本化 Tokenizer Profile

PostgreSQL `ai_models` SHALL 为每个可执行文本模型显式保存 Tokenizer Profile key/version；Rust 与 Pi SHALL 在建立 binding 和每次调用前解析同一 Registry Profile，并把 Profile 选择纳入非敏感模型行为证据。

#### Scenario: 建立文本模型 binding
- **WHEN** Runtime 为文本模型建立 Session、Conversation 或 Run binding
- **THEN** 模型记录 SHALL 引用存在且适用协议/上游模型的 Tokenizer Profile
- **AND** `behavior_fingerprint` SHALL 包含 Profile key/version 与影响预算的行为配置
- **AND** Rust/TypeScript SHALL 对相同规范化配置计算相同 fingerprint

#### Scenario: Tokenizer 配置变化
- **WHEN** ai_models 的 Profile key/version、安全相关 context window 或行为 settings 发生变化
- **THEN** 新 fingerprint SHALL 与既有 binding 不同
- **AND** 既有作用域 SHALL 在模型请求前要求显式 rebind/fork
- **AND** Runtime SHALL NOT 把变化视为凭据轮换

#### Scenario: 模型缺少兼容 Profile
- **WHEN** 文本模型未配置 Profile、引用不存在或 Profile 不适用于当前协议/模型
- **THEN** Runtime SHALL 返回 `tokenizer_profile_unavailable`
- **AND** SHALL NOT 创建可执行 binding、调用 provider 或回退默认 tokenizer

