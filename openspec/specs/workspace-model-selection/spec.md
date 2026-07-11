# workspace-model-selection Specification

## Purpose
TBD - created by archiving change manage-ai-models-and-workspace-selection. Update Purpose after archive.
## Requirements
### Requirement: 工作台所有真实文本模型调用入口必须提供模型选择

`apps/video-agent` SHALL 在每个现有真实文本模型调用命令附近提供文本模型选择，并 SHALL 自动选中后台配置的启用默认文本模型。

#### Scenario: 账号策略草稿选择模型

- **GIVEN** 操作者打开账号策略页
- **WHEN** 操作者准备生成 AI 策略草稿
- **THEN** 页面 SHALL 展示文本模型选择器
- **AND** 草稿请求 SHALL 携带选中的 `model_id`

#### Scenario: 选题生成与补充选择模型

- **WHEN** 操作者在当前选题池生成选题或在历史生成页补充选题
- **THEN** 页面 SHALL 展示文本模型选择器
- **AND** topic Agent 消息 SHALL 携带选中的 `model_id`

#### Scenario: 主题组评审选择模型

- **WHEN** 操作者触发主题组评审
- **THEN** 页面 SHALL 在评审操作附近展示文本模型选择器
- **AND** 评审请求 SHALL 携带选中的 `model_id`

#### Scenario: 脚本生成和修改选择模型

- **WHEN** 操作者从选题确认生成脚本，或在脚本 Agent 中生成或修改脚本
- **THEN** 对应确认区或对话区 SHALL 展示文本模型选择器
- **AND** 每次请求 SHALL 携带本次选中的 `model_id`

### Requirement: 素材生成必须选择具体图片模型

素材生成页面 SHALL 使用图片模型选择替代硬编码供应商选择，并 SHALL 让批量生成与单分镜重生使用当前选中的图片模型。

#### Scenario: 批量生成图片候选

- **GIVEN** 当前脚本存在可生成分镜
- **WHEN** 操作者选择图片模型并创建候选任务
- **THEN** 请求 SHALL 携带选中图片模型的 `model_id`
- **AND** 页面 SHALL NOT 只提交供应商字符串

#### Scenario: 单分镜重生

- **WHEN** 操作者对一个分镜执行重生
- **THEN** 单分镜任务 SHALL 使用操作时选中的图片模型
- **AND** 幂等键、候选数量和参考素材设置 SHALL 保持现有费用控制语义

### Requirement: 一次业务操作必须贯穿同一模型选择

工作台 SHALL 为每次业务操作提交一个模型，后端内部步骤 SHALL 继承该模型；对话会话本身 SHALL NOT 永久绑定模型。

#### Scenario: 选题质量闸门继承模型

- **GIVEN** 操作者选择模型 A 发起选题生成
- **WHEN** 系统执行候选生成、质量闸门、最多一次重写和同模型重试
- **THEN** 所有模型调用 SHALL 使用模型 A
- **AND** 系统 SHALL NOT 自动切换模型 B

#### Scenario: 下一轮对话切换模型

- **GIVEN** 对话上一轮使用模型 A
- **WHEN** 操作者下一轮选择模型 B 并发送消息
- **THEN** 新一轮 SHALL 使用模型 B
- **AND** 历史运行 SHALL 继续记录模型 A

### Requirement: 工作台必须处理默认缺失和模型停用竞态

工作台 SHALL 只展示匹配类型且已启用的模型；后端 SHALL 在提交时重新校验模型，前端 SHALL 在模型不可用时保留业务输入并刷新选项。

#### Scenario: 自动选中默认模型

- **GIVEN** 选项接口返回一个默认模型和其他启用模型
- **WHEN** 调用区域首次加载
- **THEN** 页面 SHALL 自动选择默认模型
- **AND** 操作者 SHALL 可以切换到其他启用模型

#### Scenario: 没有可用模型

- **GIVEN** 选项接口没有返回匹配模型
- **WHEN** 页面展示调用区域
- **THEN** 页面 SHALL 禁用对应生成或评审操作
- **AND** 页面 SHALL 明确提示未配置可用模型
- **AND** 页面 SHALL NOT 使用硬编码模型

#### Scenario: 提交前模型被停用

- **GIVEN** 页面已选择模型 A
- **AND** 管理员随后停用模型 A
- **WHEN** 操作者提交请求
- **THEN** 后端 SHALL 返回 `model_disabled`
- **AND** 页面 SHALL 保留用户输入并刷新模型选项
- **AND** 页面 SHALL NOT 静默改用默认模型

### Requirement: 本轮不得新增视频模型调用入口

视频模型选择 SHALL 等待作品生产和视频生成能力正式设计，不得因为模型管理已支持视频类型而伪造视频生成页面或调用。

#### Scenario: 工作台加载视频模型配置

- **WHEN** 操作者浏览当前已实现的视频工作台页面
- **THEN** 页面 SHALL NOT 新增视频生成按钮或视频模型选择器
- **AND** 系统 SHALL NOT 发起视频供应商请求

### Requirement: 工作台模型选择必须先通过 Pencil 原型确认

正式修改工作台页面前 SHALL 更新 `docs/prototypes/video-agent/video-agent.pen` 并覆盖所有受影响调用区域。

#### Scenario: 原型覆盖全部选择入口

- **WHEN** 开发者提交模型选择原型供确认
- **THEN** 原型 SHALL 覆盖账号策略、当前选题池、历史生成、脚本生成确认、脚本 Agent 和素材生成
- **AND** 原型 SHALL 展示默认选择和无可用模型状态
- **AND** 用户明确确认后 SHALL 进入正式前端编码
