# agent-prompt-evaluation Specification

## Purpose
TBD - created by archiving change establish-versioned-agent-prompt-execution. Update Purpose after archive.
## Requirements
### Requirement: candidate 必须通过完整门禁后才能激活

行为变化的 Agent/Prompt candidate SHALL 依次通过 schema/引用静态验证、历史 snapshot dry-run、安全、结构化输出、核心质量以及 token/成本阈值评测，并生成不可变 EvalReport 后，才可由后续代码发布标记为 active。

#### Scenario: candidate 全部门禁通过
- **WHEN** candidate 在固定 case set 和相同模型 fingerprint 下满足全部必需阈值
- **THEN** EvalReport SHALL 记录 candidate、基线、case set、模型、指标、阈值和逐项结论
- **AND** candidate SHALL 成为后续代码发布可激活的版本
- **AND** 评测服务 SHALL NOT 直接在线修改 Registry active 状态

#### Scenario: 任一门禁失败
- **WHEN** candidate 存在 schema 错误、安全回归、结构化输出失败、核心质量下降或超出 token/成本阈值
- **THEN** EvalReport SHALL 明确记录失败项
- **AND** candidate SHALL NOT 被标记为 active
- **AND** 普通 Session SHALL NOT 选择该版本

### Requirement: 真实模型 EvalRun 必须有显式预算确认

任何可能调用真实模型的 EvalRun SHALL 在开始前固定 case 数、最大输入/输出 token、模型 fingerprint、最大重试和成本上限，并取得操作者明确确认；未确认时 SHALL 仅允许零费用检查。

#### Scenario: 创建获批真实模型 EvalRun
- **WHEN** 操作者明确确认模型、case 数、token 上限、重试上限和成本上限
- **THEN** 系统 SHALL 创建独立 EvalRun 并保存批准快照
- **AND** 每个实际调用 SHALL 生成独立 ModelCall
- **AND** 达到任一预算上限时 SHALL 停止后续调用并如实结束 EvalRun

#### Scenario: 未取得预算确认
- **WHEN** candidate 评测尚无明确真实调用预算确认
- **THEN** 系统 SHALL 只运行静态验证和 dry-run
- **AND** SHALL NOT 调用真实模型
- **AND** SHALL NOT 以评测未执行为由自动放宽激活门禁

### Requirement: EvalRun 和 EvalReport 必须可复现且不可覆盖

EvalRun SHALL 固定 candidate、基线、case set 版本、模型 fingerprint、预算和评测器版本；完成的 EvalReport SHALL 不可修改，重复评测 SHALL 创建新记录。

#### Scenario: 重复执行同一评测配置
- **WHEN** 操作者再次运行相同 candidate、模型和 case set
- **THEN** 系统 SHALL 创建新的 EvalRun 和 EvalReport ID
- **AND** 旧报告 SHALL 保留原结果与时间
- **AND** 新结果 SHALL NOT 覆盖旧报告

#### Scenario: 评测期间模型行为配置变化
- **WHEN** EvalRun 解析到的模型 behavior_fingerprint 与批准快照不同
- **THEN** 系统 SHALL 在调用前阻断该 EvalRun
- **AND** SHALL 要求操作者重新确认预算和模型绑定

### Requirement: 首次 v1 基线必须零费用且证明行为等价

系统 SHALL 允许首次迁移的 v1 Definition 通过已有 fixture 与 golden regression 建立基线，但 SHALL 证明编译后的 Prompt 字节等价或规范化语义等价，并 SHALL NOT 因建立基线调用真实模型。

#### Scenario: v1 golden regression 等价
- **WHEN** 全部现有生产 LLM 节点的编译输出、Schema、参数和业务 fake-provider 结果与迁移前基线一致
- **THEN** 系统 SHALL 可生成 v1 baseline EvalReport
- **AND** 报告 SHALL 标记验证类型为 golden regression 且真实模型调用数为零

#### Scenario: v1 存在非等价差异
- **WHEN** 任一节点的 System/User 内容、输出 Schema、模型参数或业务结果发生未批准差异
- **THEN** v1 baseline SHALL 失败
- **AND** 差异 SHALL 先修正或作为新的行为变化 candidate 进入完整评测

### Requirement: 回滚和撤销必须保留评测证据

Definition 回滚 SHALL 通过代码发布重新激活既有 supported 版本，安全撤销 SHALL 阻断新调用；两者均 SHALL 保留历史 EvalRun、EvalReport 和 ModelCall。

#### Scenario: 回滚到 supported 版本
- **WHEN** 发布把既有 supported 版本重新设为 active
- **THEN** 新 Session SHALL 使用该既有版本及其原 digest
- **AND** 已有 Session SHALL NOT 被静默迁移
- **AND** 新旧版本评测证据 SHALL 保留

#### Scenario: 安全撤销版本
- **WHEN** 发布把存在安全问题的版本标记为 revoked
- **THEN** 绑定该版本的 Session SHALL 被阻断继续模型调用
- **AND** 历史评测与审计记录 SHALL 保持可读

### Requirement: Context Policy 与 Tokenizer Profile candidate 必须通过激活门禁

行为变化的 ContextPolicyDefinition 或 TokenizerProfile candidate SHALL 通过 schema/引用、跨语言计数、历史 snapshot dry-run、确定性、安全、预算边界和核心 Prompt/业务回归，并生成不可变 EvalReport 后才可由代码发布标为 active。

#### Scenario: Context candidate 全部门禁通过
- **WHEN** candidate 在固定 case set、模型 fingerprint、Prompt 版本和编译时钟下满足全部阈值
- **THEN** EvalReport SHALL 记录 candidate/baseline、Tokenizer Profile、Policy、case set、预算差异、选择 diff 和逐项结论
- **AND** candidate SHALL 成为后续代码发布可激活版本
- **AND** 评测 API SHALL NOT 直接修改 Registry 生命周期

#### Scenario: Context candidate 任一门禁失败
- **WHEN** candidate 导致跨语言计数不一致、非确定性选择、required 丢失、冲突误判、预算溢出、安全回归或核心行为下降
- **THEN** EvalReport SHALL 明确失败项
- **AND** candidate SHALL NOT 被普通 Session/Run 选择

### Requirement: 首次 Context baseline 必须证明全节点等价或要求显式迁移

系统 SHALL 对全部现有 Rust/Pi 生产 LLM node 建立零真实模型调用的 Context golden baseline；自动历史绑定 SHALL 只引用证明选择、顺序、裁剪与最终 Prompt 等价的报告。

#### Scenario: baseline 完全等价
- **WHEN** 新 Context Compiler 对一个 node 的历史 fixtures 产生相同最终 Prompt、Schema、参数、调用次数和 fake-provider 结果
- **THEN** EvalReport SHALL 可标记该 node 为 `equivalent`
- **AND** 对应历史绑定 SHALL 可按迁移规格审计地绑定 baseline Policy

#### Scenario: baseline 存在差异
- **WHEN** 任一 fixture 的选择、顺序、裁剪、Prompt 或业务结果存在未批准差异
- **THEN** 报告 SHALL 标记该 node 非等价
- **AND** 历史作用域 SHALL 要求显式 fork/rebind
- **AND** 常规 baseline SHALL NOT 因此调用真实模型

