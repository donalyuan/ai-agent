# script-to-asset-generation Specification

## Purpose
TBD - created by archiving change script-to-asset-generation. Update Purpose after archive.
## Requirements
### Requirement: 脚本详情必须支持生成素材候选计划

系统 SHALL 允许操作者在脚本详情页为脚本生成素材候选计划，计划必须基于脚本分镜、候选数量、供应商和参考图设置计算风险与上限。

#### Scenario: 默认生成计划

- **GIVEN** 当前脚本存在 6 个分镜
- **WHEN** 操作者打开素材候选生成设置且未修改候选数量
- **THEN** 系统 SHALL 使用每分镜 3 张图片候选作为默认值
- **AND** 计划 SHALL 显示预计生成 18 张图片候选
- **AND** 计划 SHALL 显示默认供应商为 `gpt-image-2`

#### Scenario: 超过单次图片上限

- **GIVEN** 当前脚本存在 13 个分镜
- **WHEN** 操作者请求每分镜 4 张图片候选
- **THEN** 系统 SHALL 拒绝创建生成任务
- **AND** 响应 SHALL 提示单次最多生成 48 张图片候选

### Requirement: 素材生成必须作为独立工作台入口

系统 SHALL 将脚本生成与画面生成拆分为不同二级菜单入口；原“素材生成” SHALL 更名为“画面生成”，并只承载脚本分镜图片候选生成和主画面选择。

#### Scenario: 画面生成独立二级菜单

- **GIVEN** 操作者打开视频工作台菜单
- **WHEN** 操作者展开素材管理
- **THEN** 系统 SHALL 按顺序显示 `素材库`、`画面生成` 和 `声音与字幕生成`
- **AND** `画面生成` SHALL 承载脚本分镜图片候选生成、预览、排除、重生和主画面选择
- **AND** `画面生成` SHALL NOT 创建新的逐分镜视频任务

#### Scenario: 脚本生成页不显示素材候选面板

- **GIVEN** 操作者打开 `脚本创作 / 脚本生成`
- **WHEN** 操作者选择一个脚本
- **THEN** 页面 SHALL 只显示脚本列表、时间轴详情和脚本 Agent 对话
- **AND** 页面 SHALL NOT 显示素材候选生成面板

#### Scenario: 去除说明性小标题

- **GIVEN** 操作者查看脚本详情或画面生成页
- **WHEN** 页面展示来源选题、脚本 Agent 对话或图片候选区域
- **THEN** 页面 SHALL NOT 显示 `Topic Source`、独立 `Agent` 或 `素材 Agent` 这类说明性小标题
- **AND** 页面 SHALL 使用中文业务标签展示选题类型，不裸露 `knowledge` 枚举值

### Requirement: 系统必须优先复用旧素材

系统 SHALL 在创建 AI 图片生成任务前，先为分镜提供当前账号下可用旧素材候选。

#### Scenario: 人物和固定 IP 素材优先进入候选

- **GIVEN** 当前账号素材库存在 `active` 人物或固定 IP 图片素材
- **WHEN** 操作者为脚本生成素材候选
- **THEN** 系统 SHALL 将匹配的旧素材作为 `existing_material` 候选返回
- **AND** AI 图片生成可引用这些素材作为参考图

#### Scenario: 归档素材不可作为新候选

- **GIVEN** 当前账号素材库存在 `archived` 素材
- **WHEN** 操作者为脚本生成素材候选
- **THEN** 系统 SHALL NOT 将该素材作为新的旧素材候选
- **AND** 系统 SHALL NOT 将该素材作为 AI 图片参考图

### Requirement: AI 图片必须自动生成多候选

系统 SHALL 支持为每个分镜自动生成 1-4 张 AI 图片候选，并将生成成功的图片入库为素材。

#### Scenario: 每分镜生成多候选

- **GIVEN** 当前脚本存在 3 个分镜
- **WHEN** 操作者选择每分镜 3 张图片候选并启动生成
- **THEN** 系统 SHALL 创建 9 张图片候选的异步生成任务
- **AND** API SHALL NOT 同步等待外部供应商完成

#### Scenario: 生成图片写入稳定素材 URL

