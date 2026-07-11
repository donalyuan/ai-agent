## ADDED Requirements

### Requirement: 每轮 Agent 模型调用必须显式选择模型

通用 Agent Runtime SHALL 要求每轮可能调用模型的消息携带 `model_id`，并 SHALL 将模型选择固化到本轮运行和全部内部步骤；会话本身 SHALL NOT 永久绑定模型。

#### Scenario: script Agent 本轮使用选中模型

- **GIVEN** 已存在 script Agent 会话
- **WHEN** 操作者发送消息并提交启用文本模型 ID
- **THEN** Runtime SHALL 使用该模型完成本轮意图判断、脚本生成或分镜修改
- **AND** `agent_runs` SHALL 保存模型引用与不含凭据的调用快照

#### Scenario: topic Agent 内部步骤继承模型

- **GIVEN** 已存在 topic Agent 会话
- **WHEN** 操作者发送生成或补充消息并提交启用文本模型 ID
- **THEN** Runtime SHALL 让候选生成、质量闸门、最多一次重写和同模型重试使用该模型
- **AND** Runtime SHALL NOT 自动切换到其他模型

#### Scenario: 下一轮允许切换模型

- **GIVEN** 会话上一轮使用模型 A
- **WHEN** 操作者下一轮提交模型 B
- **THEN** Runtime SHALL 使用模型 B 处理新一轮
- **AND** 上一轮运行 SHALL 保留模型 A 的快照

#### Scenario: 缺少或不可用模型

- **WHEN** 一轮消息会触发模型调用但未提交模型、模型已停用或类型不是文本
- **THEN** Runtime SHALL 返回稳定模型错误
- **AND** Runtime SHALL NOT 调用环境变量模型、默认硬编码模型或其他供应商
