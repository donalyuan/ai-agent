# Full Crew Durable API

本文件记录本 change 完成后的后端命令契约。Full Crew 只能使用 `/api/v1/production/intents` 与 `/api/v1/production/runs` 下的持久化接口；旧 `/productions` 创建入口只保留 Fast Lane。

## 通用约束

- 除 GET 外，所有命令必须携带非空 `Idempotency-Key` 请求头。
- 服务端固定使用 `actor_type=local_operator`；请求体不接受 `user_id`。
- DTO 使用 `deny_unknown_fields`，拒绝 `roles`、`auto_approve`、`skip_gates`、`plan_version`、`preferred_model_id`、任意 `context` 和未声明 `user_input`。
- `202 Accepted` 只表示命令已持久化，不表示 Run、Step、模型调用或作品任务已成功。
- 同一 actor、命令、聚合与 key 的同 digest 请求返回原结果；不同 digest 返回 `idempotency_conflict`。

## 接口

| 方法 | 路径 | 请求体 | 成功响应 |
|---|---|---|---|
| POST | `/api/v1/production/intents` | `project_id`、`topic_id`、`title`、可选 `description`、`initial_input` | `201`，持久化 `intent` |
| POST | `/api/v1/production/intents/:intent_id/runs` | `{}` | `202`，固定计划的 `run` |
| GET | `/api/v1/production/intents/:intent_id` | 无 | `200`，`intent` |
| DELETE | `/api/v1/production/intents/:intent_id` | `{}` | 仅空白意图返回 `204` |
| POST | `/api/v1/production/intents/:intent_id/archive` | `{}` | `200`，保留审计的归档 `intent` |
| GET | `/api/v1/production/runs/:run_id` | 无 | `200`，Run、Steps、Packages、Gate、资源、等待原因、允许命令和审计引用 |
| POST | `/api/v1/production/runs/:run_id/cancel` | `reason` | `202`，已持久化取消意图的 `run` |
| POST | `/api/v1/production/runs/:run_id/packages/:digest/approve` | 可选 `note` | `200`，不可变 GateDecision |
| POST | `/api/v1/production/runs/:run_id/packages/:digest/reject` | 非空 `reason`、非空 `affected_owners` | `200`，不可变 GateDecision 与新 revision epoch |
| POST | `/api/v1/production/runs/:run_id/resume` | `{}` | `202`，`run_id` 与已唤醒 `step_ids` |
| POST | `/api/v1/production/runs/:run_id/steps/:step_id/retry` | `{}` | `202`，显式新 attempt 的 `run_id/step_id` |

## 错误语义

| HTTP | 稳定错误类别 |
|---|---|
| `404` | 制作意图、产物、建议或角色不存在 |
| `409` | `source_locked`、`active_intent_conflict`、`run_already_exists`、`idempotency_conflict`、`transition_conflict`、`stale_package`、`attention_required`、`external_wait`、Gate 等待或拒绝 |
| `422` | 来源无效、能力/证据阻断、schema 或输入产物无效 |
| `429` | `resource_limit`，且副作用尚未发出 |
| `502` | 已审计的模型执行失败 |

错误响应统一包含 `error`、`message` 和可选脱敏 `details`，不得返回凭据、原始请求头、长期签名 URL 或金额字段。
