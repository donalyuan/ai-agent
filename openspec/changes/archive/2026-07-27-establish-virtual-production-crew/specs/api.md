# API 接口规格

## 基础路径

```
Base URL: http://localhost:18180/api/v1/production
```

## 认证

所有接口需要 Bearer Token 认证。

```
Authorization: Bearer <token>
```

---

## 1. 项目管理

### 1.1 创建制作项目

**Endpoint**: `POST /productions`

**Request**:
```json
{
  "title": "产品宣传片",
  "description": "展示新款智能手表的功能特点",
  "project_type": "full_crew", // "fast_lane" | "full_crew"
  "initial_input": {
    "target_audience": "25-40岁科技爱好者",
    "key_features": ["健康监测", "长续航", "时尚设计"],
    "duration_seconds": 60,
    "platform": ["抖音", "小红书"]
  }
}
```

**Response** (201 Created):
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "产品宣传片",
  "description": "展示新款智能手表的功能特点",
  "project_type": "full_crew",
  "status": "created",
  "user_id": "user-uuid",
  "metadata": {},
  "created_at": "2026-07-27T10:30:00Z",
  "updated_at": "2026-07-27T10:30:00Z"
}
```

---

### 1.2 查询项目详情

**Endpoint**: `GET /productions/:id`

**Response** (200 OK):
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "产品宣传片",
  "project_type": "full_crew",
  "status": "scripting",
  "user_id": "user-uuid",
  "created_at": "2026-07-27T10:30:00Z",
  "updated_at": "2026-07-27T10:32:00Z",
  "artifacts": {
    "creative_brief": {
      "id": "brief-uuid",
      "version": 1,
      "status": "approved",
      "created_at": "2026-07-27T10:31:00Z"
    },
    "story_bible": {
      "id": "story-uuid",
      "version": 1,
      "status": "draft",
      "created_at": "2026-07-27T10:32:00Z"
    }
  },
  "next_role": "screenwriter"
}
```

---

### 1.3 列出用户的制作项目

**Endpoint**: `GET /productions`

**Query Parameters**:
- `project_type`: 过滤项目类型（可选）
- `status`: 过滤状态（可选）
- `page`: 分页页码（默认 1）
- `page_size`: 每页数量（默认 20）

**Response** (200 OK):
```json
{
  "items": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "title": "产品宣传片",
      "project_type": "full_crew",
      "status": "scripting",
      "created_at": "2026-07-27T10:30:00Z",
      "updated_at": "2026-07-27T10:32:00Z"
    }
  ],
  "total": 1,
  "page": 1,
  "page_size": 20
}
```

---

### 1.4 删除制作项目

**Endpoint**: `DELETE /productions/:id`

**Response** (204 No Content)

---

## 2. 角色执行

### 2.1 执行特定角色

**Endpoint**: `POST /productions/:id/roles/:role_key/execute`

**role_key**: `producer` | `screenwriter` | `director` | `cinematographer` | `performance_director` | `sound_director` | `editor` | `qc`

**Request** (可选补充输入):
```json
{
  "user_input": "请强调健康监测功能，增加运动场景",
  "context": {
    "reference_works": ["Apple Watch 广告"]
  }
}
```

**Response** (200 OK):
```json
{
  "role": "screenwriter",
  "status": "completed",
  "execution_time_ms": 12500,
  "output_artifacts": [
    {
      "type": "story_bible",
      "id": "story-uuid",
      "version": 1
    },
    {
      "type": "character_bible",
      "id": "char-uuid-1",
      "version": 1,
      "character_id": "protagonist"
    },
    {
      "type": "script_draft",
      "id": "script-uuid",
      "version": 1
    }
  ],
  "model_call_id": "model-call-uuid",
  "next_role": "director"
}
```

**Error Response** (400 Bad Request):
```json
{
  "error": "missing_input_artifact",
  "message": "Role 'director' requires 'script_draft' but it is not available",
  "required_artifacts": ["story_bible", "script_draft"]
}
```

---

### 2.2 批量执行角色流程

**Endpoint**: `POST /productions/:id/execute-flow`

