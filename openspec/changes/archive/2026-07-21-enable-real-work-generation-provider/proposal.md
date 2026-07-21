## Why

作品生成当前只会通过 fake provider 产出分镜幻灯片，任务虽然显示完成，却不是 Seedance 的真实生成结果。现在已经配置可用的视频模型、TTS 模型和 TOS 暂存工具，需要在严格成本与幂等边界内接通真实 provider，并让完成任务稳定落成可播放、可追溯的作品素材。

## What Changes

- 将作品生成 Worker 从固定 fake provider 改为按运行锁定快照解析真实视频与 TTS 模型。
- 实现火山引擎 Ark Seedance 的创建、查询、取消与结果下载；按模型家族区分 Seedance 1.5 首帧/首尾帧与 Seedance 2.0 多参考图协议，并按官方 `content[]`、图片 role、`ratio`、`resolution`、`duration` 协议发送请求。
- 使用系统当前 TOS 工具暂存本地参考图片并生成短期签名 URL，禁止把长期凭据写入运行快照、审计或日志。
- 持久化上游 task ID 和 provider 状态；Worker 恢复时只查询既有任务，不确定提交不得自动重提。
- 将真实 Seedance 视频、TTS 音频和字幕交给 FFmpeg 确定性合成，校验后写入自管存储并登记最终作品素材。
- 允许受控真实验证在派生作品版本中锁定精简旁白覆盖和兼容音色，不修改来源脚本旁白。
- 增加正式 `silent` 声音模式，用于无需 TTS/ASR/字幕的真实视频派生运行，并在最终 MP4 中补静音 AAC。
- 增加真实调用成本闸门：单 Worker 并发 1、单次运行最多一个 15 秒视频任务、TTS 最多 398 字符、自动提交重试 0、本次不调用 ASR。
- 保留显式 fake 模式用于自动化测试和本地无费用验证；真实模式必须显式启用且通过配置预检。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `work-generation`: 作品运行从 fake 执行扩展为可恢复的真实 Seedance/TTS/FFmpeg 流程，并增加外部调用成本控制、参考图暂存和真实成品登记要求。

## Impact

- Worker：`services/video-worker/src/video_worker/` 的作品调度、Seedance、TOS、TTS、FFmpeg 和素材登记流程。
- 数据库：作品步骤/尝试的 provider 请求快照、上游状态和结果快照持久化；如现有字段不足则新增 migration。
- 配置：`WORK_GENERATION_*` 环境变量与 Compose Worker 配置。
- API/领域：作品计划可选 `narration_override`；声音模式增加 `silent`，进入不可变 WorkVersion/Run 快照并驱动 DAG 和资源用量。
- 外部系统：火山引擎 Ark Seedance、火山 TTS、TOS；会产生一次受控真实调用费用。
- 测试：provider HTTP mock、TOS mock、Worker 恢复/幂等/成本边界、媒体下载校验、PostgreSQL 集成及一次用户授权的真实验证。
