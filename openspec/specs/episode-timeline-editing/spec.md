# episode-timeline-editing Specification

## Purpose
TBD - created by archiving change implement-episode-timeline-audio-export. Update Purpose after archive.
## Requirements
### Requirement:总体计划追溯和协调边界
本 change SHALL 反向追溯到 `plan-phase-one-drama-mvp-a` 的总体任务 `1.2`、直接实施任务 `4.1`--`4.3` 和共享任务 `5.1`--`5.8`。实施 MUST 以总体任务 `2.1`、`2.2`、`2.3` 的交付及已归档 AssetVersion 契约为可核验前置；总体 plan 仅协调 change 顺序、范围和验收，MUST NOT 成为运行时代码依赖。完整非目标是 MVP-B `portable` payload、Fish Audio、Groq ASR、自动字幕/自动对齐、专业 NLE、回导、发布平台和跨集自动拼接，以及接受 `profile`/`export_profile` 等 `exportProfile` 别名或让 DB/manifest/HTTP 各自维护版本源。真实 `ffmpeg`/`ffprobe` adapter 与 explicit probe 是本 change 必需交付；Mock/unconfigured 仅用于测试或诊断，绝不报告成功。9:16/16:9/1:1、1080p、30fps、H.264 `yuv420p`、AAC 48 kHz、UTF-8 SRT、四类音轨、light 引用和 ExportJob 状态已冻结；本 change MUST NOT 承担其外的职责。

#### Scenario:前置或范围不满足时保持边界
- **WHEN** 实施取证发现前置切片尚未可用，或请求把总体 plan/MVP-B 能力作为 Timeline/export runtime 依赖
- **THEN** 实施显式阻塞或拒绝该请求，记录缺失前置；不得伪造 resolver、导入总体 plan 模块或产生 portable package

#### Scenario:拒绝完整非目标职责泄漏
- **WHEN** Timeline 编辑尝试承担任一列明的非目标、接受 profile 别名、忽略已冻结的 30fps/render contract，或将 `renderer_unconfigured` 报告为成功
- **THEN** 架构依赖/契约测试失败，且不写 current Cut、TimelineVersion、Clip、审计或 Outbox

### Requirement:Episode 范围内的唯一 current Cut 与不可变 TimelineVersion
系统 SHALL 为每个 Episode 持久化恰好一个 mutable current Cut，并允许发布、命名、读取和只读比较多个不可变 `TimelineVersion`。系统 MUST NOT 创建 `TimelineDraft`，也 MUST NOT 提供第二个 mutable Cut 或创建、选择、切换 Cut 的生命周期。current Cut 与 TimelineVersion MUST 含稳定 ID、持久化 `schema_version`、HTTP DTO `schemaVersion`、revision、项目/剧集归属和审计时间；两种字段名 MUST 映射同一 canonical 版本值，不得形成两个版本源。

#### Scenario:编辑 current Cut 并发布命名版本
- **WHEN** 用户以当前 revision 编辑现有 Episode 的 current Cut，随后通过 preflight 并发布一个命名版本
- **THEN** 系统立即持久化该唯一 current Cut 的新 revision，并追加不可变 TimelineVersion；既有命名版本保持不变，且不创建或切换第二个 mutable Cut

#### Scenario:拒绝跨 episode timeline mutation
- **WHEN** 请求以其他项目或 Episode 的 ID 读取或修改 current Cut
- **THEN** 系统返回 not_found 或 forbidden/episode_mismatch，且不改变 current Cut

#### Scenario:拒绝 schema 版本来源冲突
- **WHEN** 写入或读取映射中的持久化 `schema_version`、manifest `schema_version` 与 HTTP DTO `schemaVersion` 缺失或值不一致
- **THEN** 系统返回 validation，且不写入或返回相互冲突的 Timeline/Export 版本事实

### Requirement:整数帧 clip 编辑
系统 SHALL 仅以整数帧存储 Clip 的排序、source in/out、timeline start 与 duration，并以单一静态值存储 `position`、`scale`、`opacity`。系统 MUST 支持显式排序、裁剪、拆分和 `DeleteClip`，并保留来源关系；`DeleteClip` MUST 只删除 Timeline reference，绝不删除或覆盖 AssetVersion。不得接受浮点帧、负值、零长度、越界裁剪、未定义的轨道重叠或任何 keyframe/animation payload。

#### Scenario:trim 和 split 一个 clip
- **WHEN** 用户对同集可用素材 Clip 提交合法整数帧裁剪或拆分命令
- **THEN** 系统产生满足边界的新 Clip 状态、稳定排序和递增 revision，原已发布 Version 不变

#### Scenario:拒绝无效帧范围
- **WHEN** Clip 使用非整数帧、`outFrame <= inFrame`、负值或超过素材可用帧
- **THEN** 系统返回 validation 或 `frame_out_of_bounds`，不写入部分编辑

### Requirement:immutable version 与 current revision 并发
系统 SHALL 从 current Cut 发布可命名的不可变 `TimelineVersion`，允许读取和只读比较多个已发布版本，并要求写命令携带 expected revision。过期 revision MUST 返回 409 且不覆盖 current Cut 或任何 version。

#### Scenario:发布 current cut
- **WHEN** current Cut 通过 timeline preflight 且请求携带当前 revision
- **THEN** 系统保存新的不可变 Version，记录其 Clip/音频/字幕快照和来源 revision

#### Scenario:拒绝 stale 编辑
- **WHEN** 两个客户端针对同一 Cut 发送编辑，其中一个 expected revision 已过期
- **THEN** 过期请求返回 `revision_conflict` 409，已有编辑保持不变

### Requirement:立即持久化与 authoritative 409 恢复
系统 SHALL 让 `TrimClip`、`SplitClip`、`ReorderClips`、`DeleteClip`、`SetClipTransform`、`SetSoundCueMix`、`SetDuckingPolicy` 与 `UpsertManualCaption` 都携带 `expectedRevision` 并在成功时于同一 UoW 立即持久化 current Cut revision、审计和 Outbox。系统 MUST NOT 提供独立自动保存、通用 undo/redo 或版本恢复；409 MUST 零部分写入并返回 authoritative current revision/state。

#### Scenario:删除并持久化 clip 编辑
- **WHEN** 用户以 current expected revision 提交 `DeleteClip` 或任一合法静态编辑 command
- **THEN** owner 立即返回新的 Cut revision 和持久化 state，且已发布 TimelineVersion 与 AssetVersion 保持不变

#### Scenario:恢复 stale 的立即编辑
- **WHEN** command 使用过期 revision 或包含 keyframe/unknown transform field
- **THEN** owner 返回 `revision_conflict` 409 或 validation、未写入任何部分编辑；客户端必须丢弃乐观状态并重新读取 authoritative Cut

### Requirement:固定 30fps 整数帧来源充分性
系统 SHALL 以 30fps 整数帧计算 Clip 和字幕时间。任一 clip 的 source range、duration 或 timeline coverage 不足时 MUST 显式失败，MUST NOT 插入黑帧、延展最后帧或静默变更 duration。

#### Scenario:拒绝不足的来源覆盖
- **WHEN** 剪辑命令或预检发现素材帧数不足以满足 published TimelineVersion
- **THEN** 系统返回 `missing_asset` 或 `frame_out_of_bounds`，不写部分编辑或导出替代画面
