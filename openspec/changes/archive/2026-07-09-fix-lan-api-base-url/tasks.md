# fix-lan-api-base-url Tasks

## 1. OpenSpec

- [x] 创建 proposal、design、spec 增量和 tasks。
- [x] 运行 `openspec instructions apply --change "fix-lan-api-base-url" --json`。

## 2. 前端 API base URL 修复

- [x] 为 `apps/video-agent/app/lib/api.test.ts` 增加内网 IP hostname 派生 API 地址的红灯测试。
- [x] 修改 `apps/video-agent/app/lib/api.ts`，在浏览器端按当前页面 hostname 派生 `18180` API 地址。
- [x] 保持显式 `NEXT_PUBLIC_API_BASE_URL` 覆盖优先级和去除结尾斜杠行为。
- [x] 移除 `ai-agent-video-agent` Compose 默认环境中的 `NEXT_PUBLIC_API_BASE_URL=http://localhost:18180`。

## 3. 验证

- [x] 运行相关前端单元测试。
- [x] 运行前端 lint。
- [x] 运行 OpenSpec change 校验和全量 spec 校验。
- [x] 重启视频工作台前端容器并确认运行环境使用修复后的代码。
