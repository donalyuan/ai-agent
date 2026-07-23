# manual-publication-operations Specification

## Purpose
TBD - created by archiving change add-manual-publication-operations. Update Purpose after archive.
## Requirements
### Requirement: 发布计划必须消费明确作品版本交接

系统 SHALL 从现有 `publication_handoff` 幂等创建发布计划，并永久绑定交接中明确的作品、完成版本、成片和可选字幕，不得自动改用作品最新版本。

#### Scenario: 从完成版本进入发布工作台

- **GIVEN** 操作者已把某个完成作品版本交接给发布运营
- **WHEN** 前端请求为该 handoff 创建发布计划
- **THEN** 系统 SHALL 返回绑定该 handoff 的唯一发布计划
- **AND** 页面 SHALL 展示被选中的作品版本与成片

#### Scenario: 重复进入同一交接单

- **GIVEN** 某 handoff 已存在发布计划
- **WHEN** 操作者重复点击“进入发布”或重试相同幂等请求
- **THEN** 系统 SHALL 返回原发布计划
- **AND** 系统 SHALL NOT 创建重复计划或平台目标

### Requirement: 抖音和小红书目标必须独立管理

系统 SHALL 允许同一发布计划包含抖音和小红书目标，并分别保存平台文案、标签、封面、计划时间、发布包和状态；系统 SHALL NOT 创建或依赖平台账号聚合。

#### Scenario: 为两个平台准备不同文案

- **GIVEN** 某发布计划包含抖音和小红书目标
- **WHEN** 操作者分别修改两个平台的标题、正文或标签
- **THEN** 系统 SHALL 保存两个独立平台快照
- **AND** 一个目标的修改 SHALL NOT 使另一个目标的草稿或有效发布包变化

#### Scenario: 不展示账号绑定能力

- **GIVEN** 当前平台模式为人工网页交接
- **WHEN** 操作者打开发布工作台
- **THEN** 页面 SHALL NOT 展示账号绑定、多账号选择、OAuth 状态或账号同步结果
- **AND** 后端 SHALL NOT 要求 `account_id`、Cookie、Token 或 Secret

### Requirement: 人工计划时间不得触发自动发布

系统 SHALL 允许操作者为平台目标设置计划发布时间，并按时间展示待发布与逾期提醒；该时间 SHALL NOT 创建平台发布任务或触发后台 Worker。

#### Scenario: 设置计划发布时间

- **GIVEN** 某平台目标处于非终态
- **WHEN** 操作者设置未来计划时间
- **THEN** 系统 SHALL 保存该时间并用于排序和提醒
- **AND** 到达计划时间时系统 SHALL NOT 自动打开网页、上传文件或改变发布状态

#### Scenario: 发布目标逾期

- **GIVEN** 计划时间已过去且目标尚未人工确认发布
- **WHEN** 操作者查看待发布列表
- **THEN** 页面 SHALL 明确展示逾期状态
- **AND** 目标 SHALL 保持其真实业务状态

### Requirement: 发布包必须绑定平台草稿版本并校验完整性

系统 SHALL 为平台目标生成包含成片、可选封面、发布文案、检查清单和 manifest 的版本化发布包；生成前必须验证来源 artifact 存在且 SHA-256 一致。

#### Scenario: 成功生成发布包

- **GIVEN** 目标引用的成片与封面存在且完整
- **WHEN** 操作者生成发布包
- **THEN** 系统 SHALL 生成绑定当前平台、作品版本、草稿 revision 和平台规则版本的 manifest
- **AND** 系统 SHALL 提供成片、封面、文案和完整包下载
- **AND** MP4 SHALL NOT 被再次转码或有损压缩

#### Scenario: 来源 artifact 缺失或损坏

- **GIVEN** 成片或必要 artifact 不存在或哈希不匹配
- **WHEN** 操作者尝试生成发布包
- **THEN** 系统 SHALL 拒绝把目标标记为 ready
- **AND** 页面 SHALL 展示具体缺失或校验失败项

#### Scenario: 修改文案使旧包失效

- **GIVEN** 某目标已经生成当前 revision 的发布包
- **WHEN** 操作者修改平台文案、封面或引用 artifact
- **THEN** 系统 SHALL 增加草稿 revision 并使旧包不再作为当前包
- **AND** 旧包及其审计记录 SHALL 保留

### Requirement: 打开官方平台只能记录人工交接

系统 SHALL 只允许打开版本化平台 profile 中受信任的官方创作者入口，并把该动作记录为 `handed_off`；系统 SHALL NOT 控制第三方页面或自动判定发布成功。

#### Scenario: 打开抖音或小红书创作者入口

- **GIVEN** 平台目标已 ready 且发布包有效
- **WHEN** 操作者点击“去平台发布”
- **THEN** 系统 SHALL 记录交接事件并返回该平台受信任的官方入口
- **AND** 目标 SHALL 进入 handed_off
- **AND** 页面 SHALL 显示“等待人工发布”而非“已发布”

