## ADDED Requirements

### Requirement: 已完成作品生成必须具有可查看成品

系统 SHALL 仅在必需步骤成功且最终合成步骤已登记至少一个 `final_video` 作品生成素材时，将运行标记为已完成；成品素材 SHALL 保存作品、版本、运行、步骤和产物角色快照。

#### Scenario: fake provider 合成成功并登记成品

- **WHEN** fake provider 的 `compose` 步骤成功
- **THEN** Worker SHALL 优先使用作品版本中已锁定的分镜图片生成可播放预览 MP4
- **AND** 只有在分镜图片不可用时才 SHALL 使用明确的测试画面
- **AND** Worker SHALL 从成片截取并保存视频缩略图
- **AND** Worker SHALL 将该文件登记为当前项目的作品生成视频素材
- **AND** Worker SHALL 将素材 ID 写入 compose 步骤的 `result_material_ids`
- **AND** 运行 SHALL 保持已完成状态

#### Scenario: 成品登记失败

- **WHEN** compose provider 调用成功但文件生成、探测或素材登记失败
- **THEN** compose 步骤 SHALL 进入失败状态
- **AND** 运行 SHALL NOT 显示为已完成
- **AND** 系统 SHALL 保留可诊断的成品登记错误摘要

#### Scenario: Worker 恢复不重复登记

- **GIVEN** compose 步骤已经存在 `final_video` 素材
- **WHEN** Worker 再次恢复该步骤
- **THEN** Worker SHALL 复用现有素材 ID
- **AND** 系统 SHALL NOT 重复创建文件或素材记录

### Requirement: 任务详情必须展示成品查看入口

系统 SHALL 在生成任务详情中展示已登记的最终成品，并提供素材库查看入口。

#### Scenario: 查看已完成任务成品

- **GIVEN** 任务详情包含 compose 步骤的 `result_material_ids`
- **WHEN** 操作者打开任务详情
- **THEN** 页面 SHALL 读取对应视频素材并展示可播放预览
- **AND** 素材库列表和画布 SHALL 使用视频缩略图而非类型占位
- **AND** 素材详情 SHALL 使用原视频文件和浏览器原生控件播放
- **AND** 播放器 SHALL 在播放、暂停和加载失败时提供明确状态反馈
- **AND** 页面 SHALL 提供独立的播放/暂停按钮，不得仅依赖浏览器原生控件命中区域
- **AND** 页面 SHALL 提供“在素材库查看”入口

#### Scenario: 完成态缺少产物

- **GIVEN** 任务状态为已完成但没有可读取的最终素材
- **WHEN** 操作者打开任务详情
- **THEN** 页面 SHALL 明确提示成品尚未登记
- **AND** 页面 SHALL NOT 渲染伪造的播放地址
