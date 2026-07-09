# fix-lan-api-base-url

## Summary

修复 `apps/video-agent` 使用本机内网 IP 访问 `18183` 前端时接口请求仍指向浏览器本机 `localhost:18180` 的问题。

## Motivation

当前视频工作台前端 API client 默认使用 `http://localhost:18180`。当操作者通过 `http://10.1.31.7:18183` 访问前端时，浏览器中的 `localhost` 指向操作者自己的机器，而不是运行 Docker 服务的机器，导致接口无法请求。

实测后端 API 和 CORS 正常：`127.0.0.1:18180/health`、`10.1.31.7:18180/health` 均返回 200，且 `Origin: http://10.1.31.7:18183` 的预检返回 200。

## Scope

- 调整 `apps/video-agent` API base URL 解析规则。
- 在浏览器端无显式环境变量时，根据当前页面 `window.location.protocol` 与 `window.location.hostname` 派生 API 地址，端口固定映射为 `18180`。
- 移除 `ai-agent-video-agent` Compose 环境中固定的 `NEXT_PUBLIC_API_BASE_URL=http://localhost:18180`，避免覆盖浏览器端派生规则。
- 保留 `NEXT_PUBLIC_API_BASE_URL` 显式覆盖能力。
- 补充前端单元测试覆盖 localhost、内网 IP 和显式环境变量三类场景。

## Out Of Scope

- 不修改后端路由、CORS、数据库或 Docker 端口映射。
- 不修改 `admin` 前端。
- 不引入反向代理或 Next.js API proxy。
- 不处理移动端适配或公网部署域名策略。
