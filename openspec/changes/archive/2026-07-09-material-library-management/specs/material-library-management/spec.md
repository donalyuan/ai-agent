# material-library-management Specification

## ADDED Requirements

### Requirement: 素材库必须支持 URL 素材登记

系统 SHALL 允许操作者在当前账号下登记已有素材 URL，并保存文件名、类型、标签、元数据、使用次数、状态和可选缩略图 URL。

#### Scenario: 创建视频素材

- **GIVEN** 当前账号存在
- **WHEN** 操作者提交文件名、`material_type=video`、合法 `file_url`、可选 `thumbnail_url` 和标签
- **THEN** 系统 SHALL 创建 `active` 素材
- **AND** 响应 SHALL 返回素材 ID、账号 ID、文件名、类型、URL、标签、metadata、`thumbnail_url`、`usage_count=0`、状态和时间字段

#### Scenario: 创建字幕素材

- **GIVEN** 当前账号存在
- **WHEN** 操作者提交 `material_type=subtitle` 且 `file_url` 指向字幕文件 URL
- **THEN** 系统 SHALL 创建字幕素材
- **AND** metadata SHALL 可保存字幕语言和字幕格式

#### Scenario: 创建带手动缩略图的视频素材

- **GIVEN** 当前账号存在
- **WHEN** 操作者提交 `material_type=video`、合法 `file_url` 和合法 `thumbnail_url`
- **THEN** 系统 SHALL 将 `thumbnail_url` 保存到 metadata
- **AND** 页面 SHALL 在资产栏、画布节点和详情中展示该缩略图

#### Scenario: 图片素材默认使用素材 URL 预览

- **GIVEN** 当前账号存在
- **WHEN** 操作者提交 `material_type=image`、合法 `file_url` 且未提交 `thumbnail_url`
- **THEN** 系统 SHALL 创建图片素材
- **AND** 页面 SHALL 可使用 `file_url` 作为图片素材缩略图

#### Scenario: 缺少缩略图时显示类型占位

- **GIVEN** 当前账号存在音频或字幕素材
- **WHEN** 素材未配置 `thumbnail_url`
- **THEN** 页面 SHALL 显示对应素材类型占位
- **AND** 系统 SHALL NOT 自动抽取视频帧、生成音频波形或抓取远程封面

### Requirement: 素材库必须支持筛选和默认可用列表

系统 SHALL 提供当前账号下的素材列表查询，默认只返回 `active` 素材，并支持按类型、状态、关键词和标签筛选。

#### Scenario: 默认列表只展示可用素材

- **GIVEN** 当前账号下存在 `active` 和 `archived` 素材
- **WHEN** 操作者打开素材库且未显式选择状态
- **THEN** 页面和 API SHALL 只返回 `active` 素材

#### Scenario: 查看归档素材

- **GIVEN** 当前账号下存在 `archived` 素材
- **WHEN** 操作者选择状态筛选“已归档”
- **THEN** 页面和 API SHALL 展示归档素材

### Requirement: 素材库必须支持编辑、归档和恢复

系统 SHALL 允许操作者编辑素材基础信息，并将素材状态在 `active` 和 `archived` 之间切换。

#### Scenario: 编辑素材基础信息

- **GIVEN** 当前账号下存在一条素材
- **WHEN** 操作者修改文件名、URL、缩略图 URL、标签或 metadata 并保存
- **THEN** 系统 SHALL 更新素材
- **AND** 资产栏、画布节点和详情 SHALL 展示最新内容

#### Scenario: 归档后默认列表移除

- **GIVEN** 当前素材状态为 `active`
- **WHEN** 操作者归档素材
- **THEN** 系统 SHALL 将状态更新为 `archived`
- **AND** 默认素材视图 SHALL 不再展示该素材

#### Scenario: 恢复归档素材

- **GIVEN** 当前素材状态为 `archived`
- **WHEN** 操作者恢复素材
- **THEN** 系统 SHALL 将状态更新为 `active`
- **AND** 默认素材视图 SHALL 可再次展示该素材

### Requirement: 素材库页面必须采用画布工作台

`apps/video-agent` SHALL 在“素材管理 > 素材库”提供画布优先工作台：主区域是一整块素材节点画布，资产栏和详情编辑以画布上的辅助浮层或窄面板呈现，底部提供轻量画布工具栏。

#### Scenario: 空状态

- **GIVEN** 当前账号没有可用素材
- **WHEN** 操作者打开素材库
- **THEN** 页面 SHALL 展示空画布状态
- **AND** 页面 SHALL 提供“新增素材”入口

#### Scenario: 素材库画布骨架

- **GIVEN** 当前账号存在素材
- **WHEN** 操作者打开素材库
- **THEN** 页面 SHALL 展示主画布、资产浮层、详情浮层和底部画布工具栏
- **AND** 素材节点 SHALL 展示缩略图或类型占位
- **AND** 资产浮层和详情浮层 SHALL 不把画布切分成三个等价栏目
- **AND** 页面 SHALL 不展示上传、语义检索、分镜候选或素材清单确认入口

#### Scenario: 选择素材节点

- **GIVEN** 当前账号存在素材节点
- **WHEN** 操作者在画布中选择一个素材节点
- **THEN** 右侧详情区域 SHALL 展示该素材的基础信息、缩略图 URL、标签、metadata、归档或恢复操作

#### Scenario: 画布不表达编排语义

- **GIVEN** 操作者打开素材库画布
- **WHEN** 页面展示素材节点
- **THEN** 系统 SHALL NOT 保存节点位置
- **AND** 系统 SHALL NOT 将节点连线解释为任务编排、素材匹配或作品生产链路
