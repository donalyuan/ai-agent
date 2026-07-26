# model-routed-ai-execution Specification

## Purpose
TBD - created by archiving change manage-ai-models-and-workspace-selection. Update Purpose after archive.
## Requirements
### Requirement: 新 AI 调用必须显式解析有效模型

所有新文本和图片 AI 调用 SHALL 携带 `model_id`，系统 SHALL 在调用供应商前校验模型存在、未删除、已启用且类型匹配。

#### Scenario: 成功解析文本模型

- **WHEN** 文本业务请求提交一个启用的 `text` 模型 ID
- **THEN** 系统 SHALL 从数据库读取协议、地址、凭据和运行参数
- **AND** 系统 SHALL 构造本次请求专用的文本客户端

#### Scenario: 模型类型不匹配

- **WHEN** 图片生成请求提交文本模型 ID
- **THEN** 系统 SHALL 返回 `model_type_mismatch`
- **AND** 系统 SHALL NOT 调用任何供应商

#### Scenario: 不得回退旧配置

- **WHEN** 请求未提交模型、模型不存在或模型已停用
- **THEN** 系统 SHALL 返回稳定模型错误
- **AND** 系统 SHALL NOT 回退环境变量、默认硬编码模型或其他供应商

### Requirement: 每次真实调用必须记录非敏感模型快照

系统 SHALL 在真正发起供应商请求前，将 `model_id` 和不含凭据的实际配置快照写入对应运行或生成任务。

#### Scenario: 文本运行保存快照

- **WHEN** Agent 或直接生成接口准备调用文本模型
- **THEN** `agent_runs` 或对应运行记录 SHALL 保存模型引用与快照
- **AND** 快照 SHALL 包含供应商、协议、协议版本、地址、上游模型标识、推理等级和超时
- **AND** 快照 SHALL NOT 包含 API Key 或 API Secret

#### Scenario: 模型编辑不改变历史快照

- **GIVEN** 一次运行已经保存模型快照
- **WHEN** 管理员随后编辑模型名称、地址或协议
- **THEN** 历史运行 SHALL 保留原快照
- **AND** 历史审计 SHALL NOT 使用当前模型字段覆盖原值

### Requirement: 内部步骤和重试必须复用同一模型

一次业务操作内的所有模型步骤和允许的临时错误重试 SHALL 使用请求解析得到的同一模型配置。

#### Scenario: 选题生成包含多个步骤

- **WHEN** topic Agent 执行候选生成、质量闸门和最多一次重写
- **THEN** 各步骤 SHALL 使用同一 `model_id`
- **AND** 每个运行步骤 SHALL 可追溯到该模型

#### Scenario: 临时错误重试

- **WHEN** 文本或图片供应商发生允许重试的临时错误
- **THEN** 重试 SHALL 使用同一模型部署
- **AND** 系统 SHALL NOT 自动选择另一个模型或供应商

### Requirement: Python Worker 必须从数据库模型配置构造图片 provider

Python Worker SHALL 根据图片任务的 `model_id` 读取统一模型配置，并 SHALL 按 `api_protocol` 构造 `openai_images` 或 `volcengine_ark_images` provider。

#### Scenario: 执行 OpenAI Images 任务

- **GIVEN** 待执行任务引用启用的 `openai_images` 模型
- **WHEN** Worker 领取任务
- **THEN** Worker SHALL 读取数据库中的请求根地址、上游模型、Bearer Key、超时和图片设置
- **AND** Worker SHALL 在调用前保存任务模型快照

#### Scenario: 执行火山方舟图片任务

- **GIVEN** 待执行任务引用启用的 `volcengine_ark_images` 模型
- **WHEN** Worker 领取任务
- **THEN** Worker SHALL 使用数据库中的请求根地址、上游模型、Bearer Key、超时和图片设置构造 Ark provider
- **AND** Worker SHALL 在调用前保存不含凭据的任务模型快照
- **AND** Worker SHALL NOT 读取 `JIMENG_*` 或其他环境变量作为运行时兜底

#### Scenario: 旧 Jimeng 配置不得调用上游

- **GIVEN** Worker 读取到 `api_protocol=jimeng_visual` 的图片模型配置
- **WHEN** Worker 解析该模型
- **THEN** Worker SHALL 返回 `invalid_model_config`
- **AND** Worker SHALL NOT 发起供应商请求

#### Scenario: 其他不兼容图片协议不得调用上游

- **GIVEN** Worker 读取到 `api_protocol=openai_responses` 或其他未支持协议的图片模型配置
- **WHEN** Worker 解析该模型
- **THEN** Worker SHALL 返回 `invalid_model_config`
- **AND** Worker SHALL 将已领取任务标记为失败
- **AND** Worker SHALL NOT 发起供应商请求

