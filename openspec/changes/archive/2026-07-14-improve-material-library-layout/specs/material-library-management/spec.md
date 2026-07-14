## MODIFIED Requirements

### Requirement: 素材库页面必须采用画布工作台

`apps/video-agent` SHALL 在“素材管理 > 素材库”提供画布优先工作台：主区域是一整块素材节点画布，资产栏以画布上的辅助浮层呈现，详情编辑 SHALL 按选择或新增上下文在右侧打开，底部提供轻量画布工具栏。

#### Scenario: 空状态

- **GIVEN** 当前账号没有可用素材
- **WHEN** 操作者打开素材库
- **THEN** 页面 SHALL 展示空画布状态
- **AND** 页面 SHALL 提供“新增素材”入口
- **AND** 详情区域 SHALL 默认隐藏

#### Scenario: 素材库默认画布骨架

- **GIVEN** 当前账号存在素材
- **WHEN** 操作者打开素材库
- **THEN** 页面 SHALL 展示主画布、资产浮层和底部画布工具栏
- **AND** 页面 SHALL NOT 自动选择第一条素材
- **AND** 右侧详情区域 SHALL 默认隐藏
- **AND** 素材节点 SHALL 展示缩略图或类型占位
- **AND** 页面 SHALL 不展示上传、语义检索、分镜候选或素材清单确认入口

#### Scenario: 选择素材节点

- **GIVEN** 当前账号存在素材节点
- **WHEN** 操作者在资产栏或画布中选择一个素材
- **THEN** 右侧详情抽屉 SHALL 打开
- **AND** 详情抽屉 SHALL 展示该素材的基础信息、缩略图 URL、标签、类型相关 metadata、保存和归档或恢复操作
- **AND** 画布节点 SHALL 重新排布且不得被详情抽屉遮挡

#### Scenario: 关闭素材详情

- **GIVEN** 右侧详情抽屉已打开
- **WHEN** 操作者关闭详情抽屉
- **THEN** 页面 SHALL 清除当前素材选择或新建状态
- **AND** 详情抽屉 SHALL 隐藏
- **AND** 画布 SHALL 使用释放的可用宽度重新排布节点

#### Scenario: 新增素材打开详情

- **GIVEN** 操作者打开素材库
- **WHEN** 操作者点击“新增素材”
- **THEN** 右侧详情抽屉 SHALL 进入新建状态
- **AND** 素材名称输入 SHALL 获得焦点

#### Scenario: 类型相关扩展字段

- **GIVEN** 详情抽屉处于新增或编辑状态
- **WHEN** 操作者选择素材类型
- **THEN** 页面 SHALL 只展示该类型适用的扩展字段
- **AND** 视频或音频 SHALL 展示时长与格式
- **AND** 图片 SHALL 展示宽度、高度与格式
- **AND** 字幕 SHALL 展示字幕语言与字幕格式

#### Scenario: 长文件名节点保持稳定

- **GIVEN** 素材名称超过一个节点标题区域可容纳的长度
- **WHEN** 页面派生画布节点
- **THEN** 节点标题 SHALL 限制在固定两行区域并截断溢出内容
- **AND** 标题 SHALL NOT 遮挡节点元信息或相邻节点
- **AND** 节点列数 SHALL 根据工作区可用宽度计算且至少为一列

#### Scenario: 画布不表达编排语义

- **GIVEN** 操作者打开素材库画布
- **WHEN** 页面展示素材节点
- **THEN** 系统 SHALL NOT 保存节点位置
- **AND** 系统 SHALL NOT 将节点连线解释为任务编排、素材匹配或作品生产链路
