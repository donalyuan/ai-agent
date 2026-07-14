# material-library-management Specification

## Purpose
TBD - created by archiving change material-library-management. Update Purpose after archive.
## Requirements
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

### Requirement: AI 生成图片必须作为素材库资产持久化

系统 SHALL 将成功生成的 AI 图片保存为素材库资产，并保留稳定访问 URL 和生成来源信息。

#### Scenario: AI 图片候选入库

- **GIVEN** worker 成功生成并下载 AI 图片候选
- **WHEN** 系统写入素材库
- **THEN** 系统 SHALL 创建 `material_type=image` 的 `materials` 记录
- **AND** `file_url` SHALL 指向自管素材存储的稳定 URL
- **AND** `metadata.source` SHALL 为 `ai_generated`
- **AND** `metadata.generation_task_id` SHALL 记录来源任务
- **AND** `metadata.source_scene_id` SHALL 记录来源分镜

#### Scenario: 未选候选保留为素材

- **GIVEN** 某 AI 图片候选已入库但未被选为分镜主素材
- **WHEN** 操作者查看素材库
- **THEN** 系统 SHALL 仍保留该素材
- **AND** `metadata.candidate_status` SHALL 表示该素材是未选候选

### Requirement: 自管素材存储必须提供稳定访问前缀

系统 SHALL 使用本地持久化卷保存第一版 AI 生成图片，并通过 API 静态访问前缀提供稳定 URL。

#### Scenario: 本地持久化存储

- **GIVEN** worker 处理 AI 图片生成结果
- **WHEN** 图片内容下载成功
- **THEN** worker SHALL 将图片写入本地持久化素材目录
- **AND** API SHALL 通过 `/assets/...` 提供访问
- **AND** `materials.metadata.storage_provider` SHALL 为 `local`

#### Scenario: 不保存供应商临时 URL

- **GIVEN** 供应商返回临时图片 URL
- **WHEN** 系统创建素材库记录
- **THEN** `materials.file_url` SHALL NOT 保存该供应商临时 URL
- **AND** `materials.file_url` SHALL 保存自管素材存储 URL

### Requirement: AI 生成图片必须使用可读且一致的物理文件名

系统 SHALL 为新生成的 AI 图片使用 `{脚本名称}-镜头{两位序号}-第{两位候选序号}张.{实际扩展名}` 作为实际物理文件名，并 SHALL 让 `materials.file_name` 与物理 basename 完全一致。

#### Scenario: 中文脚本标题生成图片

- **GIVEN** 任务领取时脚本标题为 `别硬扛，用Debug解决烦心事`
- **AND** 当前结果属于镜头 1 的候选 1，实际图片类型为 JPEG
- **WHEN** Worker 将图片写入自管素材存储
- **THEN** 物理 basename SHALL 为 `别硬扛，用Debug解决烦心事-镜头01-第01张.jpg`
- **AND** `materials.file_name` SHALL 为相同 basename
- **AND** 文件 SHALL 位于本次生成任务 UUID 对应的目录

#### Scenario: 使用实际图片扩展名

- **WHEN** Worker 分别保存实际类型为 PNG、JPEG 和 WebP 的新生成图片
- **THEN** 文件扩展名 SHALL 分别为 `.png`、`.jpg` 和 `.webp`
- **AND** Worker SHALL NOT 使用任意上游文件名覆盖业务 basename
- **AND** Worker SHALL NOT 把所有结果统一保存为 `.png`

#### Scenario: 中文文件 URL 可访问

- **GIVEN** 新生成图片使用包含中文的物理 basename
- **WHEN** 客户端以百分号编码路径请求对应 `/assets/generated/images/...` URL
- **THEN** API 静态素材服务 SHALL 返回该物理文件
- **AND** `materials.file_name` SHALL 继续保存未做 URL 编码的 Unicode basename

### Requirement: 脚本标题必须经过跨平台安全清理和 UTF-8 字节限制

Worker SHALL 对用于文件名的脚本标题执行确定性的 Unicode 与跨平台文件名清理，并 SHALL 保证完整 basename 不超过 255 UTF-8 字节。

#### Scenario: 清理非法字符

