# API规格

## 概述

脚本Agent提供RESTful API，负责从选题生成结构化视频脚本。

## 端点

### 1. 生成脚本

生成新的视频脚本，包含标题、hook和多个分镜。

```http
POST /api/scripts/generate
Content-Type: application/json
```

**请求体**：

```json
{
  "project_id": "550e8400-e29b-41d4-a716-446655440000",
  "topic": "ChatGPT如何改变程序员工作流",
  "style": "knowledge",
  "scene_count": 6,
  "parent_id": null
}
```

**字段说明**：

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `project_id` | UUID | 是 | 所属项目ID |
| `topic` | String | 是 | 视频选题文本，10-200字 |
| `style` | String | 否 | 风格类型，默认"knowledge" |
| `scene_count` | Integer | 否 | 分镜数量，默认6，范围5-8 |
| `parent_id` | UUID | 否 | 父脚本ID，用于A/B测试 |

**style枚举值**：
- `knowledge`: 知识科普类（默认）
- `story`: 故事叙述类
- `tutorial`: 教程讲解类

**响应 200 OK**：

```json
{
  "script_id": "660e8400-e29b-41d4-a716-446655440001",
  "project_id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "程序员必看：ChatGPT的5大颠覆性用法",
  "hook": "还在手写重复代码？ChatGPT帮你3秒搞定",
  "scenes": [
    {
      "scene_id": "770e8400-e29b-41d4-a716-446655440002",
      "sequence": 1,
      "narration": "传统程序员每天要写大量重复代码，复制粘贴改参数，枯燥又容易出错",
      "visual_description": "程序员盯着屏幕，表情疲惫，快速切换多个代码文件",
      "emotion": "焦虑",
      "duration_sec": 8
    },
    {
      "scene_id": "880e8400-e29b-41d4-a716-446655440003",
      "sequence": 2,
      "narration": "现在有了ChatGPT，只需要描述需求，3秒就能生成完整代码",
      "visual_description": "程序员在ChatGPT输入需求，屏幕快速显示代码，表情惊喜",
      "emotion": "惊喜",
      "duration_sec": 10
    }
  ],
  "status": "draft",
  "parent_id": null,
  "created_at": "2026-07-01T10:30:00Z",
  "updated_at": "2026-07-01T10:30:00Z"
}
```

**响应字段说明**：

| 字段 | 类型 | 说明 |
|------|------|------|
| `script_id` | UUID | 脚本ID |
| `project_id` | UUID | 所属项目ID |
| `title` | String | 视频标题，不超过30字 |
| `hook` | String | 前3秒吸引点 |
| `scenes` | Array | 分镜列表 |
| `scenes[].scene_id` | UUID | 分镜ID |
| `scenes[].sequence` | Integer | 分镜顺序，从1开始 |
| `scenes[].narration` | String | 旁白文本，50-150字 |
| `scenes[].visual_description` | String | 视觉描述 |
| `scenes[].emotion` | String | 情绪标签 |
| `scenes[].duration_sec` | Integer | 时长（秒） |
| `status` | String | 脚本状态 |
| `parent_id` | UUID | 父脚本ID（A/B测试） |
| `created_at` | ISO8601 | 创建时间 |
| `updated_at` | ISO8601 | 更新时间 |

**错误响应**：

```json
// 400 Bad Request - 参数错误
{
  "error": "project_id不能为空"
}

// 400 Bad Request - 参数验证失败
{
  "error": "scene_count必须在5-8之间"
}

// 404 Not Found - 项目不存在
{
  "error": "项目不存在",
  "project_id": "550e8400-e29b-41d4-a716-446655440000"
}

// 503 Service Unavailable - LLM超时
{
  "error": "脚本生成超时，请稍后重试"
}

// 500 Internal Server Error - 服务器错误
{
  "error": "内部错误",
  "details": "LLM返回格式解析失败"
}
```

---

### 2. 查询脚本详情

获取指定脚本的完整信息。

```http
GET /api/scripts/:script_id
```

