# content-topic-management Specification

## ADDED Requirements

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
