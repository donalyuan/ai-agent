# local-agent-session-persistence Specification

## Purpose
TBD - created by archiving change integrate-pi-personal-agent-foundation. Update Purpose after archive.
## Requirements
### Requirement: Agent 会话必须使用 Pi SQLite Session Storage 持久化
Runtime SHALL 使用 Pi 官方 SQLite Session Repo 保存会话 metadata、树形 entries、活动 leaf 和物化索引，并 SHALL 使用稳定本地数据路径。

#### Scenario: 服务重启后恢复会话
- **GIVEN** 会话已保存消息和工具结果
- **WHEN** Runtime 进程重启并重新打开同一 SQLite 数据库
- **THEN** 会话 SHALL 可按原 `session_id` 打开
- **AND** 活动分支、消息顺序和工具结果 SHALL 保持不变

#### Scenario: SQLite 路径不可写
- **WHEN** Runtime 启动时无法创建或迁移 SQLite 数据库
- **THEN** 就绪检查 SHALL 返回失败
- **AND** Runtime SHALL NOT 退化为内存会话

### Requirement: Session entries 必须支持增量读取和树形导航
Runtime SHALL 提供按稳定 entry sequence 增量读取 entries、查看活动 leaf、移动分支和创建 fork 的能力。

#### Scenario: 客户端断线后增量恢复
- **GIVEN** 客户端已记录最后一个 entry sequence
- **WHEN** 客户端重新请求该 sequence 之后的 entries
- **THEN** Runtime SHALL 只返回新增 entries
- **AND** 返回顺序 SHALL 与持久化顺序一致

#### Scenario: 切换活动分支
- **GIVEN** 会话树中存在目标 entry
- **WHEN** 操作者把活动 leaf 移动到该 entry
- **THEN** 后续 Context SHALL 从该分支构建
- **AND** 其他分支历史 SHALL 保留

### Requirement: Context 压缩不得成为正式长期 Memory
Runtime SHALL 将 Pi compaction 和 branch summary 作为有损 Context 记录保存，完整原始 entries SHALL 继续保留，压缩摘要 SHALL NOT 自动写入正式 Memory。

#### Scenario: 长会话触发压缩
- **WHEN** 会话达到配置的 Context 阈值或操作者手动 compact
- **THEN** Runtime SHALL 追加 compaction entry
- **AND** 后续模型 Context SHALL 使用摘要与保留的近期消息
- **AND** 原始会话历史 SHALL 仍可读取和分支

#### Scenario: 模型摘要包含未确认推断
- **WHEN** compaction summary 出现新的偏好、事实或结论
- **THEN** Runtime SHALL 只把它作为当前 Session Context
- **AND** SHALL NOT 自动升级为用户、项目或领域长期 Memory

### Requirement: 会话持久化不得泄露模型凭据
Session metadata、entries、模型快照、错误与统计信息 SHALL NOT 保存 API Key、API Secret、Authorization Header 或带敏感查询参数的 URL。

#### Scenario: 保存模型快照
- **WHEN** Runtime 为会话或运行记录实际模型配置
- **THEN** 快照 SHALL 包含 model id、供应商、协议、请求根地址、上游模型、推理等级和超时
- **AND** SHALL NOT 包含任何凭据字段
