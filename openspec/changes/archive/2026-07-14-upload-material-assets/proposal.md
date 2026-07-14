## Why

当前素材库要求操作者手工填写素材 URL、缩略图 URL 和媒体元数据，既不符合真实素材导入流程，也容易产生无效地址和错误元数据。用户已确认素材库应直接上传文件，由系统保存、识别并回填信息，同时不在界面暴露内部素材地址。

## What Changes

- **BREAKING**：移除素材库前端的 URL 素材登记入口，不再允许通过页面手工创建或修改 `file_url`、`thumbnail_url` 和媒体 metadata。
- 新增项目级 multipart 素材上传 API，将文件保存到现有自管素材卷并创建 `materials` 记录。
- 自动识别素材类型、格式、MIME、文件大小、图片尺寸及音视频时长；探测结果保存到 metadata。
- 上传校验、媒体探测或数据库写入失败时清理已写文件，避免孤儿文件。
- 详情抽屉只允许编辑素材名称和标签；系统文件信息保持只读，素材地址不显示。
- 图片素材的详情预览可点击打开大图，支持关闭及放大、缩小。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `material-library-management`：从手工 URL 登记改为真实文件上传、自动元数据识别、只读系统信息和图片大图预览。

## Impact

- API：新增 `POST /api/projects/:project_id/materials/upload` multipart 接口。
- Backend：新增上传校验、文件落盘、媒体探测和失败清理；Axum 启用 multipart，API 镜像安装 `ffprobe`。
- Frontend：素材库新建流程、详情表单、API client、图片预览弹层和相关样式变化。
- Tests：扩展 Rust 路由测试、TypeScript API/模型/页面测试和 Playwright E2E。
- Storage：复用 `ASSET_STORAGE_ROOT` 和 `/assets` 静态路由，不新增存储系统或数据库表。
