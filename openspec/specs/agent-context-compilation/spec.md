# agent-context-compilation Specification

## Purpose
TBD - created by archiving change establish-governed-context-compilation. Update Purpose after archive.
## Requirements
### Requirement: Context 候选必须使用可治理的原子合同

系统 SHALL 在进入 Context Compiler 前把动态输入转换为 `ContextCandidate`；每个候选 SHALL 包含稳定 ID、来源及来源版本、trust、priority、required、稳定 `render_order`、observed_at、可选 valid_until、可选 supersedes、内容 hash，以及脱敏文本或不可变资产引用。

#### Scenario: Adapter 提交有效原子候选
- **WHEN** Rust Adapter 或 Pi Runtime 为一个 LLM node 装配动态 Context
- **THEN** 每个可独立选择的事实、参考、用户输入或消息 SHALL 成为独立 ContextCandidate
- **AND** Tool request/result SHALL 使用稳定 group ID 组成不可拆分原子组
- **AND** Adapter SHALL NOT 以整段预格式化 Prompt 或完整 Context blob 隐藏内部来源与优先级

#### Scenario: 候选合同非法
- **WHEN** 候选缺少稳定身份、来源、版本、时间、hash 或同时包含非法文本与资产表示
- **THEN** Context Compiler SHALL 返回稳定 schema 错误
- **AND** SHALL NOT 编译 Prompt、创建 ModelCall 或调用模型

### Requirement: Context Policy 必须独立治理信任、优先级、必需性与有效性

`ContextPolicyDefinition` SHALL 独立声明允许来源、trust、priority、required、freshness、原子组和稳定排序规则；trust SHALL NOT 被直接解释为裁剪优先级。

#### Scenario: 候选在预算内竞争
- **WHEN** 多个合法候选竞争有限动态预算
- **THEN** Compiler SHALL 按 P0 当前用户指令/插话、节点必需状态和 Tool 原子组，P1 当前有效已确认事实，P2 当前分支近期消息和用户明确参考，P3 普通参考与有损摘要，P4 候选内容的默认层级选择
- **AND** 具体 node SHALL 可通过精确版本 Policy 收紧来源和 required 项
- **AND** node Policy SHALL NOT 降低 P0 保护

#### Scenario: 同一层级候选超过预算
- **WHEN** 同一 priority 内只有部分候选可容纳
- **THEN** Compiler SHALL 使用 Policy 声明的稳定排序键确定性选择
- **AND** 相同输入、固定时钟、Policy 和 tokenizer SHALL 得到相同结果与 digest

#### Scenario: 预算优先级与最终展示顺序不同
- **WHEN** 高优先级候选必须优先保留，但既有 Prompt 合同要求另一个入选候选先展示
- **THEN** Compiler SHALL 先按 priority、required 与 Policy 稳定键完成预算选择
- **AND** SHALL 再按候选 `render_order` 与稳定 tie-break 生成最终逻辑输入
- **AND** SHALL NOT 通过降低用户指令优先级、伪造来源或依赖输入数组顺序维持 Prompt 顺序

### Requirement: Context 有效期、替代、去重与冲突必须确定性处理

Context Compiler SHALL 在预算选择前处理 valid_until、supersedes、稳定身份和内容 hash；不得调用 LLM 执行语义去重或事实冲突裁决。

#### Scenario: 候选过期或被替代
- **WHEN** 固定编译时钟晚于候选 valid_until，或同一权威来源存在引用该候选的有效 supersedes 版本
- **THEN** 旧候选 SHALL 失去入选资格
- **AND** required 候选过期且不存在有效替代时 SHALL 明确失败
- **AND** 预算充足 SHALL NOT 使失效候选重新入选

#### Scenario: 候选重复
- **WHEN** 候选具有相同稳定身份和版本，或内容 hash 按 Policy 构成确定性重复
- **THEN** Compiler SHALL 只保留 Policy 指定的规范候选
- **AND** 其余候选 SHALL 记录稳定 duplicate decision code

#### Scenario: 已确认事实冲突
- **WHEN** 多个仍有效的 confirmed_fact 在同一事实键上不可消解地冲突
- **THEN** Compiler SHALL 返回稳定 `context_conflict`
- **AND** SHALL NOT 按时间、priority 或输入顺序擅自选择
- **AND** 普通 reference/candidate 冲突可以并存但 SHALL 保留来源与冲突标记

### Requirement: Tokenizer Profile 必须显式且版本化

每次 Context 编译 SHALL 使用 binding 固定的不可变 `TokenizerProfile` key/version；Profile SHALL 明确使用精确 tokenizer 或声明算法、适用范围与安全余量的保守策略，且 SHALL NOT 静默回退。

#### Scenario: 使用精确 tokenizer
- **WHEN** 文本模型绑定到受支持的精确 tokenizer profile
- **THEN** Rust 与 TypeScript SHALL 对 contract fixture 计算相同 token 数
- **AND** 计数 SHALL 覆盖模型消息封装、Tool Schema、输出 Schema 与 provider 协议开销

#### Scenario: 使用声明式保守策略
- **WHEN** 模型无法精确映射且显式绑定一个保守 tokenizer profile
- **THEN** 该 Profile SHALL 固定算法、适用模型/协议范围和安全余量
- **AND** Runtime SHALL 在审计快照中标明 conservative 模式
- **AND** Runtime SHALL NOT 自动切换到字符数、字节数或其他未声明估算

