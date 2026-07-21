# work-generation-task-management Specification

## Purpose
TBD - created by archiving change add-work-generation-task-management. Update Purpose after archive.
## Requirements
### Requirement: 生成任务必须提供高密度列表和右侧步骤详情

系统 SHALL 在 `作品生产 / 生成任务` 使用高密度任务列表展示作品运行，并在选中任务后从右侧展示分步骤详情。

#### Scenario: 从作品生成结果直达当前任务

- **GIVEN** 操作者确认生成作品且 API 已成功返回 `run_id`
- **WHEN** 作品生成页面显示提交成功结果
- **THEN** 页面 SHALL 提供“查看生成任务”按钮
- **AND** 操作者点击后 SHALL 进入 `作品生产 / 生成任务`
- **AND** 任务页面 SHALL 使用 `run_id` 定位并选中本次运行
- **AND** `run_id` SHALL 只用于首次定位，后续刷新 SHALL 严格使用当前状态视图
- **AND** 页面 SHALL NOT 使用作品标题或列表位置猜测目标运行
- **AND** 页面 SHALL NOT 因 `run_id` 存在而把其他状态任务混入当前列表

#### Scenario: 查看任务列表

- **GIVEN** 当前账号存在作品生成运行
- **WHEN** 操作者打开生成任务
- **THEN** 列表 SHALL 展示作品、版本、当前阶段、聚合进度、子任务成功/失败数、非金额资源用量和时间
- **AND** 列表 SHALL 支持按状态、阶段、作品和时间筛选
- **AND** 列表 SHALL NOT 展示金额费用字段

#### Scenario: 使用三个主状态视图筛选任务

- **GIVEN** 当前账号存在不同执行状态的作品运行
- **WHEN** 操作者查看任务列表
- **THEN** 页面 SHALL 提供“未生成 / 生成中 / 已完成”三个主状态视图
- **AND** `未生成` SHALL 只包含尚未开始外部调用的排队运行
- **AND** `生成中` SHALL 包含存在运行中或待执行必要步骤的运行
- **AND** `已完成` SHALL 只包含已成功产出完整结果的运行
- **AND** 失败或状态不确定运行 SHALL 通过独立“需处理”入口展示
- **AND** 已取消、已隐藏、阶段和时间 SHALL 可通过更多筛选访问
- **AND** 各状态数量和任务总数 SHALL 由服务端按项目全局统计，不得从当前筛选结果反推
- **AND** 更多筛选 SHALL 提供明确的已取消状态入口
- **AND** 页面 SHALL NOT 将失败、状态不确定或取消运行伪装为已完成

#### Scenario: 查看任务步骤详情

- **GIVEN** 操作者在列表中选择一个作品运行
- **WHEN** 右侧详情打开
- **THEN** 详情 SHALL 按执行顺序展示方案、TTS、字幕、Seedance 分段、ASR、混音和合成中实际存在的步骤
- **AND** 每个步骤 SHALL 展示状态、模型/工具快照、耗时、用量、错误和结果素材
- **AND** 不适用于当前声音模式的步骤 SHALL 标记为未规划而非失败

#### Scenario: 点击查看必须产生明确反馈

- **GIVEN** 操作者从菜单普通进入生成任务页面且 URL 未携带 `run_id`
- **WHEN** 任务列表加载完成
- **THEN** 页面 SHALL NOT 自动把第一条任务标记为已查看
- **WHEN** 操作者点击某条任务的“查看”
- **THEN** 右侧 SHALL 立即展示任务详情加载反馈
- **AND** 详情成功返回后该任务 SHALL 标记为“查看中”并展示对应步骤详情
- **AND** 页面 SHALL 使用稳定 `run_id` 记录当前选择
- **AND** 只有从作品生成结果携带 `run_id` 直达时才 SHALL 自动定位并打开对应任务

#### Scenario: 生成任务列表和详情使用可读字号

- **GIVEN** 操作者在桌面端打开生成任务页面
- **WHEN** 页面展示筛选、任务表格和右侧步骤详情
- **THEN** 可见辅助文字 SHALL NOT 小于 `12px`
- **AND** 任务名称、进度、控件和步骤名称 SHALL 使用 `13~14px`
- **AND** 列表与详情区块标题 SHALL 使用不小于 `16px` 的明确层级
- **AND** 字号提升后任务列、长作品名和步骤审计 SHALL NOT 被裁切或产生横向溢出

