## Context

`materials.file_url` 已是素材稳定访问地址，API 与 video-worker 共享 `/app/storage/assets` 持久化卷，API 通过 `/assets` 提供静态访问。现有素材库只接受 JSON URL 登记，前端还要求操作者手工填写缩略图和类型相关 metadata。用户已确认改为真实上传，且地址不应成为用户可见或可编辑字段。

## Goals / Non-Goals

**Goals:**

- 上传图片、视频、音频和字幕文件后自动创建素材记录。
- 保存到现有共享素材卷并返回稳定 `/assets/uploads/...` 地址。
- 由系统验证文件并提取可用元数据，前端只读展示。
- 失败时不留下数据库记录或孤儿文件。
- 图片详情支持可关闭、可缩放的大图预览。

**Non-Goals:**

- 不做批量上传、分片上传、断点续传、对象存储或 CDN。
- 不自动抽取视频封面、生成音频波形或解析字幕语言。
- 不移除后端已有 JSON URL CRUD，以免破坏 AI 生成素材和已有内部调用；只从操作端页面移除该入口。
- 不修改历史素材文件或 metadata。

## Decisions

### 1. 在 Rust API 中完成上传编排

新增 `POST /api/projects/:project_id/materials/upload`，使用 multipart 字段 `file`、可选 `file_name` 和 `tags`。API 在同一请求内完成项目校验、文件验证、落盘、媒体探测和 `materials` 入库。

替代方案是由 video-worker 接收上传，但这会把同步业务写入放入异步运行时并增加跨服务事务问题。上传属于素材库控制面，放在 API 更符合现有边界。

### 2. 复用现有自管素材存储

文件保存为 `ASSET_STORAGE_ROOT/uploads/{project_id}/{upload_id}.{ext}`，物理文件名使用服务端 UUID，避免路径穿越、重名和跨平台文件名问题。`materials.file_name` 保存用户可见名称，`file_url` 保存 `/assets/uploads/...`。

项目存在性在写文件前校验。若媒体探测或入库失败，立即删除当前上传文件。

### 3. 内容验证与元数据探测

- 图片：仅接受 JPEG、PNG、WebP 和 GIF；使用图片解码读取宽高，不能解码则拒绝。
- 视频：接受 MP4、MOV 和 WebM；使用 `ffprobe` 验证并读取格式、时长和视频流宽高。
- 音频：接受 MP3、WAV、M4A 和 OGG；使用 `ffprobe` 验证并读取格式和时长。
- 字幕：接受 SRT、VTT、ASS 和 SSA；要求 UTF-8 文本并记录字幕格式。

metadata 统一包含 `source=user_upload`、`storage_provider=local`、`mime_type`、`format`、`file_size_bytes`，并按类型补充 `width`、`height`、`duration_sec` 或 `subtitle_format`。

### 4. 限制单文件大小并拒绝不支持内容

单文件上限为 500 MiB，路由单独配置请求体上限。缺少文件、空文件、扩展名不支持、内容与类型不匹配或媒体不可解析时返回 400/413，不创建素材。

multipart extractor、字段迭代和字段读取错误统一映射为稳定中文 JSON，不向客户端暴露 Axum/Multer 底层错误文案。

### 5. 前端只编辑业务字段

新建抽屉只包含文件选择、自动填充的素材名称和可选标签。点击“上传并保存”调用 multipart API。编辑抽屉只允许更新名称和标签，构造更新 payload 时保留当前素材的 `file_url`、`thumbnail_url`、类型和 metadata，避免隐藏字段丢失。

前端提交编辑时把当前 API base 下已解析的 `/assets/...` 绝对地址还原为稳定相对地址；后端应用层无论请求携带何值，都使用数据库当前记录保留 `file_url`、`thumbnail_url`、素材类型和 metadata，只更新名称和标签。

详情以紧凑只读摘要展示系统文件信息，不展示素材地址、缩略图地址、来源备注、授权备注或可编辑媒体字段。

### 6. 图片大图预览保持页面内状态

仅当当前素材可解析出图片预览 URL 时，详情预览使用按钮语义。点击后打开 `dialog`，支持关闭按钮、Escape、点击遮罩关闭和 50%-200% 缩放；关闭后焦点返回预览按钮。视频、音频和字幕占位不打开大图。

## Risks / Trade-offs

- [API 镜像增加 ffmpeg 体积] -> 仅安装运行时 `ffmpeg` 包，并把媒体探测集中在上传请求。
- [大文件占用内存] -> Axum multipart 仍可能在字段读取时占内存；本版限制 500 MiB，后续分片上传单独设计。
- [数据库失败产生孤儿文件] -> 捕获入库错误并删除刚写入的文件。
- [删除文件失败] -> 返回原始业务错误并记录清理失败；文件路径使用请求 UUID，可由后续运维扫描清理。
- [旧 JSON URL API 仍存在] -> 该 API 保留给现有内部链路，但正式素材库 UI 不暴露入口。

## Migration Plan

1. 发布包含 `ffprobe` 的 API 镜像和 multipart 路由。
2. 保持现有 `materials` 表与旧 URL API 不变，确认上传 API 可写入共享卷并能经 `/assets` 读取。
3. 发布前端上传流程和新详情抽屉。
4. 回滚时可仅回滚前端与上传路由；已上传素材仍是普通 `materials` 记录，可继续读取。

## Open Questions

无。批量、分片、对象存储和视频封面生成均明确留待后续 change。
