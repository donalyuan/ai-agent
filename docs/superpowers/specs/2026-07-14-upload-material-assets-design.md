# 素材文件上传与自动回填设计

## 已确认体验

素材库不再要求用户填写素材 URL、缩略图 URL、来源备注、授权备注、宽度、高度、格式或时长。用户选择本地文件后，只需确认自动填充的素材名称，并可选填写标签；系统保存文件、识别类型与 metadata 后创建素材。

编辑已有素材时只允许修改名称和标签。素材地址是内部渲染字段，界面不展示。图片详情预览可点击打开大图，支持关闭及 50%-200% 缩放；非图片类型占位不可打开大图。

Pencil 原型状态位于 `docs/prototypes/video-agent/video-agent.pen`：

- `n69ruH`：上传素材。
- `UFiru`：保存后的只读详情。
- `NhFLB`：大图预览。

## 架构

Rust API 新增项目级 multipart 上传接口。文件写入现有 `ASSET_STORAGE_ROOT/uploads/{project_id}/`，使用 UUID 物理文件名和真实扩展名，通过已有 `/assets` 静态服务访问。API 在同一请求内完成项目校验、文件验证、媒体探测、落盘和 `materials` 入库；失败时删除已写文件。

图片由解码器读取尺寸，音视频由 `ffprobe` 验证并读取时长及可用的视频尺寸，字幕要求 UTF-8。metadata 统一记录上传来源、存储提供方、MIME、格式和字节大小。

前端通过 `FormData` 上传。编辑保存继续调用现有 JSON 更新 API，把当前 API base 下的 `/assets/...` 还原为稳定相对地址；后端应用层强制保留数据库当前地址、类型、缩略图和 metadata，只允许名称和标签变化。

## 设计参考

继续使用项目根 `DESIGN.md`。借鉴 Ant Design 的上传反馈与抽屉操作、IBM Carbon 的表单校验和只读字段分组、GitHub Primer 的低干扰边框与对话框层级；拒绝营销区块、手工技术字段和嵌套卡片。

## 验收边界

- 单文件最大 500 MiB。
- multipart extractor 或字段读取失败统一返回稳定中文 JSON，不暴露底层解析错误。
- 支持 JPEG、PNG、WebP、GIF、MP4、MOV、WebM、MP3、WAV、M4A、OGG、SRT、VTT、ASS、SSA。
- 本轮不做批量、分片、断点续传、对象存储、视频封面、音频波形或字幕语言识别。
- 现有 JSON URL API 保留给内部生成链路，但素材库页面不再暴露。
