## 1. 运行配置与数据契约

- [x] 1.1 先补真实/fake 开关互斥、allowlist、并发、任务数、时长、TTS 字符和 ASR 禁用的失败测试，再实现成本预检。
- [x] 1.2 扩展作品步骤运行上下文与持久化契约，读取锁定模型、输入、提示词、参数、时间轴和项目元数据。
- [x] 1.3 如现有 JSONB/attempt 字段不足，新增 migration 持久化脱敏 provider 请求、上游状态和中间素材结果，并补数据库契约测试。

## 2. TOS 参考图暂存

- [x] 2.1 先补路径越界、文件缺失、类型/大小限制、确定性对象键和签名读取失败测试。
- [x] 2.2 扩展 TOS staging 支持作品参考图上传、复用、短期签名和脱敏审计快照。
- [x] 2.3 实现从作品输入快照解析 1~9 张参考图并在 Seedance 提交前完成全量预检。

## 3. Seedance 真实 Provider

- [x] 3.1 先补创建请求、`ratio` 字段、查询状态、取消、错误映射、响应脱敏和 POST 零重试测试。
- [x] 3.2 实现 Ark Seedance HTTPS create/get/cancel provider，并严格校验 endpoint、模型和响应结构。
- [x] 3.3 实现 task ID 查询前持久化、Worker 恢复只轮询原任务及不确定提交进入 `waiting_manual`。
- [x] 3.4 先补下载大小、非视频、时长异常和幂等登记测试，再实现真实视频下载、ffprobe 校验、自管存储和中间素材登记。
- [x] 3.5 补 Seedance 非 2xx 脱敏审计、明确失败暂存清理和模型家族预检测试并实现修复。
- [x] 3.6 按 TDD 实现 Seedance 1.5 `first_frame/last_frame`、`4~12s` 契约及计划器 `first_last_frames` 单任务引用模式。

## 4. TTS 与最终合成

- [x] 4.1 先补作品 TTS 模型解析、398 字符上限、零自动重提和真实音频素材登记测试。
- [x] 4.2 复用现有 Volcengine TTS provider 执行作品 TTS，并保存合成可用的自管音频素材。
- [x] 4.3 先补真实依赖缺失不得 fake 回退测试，再实现 compose 消费真实视频/TTS/字幕并幂等登记最终作品。
- [x] 4.4 验证最终文件为 `MP4(H.264)+AAC`、时长合法、缩略图可生成且素材详情可播放。

## 5. 配置、回归与受控真实验证

- [x] 5.1 更新 `.env.example`、Compose 和 Worker 启动检查，真实模式默认关闭并限制单并发。
- [x] 5.2 在容器内运行 Worker、数据库、后端和素材播放相关回归，执行 OpenSpec strict 校验。
- [x] 5.2.1 实现派生作品版本 `narration_override`，按覆盖文本计算资源、执行 TTS/字幕并保证原脚本不变。
- [x] 5.3 对获批运行执行模型版本、6 张参考图、签名 URL、单个 15 秒任务、398 字符 TTS、零 ASR/零重试预检。
- [x] 5.3.1 实现 `silent` 计划、DAG、Seedance 参数和静音 AAC 合成，并验证不调用 TTS/ASR/字幕。
- [ ] 5.4 经用户单独确认后，使用已配置的 Seedance 1.5 启动唯一一次 `12s/1080p/16:9/首尾帧` 静音运行，持续轮询同一 upstream task ID，下载并登记真实成品。
- [ ] 5.5 验证任务关联成品、缩略图和素材详情播放，关闭真实 allowlist，并同步完成 `add-work-generation` 真实验证任务。

## 真实验证记录

