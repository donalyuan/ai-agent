## Why

AI 生成图片当前沿用供应商或任务 UUID 派生文件名，素材库和物理存储中的名称无法直接表达所属脚本、镜头和候选序号。需要为后续新生成图片建立稳定、可读且跨供应商一致的文件命名规则，同时保留 UUID 任务目录避免同名脚本冲突。

## What Changes

- 新生成图片的实际文件名统一为 `{脚本名称}-镜头{两位序号}-第{两位候选序号}张.{实际扩展名}`。
- Worker 在领取任务时读取脚本标题快照，并把脚本标题、镜头序号和 1-based 候选序号贯穿 batch 与 `per_candidate` 执行路径。
- 对脚本标题执行 Unicode 规范化、非法文件名字符清理和 UTF-8 字节安全截断；空标题回退为“未命名脚本”。
- `materials.file_name` 与物理文件名保持一致，素材与候选 metadata 增加命名来源快照。
- UUID 任务目录继续作为隔离边界；不批量重命名或改写已有素材。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `material-library-management`: 增加 AI 生成图片的友好物理文件名、候选槽位稳定编号和命名审计要求。

## Impact

- Worker：`services/video-worker/src/video_worker/asset_generation.py` 的任务领取、候选结果编排、文件名生成、落盘和 metadata。
- Worker 测试：`services/video-worker/tests/test_asset_generation.py`；API 静态资源回归测试：`backend/tests/asset_generation_routes.rs`。
- 数据库：不新增表或字段；继续使用 `materials.file_name VARCHAR(255)` 和现有 JSONB metadata。
- API 与前端：无接口、页面或 Pencil 原型变化。
- 外部调用：不改变供应商请求数量、重试规则或费用边界。
