# Fix LAN API Base URL Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复视频工作台通过本机内网 IP 访问时，浏览器接口请求仍指向 `localhost:18180` 的问题。

**Architecture:** 保留显式 `NEXT_PUBLIC_API_BASE_URL` 覆盖；无显式配置时，浏览器端从当前页面 `window.location.hostname` 派生同机 `18180` API 地址；非浏览器环境继续回退 `localhost`。

**Tech Stack:** Next.js 14 + TypeScript + Vitest；OpenSpec change `fix-lan-api-base-url`。

---

## Execution Rules

- 不执行 `git add`、`git commit`、`git push`，除非用户明确要求。
- 先写失败测试并确认红灯，再写实现。
- 不修改后端 API、CORS、数据库或 Docker 端口映射。

## File Map

- Modify: `apps/video-agent/app/lib/api.test.ts`，覆盖内网 IP、回环地址和显式环境变量场景。
- Modify: `apps/video-agent/app/lib/api.ts`，实现浏览器端默认 API base URL 派生。
- Modify: `docker-compose.yml`，移除 `ai-agent-video-agent` 默认注入的 `NEXT_PUBLIC_API_BASE_URL=http://localhost:18180`。
- Modify: `openspec/changes/fix-lan-api-base-url/tasks.md`，随执行同步勾选。

## Tasks

- [x] 写 `getApiBaseUrl()` 内网 IP 场景失败测试：设置 `window.location` 为 `http://10.1.31.7:18183`，期望返回 `http://10.1.31.7:18180`。
- [x] 运行 `docker exec ai-agent-video-agent npm run test -- app/lib/api.test.ts`，确认新增测试因仍返回 `http://localhost:18180` 失败。
- [x] 在 `apps/video-agent/app/lib/api.ts` 中实现 `getDefaultApiBaseUrl()`：浏览器端返回 `${window.location.protocol}//${window.location.hostname}:18180`，否则返回 `http://localhost:18180`。
- [x] 从 `docker-compose.yml` 的 `ai-agent-video-agent.environment` 移除 `NEXT_PUBLIC_API_BASE_URL`，避免覆盖动态 fallback。
- [x] 再次运行 `docker exec ai-agent-video-agent npm run test -- app/lib/api.test.ts`，确认通过。
- [x] 运行 `docker exec ai-agent-video-agent npm run lint`。
- [x] 运行 `openspec instructions apply --change "fix-lan-api-base-url" --json` 与 `openspec validate --all`。
- [x] 重启 `ai-agent-video-agent` 并用 `curl --noproxy '*' http://10.1.31.7:18183/` 与 `curl --noproxy '*' http://10.1.31.7:18180/health` 确认页面和 API 可达。
