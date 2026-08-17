# project-memory-governance Specification

## Purpose
定义项目代理读取、维护和审计 Git 跟踪的 Markdown 项目记忆及 ADR 时必须遵循的长期规则。

## Requirements
### Requirement: 固定的项目记忆读取入口
项目代理 SHALL 在开始仓库任务时读取 `docs/agent/PROJECT.md`、`docs/agent/DECISIONS.md` 和 `docs/agent/HANDOFF.md`，并在诊断已证实问题时读取 `docs/agent/TROUBLESHOOTING.md`。项目规则 SHALL 使用相对链接指向这些入口。

#### Scenario: 开始常规仓库任务
- **WHEN** 代理开始涉及仓库的实现、修改、诊断或复审任务
- **THEN** 代理先读取 PROJECT、DECISIONS 和 HANDOFF，再按任务主题读取权威文档

#### Scenario: 诊断已知问题
- **WHEN** 代理处理故障、报错或异常行为
- **THEN** 代理读取 TROUBLESHOOTING 并以其中可复现的事实作为辅助证据

### Requirement: 事实源优先级与记忆维护
项目记忆系统 SHALL 将当前代码、测试、schema 和可执行配置置于最高事实优先级；ADR 与当前架构其次；PROJECT 与 DECISIONS 再次；HANDOFF 与 TROUBLESHOOTING 最后。任务完成后，代理 MUST 更新受影响且已确认的事实、决策、交接或排障记录，并不得把临时探索或未经证实推测写入记忆。

#### Scenario: 文档与可执行事实冲突
- **WHEN** 记忆文档与当前代码、测试、schema 或可执行配置不一致
- **THEN** 代理以可执行事实为准并修正受影响的记忆文档

#### Scenario: 完成影响长期决策的任务
- **WHEN** 一个已完成任务确认了可复用的长期决策
- **THEN** 代理更新 ADR 或 DECISIONS 索引，并在需要时更新 HANDOFF

### Requirement: 安全且可审计的 Markdown 记忆
项目记忆 SHALL 由 Git 跟踪的 Markdown 文档组成，使用相对链接并保持分类精简。记忆文档 MUST NOT 包含口令、密钥、API key、令牌、凭据、私有数据或设备相关绝对路径。排障记录 MUST 仅包含已观察、可复现且有证据的问题，不得猜测根因。

#### Scenario: 记录新的项目事实
- **WHEN** 代理需要持久化一个已确认且可复用的项目事实
- **THEN** 代理将其写入最小合适的 Markdown 分类文件并使用相对链接关联权威文档

#### Scenario: 遇到敏感或未证实信息
- **WHEN** 代理发现秘密、私有数据或未验证的根因推测
- **THEN** 代理不将该信息写入项目记忆，并保留必要的非敏感验证边界

### Requirement: ADR 决策索引
项目 SHALL 在 `docs/adr/` 提供 ADR 入口，并在 DECISIONS 中索引已接受的长期决策。每条 ADR MUST 记录状态、日期、上下文、决定、取舍和明确非目标。

#### Scenario: 采用长期方案
- **WHEN** 项目接受一个会影响后续维护的长期方案
- **THEN** 代理创建或更新对应 ADR，并在 DECISIONS 中提供相对链接索引