### Requirement: 停用模型必须阻止待执行任务产生外部调用

系统 SHALL 在接受新请求和 Worker 真正执行任务两个时点校验模型状态，并 SHALL 如实处理已经发出的在途请求。

#### Scenario: 待执行图片任务对应模型被停用

- **GIVEN** 图片任务已创建但尚未调用供应商
- **AND** 管理员停用了任务模型
- **WHEN** Worker 领取任务
- **THEN** Worker SHALL 将任务标记为失败并记录模型不可用错误
- **AND** Worker SHALL NOT 调用供应商

#### Scenario: HTTP 请求已在途

- **GIVEN** 供应商 HTTP 请求已经发出
- **WHEN** 管理员停用模型
- **THEN** 系统 SHALL 允许该在途请求自然完成并记录真实结果
- **AND** 系统 SHALL NOT 伪造取消成功

### Requirement: 系统必须提供幂等的一次性环境配置导入

系统 SHALL 提供显式命令，将现有文本 OpenAI 和 OpenAI 图片环境配置导入统一模型表；导入完成后运行时 SHALL 不再读取这些变量作为模型配置，且系统 SHALL NOT 导入旧 Jimeng 环境配置。

#### Scenario: 首次导入现有配置

- **WHEN** 操作者使用明确的原文凭据确认参数运行导入命令
- **THEN** 系统 SHALL 为存在完整 OpenAI 环境配置的部署创建模型记录
- **AND** 系统 SHALL 规范化旧完整端点为请求根地址
- **AND** 系统 SHALL 为每条导入记录保存稳定 `source_key`

#### Scenario: 旧 Jimeng 环境变量不再导入

- **GIVEN** 环境中仍存在任意 `JIMENG_*` 配置
- **WHEN** 操作者运行模型导入命令
- **THEN** 系统 SHALL 忽略这些旧配置
- **AND** 系统 SHALL NOT 创建 `jimeng_visual` 或 `volcengine_ark_images` 模型
- **AND** Ark 模型 SHALL 由 Admin 或模型 API 使用 Bearer API Key 显式创建

#### Scenario: 重复执行导入

- **GIVEN** 相同 `source_key` 已存在
- **WHEN** 操作者再次运行导入命令
- **THEN** 系统 SHALL 跳过已有记录
- **AND** 系统 SHALL NOT 覆盖后台编辑后的地址、协议、凭据或默认状态

#### Scenario: 导入后缺少可用模型

- **WHEN** 导入完成但某类没有完整配置
- **THEN** 系统 SHALL 报告该类型未配置
- **AND** 系统 SHALL NOT 创建带空凭据的可用模型
- **AND** 运行时 SHALL NOT 回退旧环境变量

### Requirement: 模型路由改造必须保持现有费用边界

模型选择和协议路由 SHALL NOT 放宽图片生成数量、重试和人工确认规则，也 SHALL NOT 发起视频生成验证。

#### Scenario: 图片候选费用上限保持不变

- **WHEN** 操作者使用任一启用图片模型创建候选任务
- **THEN** 每分镜候选数 SHALL 保持 `1-4`
- **AND** 每脚本批量图片数 SHALL 不超过 `48`
- **AND** 临时错误最多 SHALL 按既有规则重试一次

#### Scenario: 禁止自动跨模型重试

- **WHEN** 当前图片模型返回永久错误或重试后仍失败
- **THEN** 系统 SHALL 将任务标记失败
- **AND** 系统 SHALL NOT 自动调用其他模型或供应商

#### Scenario: 视频模型不产生费用

- **WHEN** 管理员创建、编辑或启停视频模型
- **THEN** 系统 SHALL NOT 调用视频供应商
- **AND** 自动化验证 SHALL 使用本地校验或 fake provider

### Requirement: 图片协议必须映射到稳定任务审计值

Backend SHALL 仅根据图片模型协议写入素材任务 provider 审计值，并 SHALL 保持任务模型引用和快照可追溯。

#### Scenario: Ark 模型创建素材任务

- **GIVEN** 操作者选择启用的 `volcengine_ark_images` 图片模型
- **WHEN** Backend 创建图片候选任务
- **THEN** 任务 SHALL 保存该 `model_id`
- **AND** 任务 `provider` SHALL 为 `volcengine-ark`
- **AND** 任务 SHALL NOT 保存 `jimeng`

#### Scenario: OpenAI Images 审计值保持不变

- **GIVEN** 操作者选择启用的 `openai_images` 图片模型
- **WHEN** Backend 创建图片候选任务
- **THEN** 任务 `provider` SHALL 继续为 `gpt-image-2`

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

