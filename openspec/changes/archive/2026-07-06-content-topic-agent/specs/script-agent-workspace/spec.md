# script-agent-workspace Specification Delta

## ADDED Requirements

### Requirement: 脚本创作必须展示来源选题

当脚本由已确认选题生成时，`apps/video-agent` 脚本创作页面 SHALL 展示脚本来源选题，并使用脚本保存的选题快照解释历史上下文。

#### Scenario: 查看由选题生成的脚本

- **GIVEN** 已存在一个由 `topic_id` 生成的脚本
- **WHEN** 操作者打开脚本详情
- **THEN** 页面 SHALL 展示来源选题标题和内容类型
- **AND** 页面 SHALL 基于 `topic_snapshot` 展示生成时的选题摘要
- **AND** 页面 SHALL NOT 因选题后续编辑而改变该脚本的历史快照展示

### Requirement: 内容策略页进入脚本创作前必须确认生成参数

系统 SHALL 在内容策略页从选题进入脚本创作前展示确认面板，使操作者确认 `style` 和 `scene_count` 后再生成脚本。

#### Scenario: approved 选题打开脚本确认面板

- **GIVEN** 操作者在内容策略页选中一条 `approved` 选题
- **WHEN** 操作者点击“生成脚本”
- **THEN** 页面 SHALL 请求 `POST /api/topics/:topic_id/prepare-script`
- **AND** 页面 SHALL 展示选题快照
- **AND** 页面 SHALL 允许操作者确认 `style` 和 `scene_count`
- **AND** 操作者确认后 SHALL 使用 `topic_id` 创建脚本

#### Scenario: archived 选题不能进入脚本创作

- **GIVEN** 操作者在内容策略页查看一条 `archived` 选题
- **WHEN** 页面展示选题操作
- **THEN** 页面 SHALL NOT 提供“生成脚本”主操作
