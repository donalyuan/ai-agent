## MODIFIED Requirements

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

## ADDED Requirements

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