**Request**:
```json
{
  "roles": ["producer", "screenwriter", "director"],
  "auto_approve": false, // 是否自动批准中间产物
  "user_input": "制作一个60秒的产品宣传视频"
}
```

**Response** (202 Accepted):
```json
{
  "flow_id": "flow-uuid",
  "status": "running",
  "completed_roles": [],
  "current_role": "producer",
  "pending_roles": ["screenwriter", "director"]
}
```

---

### 2.3 查询流程状态

**Endpoint**: `GET /productions/:id/flows/:flow_id`

**Response** (200 OK):
```json
{
  "flow_id": "flow-uuid",
  "status": "waiting_approval", // running | waiting_approval | completed | failed
  "completed_roles": ["producer", "screenwriter"],
  "current_role": null,
  "pending_roles": ["director"],
  "waiting_for": {
    "type": "script_approval_gate",
    "artifact_id": "script-uuid"
  }
}
```

---

## 3. 产物管理

### 3.1 获取特定类型产物

**Endpoint**: `GET /productions/:id/artifacts/:artifact_type`

**artifact_type**: `creative_brief` | `story_bible` | `character_bible` | `script_draft` | `directorial_treatment` | `shot_contract` | `performance_brief` | `sound_plan` | `continuity_ledger` | `take_review`

**Query Parameters**:
- `version`: 版本号（可选，默认返回最新 approved 或 draft）
- `character_id`: 角色ID（仅用于 `character_bible`、`performance_brief`）
- `shot_id`: 镜头ID（仅用于 `shot_contract`、`continuity_ledger`、`take_review`）

**Response** (200 OK):
```json
{
  "id": "artifact-uuid",
  "production_project_id": "550e8400-e29b-41d4-a716-446655440000",
  "artifact_type": "script_draft",
  "version": 1,
  "status": "approved",
  "content": {
    "scenes": [
      {
        "scene_id": "scene_001",
        "location": "健身房",
        "time_of_day": "清晨",
        "description": "阳光透过落地窗洒进现代化健身房",
        "beats": [
          {
            "type": "action",
            "content": "主角戴着智能手表进行晨跑"
          }
        ]
      }
    ]
  },
  "created_by": "screenwriter",
  "approved_by": "user-uuid",
  "approved_at": "2026-07-27T10:35:00Z",
  "created_at": "2026-07-27T10:32:00Z",
  "updated_at": "2026-07-27T10:35:00Z"
}
```

---

### 3.2 批准产物

**Endpoint**: `POST /productions/:id/artifacts/:artifact_type/:artifact_id/approve`

**Response** (200 OK):
```json
{
  "id": "artifact-uuid",
  "status": "approved",
  "approved_by": "user-uuid",
  "approved_at": "2026-07-27T10:35:00Z"
}
```

---

### 3.3 列出所有角色圣经（示例）

**Endpoint**: `GET /productions/:id/artifacts/character_bible/all`

**Response** (200 OK):
```json
{
  "items": [
    {
      "id": "char-uuid-1",
      "character_id": "protagonist",
      "version": 1,
      "status": "approved",
      "content": {
        "name": "李明",
        "archetype": "进取的都市白领"
      }
    },
    {
      "id": "char-uuid-2",
      "character_id": "coach",
      "version": 1,
      "status": "draft"
    }
  ]
}
```

---

### 3.4 列出所有镜头合约（示例）

**Endpoint**: `GET /productions/:id/artifacts/shot_contract/all`

**Response** (200 OK):
```json
{
  "items": [
    {
      "id": "shot-uuid-1",
      "shot_id": "shot_001",
      "scene_id": "scene_001",
      "version": 1,
      "status": "approved",
      "content": {
        "shot_type": "wide",
        "duration_seconds": 5
      }
    }
  ]
}
```

---

## 4. 协作建议

### 4.1 提交修改建议

**Endpoint**: `POST /productions/:id/suggestions`

**Request**:
```json
{
  "from_role": "director",
  "to_role": "screenwriter",
  "artifact_type": "script_draft",
  "artifact_id": "script-uuid",
  "suggestion_type": "revision",
  "content": {
    "reason": "需要增加更多视觉冲击力的场景",
    "specific_change": "在 scene_001 中增加慢动作特写镜头",
    "priority": "high"
  }
}
```

