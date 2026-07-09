## ADDED Requirements

### Requirement: 视频工作台必须支持本机内网 IP 访问 API

`apps/video-agent` SHALL 在本地 Docker 开发环境中支持操作者通过运行机器的内网 IP 访问 `18183` 前端，并让浏览器端 API 请求指向同一运行机器的 `18180` API 端口。

#### Scenario: 通过内网 IP 打开视频工作台

- **GIVEN** 视频工作台前端运行在 `http://10.1.31.7:18183`
- **AND** 未显式配置 `NEXT_PUBLIC_API_BASE_URL`
- **AND** `ai-agent-video-agent` Compose 默认环境未注入 `NEXT_PUBLIC_API_BASE_URL=http://localhost:18180`
- **WHEN** 前端创建 API client
- **THEN** API base URL SHALL 为 `http://10.1.31.7:18180`
- **AND** 后续 `/health`、`/api/projects` 和 `/api/video-workspace/menus` 请求 SHALL 使用该 base URL

#### Scenario: 通过本机回环地址打开视频工作台

- **GIVEN** 视频工作台前端运行在 `http://127.0.0.1:18183`
- **AND** 未显式配置 `NEXT_PUBLIC_API_BASE_URL`
- **WHEN** 前端创建 API client
- **THEN** API base URL SHALL 为 `http://127.0.0.1:18180`

#### Scenario: 显式 API base URL 仍优先生效

- **GIVEN** 已配置 `NEXT_PUBLIC_API_BASE_URL=http://api.example.test/`
- **WHEN** 前端创建 API client
- **THEN** API base URL SHALL 为 `http://api.example.test`
- **AND** 系统 SHALL NOT 使用当前页面 hostname 派生 API 地址
