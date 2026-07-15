# 作品生成 Proposal

## Why

当前流程在主图片确认后中断，无法把全部镜头一次提交为完整视频作品。需要新增作品级 Agent 规划与生成编排，在用户一次确认的前提下，按 Seedance 真实限制自动拆分视频任务，并与 TTS、已有音频、字幕和 FFmpeg 合成为成片。

## What Changes

- 新增 `作品生产 / 作品生成`，汇总全片主图片、镜头描述和旁白。
- 提供可见作品 Agent 对话，展示并允许修改全片及分段提示词；确认前不调用 Seedance。
- 方案 LLM、视频和 TTS 模型独立选择，Agent 只推荐，不自动切换。
- 支持 `15/30/45/60秒`、`4~60秒` 自定义和“跟随配音”，比例与分辨率来自视频模型真实能力。
- 用户侧一次提交；后台按 Seedance 单任务 `4~15秒`、最多 9 张参考图和提示词约束自动拆分。
- 支持独立 TTS、Seedance 原声、Seedance 原声 + TTS 三种声音模式，以及已有 BGM/环境音/动作音效混音。
- 使用 FFmpeg 输出 `MP4(H.264) + AAC`；默认烧录字幕并另存 `SRT`，可关闭烧录。
- 生成前展示模型、任务数、视频秒数、TTS 字符数和 ASR 时长，不计算金额。

## Capabilities

### New Capabilities

- `work-generation`: 定义作品 Agent、作品级一次提交、模型与输出选择、Seedance 分段、声音模式、字幕和 FFmpeg 成片。

### Modified Capabilities

无。

## Impact

- 领域：未来新增作品草稿、版本快照、规划版本、时间轴和生成运行。
- Agent：未来新增 `work` adapter，复用统一 Agent Runtime。
- 后端/Worker：未来接入 Seedance provider、条件工作流和 FFmpeg。
- 前端：未来新增作品级对话、参数、计划和确认工作区。
- 依赖：主画面输入来自 `redefine-scene-visual-generation`；TTS/字幕可复用 `add-sound-subtitle-generation`；产物通过 `extend-material-library-for-work-production` 入库。

## Non-Goals

- 不在本 change 定义任务列表/重试页面或作品库管理，它们分别属于独立 change。
- 不生成 AI 音乐、环境音或动作音效，只复用已有音频。
- 不自动发布作品。
- 不维护任何金额费用能力。
- 本轮不执行代码、外部调用、migration、原型或测试。
