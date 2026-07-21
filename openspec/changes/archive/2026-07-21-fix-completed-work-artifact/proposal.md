## Why

作品生成任务在所有步骤成功后会显示“已完成”，但 Worker 只记录 fake provider 的上游任务 ID，没有登记最终成片素材，操作者因此无法查看或播放成品。需要把合成产物写入共享素材库，并让完成态与可查看产物保持一致。

## What Changes

- Worker 在受控 fake provider 的最终合成步骤完成后生成可播放 MP4，并以作品、版本、运行和步骤快照登记到素材库。
- 成品素材 ID 和输出摘要回写到合成步骤；产物登记失败时运行不得继续保持成功态。
- 生成任务详情读取成品素材并展示内嵌播放入口，可跳转素材库查看。
- 为历史已完成但缺失成品的运行补登记一次受控 fake 成片。

## Capabilities

### New Capabilities

- `work-generation-artifact`: 约束作品生成完成态、成品登记和任务详情查看闭环。

### Modified Capabilities

无。

## Impact

- `services/video-worker` 的作品生成 Worker、共享资产存储和 PostgreSQL 素材登记。
- `apps/video-agent` 生成任务详情页面及任务页到素材库的导航。
- 不调用真实 Seedance、TTS 或 ASR，不新增外部付费调用。
