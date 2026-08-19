## ADDED Requirements

### Requirement: R2 九个版本化 JSON Schema
共享契约包 SHALL 以 JSON Schema Draft 2020-12 提供 `Project`、`Episode`、`Scene`、`Shot`、`Asset`、`AssetVersion`、`WorkflowDraft`、`WorkflowVersion` 与 `TimelineDocument` 共九份 Schema。每份 Schema SHALL 声明稳定标识、`schema_version` 和拒绝无效必填结构的规则。

#### Scenario: 验证有效契约样例
- **WHEN** 契约测试使用每个对象的有效样例进行校验
- **THEN** 九份 Schema 都接受各自样例，并保留其 `schema_version`

#### Scenario: 拒绝缺失版本或标识的文档
- **WHEN** 契约测试提交缺失稳定标识或 `schema_version` 的对象
- **THEN** 对应 Schema 拒绝该对象并提供可定位的验证错误

### Requirement: R2 层级、引用和时间线契约
Schema SHALL 表达 `Project -> Episode -> Scene -> Shot` 的显式归属与稳定引用。`AssetVersion` SHALL 引用其 Asset 且不可作为可覆盖二进制内容的容器；`WorkflowDraft` SHALL 声明 `scopeType` 和非空 `scopeIds`；`WorkflowVersion` SHALL 引用不可变定义；`TimelineDocument` SHALL 以整数帧保存片段起点、源入点和时长。

#### Scenario: 接受明确作用域的工作流草稿
- **WHEN** 用户提交带有效 `scopeType` 与显式 `scopeIds` 的 WorkflowDraft
- **THEN** Schema 接受该草稿，并使作用域可在不读取 UI 当前选择的条件下确定

#### Scenario: 拒绝浮点帧时间
- **WHEN** TimelineDocument 的帧字段包含非整数值
- **THEN** Schema 拒绝该文档

### Requirement: R3 基础领域持久化与版本规则
API 数据模型和 Alembic SHALL 为九个契约对象建立最小持久化基础，并保存稳定 ID、所属关系、`revision`、状态与版本引用。状态机 SHALL 限定为 `draft -> generated -> pending_review -> approved/rejected -> superseded/archived`；已发布或已创建的版本记录 SHALL 不被原地覆盖。

#### Scenario: 创建最小项目层级
- **WHEN** 测试创建 Project、Episode、Scene 和 Shot 的最小有效记录
- **THEN** 数据库保留稳定 ID 与父级关系，且迁移后可被读取

#### Scenario: 阻止过期草稿覆盖
- **WHEN** 写入请求使用过期 `revision` 修改草稿
- **THEN** API 或领域服务拒绝写入并返回当前版本摘要或等价的冲突结果

### Requirement: R3 DDD 所有权边界
系统 SHALL 将工作流定义、故事板层级/排序、资产版本和时间线文档分配给独立领域服务边界。工作流定义 SHALL 不复制 ShotSpec 事实；故事板操作 SHALL 使用显式领域命令；阶段 0 不实现完整业务命令，但不得建立相互覆盖的第二事实源。

#### Scenario: 检查领域服务依赖
- **WHEN** 架构测试或代码审查检查基础模块
- **THEN** Workflow、Storyboard、Asset 与 Timeline 的职责边界可从模块和接口识别，且没有重复持久化的镜头规格