### Requirement: 任务进度必须由子任务终态确定性聚合

系统 SHALL 根据当前运行已锁定步骤及其子任务状态计算聚合阶段和进度，不得仅依赖前端估算。

#### Scenario: 多个视频分段并行运行

- **GIVEN** 某运行包含多个 Seedance 子任务且状态不同
- **WHEN** 页面查询运行状态
- **THEN** API SHALL 返回已成功、运行中、排队和失败的子任务数量
- **AND** 页面 SHALL 根据服务端聚合结果展示进度

#### Scenario: 必需节点失败

- **GIVEN** 某必需步骤存在失败子任务且没有在途 attempt
- **WHEN** 系统聚合运行状态
- **THEN** 运行 SHALL 显示为失败或等待人工处理
- **AND** 后续依赖节点 SHALL 保持阻塞并说明依赖原因

### Requirement: 上游异步任务必须恢复而不得重复提交

系统 SHALL 区分“尚未取得上游任务 ID”和“已取得上游任务 ID”的失败；取得 ID 后只允许查询、恢复或按供应商能力取消原任务。

#### Scenario: 提交响应包含上游任务 ID

- **GIVEN** Provider 已返回上游任务 ID
- **WHEN** Worker 重启、超时或失去轮询租约
- **THEN** 后续 Worker SHALL 使用原上游任务 ID 恢复查询
- **AND** 系统 SHALL NOT 再次提交相同生成请求

#### Scenario: 提交前临时错误

- **GIVEN** 尚未取得上游任务 ID 且请求遇到可判定的临时错误
- **WHEN** Worker 执行自动恢复
- **THEN** Worker SHALL 在同模型最多自动重试 1 次
- **AND** 每次 attempt SHALL 使用同一业务幂等身份并保留审计
- **AND** Worker SHALL NOT 自动跨模型或供应商

#### Scenario: 无法判断是否已提交

- **GIVEN** 请求响应丢失且无法证明上游未创建任务
- **WHEN** Worker 无法通过请求 ID 或查询接口恢复
- **THEN** 系统 SHALL 停止自动重提并标记需要人工处理
- **AND** 页面 SHALL 展示重复调用风险和现有请求追踪信息

### Requirement: 失败重试必须限定为失败节点

系统 SHALL 保留全部成功节点及素材；失败重试必须创建新 attempt，只执行失败节点及其必要下游合成，不得整体重复调用。

#### Scenario: 单个 Seedance 分段失败

- **GIVEN** 多个 Seedance 分段中只有一个失败且其他分段成功
- **WHEN** 操作者请求重试
- **THEN** 页面 SHALL 展示将再次调用的单个视频任务、秒数和受影响下游步骤
- **AND** 确认后系统 SHALL 只重试失败分段
- **AND** 成功分段 SHALL 继续复用

#### Scenario: TTS 失败而视频成功

- **GIVEN** 独立 TTS 步骤失败且全部无声视频分段成功
- **WHEN** 操作者确认重试 TTS
- **THEN** 系统 SHALL 只创建新的 TTS attempt
- **AND** TTS 成功后系统 SHALL 执行必要字幕和合成步骤
- **AND** 系统 SHALL NOT 再次调用 Seedance

#### Scenario: 切换模型后重试

- **GIVEN** 操作者希望使用不同模型替代失败节点
- **WHEN** 操作者修改模型选择
- **THEN** 系统 SHALL 派生新计划或新作品版本并重新校验
- **AND** 系统 SHALL NOT 将跨模型执行记录为原节点的自动重试

### Requirement: 重试必须再次主动确认资源用量

任何会再次调用模型的人工重试 SHALL 在执行前展示模型、任务数、视频秒数、TTS 字符数或 ASR 时长并取得确认，且 SHALL NOT 展示金额。

#### Scenario: 确认失败节点重试

