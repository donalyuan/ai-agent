# work-library-version-governance Specification

## Purpose
定义作品版本、草稿复用、历史治理、详情展示和作品 Agent 修改的正式业务规则，确保版本可追溯、操作幂等且桌面工作区稳定可用。
## Requirements
### Requirement: 同一生产意图必须复用未确认草稿

系统 SHALL 将作品版本与计划修订分离；同一生产意图已有未确认且未运行的草稿时，后续规划 SHALL 更新该草稿并创建新的计划修订，不得增加作品版本号。

#### Scenario: 初始规划反复调整

- **GIVEN** 作品当前只有一个未确认初始草稿
- **WHEN** 操作者要求 Agent 重新规划或调整参数
- **THEN** 系统 SHALL 保持相同 `work_version_id` 和 `version_no`
- **AND** 系统 SHALL 创建新的 `work_plans.plan_version` 并失效旧计划

#### Scenario: 不可变版本后开始新规划

- **GIVEN** 作品当前版本已经确认或运行
- **WHEN** 操作者开始新的整体规划
- **THEN** 系统 SHALL 从当前不可变版本创建一个 `full_regeneration` 草稿
- **AND** 后续重复规划 SHALL 复用该草稿

#### Scenario: 并发保存同一规划

- **GIVEN** 同一作品尚无当前可编辑草稿
- **WHEN** 两个请求并发保存同一生产意图的计划
- **THEN** 系统 SHALL 串行化版本创建
- **AND** 最终 SHALL 只存在一个对应生产意图的草稿版本

### Requirement: 派生版本必须按来源和派生类型复用

系统 SHALL 使用作品、来源版本和派生类型识别编辑或整体重生成意图；相同识别键存在草稿时 SHALL 返回并更新已有草稿。

#### Scenario: 重复继续修改

- **GIVEN** 完成版本 V5 已存在一个来源为 V5 的 `edit` 草稿 V11
- **WHEN** 操作者再次从 V5 选择继续修改
- **THEN** 系统 SHALL 返回 V11
- **AND** 系统 SHALL NOT 创建 V12

#### Scenario: 重复整体重生成

- **GIVEN** 完成版本 V5 已存在一个来源为 V5 的 `full_regeneration` 草稿
- **WHEN** 操作者再次从 V5 选择整体重生成
- **THEN** 系统 SHALL 返回已有整体重生成草稿
- **AND** 原完成版本及其产物 SHALL 保持不变

#### Scenario: 不同生产意图创建新草稿

- **GIVEN** 当前只有来源为 V5 的 `edit` 草稿
- **WHEN** 操作者明确从 V5 发起整体重生成
- **THEN** 系统 SHALL 创建派生类型为 `full_regeneration` 的新草稿
- **AND** 两个草稿 SHALL 保留各自明确的派生类型

### Requirement: 历史冗余草稿必须按完整安全谓词治理

系统 SHALL 仅清理非当前、未运行且不存在任何下游或审计引用的失效草稿；任何已运行或被引用版本 MUST 保留。

#### Scenario: 清理纯失效草稿

- **GIVEN** 旧草稿不是当前版本，关联计划全部失效，且没有 run、attempt、artifact、timeline、diff、发布或来源引用
- **WHEN** 历史治理 migration 执行
- **THEN** 系统 SHALL 删除该草稿关联的失效计划和版本
- **AND** 系统 SHALL 保留同一作品的当前草稿

#### Scenario: 保留失败运行版本

- **GIVEN** 某版本存在失败 generation run 或 provider attempt
- **WHEN** 历史治理 migration 执行
- **THEN** 系统 SHALL 保留该版本及完整运行审计
- **AND** 系统 SHALL NOT 将其改回草稿

#### Scenario: 保留任何被引用草稿

- **GIVEN** 非当前草稿被 artifact、timeline、diff、发布交接或其他版本引用
- **WHEN** 历史治理 migration 执行
- **THEN** 系统 SHALL 原样保留该草稿
- **AND** migration SHALL NOT 使用级联删除绕过引用约束

### Requirement: 作品详情必须业务摘要优先并渐进披露技术数据

系统 SHALL 在作品详情首屏展示可读的制作方案、当前产物和调用摘要；模型 ID、trace、原始参数与其他技术快照 SHALL 默认折叠并可按需展开。

#### Scenario: 查看当前编辑草稿

- **GIVEN** 当前草稿来源于某个完成版本且尚未运行
- **WHEN** 操作者打开作品详情
- **THEN** 页面 SHALL 展示来源版本、制作方案、本次修改和复用素材摘要
- **AND** 页面首屏 SHALL NOT 递归铺陈 UUID、模型 ID、trace 或原始 JSON 键值

#### Scenario: 展开技术快照

- **GIVEN** 当前版本包含模型、提示词、参数和调用审计
- **WHEN** 操作者展开技术快照
- **THEN** 页面 SHALL 展示该选定版本的完整技术数据
- **AND** 技术数据 SHALL NOT 替换或污染业务摘要

### Requirement: 空时间轴必须使用紧凑状态

系统 SHALL 只在选定版本存在轨道内容时展示完整多轨时间轴；没有轨道内容时 SHALL 使用紧凑空状态。

#### Scenario: 草稿没有运行产物