- **GIVEN** worker 从供应商获得图片结果
- **WHEN** 图片下载到本地持久化素材存储成功
- **THEN** 系统 SHALL 创建 `materials.material_type=image` 记录
- **AND** `materials.file_url` SHALL 使用 `/assets/...` 稳定访问 URL
- **AND** `materials.metadata` SHALL 记录 `storage_provider=local`、`source=ai_generated`、`generation_task_id`、`source_scene_id` 和 `reference_material_ids`

#### Scenario: 下载失败不入库

- **GIVEN** 供应商返回图片结果
- **WHEN** worker 下载图片或写入本地素材存储失败
- **THEN** 系统 SHALL 将对应候选标记为 `failed`
- **AND** 系统 SHALL NOT 创建 `materials` 记录

### Requirement: 分镜只能选择一个主素材候选

系统 SHALL 允许操作者从分镜候选中选择一个主素材，并保证同一分镜最多只有一个 `selected` 候选。

#### Scenario: 选择候选为主素材

- **GIVEN** 某分镜存在多个 `candidate` 候选
- **WHEN** 操作者选择其中一个候选
- **THEN** 系统 SHALL 将该候选状态更新为 `selected`
- **AND** 同一分镜其他已选候选 SHALL 回到 `candidate` 或被取消选中

#### Scenario: 失败候选不可选择

- **GIVEN** 某候选状态为 `failed`
- **WHEN** 操作者尝试选择该候选
- **THEN** 系统 SHALL 拒绝选择
- **AND** 响应 SHALL 提示失败候选不可绑定分镜

### Requirement: 供应商选择和重试必须可控

系统 SHALL 支持用户在已启用供应商中选择 `gpt-image-2` 或即梦，并控制重试行为。

#### Scenario: 选择图片生成供应商

- **GIVEN** `gpt-image-2` 和即梦均已启用
- **WHEN** 操作者打开素材候选生成设置
- **THEN** 页面 SHALL 默认选择 `gpt-image-2`
- **AND** 操作者 SHALL 能切换到即梦

#### Scenario: 不自动跨供应商重试

- **GIVEN** 操作者选择 `gpt-image-2`
- **WHEN** 图片生成任务失败
- **THEN** 系统 SHALL NOT 自动切换到即梦重试
- **AND** 跨供应商重试必须由操作者人工确认

#### Scenario: 同供应商临时错误最多重试一次

- **GIVEN** 图片生成任务遇到供应商临时错误
- **WHEN** worker 处理该任务
- **THEN** 系统 SHALL 最多自动重试 1 次
- **AND** 再次失败后任务 SHALL 标记为 `failed`

### Requirement: 单镜头重生必须防止重复计费

系统 SHALL 在前端、API 和数据库三层阻止单镜头重生的重复请求创建多条可计费任务，且费用安全不得依赖前端按钮状态。

#### Scenario: 同一次用户操作快速连点

- **GIVEN** 操作者对某分镜点击“单镜头重生”
- **WHEN** 同一页面在首个请求完成前再次触发该操作
- **THEN** 前端 SHALL 只发送一个请求
- **AND** 请求 SHALL 携带本次用户操作唯一的 UUID 格式 `Idempotency-Key`

#### Scenario: 相同幂等键重试

- **GIVEN** 单镜头重生请求已创建任务
- **WHEN** 网络层使用相同 `Idempotency-Key` 重试请求
- **THEN** API SHALL 返回原任务
- **AND** 数据库 SHALL NOT 创建第二条任务

#### Scenario: 不同页面并发重生同一分镜

- **GIVEN** 某分镜没有在途图片生成任务
- **WHEN** 两个页面或设备使用不同 `Idempotency-Key` 并发请求重生该分镜
- **THEN** 系统 SHALL 只创建一条 `pending/processing` 图片任务
- **AND** 两个请求 SHALL 返回同一个任务 ID
- **AND** 两个 `Idempotency-Key` SHALL 分别永久映射到该任务

#### Scenario: 在途任务完成后再次重生

- **GIVEN** 某分镜上一条图片生成任务已进入 `completed` 或 `failed`
- **WHEN** 操作者发起新的重生操作并使用新的 `Idempotency-Key`
- **THEN** 系统 SHALL 创建新任务
- **AND** 使用旧 `Idempotency-Key` 的迟到重试仍 SHALL 返回旧任务

