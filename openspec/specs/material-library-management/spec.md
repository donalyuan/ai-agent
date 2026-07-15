# material-library-management Specification

## Purpose
TBD - created by archiving change material-library-management. Update Purpose after archive.
## Requirements
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

### Requirement: 素材库必须统一管理作品生产生成物

系统 SHALL 将作品生产产生的 TTS 音频、ASR/TTS 字幕、混音音频和最终可复用媒体登记为素材，并与已有图片、视频、音频和字幕使用同一素材生命周期管理。

#### Scenario: TTS 和字幕生成成功后自动入库

- **GIVEN** 作品运行中的 TTS 和字幕步骤成功
- **WHEN** 系统持久化步骤输出
- **THEN** 系统 SHALL 分别创建 `audio` 和 `subtitle` 素材
- **AND** 素材 SHALL 使用自管存储稳定 URL
- **AND** 素材 SHALL 关联来源作品、版本、运行和步骤

#### Scenario: 重新生成不覆盖旧素材

- **GIVEN** 某作品版本已经生成 TTS 或字幕素材
- **WHEN** 操作者确认重新生成相关节点
- **THEN** 系统 SHALL 创建新的素材记录和文件
- **AND** 系统 SHALL NOT 覆盖、改写或删除旧素材

#### Scenario: 文件落盘失败不登记素材

- **GIVEN** 作品步骤已返回媒体结果
- **WHEN** 自管存储写入或完整性校验失败
- **THEN** 系统 SHALL 将步骤结果标记失败
- **AND** 系统 SHALL NOT 创建伪成功素材记录

### Requirement: 生成素材必须保留可审计快照

系统 SHALL 为作品生产生成的素材保存足以回溯结果的来源、模型、提示词、时间轴和参数快照，且 SHALL NOT 保存明文密钥。

#### Scenario: 查看 TTS 素材来源

- **GIVEN** 素材由 TTS 步骤生成
- **WHEN** 操作者查看素材详情
- **THEN** 系统 SHALL 展示来源作品和版本、模型快照、音色快照、声音参数、文本摘要、语言、时长和来源任务
- **AND** 系统 SHALL 保留供应商请求追踪 ID
- **AND** 系统 SHALL NOT 展示或保存 `X-Api-Key`

#### Scenario: 查看字幕素材来源

- **GIVEN** 素材由 TTS 时间戳或 ASR 生成
- **WHEN** 操作者查看字幕素材详情
- **THEN** 系统 SHALL 展示字幕语言、格式、对齐来源、时间轴版本和来源音频
- **AND** 系统 SHALL 能区分 `tts_timestamp` 与 `asr` 来源

### Requirement: 已有声音素材必须可用于作品混音

系统 SHALL 允许操作者从 `active` 音频素材中选择已有 BGM、环境音和动作音效用于作品时间轴，但首版 SHALL NOT 提供这些类型的 AI 生成入口。

#### Scenario: 选择已有音频进入作品

- **GIVEN** 素材库存在标记为 BGM、环境音或动作音效的 `active` 音频
- **WHEN** 操作者在作品生成中选择已有音频
- **THEN** 系统 SHALL 将素材引用和混音参数加入作品草稿
- **AND** 系统 SHALL NOT 复制或覆盖原素材

#### Scenario: 归档音频不可用于新作品

- **GIVEN** 某音频素材状态为 `archived`
- **WHEN** 操作者为新作品选择声音素材
- **THEN** 系统 SHALL NOT 将该素材列为可选项
- **AND** 已完成历史版本 SHALL 继续保留该素材快照和引用

#### Scenario: 不展示未落地 AI 声音生成入口

- **GIVEN** AI 音乐、环境音和动作音效生成尚未配置正式能力
- **WHEN** 操作者打开素材管理
- **THEN** 页面 SHALL NOT 展示 AI 音乐、环境音生成或动作音效生成标签及可执行按钮
- **AND** 素材库 SHALL 继续允许上传和管理已有相关音频

### Requirement: 素材筛选必须覆盖作品生产声音类型

素材库 SHALL 支持按音频用途、生成来源、来源作品和来源版本筛选作品生产素材。

#### Scenario: 按音频用途筛选

- **GIVEN** 素材库同时存在 TTS、BGM、环境音和动作音效
- **WHEN** 操作者选择某一音频用途筛选
- **THEN** 系统 SHALL 只返回匹配用途的音频素材
- **AND** 未标注用途的历史音频 SHALL 保持可见且显示为未分类

#### Scenario: 从作品版本定位生成素材

- **GIVEN** 某作品版本生成了视频、音频和字幕产物
- **WHEN** 操作者按该作品版本筛选素材库
- **THEN** 系统 SHALL 返回与该版本关联的全部可复用素材
- **AND** 结果 SHALL 保留各自产物类型和生成步骤信息
