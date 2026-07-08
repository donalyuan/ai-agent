# topic-quality-gate Tasks

## 1. OpenSpec 与设计

- [x] 创建 proposal、design、spec 增量和 tasks。
- [x] 运行 `openspec instructions apply --change "topic-quality-gate" --json`，确认 change 可识别。
- [x] 写入 `docs/superpowers/specs/2026-07-08-topic-quality-gate-design.md`。

## 2. 记忆与原型

- [x] 更新项目记忆，记录质量闸门、最多一次自动重写和不新增选题状态的确认边界。
- [x] 更新 `docs/prototypes/video-agent/video-agent.pen`，覆盖质量摘要和质量报告。
- [x] 获得用户明确原型确认后再进入前端编码。

## 3. 数据库与仓储

- [x] 新增 migration：`topic_quality_evaluations`。
- [x] 添加项目、批次、时间、run 追踪索引。
- [x] 实现质量评估 domain model 和 DTO。
- [x] 实现 repository contract tests。
- [x] 实现 Postgres repository。

## 4. 后端质量闸门与 API

- [x] 实现质量闸门 LLM prompt、输出 schema 和解析校验。
- [x] 调整 `topic` Agent 生成链路为候选暂存、质量评估、必要时重写、只入库通过项。
- [x] 补充生成链路复用同一质量闸门，并注入同主题组已有选题上下文。
- [x] 实现 `GET /api/topic-generation-batches/:batch_id/quality-evaluation`。
- [x] 记录 `agent_runs` 和 `agent_steps`，覆盖生成、评估、重写和入库步骤。
- [x] 覆盖质量评估失败、重写失败、无通过项和部分通过场景。

## 5. 前端实现

- [x] 扩展 `apps/video-agent/app/lib/api.ts` 和测试。
- [x] Agent 消息展示通过数、淘汰数和是否重写。
- [x] 当前选题池卡片展示质量分和风险标签。
- [x] 历史生成批次卡展示质量摘要。
- [x] 历史生成页右侧展示质量报告。
- [x] 淘汰候选只读展示，不提供确认、生成脚本或归档操作。

## 6. 验证

- [x] 运行后端相关 Rust 测试。
- [x] 运行前端相关 Vitest。
- [x] 运行视频工作台 E2E。
- [x] 运行 `openspec instructions apply --change "topic-quality-gate" --json` 并确认任务状态与实际一致。
