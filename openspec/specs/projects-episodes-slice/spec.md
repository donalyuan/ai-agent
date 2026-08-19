# projects-episodes-slice Specification

## Purpose
TBD - created by archiving change implement-projects-episodes-slice. Update Purpose after archive.
## Requirements
### Requirement: 项目领域模型

系统 SHALL 以独立领域对象表达 Project 的稳定 ID、非空名称、状态、schema version 和从 1 开始的 revision。Project 的更新 SHALL 通过显式领域行为产生新 revision，不得由 application 直接修改字段绕过规则。

#### Scenario: 创建项目

- **WHEN** application 收到非空项目名称
- **THEN** 创建 draft Project，revision 为 1，并返回稳定 ID、名称、状态和 schema version

#### Scenario: 拒绝空项目名称

- **WHEN** application 收到空白或空字符串项目名称
- **THEN** 返回可诊断的验证错误，且不写入 Repository

### Requirement: 剧集归属与编号

Episode SHALL 归属于已存在的 Project，包含非空标题、从 1 开始的 number、状态、schema version 和 revision。一个 Project 内的 number SHALL 唯一，跨 Project 可以重复。

#### Scenario: 在项目下创建剧集

- **WHEN** application 为已存在的 Project 创建 title 和 number 合法的 Episode
- **THEN** Episode 保存 project ID、title、number、draft 状态和 revision 1

#### Scenario: 项目不存在

- **WHEN** application 使用不存在的 project ID 创建 Episode
- **THEN** 返回 project_not_found，且不产生孤立 Episode

#### Scenario: 重复剧集编号

- **WHEN** 同一 Project 已有相同 number 的 Episode
- **THEN** 返回 episode_number_conflict，且不覆盖已有 Episode

### Requirement: Repository 与 Unit of Work 边界

application SHALL 只依赖 Project/Episode Repository 与 Unit of Work 抽象；SQLAlchemy、FastAPI 和数据库连接 SHALL 位于 adapter/interface 边界。一次 command SHALL 在一个 UoW 中完成读取、领域变更、写入和 commit。

#### Scenario: 使用替换 adapter 执行 command

- **WHEN** 测试注入内存 Repository/UoW 执行创建项目和剧集 command
- **THEN** application 行为与生产 adapter 契约一致，且 domain/application 模块不需要导入 FastAPI 或 SQLAlchemy

### Requirement: Revision 乐观并发

更新 Project 或 Episode SHALL 要求调用方提供 expected revision。expected revision 与当前 revision 不一致时 SHALL 返回 revision_conflict，并且不修改数据。

#### Scenario: 过期 revision 更新

- **WHEN** 更新命令携带小于当前值的 expected revision
- **THEN** 返回 409 对应的 revision_conflict，并保留当前名称、标题、编号、状态和 revision

#### Scenario: 正确 revision 更新

- **WHEN** 更新命令携带当前 revision
- **THEN** 领域对象更新并将 revision 原子递增 1

### Requirement: 项目与剧集查询

系统 SHALL 提供按 ID 获取 Project、按 Project 列出 Episode，以及列出全部 Project 的 query。列表结果 SHALL 有确定性排序，且不会返回其他 Project 的 Episode。

#### Scenario: 查询项目剧集

- **WHEN** 客户端请求存在项目的剧集列表
- **THEN** 只返回该项目的 Episode，并按 number、ID 稳定排序

#### Scenario: 查询不存在对象

- **WHEN** 客户端请求不存在的 Project 或 Episode
- **THEN** 返回 project_not_found 或 episode_not_found，不返回空的成功对象
