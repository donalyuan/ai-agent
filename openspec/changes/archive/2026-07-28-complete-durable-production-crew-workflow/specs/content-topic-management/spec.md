## ADDED Requirements

### Requirement: Full Crew 必须受控使用已确认选题

系统 SHALL 只允许 `status=active` 的同一项目下未软删除的 `approved` 选题启动 Full Crew，并 SHALL 在创建制作意图和 ProductionRun 时保持选题为 `approved`；同一选题同时 SHALL 只有一个 active Full Crew 制作意图。只有当前 ScriptPackage 通过包级审批且正式脚本、分镜、来源关联全部原子写入成功后，选题才 SHALL 更新为 `scripted`。

#### Scenario: 从 approved 选题启动 Full Crew

- **GIVEN** 项目下存在一条未软删除的 `approved` 选题
- **WHEN** 操作者创建并启动 Full Crew
- **THEN** 系统 SHALL 保存制作意图与选题的真实关联和来源快照
- **AND** 选题状态 SHALL 保持 `approved`
- **AND** 系统 SHALL NOT提前创建脚本或更新为 `scripted`
- **AND** 系统 SHALL 原子建立该选题的 active-intent 锁

#### Scenario: 非 approved 或跨项目选题被拒绝

- **GIVEN** 项目已归档，或选题为 `idea`、`scripted`、`archived`、已软删除或属于其他项目
- **WHEN** 操作者尝试用该选题启动 Full Crew
- **THEN** 系统 SHALL 拒绝请求并返回明确来源错误
- **AND** 系统 SHALL NOT创建制作意图、Run、脚本或分镜
- **AND** 系统 SHALL NOT修改选题状态

#### Scenario: 同一选题已有 active Full Crew

- **GIVEN** approved 选题已绑定未终态 Full Crew 制作意图
- **WHEN** 操作者再次创建普通 Full Crew
- **THEN** 系统 SHALL 返回 `active_intent_conflict`
- **AND** 系统 SHALL NOT创建第二个制作意图、Run 或资源快照

#### Scenario: ScriptPackage 晋升成功

- **GIVEN** 当前选题仍为 `approved` 且当前 ScriptPackage digest 已获批准
- **WHEN** 系统执行事务化脚本晋升
- **THEN** 正式 `approved` 脚本、全部分镜、production-domain link 和选题 `scripted` 状态 SHALL 在同一事务提交
- **AND** 选题 SHALL 只引用本次成功晋升的正式脚本事实

#### Scenario: ScriptPackage 晋升失败

- **WHEN** 脚本、任一分镜、来源关联或选题状态更新失败
- **THEN** 整个晋升事务 SHALL 回滚
- **AND** 选题 SHALL 保持 `approved`
- **AND** 系统 SHALL NOT留下部分脚本、部分分镜或虚假的 `scripted` 状态

#### Scenario: 选题已被其他脚本消费

- **GIVEN** 当前选题已被其他正式脚本晋升为 `scripted`
- **WHEN** 另一个 ProductionRun 尝试晋升其 ScriptPackage
- **THEN** 系统 SHALL 返回领域冲突
- **AND** 系统 SHALL NOT覆盖原脚本关联或创建第二个晋升结果

### Requirement: Full Crew 活跃期间必须锁定来源选题

选题绑定 active Full Crew 制作意图后，系统 SHALL 拒绝其内容、项目归属、业务状态和软删除变更；ScriptPackagePromotion 是唯一允许将其从 `approved` 更新为 `scripted` 的路径。Run 确定失败或取消且没有成功或不确定领域晋升时，系统 SHALL 释放 active-intent 锁并保留选题为 `approved`。

#### Scenario: 活跃制作期间编辑选题

- **GIVEN** 选题已绑定 active Full Crew
- **WHEN** 操作者修改标题、角度、受众、看点、标签、归属或状态
- **THEN** 系统 SHALL 返回 `source_locked`
- **AND** 选题和 ProductionRun 的来源快照 SHALL 保持不变

#### Scenario: 活跃制作期间软删除选题

- **GIVEN** 选题尚未生成正式脚本但已绑定 active Full Crew
- **WHEN** 操作者请求软删除
- **THEN** 系统 SHALL 拒绝删除
- **AND** 系统 SHALL NOT仅依据“尚无 scripts 引用”允许删除

#### Scenario: 安全终止后重新制作

- **GIVEN** 原 Full Crew 已确定失败或取消且没有脚本晋升副作用
- **WHEN** 系统完成终止事务
- **THEN** 选题 SHALL 保持 `approved` 并释放 active-intent 锁
- **AND** 后续新制作意图 SHALL 建立全新的来源快照和幂等作用域