**路径参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `script_id` | UUID | 脚本ID |

**响应 200 OK**：

```json
{
  "script_id": "660e8400-e29b-41d4-a716-446655440001",
  "project_id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "程序员必看：ChatGPT的5大颠覆性用法",
  "hook": "还在手写重复代码？ChatGPT帮你3秒搞定",
  "scenes": [...],
  "status": "draft",
  "parent_id": null,
  "created_at": "2026-07-01T10:30:00Z",
  "updated_at": "2026-07-01T10:30:00Z"
}
```

**错误响应**：

```json
// 404 Not Found
{
  "error": "脚本不存在",
  "script_id": "660e8400-e29b-41d4-a716-446655440001"
}
```

---

### 3. 列出项目的所有脚本

获取指定项目下的所有脚本列表。

```http
GET /api/projects/:project_id/scripts
```

**查询参数**：

| 参数 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| `status` | String | 否 | - | 过滤状态：draft/approved/archived |
| `limit` | Integer | 否 | 20 | 返回数量，最大100 |
| `offset` | Integer | 否 | 0 | 偏移量 |

**响应 200 OK**：

```json
{
  "scripts": [
    {
      "script_id": "660e8400-e29b-41d4-a716-446655440001",
      "title": "程序员必看：ChatGPT的5大颠覆性用法",
      "status": "draft",
      "scene_count": 6,
      "parent_id": null,
      "created_at": "2026-07-01T10:30:00Z"
    }
  ],
  "total": 1,
  "limit": 20,
  "offset": 0
}
```

---

### 4. 更新脚本状态

更新脚本的状态（draft → approved → archived）。

```http
PUT /api/scripts/:script_id/status
Content-Type: application/json
```

**请求体**：

```json
{
  "status": "approved"
}
```

**status枚举值**：
- `draft`: 草稿
- `approved`: 已批准（可用于视频生成）
- `archived`: 已归档

**响应 200 OK**：

```json
{
  "script_id": "660e8400-e29b-41d4-a716-446655440001",
  "status": "approved",
  "updated_at": "2026-07-01T11:00:00Z"
}
```

**错误响应**：

```json
// 400 Bad Request
{
  "error": "无效的状态值",
  "allowed": ["draft", "approved", "archived"]
}

// 404 Not Found
{
  "error": "脚本不存在"
}
```

---

### 5. 删除脚本

删除指定脚本及其所有分镜。

```http
DELETE /api/scripts/:script_id
```

**响应 204 No Content**

**错误响应**：

```json
// 404 Not Found
{
  "error": "脚本不存在"
}

// 409 Conflict
{
  "error": "脚本已被视频引用，无法删除"
}
```

---

## 错误码汇总

| HTTP状态码 | 说明 |
|-----------|------|
| 200 | 成功 |
| 204 | 成功（无内容） |
| 400 | 请求参数错误 |
| 404 | 资源不存在 |
| 409 | 资源冲突 |
| 500 | 服务器内部错误 |
| 503 | 服务暂时不可用（LLM超时） |

---

## 性能要求

- 生成脚本接口响应时间 < 30秒（包含LLM调用）
- 查询接口响应时间 < 100ms
- 列表接口响应时间 < 200ms
- 支持20个并发生成请求

---

## 安全约束

- 所有UUID参数必须验证格式
- topic字段限制长度10-200字符，防止滥用
- scene_count限制5-8，防止资源浪费
- LLM调用必须有超时机制（30秒）
- 生成失败最多重试3次

---

## 扩展性

### 未来可能新增的字段
- `language`: 支持多语言（中文/英文）
- `duration_target`: 目标总时长（15s/30s/60s）
- `platform`: 目标平台（抖音/小红书/YouTube）
- `tone`: 语气风格（专业/轻松/幽默）

### 未来可能新增的端点
- `POST /api/scripts/:script_id/optimize`: 优化现有脚本
- `POST /api/scripts/:script_id/translate`: 翻译脚本
- `GET /api/scripts/:script_id/versions`: 获取所有A/B版本
