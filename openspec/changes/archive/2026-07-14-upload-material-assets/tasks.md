## 1. 上传契约与存储

- [x] 1.1 先补充上传检测、文件大小限制、图片尺寸和媒体探测解析的失败测试
- [x] 1.2 实现上传文件类型识别、metadata 构造、稳定路径写入和清理
- [x] 1.3 为 API 镜像安装 `ffprobe` 并启用 Axum multipart 与异步文件/进程能力

## 2. 上传 API

- [x] 2.1 先补充 multipart 图片成功、无效文件和无副作用路由测试
- [x] 2.2 实现 `POST /api/projects/:project_id/materials/upload` 及错误映射
- [x] 2.3 验证上传结果可通过 `/assets/uploads/...` 静态路由读取

## 3. 前端 API 与模型

- [x] 3.1 先补充 multipart API client、只读编辑 payload 和文件信息格式化失败测试
- [x] 3.2 实现 `uploadMaterial`、精简表单状态和保留系统字段的编辑 payload

## 4. 素材库交互

- [x] 4.1 先补充上传抽屉、地址隐藏、自动回填和图片大图预览页面测试
- [x] 4.2 实现文件选择上传、只读系统信息、精简编辑表单和错误保留
- [x] 4.3 实现图片预览对话框、50%-200% 缩放、Escape/遮罩关闭和焦点恢复
- [x] 4.4 更新素材库样式并保持画布、抽屉和弹层无重叠

## 5. 端到端与文档

- [x] 5.1 更新 Playwright 路由与素材库上传/大图预览验收
- [x] 5.2 同步素材工作流 memory，记录上传取代 URL 登记的已确认决策
- [x] 5.3 运行 Rust、前端单测、E2E、lint、build 和 OpenSpec 严格校验
- [x] 5.4 归档 `upload-material-assets` change 并确认主规格同步
