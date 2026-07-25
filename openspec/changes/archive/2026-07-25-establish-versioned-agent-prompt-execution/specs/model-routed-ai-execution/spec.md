## MODIFIED Requirements

### Requirement: Pi Runtime 必须保存非敏感模型执行快照
Runtime SHALL 在 Session 创建时保存固定 `model_id + behavior_fingerprint` binding，并 SHALL 在每次模型调用前把实际解析的非敏感模型配置写入独立 ModelCall；Session entry SHALL 只保存稳定 binding、model_call_id 或摘要关联。

#### Scenario: 模型编辑不覆盖历史 Session 快照
- **GIVEN** 会话已使用模型 A 完成一轮执行
- **WHEN** 随后编辑模型 A 的地址、上游模型、reasoning、输出上限或行为 settings
- **THEN** 历史 Session binding、entry 和 ModelCall SHALL 保留原快照
- **AND** 新一轮 SHALL 因 behavior_fingerprint 不一致在请求前阻断
- **AND** SHALL NOT 追加快照后静默继续执行

#### Scenario: 快照不含凭据
- **WHEN** Runtime 保存模型 binding、ModelCall 或返回会话信息
- **THEN** 数据 SHALL NOT 包含 API Key、API Secret、Authorization Header、Cookie 或凭据环境变量
- **AND** 凭据轮换 SHALL NOT 改变 behavior_fingerprint

## ADDED Requirements

### Requirement: 文本模型行为必须使用稳定 fingerprint 固定

Rust 与 Pi Runtime SHALL 对影响文本模型行为的非敏感配置计算相同语义的 `behavior_fingerprint`，并 SHALL 在 Session、Conversation 或非会话 Run 作用域固定该值。

#### Scenario: 计算模型 behavior fingerprint
- **WHEN** Runtime 解析一个启用文本模型
- **THEN** fingerprint SHALL 包含协议、规范化请求地址身份、上游模型、reasoning、输出上限、context window 和其他行为相关 settings
- **AND** SHALL 使用稳定 canonical serialization 与 hash 算法
- **AND** SHALL NOT 包含 API Key、认证头或其他凭据

#### Scenario: Rust 与 Pi 解析相同模型配置
- **WHEN** 两个 Runtime 对同一规范化模型配置计算 fingerprint
- **THEN** 结果 SHALL 相同
- **AND** 跨语言 contract fixture SHALL 覆盖字段顺序、空值和默认值规范化

#### Scenario: 仅凭据发生轮换
- **WHEN** 模型部署只更新凭据且行为字段不变
- **THEN** behavior_fingerprint SHALL 保持不变
- **AND** 已绑定 Session/Conversation SHALL 可在重新解析凭据后继续执行

### Requirement: Agent 模型必须满足 Definition 能力要求

系统 SHALL 在建立 binding 和每次调用前校验模型类型、协议、Tool Calling、结构化输出、视觉、reasoning 与 context window 等能力满足 AgentDefinition，且 SHALL fail-closed。

#### Scenario: 模型能力满足 Agent
- **WHEN** 操作者选择的启用模型满足 Definition 全部必需能力
- **THEN** 系统 SHALL 保存能力校验结果和模型 fingerprint
- **AND** 后续调用 SHALL 使用该固定 binding

#### Scenario: 模型能力不兼容
- **WHEN** 模型缺少任一必需能力或当前配置无法证明能力
- **THEN** 系统 SHALL 返回稳定 `model_capability_mismatch`
- **AND** SHALL NOT 创建可执行 binding 或调用供应商
- **AND** SHALL NOT 回退默认模型

### Requirement: 文本模型调用与重试必须具备调用级模型证据

所有 Rust/Pi 生产文本模型步骤 SHALL 在 prepared ModelCall 中保存实际 model_id、behavior_fingerprint 与不含凭据的配置快照；每个业务层或 Runtime 重试 SHALL 使用相同 binding 并创建新记录。

#### Scenario: 多步骤 topic Agent
- **WHEN** topic Agent 执行候选生成、质量评审、重写和再评审
- **THEN** 每一步 SHALL 保存独立 ModelCall 和模型配置快照
- **AND** 所有步骤 SHALL 使用相同 model_id 与 behavior_fingerprint
- **AND** 任一步 SHALL 可追溯到对应 Agent Run/Step

#### Scenario: 允许的临时错误重试
- **WHEN** Runtime 按既有策略重试文本模型调用
- **THEN** 重试 SHALL 使用固定模型 binding
- **AND** 每个 attempt SHALL 保存独立模型证据和终态
- **AND** 系统 SHALL NOT 自动选择另一模型或供应商

#### Scenario: 发现不可审计透明重试
- **WHEN** 底层 provider 配置会在单个 ModelCall 内执行不可观察的多次外部请求
- **THEN** Runtime SHALL 关闭该透明重试并提升到受审计 wrapper
- **AND** SHALL 保持已批准的 attempt 上限和退避规则
- **AND** 对已产生部分流输出的调用 SHALL NOT 静默重试