- **GIVEN** 脚本标题包含 NFC 可规范化字符、路径分隔符、Windows 非法字符、Unicode 控制字符或结尾点和空格
- **WHEN** Worker 生成图片文件名
- **THEN** Worker SHALL 先执行 Unicode NFC 规范化
- **AND** Worker SHALL 删除 `/`、`\\`、`< > : \" | ? *` 和 Unicode 控制字符
- **AND** Worker SHALL 去除标题首尾空白以及结尾的点和空格
- **AND** 最终文件名 SHALL NOT 创建额外路径层级

#### Scenario: 超长中文标题安全截断

- **GIVEN** 清理后的脚本标题使完整 basename 超过 255 UTF-8 字节
- **WHEN** Worker 构造文件名
- **THEN** Worker SHALL 为镜头、候选和扩展名后缀预留字节
- **AND** Worker SHALL 在 Unicode code point 边界截断标题
- **AND** 完整 basename SHALL 不超过 255 UTF-8 字节

#### Scenario: 空标题使用回退值

- **GIVEN** 脚本标题为空、仅包含空白或清理后为空
- **WHEN** Worker 构造镜头 2 候选 3 的 PNG 文件名
- **THEN** basename SHALL 为 `未命名脚本-镜头02-第03张.png`

### Requirement: 候选编号必须表示原始请求槽位且不得因失败重排

Worker SHALL 使用单镜头内从 1 开始的原始候选请求槽位形成文件名、rank 和 metadata，不得按成功结果列表位置重新编号。

#### Scenario: Batch 中间候选失败

- **GIVEN** OpenAI batch 请求包含候选 1、2、3
- **AND** 候选 2 的结果无效或落盘失败，候选 1 和 3 成功
- **WHEN** Worker 保存成功图片
- **THEN** 两个文件名 SHALL 分别包含 `第01张` 和 `第03张`
- **AND** 候选 3 SHALL NOT 被重排为 `第02张`

#### Scenario: Per-candidate 中间候选失败

- **GIVEN** Ark `per_candidate` 执行候选 1、2、3
- **AND** 候选 2 失败，候选 3 成功
- **WHEN** Worker 保存候选 3
- **THEN** 文件名和 metadata 的候选序号 SHALL 为 `3`
- **AND** 当前候选的临时错误重试 SHALL NOT 改变其候选序号

#### Scenario: 多镜头多候选

- **GIVEN** 一个任务包含镜头 1、2 且每个镜头生成两个候选
- **WHEN** 所有候选成功保存
- **THEN** 文件名 SHALL 分别包含 `镜头01-第01张`、`镜头01-第02张`、`镜头02-第01张` 和 `镜头02-第02张`
- **AND** 所有文件 SHALL 位于本次生成任务 UUID 目录内

### Requirement: 图片命名来源必须形成任务级快照并可审计

Worker SHALL 在领取图片任务时读取一次脚本标题快照，并 SHALL 在成功素材和候选 metadata 中记录脚本标题快照、镜头序号和候选序号。

#### Scenario: 任务领取后脚本改名

- **GIVEN** Worker 已领取任务并读取脚本标题快照
- **WHEN** 脚本在候选落盘前或落盘后被改名
- **THEN** 当前任务 SHALL 继续使用领取时标题快照命名
- **AND** 已生成物理文件和 `materials.file_name` SHALL NOT 被追改
- **AND** 后续新任务 SHALL 使用其各自领取时的标题快照

#### Scenario: 成功素材 metadata 可核对命名来源

- **WHEN** Worker 创建成功图片素材和对应分镜候选
- **THEN** `materials.metadata.script_title_snapshot` SHALL 保存领取时脚本标题原值
- **AND** `materials.metadata.scene_sequence` SHALL 保存 1-based 镜头序号
- **AND** `materials.metadata.candidate_index` SHALL 保存 1-based 候选槽位
- **AND** 对应 `scene_asset_candidates.metadata` SHALL 保存相同三个值

#### Scenario: 失败候选保留槽位审计

- **WHEN** 某个候选生成或落盘失败
- **THEN** 失败候选 metadata SHALL 记录 `script_title_snapshot`、`scene_sequence` 和 `candidate_index`
- **AND** 该失败 SHALL NOT 改变其他候选的 metadata 编号

#### Scenario: 历史文件保持不变

- **GIVEN** 系统已有部署前生成的图片文件和素材记录
- **WHEN** 新命名规则部署
- **THEN** 系统 SHALL NOT 扫描、重命名或改写既有物理文件
- **AND** 系统 SHALL NOT 修改既有 `materials.file_name`、`file_url` 或 metadata
