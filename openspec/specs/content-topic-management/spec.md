# content-topic-management Specification

## Purpose
TBD - created by archiving change content-topic-agent. Update Purpose after archive.
## Requirements
### Requirement: 系统必须提供独立选题池

系统 SHALL 为视频工作台提供独立选题实体，使具体选题不再只作为脚本生成输入文本存在。每个选题 SHALL 归属一个真实项目，并具有可追踪状态、来源、内容类型、评分和标签。

#### Scenario: 人工新增选题

- **GIVEN** 数据库中存在一个项目
- **WHEN** 操作者提交选题标题、角度、受众、看点、内容类型和标签
- **THEN** 系统 SHALL 创建一条 `content_topics` 记录
- **AND** 选题状态 SHALL 为 `idea`
- **AND** 选题来源 SHALL 为 `manual`

#### Scenario: 查询项目选题池

- **GIVEN** 项目下存在多条不同状态和来源的选题
- **WHEN** 操作者按状态、来源或批次查询选题
- **THEN** 系统 SHALL 只返回该项目下匹配条件的选题
- **AND** 系统 SHALL NOT 返回其他项目的选题

#### Scenario: 选题状态流转

- **GIVEN** 已存在一条 `idea` 选题
- **WHEN** 操作者确认该选题
- **THEN** 系统 SHALL 将状态更新为 `approved`
- **AND** 该选题 SHALL 可以进入脚本生成确认流程

### Requirement: 选题 Agent 必须自动入库并记录批次

系统 SHALL 允许操作者基于项目定位和补充要求生成选题候选。Agent 生成结果 SHALL 自动写入选题池，状态为 `idea`，并关联一次生成批次。

#### Scenario: Agent 批量生成选题

- **GIVEN** 数据库中存在一个项目
- **AND** 操作者提交非空补充要求和生成数量
- **WHEN** 系统调用 `topic` Agent 生成候选
- **THEN** 系统 SHALL 创建 `topic_generation_batches` 记录
- **AND** 系统 SHALL 将生成候选写入 `content_topics`
- **AND** 每条候选的 `source` SHALL 为 `agent`
- **AND** 每条候选的 `status` SHALL 为 `idea`
- **AND** Agent 回复 metadata SHALL 包含 `batch_id`、`created_topic_ids` 和 `topic_count`

#### Scenario: 对历史生成批次补充选题

- **GIVEN** 项目下存在一个 `succeeded` 且仍有可见选题的生成批次
- **AND** 操作者基于该批次提交非空补充要求和生成数量
- **WHEN** 系统调用 `topic` Agent 生成补充候选
- **THEN** 系统 SHALL 创建新的 `topic_generation_batches` 记录
- **AND** 新批次的 `supplement_of_batch_id` SHALL 指向原始批次
- **AND** 系统 SHALL 将原始生成要求和同主题组已有选题作为上下文提供给 LLM
- **AND** 系统 SHALL 要求 LLM 生成与原主题相关且不重复已有选题的新候选
- **AND** 系统 SHALL NOT 修改原始批次的 `prompt`、`requested_count`、`source_run_id` 或 `metadata`
- **AND** 新生成的 `content_topics.batch_id` SHALL 指向补充批次本身
- **AND** Agent 回复 metadata SHALL 包含新批次 `batch_id`、`supplement_of_batch_id`、`created_topic_ids` 和 `topic_count`

#### Scenario: 补充批次必须归属同一项目

- **GIVEN** 数据库中存在项目 A 和项目 B
- **AND** 项目 A 下存在一个生成批次
- **WHEN** 操作者在项目 B 下请求补充项目 A 的批次
- **THEN** 系统 SHALL 拒绝请求
- **AND** 系统 SHALL NOT 创建新的生成批次
- **AND** 系统 SHALL NOT 写入新的选题

#### Scenario: 不允许补充不可管理批次

- **GIVEN** 目标生成批次不存在、状态不是 `succeeded`，或当前没有未软删除选题
- **WHEN** 操作者请求补充该批次
- **THEN** 系统 SHALL 拒绝请求
- **AND** 系统 SHALL 返回明确错误
- **AND** 系统 SHALL NOT 创建新的生成批次
- **AND** 系统 SHALL NOT 写入新的选题

#### Scenario: 查询选题生成批次历史

