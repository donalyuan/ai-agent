# ADR-0009：Agnes 异步视频操作与派生物边界

## 状态

已接受

## 决策

Agnes video operation 使用 `run_id + logical_operation` 作为唯一幂等 intent，规范化持久化 `submit/poll/cancel/result` 状态、provider request id、poll fingerprint、retention policy/version/hold 与 immutable `VideoTakeCandidate`。probe 不预设 model/mode ID，只从账号返回中冻结一个非 2.5 preview 的稳定候选；真实调用仍要求 explicit enabled profile、MVP-A、credential、runnable snapshot。

ProviderCall 继续由 catalog 唯一拥有，RunEvent 继续由 workflows/runs 唯一拥有。视频 terminal result 先经 bounded media validation 和 StoragePort/AssetVersion，再进入 `pending_review`；`accept|reject|retake` 是唯一 review action，accept 以 scenes exact current CAS 为门，retake 使用新 logical operation，取消后的晚到结果只能是未引用 candidate。

Agnes 不生成 thumbnail、proxy、waveform、keyframe 或 canonical normalized metadata；这些由独立 MediaInspect/MediaDerivative port 归 Media Worker 所有。派生物 pending/failed/stale 只阻断 Timeline/preview/export，不撤销 accepted/current。

## 后果

真实 Agnes 未配置时保留 `unconfigured`，默认测试保持 deterministic Mock + `local_workspace`。视频 operation 与 candidate 不再写入 `phase_one_documents`，SQLAlchemy 使用 `0017_agnes_video_owner` 规范化表与 revision CAS。
