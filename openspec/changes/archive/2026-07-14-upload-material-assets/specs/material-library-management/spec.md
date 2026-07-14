## ADDED Requirements

### Requirement: 素材库必须支持文件上传和自动元数据识别

系统 SHALL 允许操作者在当前账号下上传受支持的图片、视频、音频或字幕文件，并由系统保存文件、识别类型、生成稳定地址、提取 metadata 和创建 `active` 素材。

#### Scenario: 上传图片素材

- **GIVEN** 当前账号存在
- **WHEN** 操作者上传可解码的 JPEG、PNG、WebP 或 GIF 图片
- **THEN** 系统 SHALL 将文件保存到自管素材存储
- **AND** 系统 SHALL 创建 `material_type=image`、`usage_count=0`、`status=active` 的素材
- **AND** metadata SHALL 包含 `source=user_upload`、`storage_provider=local`、MIME、格式、字节大小、宽度和高度
- **AND** `file_url` SHALL 指向 `/assets/uploads/...` 稳定地址

#### Scenario: 上传视频或音频素材

- **GIVEN** 当前账号存在
- **WHEN** 操作者上传受支持且可被媒体探测器解析的视频或音频
- **THEN** 系统 SHALL 创建对应类型的素材
- **AND** metadata SHALL 包含格式、MIME、字节大小和时长
- **AND** 视频 metadata SHALL 在可用时包含宽度和高度

#### Scenario: 上传字幕素材

- **GIVEN** 当前账号存在
- **WHEN** 操作者上传 UTF-8 编码的 SRT、VTT、ASS 或 SSA 文件
- **THEN** 系统 SHALL 创建 `material_type=subtitle` 的素材
- **AND** metadata SHALL 包含 `subtitle_format`、MIME 和字节大小

#### Scenario: 上传失败不留下数据

- **WHEN** 上传缺少文件、文件为空、超过 500 MiB、类型不受支持、内容不可解析或数据库写入失败
- **THEN** 系统 SHALL NOT 创建素材记录
- **AND** 系统 SHALL 删除本次请求已经写入的文件
- **AND** API SHALL 返回可理解的错误

#### Scenario: 上传时只填写业务字段

- **GIVEN** 操作者打开上传素材抽屉
- **WHEN** 操作者选择文件
- **THEN** 页面 SHALL 从文件名自动填充素材名称
- **AND** 操作者 SHALL 只需确认素材名称并可选填写标签
- **AND** 页面 SHALL NOT 要求操作者填写素材地址、缩略图地址、类型、格式、尺寸、时长、来源备注或授权备注

### Requirement: 图片素材必须支持大图预览

页面 SHALL 允许操作者从图片素材详情打开大图预览，并提供稳定的关闭和缩放交互。

#### Scenario: 打开图片大图

- **GIVEN** 当前详情素材具有图片预览地址
- **WHEN** 操作者点击详情图片
- **THEN** 页面 SHALL 打开大图预览对话框
- **AND** 对话框 SHALL 展示图片、素材名称和只读文件摘要

#### Scenario: 缩放和关闭大图

- **GIVEN** 大图预览已打开
- **WHEN** 操作者使用放大或缩小按钮
- **THEN** 图片缩放 SHALL 限制在 50% 至 200%
- **WHEN** 操作者点击关闭按钮、遮罩或按 Escape
- **THEN** 对话框 SHALL 关闭
- **AND** 焦点 SHALL 返回打开预览的控件

#### Scenario: 非图片素材不打开大图

- **GIVEN** 当前素材只有视频、音频或字幕类型占位
- **WHEN** 操作者查看详情
- **THEN** 页面 SHALL NOT 将类型占位作为大图预览入口

## MODIFIED Requirements

### Requirement: 素材库必须支持编辑、归档和恢复

系统 SHALL 允许操作者编辑素材名称和标签，并将素材状态在 `active` 和 `archived` 之间切换；文件地址、缩略图地址、素材类型和系统 metadata SHALL 保持只读。

#### Scenario: 编辑素材基础信息

- **GIVEN** 当前账号下存在一条素材
- **WHEN** 操作者修改素材名称或标签并保存
- **THEN** 系统 SHALL 更新素材名称或标签
- **AND** 系统 SHALL 保留原 `file_url`、`thumbnail_url`、素材类型和 metadata
- **AND** 资产栏、画布节点和详情 SHALL 展示最新内容
- **AND** 页面 SHALL NOT 展示或允许编辑素材地址

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

`apps/video-agent` SHALL 在“素材管理 > 素材库”提供画布优先工作台：主区域是一整块素材节点画布，资产栏以画布上的辅助浮层呈现，上传或详情编辑 SHALL 按上下文在右侧打开，底部提供轻量画布工具栏。

#### Scenario: 空状态

- **GIVEN** 当前账号没有可用素材
- **WHEN** 操作者打开素材库
- **THEN** 页面 SHALL 展示空画布状态
- **AND** 页面 SHALL 提供“上传素材”入口
- **AND** 详情区域 SHALL 默认隐藏

#### Scenario: 素材库默认画布骨架

- **GIVEN** 当前账号存在素材
- **WHEN** 操作者打开素材库
- **THEN** 页面 SHALL 展示主画布、资产浮层和底部画布工具栏
- **AND** 页面 SHALL NOT 自动选择第一条素材
- **AND** 右侧详情区域 SHALL 默认隐藏
- **AND** 素材节点 SHALL 展示缩略图或类型占位
- **AND** 页面 SHALL 不展示语义检索、分镜候选或素材清单确认入口

#### Scenario: 选择素材节点

- **GIVEN** 当前账号存在素材节点
- **WHEN** 操作者在资产栏或画布中选择一个素材
- **THEN** 右侧详情抽屉 SHALL 打开
- **AND** 详情抽屉 SHALL 展示预览、素材名称、标签、只读系统文件信息、保存和归档或恢复操作
- **AND** 详情抽屉 SHALL NOT 展示素材地址、缩略图地址、来源备注、授权备注或可编辑媒体 metadata
- **AND** 画布节点 SHALL 重新排布且不得被详情抽屉遮挡

#### Scenario: 关闭素材详情

- **GIVEN** 右侧详情抽屉已打开
- **WHEN** 操作者关闭详情抽屉
- **THEN** 页面 SHALL 清除当前素材选择或上传状态
- **AND** 详情抽屉 SHALL 隐藏
- **AND** 画布 SHALL 使用释放的可用宽度重新排布节点

#### Scenario: 上传素材打开详情

- **GIVEN** 操作者打开素材库
- **WHEN** 操作者点击“上传素材”
- **THEN** 右侧详情抽屉 SHALL 进入上传状态
- **AND** 文件选择控件 SHALL 可用
- **AND** 选择文件后素材名称 SHALL 自动填充并可编辑

#### Scenario: 系统文件信息只读

- **GIVEN** 详情抽屉正在展示已保存素材
- **WHEN** 素材 metadata 包含格式、尺寸、时长或字节大小
- **THEN** 页面 SHALL 以只读摘要展示适用于当前类型的信息
- **AND** 页面 SHALL NOT 将这些信息渲染为输入控件

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

## REMOVED Requirements

### Requirement: 素材库必须支持 URL 素材登记

**Reason**: 正式素材库改为直接上传文件并由系统生成地址和 metadata，继续要求操作者手填 URL 会产生重复入口和错误数据。

**Migration**: 前端统一使用 `POST /api/projects/:project_id/materials/upload`；后端已有 JSON 创建接口保留给 AI 生成素材等内部调用，但不再作为素材库页面能力。
