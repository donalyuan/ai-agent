# material-library-management Proposal

## 背景

视频工作台已经完成内容策略、选题池、脚本创作和账号策略资料等前置能力。下一阶段进入 Phase 3 素材管理，但当前仓库只有早期 schema 中的 `materials` 和 `material_embeddings` 表，没有素材管理页面、API、仓储逻辑或正式原型。

用户已确认第一版先做“素材库管理”，不直接进入上传、Embedding、语义检索、素材检索 Agent、分镜候选或作品生产读取素材清单。

## 目标

1. 启用“素材管理”一级菜单，并新增二级入口“素材库”。
2. 基于 `materials` 建立当前账号下的素材登记管理闭环。
3. 支持 `video`、`image`、`audio`、`subtitle` 四类素材，前端显示为“视频 / 图片 / 音频 / 字幕”。
4. 支持手动录入已有素材 URL，不做文件上传。
5. 支持可选手动缩略图 URL；图片素材未配置缩略图时可用 `file_url` 预览，其他类型显示类型占位。
6. 支持素材列表、筛选、详情、新增、编辑、归档和恢复。
7. 使用 `active / archived` 表达素材生命周期，默认只展示可用素材。
8. 采用画布优先的素材画布工作台：主画布占据素材库主工作区，资产栏和详情编辑作为画布上的辅助浮层或窄面板，底部提供轻量画布工具栏。

## 非目标

1. 不实现真实文件上传、对象存储或本地文件存储策略。
2. 不自动抓取远程素材元数据。
3. 不自动抽取视频帧、生成音频波形或抓取封面。
4. 不保存画布节点位置，不实现节点连线语义、任务编排或 DAG。
5. 不写入 Embedding 或 Milvus。
6. 不实现语义检索、素材检索 Agent、素材候选匹配或素材替换。
7. 不实现脚本分镜素材关联、素材清单确认或作品生产读取素材清单。
8. 不做发布平台素材同步。
9. 不覆盖移动端原型、移动端适配或移动端验收。

## 影响范围

- 数据库：扩展 `materials.material_type`，新增 `materials.status` 和查询索引，并持久化启用素材库菜单。
- 后端：新增 Material domain model、repository、DTO、API 和错误映射。
- 前端：新增 `apps/video-agent` 的素材库页面级组件、API wrapper、画布状态编排和样式。
- 原型：更新 `docs/prototypes/video-agent/video-agent.pen`，覆盖“素材管理 > 素材库”画布工作台和空状态。
- 测试：覆盖 schema、repository、API、前端页面、E2E 和 OpenSpec 校验。