- **GIVEN** 项目下存在多个 `topic_generation_batches`
- **AND** 每个批次关联不同数量的选题
- **WHEN** 操作者打开内容策略页或查看历史生成
- **THEN** 系统 SHALL 按生成时间倒序返回该项目的生成批次
- **AND** 每个批次 SHALL 包含 `batch_id`、`prompt`、`status`、`requested_count`、`topic_count`、`supplement_of_batch_id` 和生成时间
- **AND** 系统 SHALL 只返回 `succeeded` 且 `topic_count > 0` 的生成批次作为历史生成入口
- **AND** 系统 SHALL NOT 返回失败批次或未产出选题的空批次
- **AND** 系统 SHALL NOT 返回其他项目的生成批次

#### Scenario: LLM 输出非法时整批失败

- **GIVEN** 操作者发起选题生成
- **WHEN** LLM 输出为空、无法解析、字段缺失或 score 越界
- **THEN** 系统 SHALL 标记本次批次为 `failed`
- **AND** 系统 SHALL NOT 部分写入选题
- **AND** 系统 SHALL 记录失败 run 和错误信息

### Requirement: 内容策略页必须提供选题池闭环

`apps/video-agent` SHALL 在“内容策略”页面提供项目策略摘要、选题生成、选题池筛选、选题详情和进入脚本创作的入口。

#### Scenario: 展示策略摘要和选题池

- **GIVEN** 操作者已选择一个项目
- **WHEN** 操作者打开“内容策略”页面
- **THEN** 页面 SHALL 展示项目名称、定位、描述、选题总数、已确认数和已成稿数
- **AND** 页面 SHALL 展示该项目的选题池

#### Scenario: 按状态和生成批次查看选题池

- **GIVEN** 项目下存在多个生成批次，且选题状态包含 `idea`、`approved`、`scripted` 和 `archived`
- **WHEN** 操作者打开“内容策略”页面
- **THEN** 页面 SHALL 提供 `全部`、`待评估`、`已确认`、`已成稿` 和 `已归档` 状态筛选
- **AND** 页面 SHALL 提供历史生成批次列表
- **AND** 历史生成批次列表 SHALL 只展示成功且有实际选题的批次
- **AND** 页面默认 SHALL 按最新生成批次展示选题
- **AND** 操作者 SHALL 可以切换到任一历史批次或查看全部选题

#### Scenario: 从已确认选题进入脚本确认

- **GIVEN** 已存在一条 `approved` 选题
- **WHEN** 操作者点击“生成脚本”
- **THEN** 页面 SHALL 打开脚本生成确认面板
- **AND** 面板 SHALL 展示选题快照
- **AND** 面板 SHALL 要求操作者确认 `style` 和 `scene_count`

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

### Requirement: 系统必须支持主题组选题评审快照

系统 SHALL 为内容策略主题组提供 AI 评审快照，使操作者可以在同一主题组内快速识别优先、备选、淘汰和疑似重复选题。评审快照 SHALL 只作为决策辅助，不得自动修改 `ContentTopic.status`。

#### Scenario: 主题组脚本优先级复用评审快照

- **GIVEN** 某主题组存在最新成功评审快照
- **WHEN** 系统计算主题组脚本产出优先级
- **THEN** 系统 SHALL 复用该评审快照中的推荐层级、风险标记和相似选题引用
- **AND** 系统 SHALL 使用确定性规则计算排名指标和推荐候选
- **AND** 系统 SHALL NOT 在历史列表加载时额外调用 AI 重新排名
- **AND** 系统 SHALL NOT 因排名计算修改任何 `content_topics` 记录

### Requirement: 系统必须在选题入库前执行质量闸门

系统 SHALL 在 `topic` Agent 生成候选选题后、写入 `content_topics` 前执行质量评估。质量评估 SHALL 判断候选是否贴合项目定位、是否具体、是否重复、是否可脚本化、是否存在风险，以及评分是否可信。

#### Scenario: 生成候选必须先通过质量闸门

- **GIVEN** 操作者在内容策略页提交选题生成要求
- **WHEN** `topic` Agent 生成候选选题
- **THEN** 系统 SHALL 在写入 `content_topics` 前评估每条候选质量
- **AND** 系统 SHALL 只将质量闸门通过的候选写入 `content_topics`
- **AND** 写入的选题 SHALL 保持 `idea` 状态
- **AND** 系统 SHALL NOT 为淘汰候选创建 `content_topics` 记录

#### Scenario: 质量闸门必须记录评估快照

- **GIVEN** 系统完成一次候选质量评估
- **WHEN** 评估输出可解析且通过后端校验
- **THEN** 系统 SHALL 保存一条 `topic_quality_evaluations` 记录
- **AND** 评估记录 SHALL 关联 `project_id`、`batch_id` 和 `source_run_id`
- **AND** 评估结果 SHALL 包含批次摘要、每条候选的质量分、决策、风险标记和原因
- **AND** 通过项写入 `content_topics` 时 SHALL 在 `metadata` 中保存质量摘要

