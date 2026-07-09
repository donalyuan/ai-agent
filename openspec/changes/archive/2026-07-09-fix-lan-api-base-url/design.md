# fix-lan-api-base-url Design

## Context

`apps/video-agent/app/lib/api.ts` 当前通过 `getApiBaseUrl()` 读取 `NEXT_PUBLIC_API_BASE_URL`，无配置时回退到 `http://localhost:18180`。该值在浏览器端被用于 `fetch`，所以当页面通过内网 IP 打开时，接口仍访问浏览器所在机器的 `localhost`。

Docker Compose 当前已将 API 暴露到宿主机 `18180`，视频工作台暴露到 `18183`。本次问题不是后端不可达，也不是 CORS 拒绝，而是前端请求目标 host 不随页面 host 变化。

同时，`docker-compose.yml` 当前给 `ai-agent-video-agent` 显式注入 `NEXT_PUBLIC_API_BASE_URL=http://localhost:18180`。该配置会覆盖代码里的默认 fallback，因此必须从视频工作台服务环境中移除；需要固定 API 地址的部署仍可在外部显式设置该变量。

## Decision

在 `apps/video-agent` 中新增浏览器端默认派生规则：

1. 若存在 `NEXT_PUBLIC_API_BASE_URL`，继续优先使用该值并去除结尾斜杠。
2. 若运行在浏览器端且没有显式环境变量，使用 `window.location.protocol` 与 `window.location.hostname` 生成 `http(s)://<当前页面hostname>:18180`。
3. 若运行在非浏览器环境或测试未提供 `window.location`，仍回退到 `http://localhost:18180`。
4. `ai-agent-video-agent` 的 Compose 默认环境不再设置 `NEXT_PUBLIC_API_BASE_URL`，让本地开发默认走第 2 条规则。

这样同一份前端代码可同时支持：

- `http://127.0.0.1:18183` -> `http://127.0.0.1:18180`
- `http://10.1.31.7:18183` -> `http://10.1.31.7:18180`
- 显式 `NEXT_PUBLIC_API_BASE_URL=http://api.example.test` -> `http://api.example.test`

## Risks

- 如果未来 API 不再与前端部署在同一 host 的 `18180` 端口，必须通过 `NEXT_PUBLIC_API_BASE_URL` 显式覆盖。
- 该规则只解决本地 Docker 开发环境同机多端口访问，不作为生产部署域名策略。

## Verification

- 先写 `getApiBaseUrl()` 单元测试并确认内网 IP 场景红灯失败。
- 实现后运行 `docker exec ai-agent-video-agent npm run test -- app/lib/api.test.ts`。
- 运行 `docker exec ai-agent-video-agent npm run lint`。
- 运行 `openspec instructions apply --change "fix-lan-api-base-url" --json` 与 `openspec validate --all`。
- 运行 `docker compose -f /server/docker-compose.yml config` 确认 `ai-agent-video-agent` 不再注入 `NEXT_PUBLIC_API_BASE_URL`。
- 重启 `ai-agent-video-agent` 后确认页面 bundle 不再包含 `http://localhost:18180` 的默认 API base 行为。
