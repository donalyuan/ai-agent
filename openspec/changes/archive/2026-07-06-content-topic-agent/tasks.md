# content-topic-agent Tasks

## 1. OpenSpec 与设计边界

- [x] 创建 `openspec/changes/content-topic-agent/proposal.md`。
- [x] 创建 `openspec/changes/content-topic-agent/design.md`，覆盖 DDD、BDD、SDD、TDD。
- [x] 创建选题管理、对话 Runtime 和脚本生成关联的规格增量。
- [x] 运行 `openspec instructions apply --change "content-topic-agent" --json`，确认新 change 可被识别。

## 2. 记忆与产品约定

- [x] 更新 `MEMORY.md`，记录内容策略与选题池第一版已确认边界。
- [x] 更新 `docs/memory/video-agent-workspace-flow.md`，记录 Phase 2 选题池和脚本关联细节。

## 3. 数据库与仓储

- [x] 新增 migration：`content_topics`、`topic_generation_batches`、`scripts.topic_id`。
- [x] 为选题表添加项目、状态、来源、批次、创建时间索引。
- [x] 为 scripts 添加 `topic_id` 查询索引。
- [x] 实现选题 repository contract tests。
- [x] 实现批次 repository contract tests。
- [x] 实现 repository。

## 4. 后端 API

- [x] 实现选题 DTO 和输入校验。
- [x] 实现 `GET /api/projects/:project_id/topics`。
- [x] 实现 `POST /api/projects/:project_id/topics`。
- [x] 实现 `PUT /api/topics/:topic_id`。
- [x] 实现 `PUT /api/topics/:topic_id/status`。
- [x] 实现 `POST /api/topics/:topic_id/prepare-script`。
- [x] 实现 `GET /api/projects/:project_id/topic-generation-batches`。
- [x] 扩展 `POST /api/scripts/generate` 支持可选 `topic_id`。
- [x] 覆盖项目不存在、选题不存在、状态非法、项目归属不匹配等错误测试。
- [x] 覆盖生成批次历史按项目隔离、倒序和实际选题数统计。

## 5. 选题 Agent Runtime

- [x] 新增 `topic` Agent adapter。
- [x] 复用通用 Runtime 保存会话、消息、run 和 step。
- [x] 实现项目上下文读取步骤。
- [x] 实现 LLM 结构化选题生成和解析。
- [x] 实现整批校验和整批失败语义。
- [x] 实现生成批次保存和选题自动入库。
- [x] 覆盖有效输出、空输出、非法 JSON、字段缺失、score 越界和 LLM 失败测试。

## 6. 脚本关联

- [x] 脚本生成时校验 `topic_id` 属于同一项目。
- [x] 只允许 `approved` 选题生成脚本。
- [x] 脚本生成成功后写入 `scripts.topic_id`。
- [x] 脚本生成成功后写入 `scripts.content.topic_snapshot`。
- [x] 脚本生成成功后将选题状态更新为 `scripted`。
- [x] 脚本生成失败时不创建脚本、不更新选题状态。

## 7. 前端原型门禁

- [x] 更新 `docs/prototypes/video-agent/video-agent.pen`，加入内容策略页。
- [x] 原型覆盖策略摘要、选题生成、选题池、选题详情侧栏和脚本确认面板。
- [x] 获得用户明确确认后再进入前端编码。

## 8. 前端实现

- [x] 更新 `apps/video-agent/app/lib/api.ts` 和测试，增加选题 API client。
- [x] 在 `apps/video-agent` 启用内容策略页面。
- [x] 启用后端菜单中的内容策略和选题生成入口。
- [x] 实现策略摘要区。
- [x] 实现选题生成区。
- [x] 实现选题池筛选和详情侧栏。
- [x] 实现完整状态筛选：全部、待评估、已确认、已成稿、已归档。
- [x] 实现历史生成批次列表，并支持默认最新批次、切换历史批次和查看全部选题。
- [x] 实现手动新增、编辑、确认和归档。
- [x] 实现 approved 选题的脚本确认面板。
- [x] 脚本生成成功后刷新脚本列表和选题状态。
- [x] 覆盖前端 Vitest 和 E2E。

## 9. 全量验证

- [x] 运行后端直接相关 Rust 测试。
- [x] 运行前端直接相关 Vitest。
- [x] 运行视频工作台 E2E。
- [x] 运行 `openspec instructions apply --change "content-topic-agent" --json` 并确认任务进度与实际一致。
