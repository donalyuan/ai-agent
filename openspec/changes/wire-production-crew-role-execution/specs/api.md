# 接口规格：角色执行管道集成

## 修改的 API 端点

### POST /api/v1/production/productions/:id/roles/:role_key/execute

当前状态：返回 `NOT_IMPLEMENTED`（501）

目标状态：真实执行 AI 角色，产出结构化产物。

**成功响应（200 OK）**：
```json
{
  "role": "producer",
  "status": "completed",
  "execution_time_ms": 3200,
  "output_artifacts": [
    {
      "type": "creative_brief",
      "id": "artifact-uuid",
      "version": 1
    }
  ],
  "model_call_id": "model-call-uuid",
  "next_role": "screenwriter"
}
```

**错误响应 - 输入产物缺失（400 Bad Request）**：
```json
{
  "error": "missing_input_artifact",
  "message": "角色 'director' 需要 'script_draft'，但当前无已批准的版本",
  "required_artifacts": ["story_bible", "script_draft"]
}
```

**错误响应 - 产物 schema 无效（422 Unprocessable Entity）**：
```json
{
  "error": "invalid_artifact_schema",
  "message": "Producer 输出不符合 creative_brief schema：target_audience 不能为空",
  "details": "creative_brief: target_audience is required"
}
```

**错误响应 - Gate 等待审批（409 Conflict）**：
```json
{
  "error": "waiting_approval",
  "message": "ScriptApprovalGate: 需要批准 ScriptDraft 才能继续",
  "artifact_id": "script-uuid"
}
```

---

## 新增 AgentDefinition 规格

每个制作角色对应一个 `AgentDefinition`，遵循现有 `registry.json` 格式：

| agent_key | node_key | 输出 schema |
|---|---|---|
| `production.producer` | `production.producer.execute` | creative_brief |
| `production.screenwriter` | `production.screenwriter.execute` | story_bible, character_bible[], script_draft |
| `production.director` | `production.director.execute` | directorial_treatment, shot_contract[] |
| `production.cinematographer` | `production.cinematographer.execute` | collaboration_suggestions[] |
| `production.performance_director` | `production.performance_director.execute` | performance_brief[] |
| `production.sound_director` | `production.sound_director.execute` | sound_plan |
| `production.editor` | `production.editor.execute` | continuity_ledger[] |
| `production.qc` | `production.qc.execute` | take_review[] |
| `production.character_critic` | `production.character_critic.execute` | collaboration_suggestions[] |

所有角色：`executor_owner: rust`，`structured_output: true`，`tool_calling: false`

---

## 约束

1. `agent_key` 必须以 `production.` 前缀区分制作团队角色
2. `node_key` 格式：`{agent_key}.execute`（单节点，无多节点分支）
3. 模型选择：从 `ProductionProject.metadata.preferred_model_id` 取；若未设置，从 `AppConfig` 默认值取；若仍未配置，返回 `500 internal_error`
4. `AuditedCallOwner`：使用 `AuditedCallOwner::AgentRun(run_id)`，每次角色执行创建一个 Run 记录
5. 每个角色执行产出的产物 status 默认为 `draft`，不自动 approve

---

## 非目标

- 不改变现有 `GET /productions/:id/artifacts/*` 接口
- 不实现 `execute_flow` 的实际顺序编排
- 不支持 streaming 输出
