## Context

当前任务状态由必需步骤终态聚合；fake provider 完成步骤时只写 attempt，不产生 `materials` 记录。素材库已经提供本地共享存储、作品生成快照约束和成品播放能力，因此缺口在 Worker 的合成产物登记和任务详情消费。

## Goals / Non-Goals

**Goals:**

- 让 compose 成功与最终视频素材登记原子地衔接，重复恢复不得重复生成或登记成品。
- 让任务详情能直接播放最终成品，并保留素材库作为长期管理入口。
- 保持 fake provider 无外部费用，真实 provider 仍由后续协议 change 负责。

**Non-Goals:**

- 本 change 不实现真实 Seedance/TTS/ASR 调用。
- 本 change 不改变任务列表状态分组、取消、重试或跨 Agent 工作流边界。

## Decisions

1. **由 Worker 在 compose 完成时登记成品。** 作品生成的运行上下文、共享存储和数据库都在 Worker 可访问边界内，避免前端或详情 GET 产生副作用。
2. **按运行步骤做幂等检查。** 以 `generation_step_id + artifact_role=final_video` 查询既有素材；Worker 恢复时复用既有素材 ID，不重复写文件或数据库记录。
3. **完成态只消费已登记的 result_material_ids。** 合成登记失败转为失败并保留错误摘要，避免 UI 将“无成品”误报为成功。
4. **前端按素材 ID读取完整素材对象。** 任务详情只保留步骤事实和 ID，播放地址、文件名及素材库元数据继续由素材接口提供，避免复制素材领域契约。

## Risks / Trade-offs

- [Risk] fake 成片是黑色视频，不代表真实模型质量。→ 在 Worker 和 proposal 中明确其仅用于无费用流程验收，真实 provider 另行接入。
- [Risk] 旧成功运行缺少成品。→ 部署后执行一次幂等补登记脚本，并在 UI 对仍缺失的运行显示明确提示。
- [Risk] FFmpeg 或共享存储不可用。→ 合成步骤转失败并保留 `artifact_materialization` 错误，不伪造成功。

## Migration Plan

1. 部署 Worker 和任务页代码。
2. 对历史 fake 成功运行执行幂等成品补登记。
3. 运行 Worker、前端和 API 回归测试。

## Open Questions

真实 Seedance 输出如何映射到 `result_material_ids`，留待真实 provider 接入 change 明确。