#### Scenario: 低通过率触发最多一次自动重写

- **GIVEN** 首轮候选质量评估完成
- **AND** 通过率低于系统阈值
- **WHEN** 本轮生成尚未触发过自动重写
- **THEN** 系统 SHALL 基于淘汰原因自动请求 `topic` Agent 重写候选
- **AND** 系统 SHALL 再次执行质量评估
- **AND** 系统 SHALL NOT 在同一轮生成中触发第二次自动重写

#### Scenario: 质量评估失败不污染选题池

- **GIVEN** `topic` Agent 已生成候选选题
- **WHEN** 质量评估调用失败、输出无法解析、缺少必填字段或包含非法枚举
- **THEN** 系统 SHALL 标记生成批次为 `failed`
- **AND** 系统 SHALL 保存失败原因
- **AND** 系统 SHALL NOT 写入任何候选到 `content_topics`

#### Scenario: 重写后仍低质时只入库通过项

- **GIVEN** 系统已触发一次自动重写
- **WHEN** 重写后的质量评估存在通过项和淘汰项
- **THEN** 系统 SHALL 只将通过项写入 `content_topics`
- **AND** 系统 SHALL 在质量评估快照中保留淘汰项和原因
- **AND** 淘汰项 SHALL NOT 出现在当前选题池

#### Scenario: 重写后没有通过项时批次失败

- **GIVEN** 系统已触发一次自动重写
- **WHEN** 重写后的质量评估没有任何通过项
- **THEN** 系统 SHALL 标记生成批次为 `failed`
- **AND** 系统 SHALL NOT 写入任何候选到 `content_topics`
- **AND** Agent 回复 SHALL 说明本批次未产生可用选题

#### Scenario: 补充批次必须经过同一质量闸门

- **GIVEN** 操作者对历史主题组请求补充选题
- **WHEN** `topic` Agent 生成补充候选
- **THEN** 系统 SHALL 将同主题组已有可见选题作为质量评估上下文
- **AND** 系统 SHALL 使用同一质量闸门评估补充候选
- **AND** 系统 SHALL 避免把重复已有主题组选题的候选写入 `content_topics`

#### Scenario: 查询批次质量报告

- **GIVEN** 某生成批次存在质量评估快照
- **WHEN** 操作者打开该生成批次的质量报告
- **THEN** 系统 SHALL 返回该批次最新质量评估
- **AND** 响应 SHALL 包含通过数量、淘汰数量、是否重写、质量摘要和候选明细
- **AND** 系统 SHALL NOT 返回其他项目或其他批次的质量评估

#### Scenario: 质量闸门不得改变选题生命周期

- **GIVEN** 质量闸门完成评估
- **WHEN** 系统写入通过候选
- **THEN** 通过候选 SHALL 仍以 `idea` 状态进入选题池
- **AND** 系统 SHALL NOT 自动确认、归档、删除选题或生成脚本
- **AND** 系统 SHALL NOT 新增 `quality_rejected` 作为 `ContentTopic.status`

### Requirement: 内容策略页必须在超宽桌面保持可读比例

`apps/video-agent` 内容策略当前选题池视图 SHALL 在超宽桌面视口下限制工作区和选题池的横向扩张，使选题 Agent、选题池和选题详情保持可读比例。

#### Scenario: 超宽桌面下选题池不无限拉伸

- **GIVEN** 操作者在 2552px 宽桌面视口打开内容策略当前选题池
- **WHEN** 页面完成加载并展示选题池、选题 Agent 和选题详情
- **THEN** 内容策略工作区 SHALL NOT 铺满整个剩余视口宽度
- **AND** 选题池宽度 SHALL NOT 超过 1040px
- **AND** 选题详情栏宽度 SHALL 至少为 420px
- **AND** 选题卡片内状态胶囊、质量胶囊、评分和移除按钮 SHALL NOT 超出卡片边框
- **AND** 页面 SHALL NOT 改变选题筛选、选题列表滚动和选题详情操作能力

### Requirement: 内容策略必须沉淀账号策略资料

系统 SHALL 为内容策略中的当前内容账号保存结构化策略资料。策略资料 SHALL 归属一个真实 `projects.id`，并作为选题 Agent、质量闸门和主题组评审的上下文来源。

#### Scenario: 查询账号策略资料

- **GIVEN** 数据库中存在一个项目
- **AND** 该项目已保存 `strategy_profile`
- **WHEN** 前端请求 `GET /api/projects`
- **THEN** 系统 SHALL 返回该项目的 `strategy_profile`
- **AND** 前端 SHALL 将该项目展示为内容账号

