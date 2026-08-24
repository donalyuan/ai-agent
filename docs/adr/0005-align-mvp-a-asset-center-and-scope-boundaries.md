# ADR-0005：对齐 MVP-A 资产中心与能力边界

- 状态：已接受
- 日期：2026-08-22

## 背景

阶段一 PRD 与既有 OpenSpec 已覆盖创作工作台、候选审核、Timeline 和 Provider/Model/Skill 设置，但缺少项目级资产目录、通用上传到版本登记、试听和使用位置的用户闭环。部分条目还会把结构化故事生成误解为小说正文生成、把技术媒体检查误解为自动内容质检，或让局部 mask/选区/时间范围编辑在没有 owner contract 的情况下进入 MVP-A。Provider 并发/配额、运行资源和 FFmpeg codec/container 的准入条件也需要在付费或媒体操作前统一失败。

## 决策

- 将 `implement-project-asset-center`、`extend-projects-episodes-creative-slice` 和 `implement-asset-bible-continuity-slice` 纳入阶段一，阶段一由 1 个总体协调 change 与 17 个 child 组成；项目资产中心是第五个前端/业务闭环，Project/Episode creative owner 与 AssetBible continuity owner 补齐其领域前置。
- projects owner 持有 `creationMode`、`CreativeBrief`、项目设置、文本费用阈值和项目级 `StorySpec current`；episodes owner 持有每集 `ScriptSpec current`。AssetBible owner 持有六类 typed entry/version、project -> episode -> scene -> shot 四层 override、resolved snapshot、impact preview 与 `ContinuityRevisionTask`，其他 consumer 只读运行时冻结的 snapshot。
- 初始 AssetBible specs 必须经显式审核/ack；连续性 stale/pending 或未接受的 resolved snapshot 在外部媒体调用前阻断。AssetEdit Session/turn/Plan/execution 冻结 snapshot，不能复制或反写 entry/override。
- failed Run 保持终态，只能创建可追溯的 successor Run；Skill 路由歧义或低置信度必须由用户裁决，不能默认选择第一项。
- 重拍接受只产生 Timeline replacement handoff，由 Timeline owner 通过 `ReplaceClipSource` 创建新的不可变 TimelineVersion；项目批量导出使用 `EpisodeExportBatch`，逐集产生独立 MP4/SRT/light artifacts。
- MVP-A 文本链路只生成结构化 `StorySpec`、每集 `ScriptSpec`、Scene、Shot 和 `ShotSpec`；小说或已有材料只作为 `SourceMaterial`，不生成小说正文、章节或章节草稿。
- 项目资产中心拥有目录、筛选、元数据/授权版本、上传恢复/取消、派生状态、音频试听和只读 usage projection；各领域 owner 仍是引用事实源。自动语义/视觉 QC 与统一审核中心延后 MVP-B。
- 通用上传固定为 `AssetVersionReservation -> Storage operation -> verified StoredObjectRef -> AssetVersion registration`，operation key 为 `asset-upload:{projectId}:{assetId}:{reservationId}`。Storage 不拥有 AssetVersion；取消或晚到结果不得自动登记版本或替换 current reference。
- MVP-A 的 Agent 素材编辑只接受完整 image/video `AssetVersion` 作为 base 和显式 refs。mask、图片选区/局部区域以及视频/音频时间范围编辑延后 MVP-B，并在 execution intent、Outbox、ProviderCall、Storage operation 或付费提交前返回 `unsupported_feature`。
- Provider catalog 必须表达 operation 级并发、限流、429/Retry-After 和 quota known/unknown/exhausted；历史引用模型只能 disable，不能无证明删除。
- 付费、上传、派生、preview/export 前必须检查 CPU、内存、容量和必要 FFmpeg decoder/encoder/pixel-format/mux/demux capability；不足时明确阻断，不降级或伪报成功。

## 后果

- 阶段一经 ADR-0006 扩展后共有 19 个 active change、463 项任务；当前已完成 Project/Episode creative、结构化文本与 Scene/Shot 三个 child，总体协调进度为 7/54，数量和进度以后以 `openspec list --json` 为准。
- 资产中心、TOS、Media Worker、Timeline 与各 usage owner 必须通过 typed command/query 交接，不能复制 UploadSession、StoredObject、AssetVersion、派生状态或引用事实。
- 前端不得展示尚不支持的局部媒体编辑控件；Provider capability snapshot 只能收紧 MVP-A 能力，不能绕过平台和阶段边界。
- Project/Episode creative owner、结构化文本/Skill runtime 与 Scene/Shot owner 已实现；AssetBible、资产中心、Provider/TOS、Timeline、resilience 和前端闭环仍未完成，不得由前三个切片推断为已实现。
