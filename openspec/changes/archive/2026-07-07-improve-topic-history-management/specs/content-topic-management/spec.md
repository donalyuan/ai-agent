## ADDED Requirements

### Requirement: 系统必须支持选题软删除

系统 SHALL 支持将未生成脚本的选题从管理视图软删除。软删除 SHALL 不复用 `archived` 状态，并且 SHALL 保留原始选题记录用于数据一致性和后续审计。

#### Scenario: 软删除未生成脚本选题

- **GIVEN** 数据库中存在一条未生成脚本、未软删除的选题
- **AND** 不存在任何脚本引用该选题
- **WHEN** 操作者删除该选题
- **THEN** 系统 SHALL 为该选题记录软删除时间
- **AND** 系统 SHALL NOT 改写该选题的业务状态
- **AND** 系统 SHALL 从默认选题管理视图中移除该选题

#### Scenario: 已生成脚本选题不可删除

- **GIVEN** 数据库中存在一条状态为 `scripted` 或已被脚本引用的选题
- **WHEN** 操作者删除该选题
- **THEN** 系统 SHALL 拒绝删除请求
- **AND** 系统 SHALL 返回明确错误
- **AND** 系统 SHALL NOT 修改该选题记录

#### Scenario: 默认查询排除软删除选题

- **GIVEN** 项目下同时存在正常选题和已软删除选题
- **WHEN** 操作者查询选题池、选题统计或生成批次历史
- **THEN** 系统 SHALL 只统计未软删除选题
- **AND** 系统 SHALL NOT 在默认选题池中返回已软删除选题
- **AND** 生成批次的 `topic_count` SHALL 只计算未软删除选题

#### Scenario: 软删除选题不能进入脚本生成

- **GIVEN** 数据库中存在一条已软删除选题
- **WHEN** 操作者请求该选题进入脚本生成确认流程
- **THEN** 系统 SHALL 拒绝请求
- **AND** 系统 SHALL NOT 创建脚本