- **GIVEN** 选定草稿没有视频、音频或字幕轨道
- **WHEN** 页面渲染时间轴区域
- **THEN** 页面 SHALL 展示“暂无运行产物”和来源版本入口
- **AND** 页面 SHALL NOT 渲染空标尺和空轨道占位

#### Scenario: 完成版本存在轨道

- **GIVEN** 选定完成版本存在视频、音频或字幕轨道
- **WHEN** 页面渲染时间轴区域
- **THEN** 页面 SHALL 展示该版本的完整多轨时间轴
- **AND** 各轨道 SHALL 使用该版本绑定的产物引用

### Requirement: 版本记录必须按业务用途分组并默认折叠历史

系统 SHALL 把版本记录分为当前草稿、可用成片、失败与早期记录，并在分组标题展示准确数量；失败与早期记录 SHALL 默认折叠。

#### Scenario: 作品同时存在草稿完成和失败版本

- **GIVEN** 作品存在一个当前草稿、一个完成版本、多个失败版本和早期未运行草稿
- **WHEN** 操作者打开详情
- **THEN** 页面 SHALL 直接展示当前草稿和可用成片
- **AND** 页面 SHALL 把失败版本和早期草稿放入带准确数量的折叠历史组

#### Scenario: 展开失败与早期记录

- **GIVEN** 历史组处于折叠状态
- **WHEN** 操作者展开历史组并选择失败版本
- **THEN** 页面 SHALL 展示所选失败版本的失败阶段、错误、保留产物和任务入口
- **AND** 当前草稿与可用成片分组 SHALL 保持可见

### Requirement: 作品详情必须固定主体高度并限定历史滚动

系统 SHALL 在桌面详情工作区保持详情主体和版本栏高度固定；主业务区 SHALL NOT 形成独立纵向滚动，只有展开后的失败与早期记录 SHALL 在版本栏剩余空间内滚动。

#### Scenario: 详情内容超过可视高度

- **GIVEN** 作品包含超过版本栏可用高度的失败与早期记录
- **WHEN** 操作者展开历史组
- **THEN** 详情 Surface 和版本栏 SHALL 保持展开前的高度
- **AND** 只有历史记录内容 SHALL 在限定区域内纵向滚动
- **AND** 当前草稿、可用成片、分组标题、版本操作区和 Agent 区 SHALL 保持可见且不得被覆盖

### Requirement: 当前草稿必须通过作品 Agent 对话修改

系统 SHALL 以绑定作品和项目的 Agent 对话作为当前草稿的唯一自然语言修改入口；服务端 SHALL 将合法结构化补丁写入同一编辑草稿并生成等待人工确认的差异。

#### Scenario: 通过自然语言持续修改当前草稿

- **GIVEN** V11 是来源于完成版本 V5 的当前 `edit` 草稿
- **WHEN** 操作者发送“保留配音，让画面节奏更紧凑”
- **THEN** 系统 SHALL 更新 V11 且 SHALL NOT 创建 V12
- **AND** Assistant 消息 SHALL 返回草稿版本标识、版本号、完整 diff 和 `requires_confirmation=true`
- **AND** 页面 SHALL 提供查看影响并确认的入口

#### Scenario: Agent 输出不满足补丁契约

- **GIVEN** 文本模型返回非法 JSON、未知字段或空补丁
- **WHEN** Runtime 校验模型输出
- **THEN** 系统 SHALL NOT 修改作品草稿或创建新版本
- **AND** 页面 SHALL 保留用户消息并允许人工重试

#### Scenario: 会话作品不属于绑定项目

- **GIVEN** 会话绑定项目与目标作品的 `project_id` 不一致
- **WHEN** 操作者创建会话或发送作品修改消息
- **THEN** 服务端 SHALL 拒绝请求
- **AND** 系统 SHALL NOT 调用文本模型或写入作品版本

#### Scenario: Agent 修改不自动触发下游生成

- **GIVEN** 作品 Agent 已生成待确认差异
- **WHEN** 本轮对话完成
- **THEN** 系统 SHALL 每轮最多执行一次文本模型调用且不自动重试
- **AND** 系统 SHALL NOT 自动调用视频、TTS、ASR 或发布能力

### Requirement: 作品详情业务工作区必须严格映射 v3 原型

系统 SHALL 在现有全站统一顶栏和左侧菜单骨架内，严格映射“桌面宽屏 - 作品详情 Agent 对话 v3 半版待确认”的业务工作区结构与视觉层级。

#### Scenario: 宽屏打开作品详情

- **GIVEN** 桌面视口为 `1920x980`
- **WHEN** 操作者打开作品详情
- **THEN** 页面 SHALL 展示页面说明区、52px 主操作栏、主业务区和 400px 版本栏
- **AND** 当前草稿区域 SHALL 展示 Agent 会话，不得展示原始提示词表单、“保存草稿修改”或“分析版本差异”按钮

#### Scenario: 历史版本超过可视高度

- **GIVEN** 失败与早期记录超过版本栏剩余空间
- **WHEN** 操作者展开历史组并滚动历史记录
- **THEN** 详情卡片 SHALL NOT 随历史数量增长或越界
- **AND** 所有文字、控件和可见版本卡片 SHALL 无重叠、裁切或横向溢出
