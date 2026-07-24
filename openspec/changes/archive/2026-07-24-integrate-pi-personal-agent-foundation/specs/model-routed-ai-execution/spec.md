## ADDED Requirements

### Requirement: Pi Runtime 必须复用 PostgreSQL 模型配置唯一来源
Pi Runtime SHALL 按每个 Session 请求提交的 `model_id` 从 PostgreSQL `ai_models` 解析启用文本模型，并 SHALL 使用该记录构造请求级 Pi Provider/Model。

#### Scenario: 解析 Responses 文本模型
- **WHEN** 会话提交启用且协议为 `openai_responses` 的文本模型 ID
- **THEN** Runtime SHALL 使用数据库中的请求根地址、上游模型、Bearer Key、推理等级、输出上限和超时
- **AND** SHALL 通过 Pi `openai-responses` provider 执行

#### Scenario: 解析 Chat Completions 文本模型
- **WHEN** 会话提交启用且协议为 `openai_chat_completions` 的文本模型 ID
- **THEN** Runtime SHALL 使用同一数据库记录构造 Pi `openai-completions` provider
- **AND** SHALL NOT 根据 URL 形态猜测协议

#### Scenario: 不得使用第二模型目录
- **WHEN** 模型记录不存在、停用、删除或不兼容
- **THEN** Runtime SHALL 返回稳定错误
- **AND** SHALL NOT 使用环境变量、Pi 内建模型目录、默认模型或其他供应商兜底

### Requirement: Pi Runtime 必须保存非敏感模型执行快照
Runtime SHALL 在模型调用前把实际解析的非敏感模型配置作为 Session entry 保存，并 SHALL 在同一轮内部步骤和允许重试中使用相同配置。

#### Scenario: 模型编辑不覆盖历史 Session 快照
- **GIVEN** 会话已使用模型 A 完成一轮执行
- **WHEN** 随后编辑模型 A 的地址或上游模型
- **THEN** 历史 Session entry SHALL 保留原快照
- **AND** 新一轮 SHALL 重新解析当前启用配置并追加新快照

#### Scenario: 快照不含凭据
- **WHEN** Runtime 保存模型快照或返回会话信息
- **THEN** 数据 SHALL NOT 包含 API Key、API Secret、Authorization Header 或凭据环境变量
