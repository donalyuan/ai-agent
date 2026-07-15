# 声音与字幕生成 Proposal

## Why

首版作品需要由 Agent 生成 TTS 配音和可对齐字幕，但当前没有独立的声音生产入口，也没有可更新的音色目录。需要建设可单独使用、又能被作品编排复用的声音与字幕生成能力。

## What Changes

- 新增 `素材管理 / 声音与字幕生成`，首版只提供 `TTS配音 / 字幕` 两个标签和可见声音 Agent 对话。
- 首版 TTS 使用 `doubao-seed-tts-2.0`、资源 `seed-tts-2.0` 和 HTTP Chunked V3 单向流式接口。
- 通过 `ListSpeakers 2025-05-20` 按 `ResourceID` 分页同步动态音色目录，禁止前端或代码写死。
- 展示模型真实支持的音色、语言/口音、情绪风格和可调参数；Agent 可推荐，用户必须确认。
- TTS 启用 `enable_subtitle`，字幕 Agent 负责断句和样式，供应商时间戳负责对齐。
- 已有音频可通过 `doubao-seed-asr-2.0` 生成字幕；不支持时间戳时不得伪造。
- 试听、生成、重试前展示字符数/音频时长和任务数量，不计算金额。
- TTS 和字幕结果自动进入素材库，重新生成不覆盖旧素材。

## Capabilities

### New Capabilities

- `sound-subtitle-generation`: 定义声音 Agent、动态音色目录、TTS、试听、字幕对齐、ASR 和素材入库。

### Modified Capabilities

无。

## Impact

- 数据：未来新增音色目录同步、声音任务、字幕时间轴和模型/音色快照。
- Admin：未来提供音色主动/定期同步和状态可观测。
- 后端/Worker：未来接入 TTS V3、ASR、流式落盘和字幕生成。
- 前端：未来新增双标签声音工作区和动态声音参数选择。
- 依赖：生成结果通过 `extend-material-library-for-work-production` 的素材契约入库。

## Non-Goals

- 不生成 AI 音乐/BGM、环境音或动作音效，也不展示空入口。
- 不负责 Seedance、作品时间轴混音或最终成片合成。
- 不由 LLM 猜测音色或模型支持的声音风格。
- 不维护价格、币种、金额或费用上限。
- 本轮只写 OpenSpec，不执行代码、外部调用、原型或测试任务。