- **GIVEN** 失败节点具备可重试条件
- **WHEN** 操作者打开重试确认
- **THEN** 页面 SHALL 展示再次执行的节点、模型、参数、任务数和资源用量
- **AND** 页面 SHALL 展示将复用的成功素材
- **AND** 只有确认后系统才 SHALL 创建新 attempt

#### Scenario: 重试确认重复提交

- **GIVEN** 某失败节点重试请求已成功创建 attempt
- **WHEN** 客户端使用相同 `Idempotency-Key` 重复请求
- **THEN** API SHALL 返回原 attempt
- **AND** 系统 SHALL NOT 再次创建供应商任务

### Requirement: 任务取消必须遵守阶段和供应商能力

系统 SHALL 允许取消排队任务；运行中任务只有在对应供应商声明支持取消时才能转发取消，并保留取消审计。

#### Scenario: 取消排队运行

- **GIVEN** 作品运行尚未开始任何外部调用且处于排队状态
- **WHEN** 操作者确认取消
- **THEN** 系统 SHALL 将未开始步骤标记取消
- **AND** Worker SHALL NOT 再领取这些步骤

#### Scenario: 供应商支持运行中取消

- **GIVEN** 某上游任务正在运行且供应商支持取消
- **WHEN** 操作者确认取消
- **THEN** 系统 SHALL 向原上游任务发送取消请求
- **AND** 系统 SHALL 记录请求和供应商最终状态

#### Scenario: 供应商不支持运行中取消

- **GIVEN** 某上游任务正在运行且供应商不支持取消
- **WHEN** 操作者尝试取消
- **THEN** 系统 SHALL 拒绝伪造已取消状态
- **AND** 页面 SHALL 说明任务仍需等待上游终态

### Requirement: 失败任务隐藏不得删除审计

系统 SHALL 允许失败运行从默认视图隐藏，但运行、步骤、attempt、错误、输入输出摘要、资源用量和供应商追踪审计必须永久保留。

#### Scenario: 隐藏失败任务

- **GIVEN** 某运行处于失败终态且尚未隐藏
- **WHEN** 操作者确认隐藏
- **THEN** 系统 SHALL 记录隐藏时间并从默认列表排除该运行
- **AND** 系统 SHALL NOT 删除任务、错误或已成功素材
- **AND** 系统 SHALL NOT 调用 Worker 或供应商

#### Scenario: 查看已隐藏失败任务

- **GIVEN** 当前存在已隐藏失败运行
- **WHEN** 操作者选择显示已隐藏任务
- **THEN** 页面 SHALL 恢复展示完整任务和步骤审计
- **AND** 任务原终态 SHALL 保持不变

### Requirement: 作品任务必须与全局工作流任务保持边界

生成任务页面 SHALL 只展示作品生产领域运行，不承担跨 Agent、队列、Worker 或平台级系统监控。

#### Scenario: 生成任务页保留全局业务导航

- **GIVEN** 操作者进入 `作品生产 / 生成任务`
- **WHEN** 页面渲染左侧业务流程菜单
- **THEN** 页面 SHALL 保留内容策略、脚本创作、素材管理、作品生产、发布运营、数据分析和工作流任务全部一级菜单
- **AND** 页面 SHALL 只在作品生产分组内高亮生成任务
- **AND** 页面 SHALL NOT 因任务页专属布局隐藏其他业务菜单

#### Scenario: 生成任务页复用作品生成工作台骨架

- **GIVEN** 操作者在 `作品生成` 与 `生成任务` 之间切换
- **WHEN** 页面完成路由切换
- **THEN** 两个页面的顶部栏、品牌区、左侧导航宽度和内容起点 SHALL 保持一致
- **AND** `生成任务` SHALL 以当前 `作品生成` 页样式为基准
- **AND** 页面 SHALL NOT 使用二级菜单条件创建独立全局工作台骨架

#### Scenario: 查看作品生成任务

- **GIVEN** 系统同时存在作品生产任务和其他 Agent 工作流任务
- **WHEN** 操作者打开 `作品生产 / 生成任务`
- **THEN** 页面 SHALL 只返回作品生产运行及其步骤
- **AND** 跨 Agent/队列任务 SHALL 继续由一级菜单 `工作流任务` 承担