#### Scenario: Tokenizer Profile 不可用
- **WHEN** 模型缺少 Profile、版本不存在/失效、实现不支持其适用范围或计数失败
- **THEN** Runtime SHALL 在模型请求前返回稳定 `tokenizer_profile_unavailable`
- **AND** SHALL NOT 使用默认估算、旧 Profile 或 provider 回退

### Requirement: Context token 预算必须保留完整固定开销与输出上限

动态 Context 预算 SHALL 等于模型 context window 减去 System Prompt、User 模板固定部分、Tool Schema、输出 Schema、协议封装、最大输出 token 和 Profile 声明的安全余量；Adapter SHALL NOT 自行覆盖该预算。

#### Scenario: 计算可用动态预算
- **WHEN** Runtime 已固定模型、Prompt、Policy、Tokenizer Profile、Tool 与输出 Schema
- **THEN** Compiler SHALL 先对所有固定开销计数
- **AND** SHALL 完整保留有效最大输出 token
- **AND** SHALL 把每项固定开销和动态可用量写入预算账本

#### Scenario: 固定部分或必需 Context 超限
- **WHEN** 固定开销已耗尽窗口，或全部合法裁剪后 required 候选仍无法容纳
- **THEN** Compiler SHALL 返回带具体 reason code 的 `context_budget_exceeded`
- **AND** SHALL NOT 截断 JSON、Tool 原子组、已确认事实或其他 required 内容
- **AND** SHALL NOT 静默借用输出预算

### Requirement: Context 编译必须使用两阶段 Prompt 与最终复核

Runtime SHALL 按“解析 binding/policy、编译固定 Prompt、计算动态预算、选择 Context、完成 Prompt、最终 token 复核、持久化 Context、持久化 ModelCall、调用模型”的顺序执行。

#### Scenario: 最终请求通过复核
- **WHEN** Context 选择完成并渲染最终 Prompt
- **THEN** Runtime SHALL 使用同一 Tokenizer Profile 对最终逻辑请求重新计数
- **AND** 最终输入与预留输出之和 SHALL 不超过模型 context window
- **AND** ContextSnapshot 与 prepared ModelCall 持久化成功后才可调用 provider

#### Scenario: 最终复核失败
- **WHEN** 最终请求 token 超过已固定预算
- **THEN** Runtime SHALL 返回 `context_budget_exceeded`
- **AND** SHALL NOT 临时重新裁剪、侵占输出预算或透明重试

#### Scenario: Provider 仍报告 context overflow
- **WHEN** 最终复核通过但 provider 返回 context overflow
- **THEN** Runtime SHALL 把调用记录为失败并标记 tokenizer/profile 兼容性缺陷
- **AND** SHALL NOT 在同一 ModelCall 中缩短请求后再次调用

### Requirement: Context 编译成功与失败必须形成不可变审计证据

成功编译 SHALL 创建不可变 `ContextSnapshot`；编译失败 SHALL 创建不可变 `ContextCompileAttempt`，且没有实际模型调用时 SHALL NOT 创建 ModelCall。

#### Scenario: 保存成功 ContextSnapshot
- **WHEN** Context 编译和最终复核成功
- **THEN** Snapshot SHALL 保存固定时钟、Policy 与 tokenizer/profile 精确版本、模型窗口、预算账本，以及每个 decision 的 trust、priority、required、render_order、全部 decision、最终顺序和 digest
- **AND** 采用项 SHALL 保存脱敏逻辑全文或不可变资产引用
- **AND** 排除项 SHALL 只保存稳定身份、来源/version、内容 hash、token 数和 decision code

#### Scenario: 保存失败 ContextCompileAttempt
- **WHEN** 编译因 schema、过期 required、冲突、预算或 tokenizer 错误失败
- **THEN** Runtime SHALL 在返回错误前保存脱敏后的失败阶段、预算和最小化 candidate decision 证据
- **AND** SHALL NOT 保存排除项未发送全文
- **AND** SHALL NOT 创建 prepared ModelCall

### Requirement: Context 结果必须支持跨 Runtime 确定性回放

Rust/PostgreSQL 与 Pi/SQLite SHALL 使用相同 `schema_version`、枚举、canonical digest 和 dry-run 语义保存 Context 记录。

#### Scenario: dry-run 回放成功快照
- **WHEN** 操作者回放一个历史 ContextSnapshot
- **THEN** Runtime SHALL 使用历史固定时钟、Policy、Tokenizer Profile、候选元数据与采用项内容重建预算和选择结果
- **AND** SHALL 校验最终 digest 并返回结构化 diff
- **AND** SHALL 保证零模型调用、零 Tool、零领域写入和零 Session/Run 变更

#### Scenario: 后来源数据发生变化
- **WHEN** 历史 Snapshot 引用的当前数据库事实已经修改或删除
- **THEN** dry-run SHALL 使用历史不可变证据而不是当前来源覆盖快照
- **AND** 缺少历史 Policy 或 tokenizer 实现时 SHALL 明确报告历史依赖不可用，不得使用当前版本伪造成功

