# Asset Generation Runtime Visibility Implementation Plan

> **For agentic workers:** Execute this plan inline with TDD. Do not start or recreate the image Worker until the user separately confirms the pending generation cost.

**Goal:** 让图片生成任务创建后立即可见、在途期间自动刷新、完成后正确加载候选图，并让 Worker 配置失败安全进入终态。

**Architecture:** API 继续作为任务事实来源；前端只在当前脚本存在在途图片任务时轮询任务与候选。Worker 默认关闭，通过显式环境变量开启；领取任务后的供应商配置或永久错误统一写回失败任务与失败候选。相对素材 URL 在 API client 边界解析为 API 绝对地址。

**Tech Stack:** Next.js 14、React、Vitest、Rust/Axum API、Python/FastAPI Worker、PostgreSQL、Docker Compose。

---

### Task 1: 图片任务展示与轮询 RED/GREEN

**Files:**
- Modify: `apps/video-agent/app/page.test.tsx`
- Modify: `apps/video-agent/app/page.tsx`
- Modify: `apps/video-agent/app/pages/script-creation/AssetCandidatePanel.tsx`
- Modify: `apps/video-agent/app/pages/script-creation/assetModel.ts`
- Modify: `apps/video-agent/app/styles.css`

- [x] 新增页面测试，明确断言“AI 图片生成任务”“整批图片候选”“18 张”“排队中”，避免用视频任务状态误判。
- [x] 运行定向测试并确认因图片任务被过滤而失败。
- [x] 增加图片任务条目，展示批量/单镜头范围、候选数、状态、结果计数和错误。
- [x] 新增轮询测试：初始 `pending`，触发轮询后返回 `completed` 和新候选，页面更新并清除定时器。
- [x] 仅在当前素材生成页存在 `pending/processing + image_candidates` 时轮询任务和候选。
- [x] 运行页面定向测试并确认通过。

### Task 2: API 素材 URL 解析 RED/GREEN

**Files:**
- Modify: `apps/video-agent/app/lib/api.test.ts`
- Modify: `apps/video-agent/app/lib/api.ts`

- [x] 新增 API 测试：`/assets/...` 候选、素材和缩略图解析为 `${client.baseUrl}/assets/...`，外部绝对 URL 保持不变。
- [x] 运行 API 定向测试并确认失败。
- [x] 在 API wrapper 响应边界统一规范化 `Material` 和 `SceneAssetCandidate` URL。
- [x] 运行 API 定向测试并确认通过。

### Task 3: Worker 领取后异常终态 RED/GREEN

**Files:**
- Modify: `services/video-worker/tests/test_asset_generation.py`
- Modify: `services/video-worker/src/video_worker/asset_generation.py`

- [x] 新增供应商配置失败测试，断言任务、预期失败候选数和错误信息全部写回。
- [x] 新增永久供应商错误测试，断言不重试且任务进入 `failed`。
- [x] 运行 Worker 定向测试并确认任务当前会抛错。
- [x] 在领取任务后统一捕获配置/永久错误，写入失败候选与任务终态；临时错误仍最多重试一次。
- [x] 新增并测试 `OPENAI_IMAGE_BASE_URL`，缺省时安全移除文本端点 `/responses` 后缀。
- [x] 运行 Worker 定向测试并确认通过。

### Task 4: Worker 费用门禁和运行配置

**Files:**
- Modify: `services/video-worker/tests/test_health.py`
- Modify: `services/video-worker/src/video_worker/main.py`
- Modify: `docker-compose.yml`
- Modify: `.env.example`

- [x] 新增健康测试，断言响应包含 `asset_generation_worker: enabled|disabled`。
- [x] Worker 健康响应暴露实际后台状态。
- [x] Worker Compose 加载项目 `.env`，并将 `ASSET_GENERATION_WORKER_ENABLED` 改为默认 `false` 的显式环境开关。
- [x] API 与 Worker 保持同一 `ai-agent-generated-assets` 卷；补充图片供应商环境模板。
- [x] 只运行 `docker compose config` 验证，不执行 `up/restart/process-next`。

### Task 5: 收尾验证

**Files:**
- Modify: `openspec/changes/script-to-asset-generation/tasks.md`

- [x] 运行前端全量测试、lint 和 build。
- [x] 运行 Worker 全量测试。
- [x] 运行 `openspec validate script-to-asset-generation --strict` 和 apply instructions。
- [x] 运行 `git diff --check`。
- [x] 查询运行库，确认当前 18 张任务仍为 `pending`，没有新增素材或供应商调用。

本计划不执行 `git add`、`git commit`、`git push`，也不启动任何可计费图片生成。

### Task 6: 403 根因修复与受控重试

**Files:**
- Modify: `services/video-worker/tests/test_asset_generation.py`
- Modify: `services/video-worker/src/video_worker/asset_generation.py`
- Modify: `openspec/changes/script-to-asset-generation/tasks.md`

- [x] 新增 `/responses -> /v1` 和兼容 `User-Agent` 失败测试。
- [x] 新增两分镜永久错误仅调用供应商一次、其余候选直接失败的测试。
- [x] 捕获 HTTP 状态与响应摘要，区分临时错误和永久错误。
- [x] 重建 Worker 镜像并保持后台关闭，先运行全量 Worker 测试。
- [x] 删除任务 `c16191cc-...` 的 18 个失败占位候选，将同一任务重置为一次人工配置修复重试。
- [x] 仅以显式 `ASSET_GENERATION_WORKER_ENABLED=true` 启动并监控；首个请求返回 `HTTP 403 Forbidden: Image generation is not enabled for this group` 后熔断，Worker 已恢复关闭，未再次重试。
