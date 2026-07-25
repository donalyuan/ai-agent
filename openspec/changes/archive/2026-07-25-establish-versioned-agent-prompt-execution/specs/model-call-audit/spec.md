## ADDED Requirements

### Requirement: 每次实际模型调用必须先建立独立 ModelCall

Rust 与 Pi Runtime SHALL 为每次实际模型调用建立独立、不可复用的 `ModelCall`；脱敏后的完整逻辑输入持久化成功前 SHALL NOT 发起外部请求。

#### Scenario: 调用前持久化成功
- **WHEN** Runtime 已完成 Prompt 编译、模型绑定校验和脱敏
- **THEN** 系统 SHALL 先保存状态为 `prepared` 的 ModelCall
- **AND** 记录 SHALL 包含拥有者、node key、attempt、Agent/Prompt 版本、PromptSnapshot、模型 fingerprint、非敏感参数与 Tool Schema
- **AND** 只有持久化成功后 SHALL 调用模型供应商

#### Scenario: 调用前持久化失败
- **WHEN** ModelCall repository 不可用、schema 校验失败或脱敏检查拒绝输入
- **THEN** 系统 SHALL 返回稳定审计持久化错误
- **AND** SHALL NOT 发起任何供应商请求
- **AND** SHALL NOT 伪造 Assistant 成功消息或领域写入

#### Scenario: 重试模型调用
- **WHEN** 业务层或 Runtime 按既有策略执行一次允许的重试
- **THEN** 每个 attempt SHALL 创建新的 ModelCall ID
- **AND** 每条记录 SHALL 通过 root call ID 和递增 attempt 关联
- **AND** 新 attempt SHALL NOT 覆盖前一 attempt 的输入、错误或终态

### Requirement: ModelCall 必须保留唯一终态和完整结果证据

ModelCall SHALL 从 `prepared` 单向进入一次 `succeeded`、`failed` 或 `aborted` 终态，并 SHALL 保存脱敏输出、usage、错误和关联运行证据。

#### Scenario: 模型调用成功
- **WHEN** 供应商返回完整成功结果
- **THEN** ModelCall SHALL 保存脱敏后的实际输出、结构化解析状态、token/usage 和完成时间
- **AND** SHALL 只转换一次为 `succeeded`
- **AND** Agent Run/Step 或 Pi entry SHALL 关联该 `model_call_id`

#### Scenario: 模型调用失败或取消
- **WHEN** 供应商、流处理、结构化解析返回错误，或操作者取消调用
- **THEN** ModelCall SHALL 保存脱敏错误和已知的部分 usage/输出摘要
- **AND** SHALL 转换为 `failed` 或 `aborted`
- **AND** 失败记录 SHALL 保留且不得被后续重试改写

#### Scenario: 重复写入终态
- **GIVEN** ModelCall 已进入任一终态
- **WHEN** 事件重放或重复回调尝试再次收尾
- **THEN** repository SHALL 拒绝第二次终态转换
- **AND** 原终态与证据 SHALL 保持不变

### Requirement: ModelCall 快照必须跨 Runtime 使用统一版本化格式

Rust PostgreSQL 与 Pi SQLite SHALL 使用带 `schema_version` 的相同逻辑字段、枚举和导出语义，并 SHALL 分别由数据拥有者持久化，禁止跨库双写事务。

#### Scenario: 导出 Rust 与 Pi 调用
- **WHEN** 操作者分别导出 Rust 和 Pi ModelCall
- **THEN** 两种导出 SHALL 使用相同 schema_version 和字段含义
- **AND** envelope SHALL 标明 source runtime、记录 hash 和拥有者引用
- **AND** 离线消费者 SHALL 能按 schema 合并而无需猜测字段映射

#### Scenario: 快照包含多模态输入
- **WHEN** 逻辑输入包含图片、音频或视频资产
- **THEN** 快照 SHALL 只保存不可变资产 ID、版本或 hash、MIME 和必要元数据
- **AND** SHALL NOT 保存 base64 二进制或临时签名 URL

### Requirement: 审计持久化必须执行统一脱敏

ModelCall、审计 API、导出、错误和日志 SHALL NOT 保存或返回凭据、认证头、Cookie、原始请求头、带敏感查询参数的 URL 或 schema 标记为 secret 的内容。

#### Scenario: 逻辑输入混入已知 secret
- **WHEN** 持久化前扫描发现 API Key、Authorization、Cookie 或 secret 标记字段
- **THEN** 系统 SHALL 按 schema 执行删除或不可逆遮蔽
- **AND** 无法安全脱敏时 SHALL 拒绝模型调用
- **AND** 错误信息 SHALL NOT 回显 secret 原文

#### Scenario: 保存普通文本输入
- **WHEN** 文本输入不包含 secret 且通过脱敏校验
- **THEN** 系统 SHALL 保存脱敏后的逻辑全文而不是不可审计的 hash-only 摘要
- **AND** SHALL 保留每个动态片段的来源和信任等级

### Requirement: 系统必须提供摘要列表、脱敏详情和统一导出入口

Rust backend 与 Pi Runtime SHALL 为各自拥有的 ModelCall 提供可分页摘要列表、脱敏详情和版本化导出入口，且列表 SHALL NOT 默认返回完整 Prompt 或输出正文。

#### Scenario: 查询调用列表
- **WHEN** 操作者按拥有者、node、版本、模型、状态或时间筛选 ModelCall
- **THEN** API SHALL 返回稳定分页结果
- **AND** 每项 SHALL 只包含 ID、关联摘要、版本、模型、状态、token/cost 摘要和时间

#### Scenario: 查询调用详情
- **WHEN** 操作者请求一个存在且归属明确的 ModelCall 详情
- **THEN** API SHALL 返回脱敏后的完整逻辑输入、输出、来源、Schema、模型行为快照和终态证据
- **AND** SHALL NOT 使用当前 Definition 或当前模型配置覆盖历史值

### Requirement: 默认回放必须是无副作用 dry-run

ModelCall replay 默认 SHALL 只验证历史快照、重新编译并产生结构化 diff，且 SHALL 保证零模型调用、零 Tool 和零领域写入。

#### Scenario: 执行默认 replay
- **WHEN** 操作者对历史 ModelCall 请求 dry-run replay
- **THEN** 系统 SHALL 加载历史 definition/version 与编译输入并生成验证结果和 diff
- **AND** SHALL NOT 调用供应商、执行 Tool、修改 Session Tree、创建业务 Run 或写领域数据

#### Scenario: 请求真实模型对比
- **WHEN** 操作者要求用模型重新执行历史案例
- **THEN** replay API SHALL NOT 原地执行或覆盖源 ModelCall
- **AND** 系统 SHALL 要求创建引用源记录且带预算的独立 EvalRun

### Requirement: ModelCall 保留和删除必须跟随数据所有权

版本撤销、回滚、fork 和 rebind SHALL NOT 删除 ModelCall；只有操作者明确删除拥有者 Session 或 Run 时，对应原始 ModelCall 才 SHALL 按所有权规则级联删除。

#### Scenario: 删除拥有者 Session 或 Run
- **WHEN** 操作者通过既有明确删除流程删除拥有者 Session 或 Run
- **THEN** 对应 ModelCall SHALL 级联删除
- **AND** 仅不含原始内容的聚合 EvalReport SHALL 被允许保留
- **AND** 保留报告 SHALL 标记来源已删除

#### Scenario: 撤销 Definition 版本
- **WHEN** 某 Agent/Prompt 版本被标记为 revoked
- **THEN** 引用该版本的历史 ModelCall SHALL 保留可读和可导出
- **AND** 新模型调用 SHALL 被阻断
