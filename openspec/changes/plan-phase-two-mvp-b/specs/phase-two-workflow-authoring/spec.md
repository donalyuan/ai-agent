## ADDED Requirements

### Requirement:草稿与发布版本分离
系统 SHALL 允许用户创建、编辑、自动保存和比较 `WorkflowDraft`，发布时生成不可变 `WorkflowVersion`；运行 MUST 只绑定已发布版本并冻结输入、Provider、Skill 和 capability snapshot。

#### Scenario:发布有效草稿
- **WHEN** 用户提交当前 revision 的有效草稿并通过图校验
- **THEN** 系统追加不可变 WorkflowVersion，保留草稿和历史版本，运行可引用新版本

#### Scenario:拒绝过期草稿发布
- **WHEN** 发布请求的 expectedRevision 已过期或草稿包含未知字段
- **THEN** 返回 409/validation，零版本、Run 或 Outbox 写入

### Requirement:类型化图校验与受控循环
节点端口 SHALL 声明数据类型和 cardinality；服务端 MUST 权威校验连线、DAG、子流程边界、Loop 最大次数和成本上限。客户端即时校验不构成发布依据。

#### Scenario:阻断非法连线
- **WHEN** 用户连接不兼容端口、跨 scope 节点或形成无界环
- **THEN** 图保持原样并返回可定位 diagnostic，不创建 WorkflowVersion

#### Scenario:执行受控 Loop
- **WHEN** 已发布图包含带最大次数和预算上限的 Loop
- **THEN** Run 按冻结上限执行，达到上限后以可审计状态结束，不无限重试或隐式收费

### Requirement:控制节点与工作流模板
工作流 SHALL 提供条件、并行、合并、重试和人工审核控制节点；每种节点 MUST 声明输入输出、状态转换、重试/等待边界和预算影响。系统 SHALL 提供带版本和发布权限的模板目录；从模板创建 Draft 时 MUST 重绑定项目 scope、owner 和权限，且固定模板版本，不得隐式跟随最新模板。

#### Scenario:控制节点等待人工审核
- **WHEN** 已发布图执行到人工审核节点
- **THEN** Run 进入可恢复的等待状态，审核决定通过 typed command 唤醒后续分支；重复唤醒不重复执行后续 operation

#### Scenario:模板复制隔离项目
- **WHEN** 用户从模板目录创建新项目的 WorkflowDraft
- **THEN** 系统复制指定模板版本并重绑定新项目 scope；模板或源项目后续变更不影响 Draft，冲突和无权限模板均零写入

### Requirement:画布布局与大图性能
系统 SHALL 持久化节点位置、尺寸、分组、视口和缩放层级，并支持缩放、平移、框选、复制、对齐和小地图。画布 MUST 使用 `onlyRenderVisibleElements`、分层节点内容及缩略图/代理预览；ELK 自动布局不得覆盖用户手工位置。

#### Scenario:重启后恢复画布
- **WHEN** 用户保存布局后重新打开同一 Draft
- **THEN** 画布恢复同一布局和视口；布局 CAS 冲突返回 409，不覆盖另一窗口的布局

#### Scenario:大画布保持有界渲染
- **WHEN** 打开超过性能基准节点数的 Workflow
- **THEN** 只渲染可见节点，缩小时隐藏预览和参数，原始视频不在节点内加载，并生成可量化的渲染/交互指标

### Requirement:Run 暂停与恢复
Run SHALL 在 B1 之后的阶段二后续批次支持 `pause_requested|paused|resume_requested|running` 状态和显式 pause/resume command；该能力不属于首批 B1 退出门。pause/resume MUST 通过 Temporal signal 和可重放事件驱动，保持原 frozen WorkflowVersion、Provider、Skill、capability 和 input snapshot；暂停期间不得启动新的付费 operation。

#### Scenario:暂停后 Worker 重启
- **WHEN** 运行已进入 `paused` 后 Worker 重启
- **THEN** 恢复后仍保持 `paused`，不重复提交 Provider；用户 resume 后从原节点状态继续

#### Scenario:暂停与取消竞态
- **WHEN** 同时提交 pause、resume 或 cancel，或用户无权操作 Run
- **THEN** 按固定状态机和权限返回稳定结果，重复请求幂等，失败不写入第二个 signal、ProviderCall 或 Outbox

### Requirement:故事板结构命令保持 owner 边界
Scene/Shot 的删除、拆分、合并和跨场移动 MUST 通过其 owner typed command、project scope、expectedRevision 和幂等键执行。命令提交前 MUST 计算对 ShotSpec、AssetBible 和 Timeline 引用的影响/stale 集合；不得由 Workflow、Review 投影或模板复制直接覆盖历史事实、已冻结 Run 或已发布导出。

#### Scenario:结构变更产生影响分析
- **WHEN** editor 对当前 Scene/Shot 提交拆分或跨场移动
- **THEN** owner 返回明确的 impact/stale target set，需显式确认后才追加新 revision；冲突、越权或引用不完整时零写入
