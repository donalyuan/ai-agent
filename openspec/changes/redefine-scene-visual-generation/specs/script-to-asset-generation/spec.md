# script-to-asset-generation Delta Specification

## MODIFIED Requirements

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

## ADDED Requirements

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

## REMOVED Requirements

### Requirement: AI 视频生成必须二次确认

**Reason**: 视频生成已从逐分镜素材候选域迁移到作品级一次确认和统一编排；继续保留该行为会产生两套视频任务入口。

**Migration**: 历史 `draft` 和视频生成记录只读保留审计；所有新 Seedance 请求只能从 `作品生产 / 作品生成` 创建。