#### Scenario: 更新账号策略资料

- **GIVEN** 数据库中存在一个项目
- **WHEN** 操作者提交账号名称、定位摘要、描述和结构化策略资料
- **THEN** 系统 SHALL 更新该项目的 `name`、`positioning`、`description` 和 `strategy_profile`
- **AND** 系统 SHALL 返回更新后的账号资料
- **AND** 系统 SHALL NOT 修改该项目下已有选题、脚本、素材或生成批次

#### Scenario: AI 生成账号策略草稿

- **GIVEN** 数据库中存在一个项目
- **AND** 操作者提交账号策略补充方向
- **WHEN** 操作者请求 AI 生成策略草稿
- **THEN** 系统 SHALL 基于项目名称、定位摘要、描述和补充方向生成结构化策略草稿
- **AND** 系统 SHALL 返回 `draft` 和 `draft_summary`
- **AND** 系统 SHALL NOT 自动保存草稿
- **AND** 系统 SHALL NOT 修改该项目的 `strategy_profile`

#### Scenario: AI 草稿必须人工确认后保存

- **GIVEN** 系统已返回一份账号策略草稿
- **WHEN** 操作者修改或确认草稿并点击保存
- **THEN** 系统 SHALL 通过账号策略资料更新接口保存最终资料
- **AND** 后续选题生成、质量闸门和主题组评审 SHALL 读取保存后的资料

#### Scenario: AI 草稿失败不污染账号资料

- **GIVEN** 当前账号已有策略资料或编辑表单内容
- **WHEN** AI 草稿生成失败、输出无法解析或输出缺少必填结构
- **THEN** 系统 SHALL 返回明确错误
- **AND** 前端 SHALL 保留当前表单内容
- **AND** 系统 SHALL NOT 创建或修改任何 `projects` 记录

#### Scenario: 策略资料字段必须被校验

- **GIVEN** 操作者正在编辑账号策略资料
- **WHEN** 请求中的账号名称为空、文本字段超长、数组项超长或数组项数量超过限制
- **THEN** 系统 SHALL 拒绝保存
- **AND** 系统 SHALL 返回明确的参数错误
- **AND** 系统 SHALL NOT 部分更新账号资料

#### Scenario: AI 草稿生成必须受成本控制

- **GIVEN** 操作者打开内容策略页或切换当前账号
- **WHEN** 页面加载账号策略资料
- **THEN** 系统 SHALL NOT 自动触发 AI 草稿生成
- **AND** AI 草稿生成 SHALL 只能由操作者手动触发
- **AND** 单次草稿请求 SHALL 限制 LLM 输出长度并至多执行一次临时错误重试

#### Scenario: 策略资料按项目隔离

- **GIVEN** 数据库中存在项目 A 和项目 B
- **AND** 操作者更新项目 A 的策略资料
- **WHEN** 系统保存该请求
- **THEN** 系统 SHALL 只修改项目 A
- **AND** 系统 SHALL NOT 修改项目 B 的任何策略资料

### Requirement: 选题 Agent 必须使用账号策略资料

系统 SHALL 在选题生成、质量闸门和主题组评审中使用同一份账号策略上下文。账号策略上下文 SHALL 包含账号名称、定位摘要、描述、目标受众、内容支柱、风格语气、禁区、参考账号和选题偏好。

#### Scenario: 选题生成注入账号策略资料

- **GIVEN** 当前账号已保存结构化策略资料
- **WHEN** 操作者请求 `topic` Agent 生成选题
- **THEN** 系统 SHALL 在选题生成 prompt 中包含账号策略上下文
- **AND** 生成候选 SHALL 继续经过质量闸门后再入库

#### Scenario: 质量闸门注入账号策略资料

- **GIVEN** `topic` Agent 已生成候选选题
- **AND** 当前账号存在禁区、受众和选题偏好
- **WHEN** 系统执行质量闸门
- **THEN** 质量闸门 prompt SHALL 包含账号策略上下文
- **AND** 质量闸门 SHALL 将账号禁区作为风险判断上下文

#### Scenario: 主题组评审注入账号策略资料

- **GIVEN** 某主题组存在多个可见选题
- **AND** 当前账号已保存结构化策略资料
- **WHEN** 操作者触发主题组评审
- **THEN** 系统 SHALL 在主题组评审 prompt 中包含账号策略上下文
- **AND** 评审结果 SHALL 仍只作为决策辅助
- **AND** 系统 SHALL NOT 因评审自动修改 `ContentTopic.status`