#### Scenario: 首次响应丢失后人工重试

- **GIVEN** 单镜头重生请求可能已经到达服务端，但前端未收到成功响应
- **WHEN** 操作者再次点击“单镜头重生”
- **THEN** 前端 SHALL 复用上一次未确认请求的 `Idempotency-Key`
- **AND** 只有收到成功任务响应后才 SHALL 为下一次明确重生生成新 key

### Requirement: 图片生成任务和结果必须持续可见

系统 SHALL 在任务创建后展示图片任务状态，并在任务处于在途状态时刷新任务与候选素材，确保操作者能观察排队、生成、完成和失败结果。

#### Scenario: 批量任务创建后立即显示

- **GIVEN** 操作者为一个脚本创建图片候选任务
- **WHEN** API 返回 `pending` 图片任务
- **THEN** 页面 SHALL 展示该图片任务、候选数量和“排队中”状态
- **AND** 不得只展示视频待确认任务

#### Scenario: 在途任务完成后自动展示候选

- **GIVEN** 当前脚本存在 `pending` 或 `processing` 图片任务
- **WHEN** Worker 将任务更新为终态并写入候选素材
- **THEN** 页面 SHALL 自动刷新任务和候选素材
- **AND** 图片任务全部进入终态后 SHALL 停止轮询

#### Scenario: 生成任务失败

- **GIVEN** Worker 无法配置或调用所选图片供应商
- **WHEN** 已领取任务无法继续生成
- **THEN** Worker SHALL 将任务更新为 `failed`
- **AND** 页面 SHALL 展示失败状态和错误信息
- **AND** 任务 SHALL NOT 永久停留在 `processing`

#### Scenario: 图片供应商返回永久错误

- **GIVEN** 批量图片任务包含多个分镜
- **WHEN** 供应商在首个分镜返回鉴权、权限或非法请求等永久错误
- **THEN** Worker SHALL 停止该任务剩余分镜的供应商调用
- **AND** Worker SHALL 为未执行候选写入失败状态
- **AND** 任务错误 SHALL 保留供应商 HTTP 状态和响应摘要

#### Scenario: 从文本端点推导图片端点

- **GIVEN** `OPENAI_IMAGE_BASE_URL` 未配置且 `OPENAI_BASE_URL` 为 OpenAI-compatible `/responses` 端点
- **WHEN** Worker 创建 `gpt-image-2` provider
- **THEN** 图片 API 根路径 SHALL 使用对应 `/v1` 地址
- **AND** 图片请求 SHALL 携带兼容 `User-Agent`

### Requirement: 自管素材 URL 必须从 API 地址加载

系统 SHALL 将 `/assets/...` 相对素材 URL 解析到当前 API `baseUrl`，确保工作台与 API 使用不同端口时仍能加载候选图和素材库预览。

#### Scenario: 工作台和 API 使用不同端口

- **GIVEN** 工作台运行在 `18183` 且 API 运行在 `18180`
- **WHEN** 候选或素材的 `file_url` 为 `/assets/generated/images/example.png`
- **THEN** 浏览器 SHALL 从 API `baseUrl` 加载该图片
- **AND** 不得向工作台端口请求 `/assets/...`

### Requirement: 失败素材生成任务必须支持可审计清理

系统 SHALL 允许操作者将失败素材生成任务及其失败候选从当前页面隐藏，同时保留任务、错误、数量、生成参数、结果摘要和费用审计；清理 SHALL NOT 调用 Worker 或供应商。

#### Scenario: 二次确认后清理失败任务

- **GIVEN** 素材生成任务状态为 `failed` 且尚未清理
- **WHEN** 操作者点击“清理失败任务”并在确认弹窗中确认
- **THEN** 系统 SHALL 写入该任务的 `dismissed_at`
- **AND** 任务状态 SHALL 保持 `failed`
- **AND** 页面 SHALL 不再显示该任务及其关联的 `failed` 候选
- **AND** 系统 SHALL NOT 调用 Worker 或供应商
- **AND** 该操作 SHALL NOT 产生外部费用

