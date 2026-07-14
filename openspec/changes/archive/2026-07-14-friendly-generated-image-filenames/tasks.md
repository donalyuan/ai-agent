## 1. 规格确认

- [x] 1.1 核对现有 Worker 命名、任务领取、batch/`per_candidate` 编排、素材字段和静态资源路由，补齐 DDD、BDD、SDD、TDD 设计与增量规格。
- [x] 1.2 执行 OpenSpec strict validate，并取得用户对文件格式、NFC 清理、255 UTF-8 字节限制、领取时快照、候选槽位和历史兼容边界的明确确认。

## 2. TDD 锁定命名与编号

- [x] 2.1 先写图片命名纯函数测试，覆盖中文、NFC、路径分隔符、Windows 非法字符、控制字符、首尾点/空格、空标题和 255 UTF-8 字节安全截断，运行并确认 RED。
- [x] 2.2 先写 PNG/JPEG/WebP、多镜头多候选和 UUID 任务目录测试，断言物理 basename、`file_url` 与 `materials.file_name` 一致，运行并确认 RED。
- [x] 2.3 先写 batch 与 `per_candidate` 候选槽位测试，覆盖中间结果无效、落盘失败、临时重试和永久停止后的成功候选不重排，运行并确认 RED。
- [x] 2.4 先写任务领取与 metadata 测试，断言标题领取时快照以及成功素材、成功候选、失败候选的 `script_title_snapshot`、`scene_sequence`、`candidate_index`，运行并确认 RED。
- [x] 2.5 先写 API 静态资源测试，使用百分号编码的中文文件名请求 `/assets/...` 并断言返回物理文件，运行并确认当前路由行为。

## 3. Worker 实施

- [x] 3.1 实现图片命名专用的 NFC 清理、非法字符删除、空标题回退和完整 basename UTF-8 字节截断，不放宽通用 `safe_filename()`。
- [x] 3.2 在任务领取事务中读取 `scripts.title`，并通过 `PendingImageGenerationTask.script_title_snapshot` 贯穿本次任务。
- [x] 3.3 让内部成功与失败结果显式携带 1-based 候选槽位，修正 batch、`per_candidate`、rank 和部分失败路径，不再从成功列表位置反推编号。
- [x] 3.4 在落盘前按 magic bytes 识别 PNG/JPEG/WebP，使用根生成任务 UUID 目录和友好 basename 保存文件。
- [x] 3.5 同步写入 `materials.file_name`、`file_url`、素材 metadata、成功候选 metadata 和失败候选 metadata，保持现有来源与任务审计字段不变。
- [x] 3.6 运行 Worker 聚焦测试，确认全部 RED 用例转为 GREEN，且供应商调用次数、重试与停止规则无回归。
- [x] 3.7 同步项目 memory 中的图片命名、标题快照、候选槽位、metadata 和历史兼容约定。

## 4. 综合验证与部署

- [x] 4.1 在容器内运行 Worker 全量测试和 API 静态资源聚焦测试；全部测试使用 fake provider，不调用 OpenAI 或 Ark。
- [x] 4.2 运行 `openspec validate friendly-generated-image-filenames --strict --no-interactive`、`openspec instructions apply --change friendly-generated-image-filenames --json` 和 `git diff --check`，同步勾选实际完成任务。
- [x] 4.3 部署前检查 `pending/processing` 图片任务并避免重建中断在途任务；部署 Worker 后确认健康状态，不为命名验证额外创建计费任务。
- [x] 4.4 观察下一条用户自然创建的图片任务，核对物理文件名、静态访问、`materials.file_name` 和 metadata；不得为本项验证额外增加供应商调用。