### Requirement: 内容策略必须提供独立账号策略二级页面

`apps/video-agent` SHALL 在内容策略下提供独立二级页面“账号策略”，用于展示、AI 生成草稿、编辑和保存当前账号策略资料。前端 SHALL 使用“账号”文案呈现当前内容生产边界，不再在用户界面称其为“项目”。当前选题池 SHALL NOT 展示账号策略区块、策略资料状态/摘要或编辑入口。

#### Scenario: 通过独立页面展示当前账号策略资料

- **GIVEN** 操作者已选择一个账号
- **WHEN** 操作者打开内容策略下的“账号策略”二级页面
- **THEN** 页面 SHALL 展示账号名称、定位摘要、目标受众、内容支柱、风格语气、禁区、参考账号和选题偏好
- **AND** 页面 SHALL 提供编辑保存入口
- **AND** 页面 SHALL 提供 AI 生成策略草稿入口

#### Scenario: 独立页面展示策略资料缺失提示

- **GIVEN** 当前账号没有结构化策略资料
- **WHEN** 操作者打开内容策略下的“账号策略”二级页面
- **THEN** 页面 SHALL 展示策略资料待补齐提示
- **AND** 页面 SHALL 提供编辑策略资料入口
- **AND** 页面 SHALL 提供 AI 生成策略草稿入口
- **AND** 页面 SHALL NOT 自动触发 AI 草稿生成

#### Scenario: 当前选题池不展示账号策略区块

- **GIVEN** 操作者已选择一个账号
- **WHEN** 操作者打开内容策略下的“当前选题池”二级页面
- **THEN** 页面 SHALL NOT 展示账号策略区块
- **AND** 页面 SHALL NOT 展示策略资料状态、策略资料摘要或账号策略编辑入口
- **AND** 页面 SHALL NOT 展示完整账号策略编辑表单
- **AND** 页面 SHALL NOT 阻塞人工新增选题

#### Scenario: 编辑并保存账号策略资料

- **GIVEN** 操作者打开内容策略下的“账号策略”二级页面
- **WHEN** 操作者修改字段并保存
- **THEN** 页面 SHALL 调用账号策略资料更新接口
- **AND** 保存成功后页面 SHALL 展示最新资料
- **AND** 顶部账号选择器 SHALL 同步显示最新账号名称

#### Scenario: AI 草稿预填编辑表单

- **GIVEN** 操作者打开内容策略下的“账号策略”二级页面
- **WHEN** 操作者点击 AI 生成策略草稿且请求成功
- **THEN** 页面 SHALL 使用草稿预填结构化策略字段
- **AND** 页面 SHALL 展示草稿摘要
- **AND** 页面 SHALL NOT 在保存前更新当前账号正式策略资料

#### Scenario: 保存失败不覆盖本地旧资料

- **GIVEN** 当前页面已有账号策略资料
- **WHEN** 保存请求失败
- **THEN** 页面 SHALL 展示错误信息
- **AND** 页面 SHALL 保留保存前的账号策略资料展示

#### Scenario: 取消只处理未保存修改

- **GIVEN** 操作者打开内容策略下的“账号策略”二级页面
- **AND** 表单内容、AI 草稿补充方向和草稿摘要均与当前账号正式资料一致
- **WHEN** 操作者查看页面底部操作区
- **THEN** “取消”按钮 SHALL 处于不可操作状态
- **WHEN** 操作者修改表单字段或生成 AI 草稿
- **THEN** “取消”按钮 SHALL 变为可操作
- **AND** 点击“取消” SHALL 恢复当前账号正式资料对应的表单内容
- **AND** 点击“取消” SHALL 清空 AI 草稿补充方向和草稿摘要

#### Scenario: 账号策略页面布局贴合原型

- **GIVEN** 操作者打开内容策略下的“账号策略”二级页面
- **WHEN** 页面展示账号策略资料卡片
- **THEN** 右侧“结构化策略”区域 SHALL 与左侧“基础资料”区域顶边对齐
- **AND** 右侧“结构化策略”区域 SHALL 延展到 AI 草稿区域所在行
- **AND** 页面 SHALL NOT 在右侧结构化策略区域下方留下大块空白

#### Scenario: 长文本策略字段使用文本域

- **GIVEN** 操作者打开内容策略下的“账号策略”二级页面
- **WHEN** 页面展示结构化策略编辑表单
- **THEN** “目标受众”字段 SHALL 使用多行文本域
- **AND** “目标受众”字段 SHALL 支持粘贴和编辑较长人群描述

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