#### Scenario: 未准备完成时打开平台

- **GIVEN** 平台目标缺少有效发布包或必要文案
- **WHEN** 操作者尝试打开官方入口
- **THEN** 系统 SHALL 阻止交接动作
- **AND** 系统 SHALL NOT 改变目标状态

### Requirement: 发布结果必须由操作者明确确认

系统 SHALL 仅在操作者提交平台官方作品链接和实际发布时间后把目标标记为 `published`，并明确标识该结果为人工确认而非平台同步。

#### Scenario: 登记人工发布成功

- **GIVEN** 某目标处于 handed_off
- **WHEN** 操作者提交匹配目标平台官方域名的 HTTPS 作品链接和实际发布时间
- **THEN** 系统 SHALL 把目标标记为 published
- **AND** 系统 SHALL 保存准备快照、人工确认结果和追加式发布事件
- **AND** 页面 SHALL 标识“人工确认已发布”

#### Scenario: 拒绝非官方作品链接

- **GIVEN** 操作者提交空链接、非 HTTPS 链接或非目标平台官方域名链接
- **WHEN** 系统校验人工发布结果
- **THEN** 系统 SHALL 拒绝标记 published
- **AND** 原目标状态 SHALL 保持不变

#### Scenario: 修正误录结果

- **GIVEN** 已发布目标的作品链接或时间录入有误
- **WHEN** 操作者提交结果修正
- **THEN** 系统 SHALL 更新当前结果投影并追加修正事件
- **AND** 系统 SHALL NOT 删除或覆盖原发布事件

### Requirement: 平台目标状态必须真实且相互隔离

系统 SHALL 按 `draft`、`ready`、`handed_off`、`needs_attention`、`published`、`cancelled` 管理平台目标，并从目标状态确定性派生计划整体状态。

#### Scenario: 一个平台发布成功而另一个待处理

- **GIVEN** 抖音目标已人工确认发布且小红书目标仍需处理
- **WHEN** 操作者查看发布计划
- **THEN** 系统 SHALL 保留两个目标各自状态
- **AND** 计划 SHALL 展示部分完成而非全部成功

#### Scenario: 交接失败后重新准备

- **GIVEN** 操作者在官方平台发现文件或文案需要调整
- **WHEN** 操作者把目标标记为 needs_attention 并完成修正
- **THEN** 系统 SHALL 要求生成新 revision 发布包后才能再次 ready
- **AND** 既有交接与发布包事件 SHALL 保留

#### Scenario: 取消未完成目标

- **GIVEN** 某目标尚未 published
- **WHEN** 操作者确认取消
- **THEN** 系统 SHALL 把该目标标记为 cancelled
- **AND** 系统 SHALL 保留计划、发布包和事件审计

### Requirement: 人工发布运营必须禁止凭据和非官方自动化

系统 SHALL NOT 保存、返回或打包平台 Cookie、Token、Secret、Authorization、签名 URL 查询参数或内部存储绝对路径，也 SHALL NOT 使用浏览器自动化和未公开接口控制第三方发布页面。

#### Scenario: 敏感字段进入写请求

- **GIVEN** 发布草稿、结果或事件 payload 包含敏感键名或凭据值
- **WHEN** 后端处理写请求
- **THEN** 系统 SHALL 拒绝持久化该 payload
- **AND** 日志 SHALL NOT 输出敏感值

#### Scenario: 下载完整发布包

- **GIVEN** 某目标存在有效发布包
- **WHEN** 操作者下载包并检查 manifest 和文本文件
- **THEN** 包内 SHALL NOT 包含凭据、签名查询参数或服务器内部绝对路径
- **AND** 包内文件 SHALL 只引用操作者可使用的作品产物和发布内容

### Requirement: 发布工作台必须支持待发布和发布记录管理

系统 SHALL 提供高密度发布工作台，以待发布和发布记录视图展示作品版本、平台、计划时间、真实状态、最近动作和人工结果，并提供加载、空、错误和写入失败状态。

#### Scenario: 查看待发布列表

- **GIVEN** 当前项目存在 draft、ready、handed_off 或 needs_attention 目标
- **WHEN** 操作者打开发布工作台的待发布视图
- **THEN** 页面 SHALL 按计划时间和状态展示目标
- **AND** 页面 SHALL 支持按平台、状态、时间和关键词筛选

#### Scenario: 查看发布记录

- **GIVEN** 当前项目存在 published 或 cancelled 目标
- **WHEN** 操作者切换到发布记录
- **THEN** 页面 SHALL 展示作品版本、平台、人工结果、发布时间和审计入口
- **AND** 页面 SHALL NOT 把 handed_off 目标归入已发布记录

#### Scenario: 页面读取或写入失败

- **GIVEN** 发布 API 返回错误或网络失败
- **WHEN** 操作者读取列表或执行状态动作
- **THEN** 页面 SHALL 展示具体错误并允许安全重试
- **AND** 页面 SHALL NOT 通过乐观状态把失败动作显示为成功
