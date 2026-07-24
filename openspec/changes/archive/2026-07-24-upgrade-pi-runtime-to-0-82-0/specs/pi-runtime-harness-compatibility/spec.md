## ADDED Requirements

### Requirement: Pi Runtime 依赖必须作为同一发布单元升级
系统 MUST 将 `@earendil-works/pi-agent-core`、`@earendil-works/pi-ai` 和 `@earendil-works/pi-storage-sqlite-node` 精确锁定到同一个已发布 npm 稳定版本，并 MUST 要求对应 GitHub tag 存在。manifest、lock spec 和 resolved 版本必须一致。

#### Scenario: 三包共同版本通过发布检查
- **WHEN** 三个 npm 包均发布 `0.82.0` 且 GitHub 存在 `v0.82.0`
- **THEN** Runtime 依赖和 lockfile 使用精确版本 `0.82.0`
- **AND** resolved 来源与 integrity 必须来自已审核的 npm 发布产物

#### Scenario: 发布或 lockfile 不完整
- **WHEN** 任一包缺少目标版本，或 manifest、lock spec、resolved 版本不一致
- **THEN** 系统 MUST 停止升级
- **AND** MUST NOT 使用浮动版本或 GitHub commit 作为替代

### Requirement: Harness 破坏性升级不得改变 Novex 工具契约
系统 MUST 使用 Pi `0.82.0` 的 `toolContext + AgentHarnessTool` 契约执行本地工具，同时 SHALL 保持 `chat`/`workspace` profile、`read/write/edit/bash` 工具名称、现有参数 schema、结果和 SSE 事件行为。

#### Scenario: Workspace 工具通过 context 执行
- **GIVEN** 会话使用 `workspace` profile
- **WHEN** Harness 执行 write 后以 `old_text/new_text` 调用 edit
- **THEN** 两个工具 MUST 从当前 turn 的 `ExecutionToolContext` 获取配置的工作目录
- **AND** 文件最终内容、tool start/update/result 和唯一运行终态 MUST 与升级前契约一致

#### Scenario: Chat profile 保持无本地工具
- **GIVEN** 会话使用 `chat` profile
- **WHEN** Harness 构造本轮工具集合
- **THEN** 系统 MUST 不注册 read、write、edit 或 bash

#### Scenario: 上游 factory 已发布
- **WHEN** `0.82.0` npm tarball 导出上游 read/write/edit/bash factory，但其输入或结果语义与 Novex 不同
- **THEN** 系统 MUST 保留 Novex 自有适配器
- **AND** MUST NOT 在本次依赖升级中改变既有 tool transcript 协议

### Requirement: Provider 与安全行为必须跨升级保持
系统 MUST 保持 PostgreSQL `ai_models` 作为模型和凭据唯一来源，并 MUST 保持 Responses/Chat Completions 映射、SSE 唯一终态、取消传播和凭据脱敏行为。

#### Scenario: Fake provider Tool Loop 完成
- **WHEN** fake provider 连续返回工具调用和最终 Assistant 消息
- **THEN** Runtime MUST 按实际顺序流式返回事件
- **AND** MUST 在终态前持久化消息与工具结果
- **AND** MUST 只返回一个运行终态

#### Scenario: 运行取消或失败
- **WHEN** 操作者 abort，或 Provider 返回错误
- **THEN** Runtime MUST 传播取消或返回稳定失败语义
- **AND** SSE、错误、Session 和日志 MUST NOT 暴露 API Key、Authorization 或已知 secret

### Requirement: SQLite Session 必须在容器重建后保持可用
系统 MUST 在重建升级后的 Runtime 前对实际 `/data` volume 建立一致性备份，并 MUST 在重建及再次重启后验证 SQLite 可用和升级前 Session ID 集合不丢失。系统不得删除原 volume。

#### Scenario: 当前 Session 列表为空
- **WHEN** 升级前 `GET /sessions` 为空
- **THEN** 系统仍 MUST 备份并验证 SQLite 文件非空、可读
- **AND** 重建与重启后的 Session 列表仍为空且 `/ready` 报告 SQLite 正常

#### Scenario: 重建后 Session 不可读
- **WHEN** Runtime 启动、migration 或 Session 读取失败
- **THEN** 系统 MUST 停止完成流程并保留日志与 backup volume
- **AND** 数据恢复前 MUST 报告 source/backup 并取得用户最终确认

### Requirement: 升级完成必须通过完整回归和事实同步
系统 MUST 在 Node.js 24 容器中通过 clean install、build、lint、fake-provider 测试和 high-level audit，并 MUST 通过 Rust workspace 与 Video Worker 本地回归。所有验证不得调用真实模型、视频生成或平台发布。

#### Scenario: 全部门禁通过
- **WHEN** Runtime、SQLite、Compose、Rust 和 Video Worker 验证均通过
- **THEN** 系统更新直接相关 memory、ARCHITECTURE 和 README 中的 Pi 版本为 `0.82.0`
- **AND** OpenSpec change 达到 `all_done`
- **AND** 系统只报告可归档，不自动归档或提交 Git

#### Scenario: 任一门禁失败
- **WHEN** build、lint、test、audit、健康检查、持久化或跨服务回归任一失败
- **THEN** 系统 MUST 保持对应任务未完成
- **AND** MUST NOT 报告升级完成
