# ADR-0010：Agent AssetEdit 审查与版本交接边界

## 状态

已接受（阶段一实现）。

## 决策

AssetEdit 只拥有 image/video 编辑意图、会话/对话事实、execution intent、结果候选、影响分析和人工 `accept|reject|retake` 审计。输入必须绑定同一项目的完整 `AssetVersion`（含 version ID、revision、content hash、project/mime owner metadata）；mask、选区、局部区域和时间范围在产生 execution/outbox 前以 `unsupported_feature` 拒绝。

AssetVersion 仍由 Assets owner append-only 管理。Provider terminal success 只交接一个已验证的 immutable version；AssetEdit candidate 不复制 bytes/object/hash，也不改变 current。`accept` 必须以 candidate/provenance/version/hash/target exact CAS 调用 Scenes owner 更新 storyboard eligibility，AssetEdit 不创建 Timeline Clip/SoundCue。

会话的 conversation/message/turn 使用独立 owner 表，message sequence 与 turn revision 采用 CAS；重复 correlation 只返回既有 turn，冲突内容拒绝。AssetBible 只通过 accepted snapshot/task read port 读取，snapshot stale 或 pending task 阻断 plan/execute/accept，并保持 `continuity_stale` 诊断。

数据库与共享契约只保存 canonical `schema_version`；HTTP `schemaVersion` 仅做同值映射，缺失/冲突不得写入。默认 Provider/Storage 继续 Mock/Local；真实依赖未配置时保留显式 `unconfigured`，不伪造成功。

## 影响

新增 `asset_edit_*` 及 conversation owner 表和 Alembic `0018_asset_edit_owner`；SQLAlchemy 更新使用 revision CAS；HTTP API 为 additive，响应不含媒体 bytes。Timeline、Provider adapter、prompt generation、自动接受、跨项目复制、历史/发布版本改写及 mask/time-range 仍属非目标。
