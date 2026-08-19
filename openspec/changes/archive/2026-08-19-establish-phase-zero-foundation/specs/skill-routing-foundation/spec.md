## ADDED Requirements

### Requirement: R5 自有 SkillRegistry
系统 SHALL 提供自有 `SkillRegistry`，用于读取和解析已配置的本地 Skill manifest，并记录名称、版本、来源、许可证、启用状态、适用阶段、能力、输入输出 Schema 和允许工具。Registry SHALL 固定已解析版本，而不得隐式追踪外部仓库分支。

#### Scenario: 解析有效本地 manifest
- **WHEN** Registry 加载一个包含必填元数据的本地 manifest
- **THEN** `list`、`search`、`read` 和 `resolve` 可返回固定版本及其许可和工具边界

#### Scenario: 拒绝不完整或禁用 Skill
- **WHEN** manifest 缺失必填元数据或其 Skill 未启用
- **THEN** Registry 不将其作为可路由候选，并返回可诊断原因

### Requirement: R5 确定性 SkillRouter
`SkillRouter` SHALL 按 `deterministic_filter -> lexical_rank -> optional_semantic_adapter -> policy_decide` 的固定顺序处理候选。过滤 SHALL 使用项目类型、阶段、目标模型、许可证、启用状态、输入契约和工具权限；结果 SHALL 记录候选、得分、选择、路由原因和回退路径。

#### Scenario: 相同输入产生相同基础路由
- **WHEN** 路由器以相同上下文和 Registry 状态运行两次且未配置 semantic adapter
- **THEN** 两次返回相同候选排序、选择与路由原因

### Requirement: R5 可选语义适配器和人工裁决
semantic adapter SHALL 仅对已经确定性过滤的候选排序，且不得成为启动前置条件或独立 Compose 服务。适配器不可用、低置信度或并列候选时，路由器 SHALL 保留确定性排序并返回人工选择状态，而不得伪造确定结果。

#### Scenario: 语义适配器不可用
- **WHEN** 路由器未配置或无法调用 semantic adapter
- **THEN** 路由继续使用确定性排序，并在审计结果中记录该回退