#### Scenario: 清理保留审计和成功素材

- **GIVEN** 某失败任务保留错误、候选数量、生成参数、结果摘要或部分成功素材
- **WHEN** 该任务被清理
- **THEN** 数据库 SHALL 保留任务记录及上述审计字段
- **AND** 系统 SHALL 只隐藏该任务关联的 `failed` 候选
- **AND** 已成功生成并入库的素材及非失败候选 SHALL NOT 被删除或隐藏

#### Scenario: 非失败任务不可清理

- **GIVEN** 素材生成任务状态为 `draft`、`pending`、`processing` 或 `completed`
- **WHEN** 客户端请求清理该任务
- **THEN** API SHALL 返回 `409 Conflict`
- **AND** 任务及候选 SHALL 保持不变
- **AND** 系统 SHALL NOT 调用 Worker 或供应商

#### Scenario: 重复清理保持幂等

- **GIVEN** 某失败任务已经写入 `dismissed_at`
- **WHEN** 客户端再次请求清理同一任务
- **THEN** API SHALL 返回该任务既有清理结果
- **AND** `dismissed_at` SHALL NOT 被改写为新的时间
- **AND** 系统 SHALL NOT 产生新的任务、候选或外部调用

#### Scenario: 清理确认弹窗说明影响

- **GIVEN** 操作者准备清理失败任务
- **WHEN** 页面显示二次确认弹窗
- **THEN** 弹窗 SHALL 说明任务及失败候选将从页面隐藏
- **AND** 弹窗 SHALL 说明数据库继续保留任务状态、错误、数量和费用审计
- **AND** 弹窗 SHALL 说明清理不会调用供应商且不会产生额外费用

### Requirement: 候选素材区必须展示当前镜头完整内容

系统 SHALL 在当前镜头的候选素材上方展示镜头脚本，并明确区分旁白和画面；左侧分镜列表 SHALL 保持紧凑。

#### Scenario: 展示当前镜头旁白和画面

- **GIVEN** 操作者选择一个包含旁白和画面描述的分镜
- **WHEN** 页面展示该分镜的素材候选
- **THEN** 候选区顶部 SHALL 显示镜头序号和时长
- **AND** 左栏 SHALL 完整显示 `Scene.narration`
- **AND** 右栏 SHALL 完整显示 `Scene.visual_description`
- **AND** 旁白或画面 SHALL NOT 被挤入左侧分镜列表

#### Scenario: 没有候选时仍展示镜头内容

- **GIVEN** 当前分镜尚未生成任何素材候选
- **WHEN** 操作者选择该分镜
- **THEN** 候选区 SHALL 仍展示该分镜的旁白和画面
- **AND** 候选空状态 SHALL 显示在镜头内容之后

#### Scenario: 镜头字段为空

- **GIVEN** 当前分镜的旁白或画面描述为空
- **WHEN** 页面展示镜头内容
- **THEN** 对应分栏 SHALL 显示“未填写旁白”或“未填写画面”
- **AND** 镜头内容区域 SHALL 保持稳定布局

### Requirement: Worker 后台消费必须显式启用

系统 SHALL 默认关闭图片任务后台消费，并只在运维显式设置 `ASSET_GENERATION_WORKER_ENABLED=true` 后执行可计费任务。

#### Scenario: 未显式启用 Worker

- **GIVEN** 环境未设置 `ASSET_GENERATION_WORKER_ENABLED=true`
- **WHEN** Worker 服务启动
- **THEN** Worker SHALL 保持健康但不自动领取图片任务
- **AND** 健康响应 SHALL 暴露后台消费为关闭状态

### Requirement: API 启动前必须应用待执行数据库迁移

系统 SHALL 在构建运行态数据库连接后、开始提供 HTTP 服务前执行仓库内全部待执行 SQLx migration，避免新路由访问旧 schema。

#### Scenario: 运行库缺少最新 migration

- **GIVEN** API 连接的数据库尚未执行最新 migration
- **WHEN** API 构建运行态数据库连接
- **THEN** 系统 SHALL 先执行待执行 migration 并记录到 `_sqlx_migrations`
- **AND** 只有 migration 成功后 API 才 SHALL 继续启动
- **AND** migration 失败时 API SHALL 启动失败，不得以不完整 schema 提供服务