- 真实 provider、成本闸门和媒体链路已完成；Worker `189` 个测试、前端 `75` 个测试、TypeScript、Rust 编译/作品任务路由和 OpenSpec strict 校验均通过。
- 6 张参考图已完成 TOS 上传、签名 HTTPS 回读、SHA-256 核验并清理，`6/6` 成功。
- 用户已确认派生版本使用 64 字精简中文旁白与 `云舟 2.0 / zh_male_m191_uranus_bigtts`；新运行 `d2eea1d4-f418-45e2-8e05-7ab02dc11eaa` 已锁定视频模型版本 `1`、TTS 模型版本 `2`、TOS 配置版本 `2`。
- 新运行预检为 `1×15s / TTS 64 字 / ASR 0 / 自动提交重试 0 / 并发 1`；6 张参考图签名回读 `6/6` 成功并已清理预检对象。
- 首次真实 TTS attempt `d08ae2e5-00ff-4354-98bb-56297adba6f3` 返回 `SpeechProviderError`，发生在错误详情审计完善前，当前无法证明是明确拒绝还是不确定响应；系统已按规则进入 `waiting_manual`，Seedance attempt 仍为 `0`。
- 已补齐后续 TTS `error_code/http_status/provider_error_code/provider_error_message/upstream_log_id` 脱敏审计并通过专项测试；在用户单独授权人工 TTS 重试前，真实 Worker 已关闭且 allowlist 已清空。
- 用户已确认不重试 TTS，改为创建独立 `silent` 派生运行并执行唯一一次 Seedance 首次提交。
- `silent` 运行 `3cc84910-74ed-4549-b64e-599256f75f42` 已锁定视频模型版本 `1` 与 TOS 配置版本 `2`；DAG 仅视频、mix、compose 为必需，预检为 `1×15s / TTS 0 / ASR 0 / 重试 0 / 并发 1`，6 图签名回读 `6/6` 成功并已清理。
- 该静音运行首次 Seedance POST 明确返回 HTTP `404` 且没有 upstream task ID；请求审计确认 `doubao-seedance-1-5-pro-251215 / 6 图 role=reference_image / 15s / 16:9 / 1080p / generate_audio=false`。官方协议与账号只读模型目录证明 1.5 为 `Retiring` 且采用单首帧或 `first_frame/last_frame`，1~9 张 `reference_image` 属于 2.0；旧运行保持失败，6 个暂存对象已清理，禁止原运行自动重提。
- 用户已确认改用 Seedance 2.0 创建独立静音派生运行并只提交一次；边界继续为 `1×15s / 6 图 / 16:9 / 1080p / TTS 0 / ASR 0 / 并发 1 / 自动重试 0`。
- Seedance 2.0 模型配置 `4bffe1d4-65d3-42bb-b1fc-fd506ac7b105`、静音计划 `810566e0-a8d9-4340-95c0-04f93dccaa92` 和运行 `153e171a-2a6d-42a3-8ebe-92df2bf5454e` 已通过预检。唯一 POST 明确返回 HTTP `404 / ModelNotOpen`：当前账号尚未开通 `doubao-seedance-2-0-260128`，无 upstream task ID；脱敏 provider message/request ID 已写入 attempt `fd593936-dd2f-4fa6-ae52-d6f83c800da3`，6 个 TOS 对象自动清理成功。真实开关、Worker 和 allowlist 已关闭，等待账号开通后另行授权新派生运行，禁止复用失败运行重提。
- 用户已纠正模型边界并明确继续使用其配置的 `doubao-seedance-1-5-pro-251215`。官方文档核验：1.5 Pro 支持首帧或首尾帧，时长为整数 `4~12s`，支持 `480p/720p/1080p`；本次保持 `1080p/16:9`，将第 1/6 张映射为 `first_frame/last_frame`，中间分镜只参与提示词，不再作为 provider 图片输入。未经新的真实调用确认不得 POST。
- 1.5 模型配置已通过管理 API 修正并升级到版本 `2`：`reference_image_mode=first_last_frames / max_reference_images=2 / duration=4~12s / 480p,720p,1080p`。最终计划 `fcb79c47-f640-4329-8e51-7e4617eff181`、WorkVersion `076f1660-a001-43f9-a017-c1c8c89af06b`、待执行运行 `9cfd24b9-a77f-4388-8662-e443aae0737f` 已锁定 `1×12s / 1080p / 16:9 / 首尾2图 / TTS 0 / ASR 0`；当前 attempt 为 `0`，Worker、真实开关和 allowlist 均关闭。
- 误建的 Seedance 2.0 配置因已有历史运行引用不能物理删除，已通过正式状态接口停用，不再进入模型选项；历史失败运行继续保留审计。
- 经用户一次性确认，运行 `9cfd24b9-a77f-4388-8662-e443aae0737f` 已提交唯一一次 Seedance 1.5 POST；上游明确返回 HTTP `404 / InvalidEndpointOrModel.NotFound`，表示模型或 endpoint 不存在或当前账号无访问权限，未返回 upstream task ID。attempt `c73bb527-57dc-40de-bcfb-4eeed96d97f6` 已保存脱敏 provider code/message/request ID，请求审计确认 `12s / 1080p / 16:9 / first_frame+last_frame / generate_audio=false`，2 个 TOS 对象自动清理成功；Worker、真实开关和 allowlist 已关闭，禁止自动重提。
