## ADDED Requirements

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