### Requirement: 素材生成页必须支持预览 AI 图片候选

系统 SHALL 允许操作者在 `素材管理 / 素材生成` 页面查看具有有效图片预览 URL 的 AI 图片候选大图，且预览行为不得改变候选选择状态。

#### Scenario: 打开 AI 图片候选大图

- **GIVEN** 当前分镜存在一张具有有效预览 URL 的 `ai_generated` 图片候选
- **WHEN** 操作者点击该候选缩略图
- **THEN** 系统 SHALL 打开显示完整候选图片的大图预览弹层
- **AND** 弹层 SHALL 显示候选文件名
- **AND** 系统 SHALL NOT 选择或排除该候选

#### Scenario: 缩放并关闭候选大图

- **GIVEN** AI 图片候选大图预览已经打开
- **WHEN** 操作者使用缩放控件
- **THEN** 系统 SHALL 在 50%-200% 范围内按 25% 步长缩放图片
- **AND** 操作者 SHALL 能通过关闭按钮、Escape 或点击遮罩关闭弹层
- **AND** 关闭后焦点 SHALL 返回打开弹层的候选缩略图

#### Scenario: 无图片结果的候选不可预览

- **GIVEN** AI 图片候选处于失败、等待生成或没有有效预览 URL 的状态
- **WHEN** 操作者查看该候选卡片
- **THEN** 系统 SHALL NOT 将占位区域渲染为大图预览按钮
- **AND** 原有失败或等待生成状态 SHALL 保持可见

#### Scenario: 非 AI 图片候选不扩大预览范围

- **GIVEN** 当前分镜存在旧素材候选或当前主素材
- **WHEN** 操作者查看素材生成页候选卡片
- **THEN** 本次变更 SHALL NOT 为这些卡片新增大图预览入口
- **AND** 其原有选择、排除和状态展示 SHALL 保持不变

### Requirement: 作品生成必须读取完整主画面清单

系统 SHALL 将画面生成中每个分镜唯一的已选图片作为作品生成输入，并在缺失、归档或失败时阻止创建可执行作品计划。

#### Scenario: 所有分镜已选择主画面

- **GIVEN** 脚本每个分镜均有一个可用的 `selected` 图片候选
- **WHEN** 操作者进入作品生成
- **THEN** 系统 SHALL 按分镜顺序加载全部主画面、镜头描述和旁白
- **AND** 系统 SHALL 保留对应素材 ID、分镜版本和来源快照

#### Scenario: 存在缺失主画面的分镜

- **GIVEN** 至少一个分镜没有可用的已选主图片
- **WHEN** 操作者请求生成作品方案
- **THEN** 系统 SHALL 阻止创建可执行计划
- **AND** 页面 SHALL 标出缺失主画面的分镜并提供返回画面生成的入口

#### Scenario: 主画面在计划后变化

- **GIVEN** 作品计划引用的主图片或分镜内容已变化
- **WHEN** 操作者尝试确认旧作品计划
- **THEN** 系统 SHALL 判定输入快照过期并拒绝提交
- **AND** 系统 SHALL 要求重新生成作品计划

### Requirement: 历史逐分镜视频任务必须只读保留

系统 SHALL 保留既有 `video_draft/video_generation` 任务及其结果用于历史审计，但新画面生成和作品生成流程 SHALL NOT 继续写入该模型。

#### Scenario: 查看历史逐分镜视频任务

- **GIVEN** 数据库存在历史逐分镜视频任务
- **WHEN** 操作者查看对应历史记录
- **THEN** 系统 SHALL 以只读方式展示任务状态、参数、错误和结果
- **AND** 系统 SHALL NOT 提供再次确认、重试或继续执行该旧任务的入口

#### Scenario: 新作品生成不写旧任务模型

- **GIVEN** 操作者从画面生成进入作品生成
- **WHEN** 操作者确认作品级运行
- **THEN** 系统 SHALL 创建作品生产领域的运行和子任务
- **AND** 系统 SHALL NOT 创建新的 `video_draft/video_generation` 记录