**Response** (201 Created):
```json
{
  "id": "suggestion-uuid",
  "production_project_id": "550e8400-e29b-41d4-a716-446655440000",
  "from_role": "director",
  "to_role": "screenwriter",
  "status": "pending",
  "created_at": "2026-07-27T10:40:00Z"
}
```

---

### 4.2 响应修改建议

**Endpoint**: `POST /productions/:id/suggestions/:suggestion_id/respond`

**Request**:
```json
{
  "status": "accepted", // "accepted" | "rejected"
  "response_note": "已采纳，将在下一版本中修改"
}
```

**Response** (200 OK):
```json
{
  "id": "suggestion-uuid",
  "status": "accepted",
  "responded_by": "user-uuid",
  "responded_at": "2026-07-27T10:45:00Z",
  "response_note": "已采纳，将在下一版本中修改"
}
```

---

### 4.3 列出待处理建议

**Endpoint**: `GET /productions/:id/suggestions`

**Query Parameters**:
- `to_role`: 过滤目标角色（可选）
- `status`: 过滤状态（可选）

**Response** (200 OK):
```json
{
  "items": [
    {
      "id": "suggestion-uuid",
      "from_role": "director",
      "to_role": "screenwriter",
      "artifact_type": "script_draft",
      "status": "pending",
      "content": {
        "reason": "需要增加更多视觉冲击力的场景",
        "priority": "high"
      },
      "created_at": "2026-07-27T10:40:00Z"
    }
  ],
  "total": 1
}
```

---

## 5. Fast Lane

### 5.1 快速生成

**Endpoint**: `POST /productions/:id/fast-lane`

**Request**:
```json
{
  "prompt": "制作一个30秒的智能手表健身场景视频",
  "platform": "douyin",
  "duration_seconds": 30
}
```

**Response** (202 Accepted):
```json
{
  "job_id": "job-uuid",
  "status": "queued",
  "estimated_time_seconds": 180
}
```

---

### 5.2 查询快速生成状态

**Endpoint**: `GET /productions/:id/fast-lane/:job_id`

**Response** (200 OK):
```json
{
  "job_id": "job-uuid",
  "status": "completed", // queued | processing | completed | failed
  "video_url": "https://cdn.example.com/videos/output.mp4",
  "completed_at": "2026-07-27T10:50:00Z"
}
```

---

## 错误响应

所有接口遵循统一错误格式：

```json
{
  "error": "error_code",
  "message": "Human-readable error message",
  "details": {
    "field": "additional context"
  }
}
```

常见错误码：
- `missing_input_artifact`: 缺少必需的输入产物
- `invalid_role_sequence`: 角色执行顺序不合法
- `gate_rejected`: 质量闸门拦截
- `budget_exceeded`: 预算超限
- `artifact_not_found`: 产物不存在
- `invalid_artifact_schema`: 产物结构不符合 schema
- `unauthorized`: 未授权
- `project_not_found`: 项目不存在

---

## 分页规范

所有列表接口支持分页：

**Query Parameters**:
- `page`: 页码（从 1 开始，默认 1）
- `page_size`: 每页数量（默认 20，最大 100）

**Response**:
```json
{
  "items": [...],
  "total": 100,
  "page": 1,
  "page_size": 20
}
```

---

## 审计与追溯

所有角色执行和产物修改自动生成审计日志，可通过以下接口查询：

**Endpoint**: `GET /productions/:id/audit-log`

**Response** (200 OK):
```json
{
  "items": [
    {
      "event_type": "role_executed",
      "role": "screenwriter",
      "model_call_id": "model-call-uuid",
      "timestamp": "2026-07-27T10:32:00Z"
    },
    {
      "event_type": "artifact_approved",
      "artifact_type": "script_draft",
      "artifact_id": "script-uuid",
      "approved_by": "user-uuid",
      "timestamp": "2026-07-27T10:35:00Z"
    }
  ]
}
```
