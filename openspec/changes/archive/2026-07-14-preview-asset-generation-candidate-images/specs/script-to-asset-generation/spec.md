## ADDED Requirements

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
