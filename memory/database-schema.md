---
name: database-schema
description: 简化版数据库表结构（无权限系统）
metadata:
  node_type: memory
  type: reference
  originSessionId: 322147b7-81d9-49fa-b62d-971d5fa7a0f8
---

# 数据库表结构

## 设计原则
- **极简优先**：MVP不做权限，不加tenant_id
- **JSONB灵活**：复杂结构用JSONB，后续可调整
- **索引克制**：只加查询热点字段索引
- **外键强制**：保证数据一致性

## 核心表（11张）

### 项目管理（2张）
- **projects**: 内容项目
- **accounts**: 平台账号（抖音/小红书）

### 素材库（2张）
- **materials**: 素材文件
- **material_embeddings**: 向量索引（关联Milvus）

### 内容生产（2张）
- **scripts**: 脚本
- **scenes**: 分镜

### 视频系统（2张）
- **generation_tasks**: 生成任务（异步）
- **videos**: 最终视频

### 发布系统（3张）
- **publish_tasks**: 发布任务
- **metrics**: 数据指标（时序）
- **revenues**: 收益记录

## Agent系统（2张）
- **agent_runs**: Agent运行记录（6种Agent）
- **agent_steps**: Agent步骤（调试用）

## 爆款分析（2张）
- **viral_videos**: 爆款视频库
- **content_strategies**: 内容策略

## 关键字段说明

### JSONB字段
- `accounts.credentials`: 平台凭据（Cookie/Token）
- `generation_tasks.params`: 视频生成参数
- `agent_runs.input/output`: Agent输入输出
- `viral_videos.analysis`: LLM分析结果

### 状态字段
- `scripts.status`: draft, approved, archived
- `generation_tasks.status`: pending, processing, completed, failed
- `videos.status`: draft, ready, published
- `publish_tasks.status`: pending, scheduled, published, failed

### 关联关系
```
projects
  ├─> accounts (1:N)
  ├─> materials (1:N)
  ├─> scripts (1:N)
  └─> videos (1:N)

scripts
  ├─> scenes (1:N)
  ├─> generation_tasks (1:N)
  └─> videos (1:1)

videos
  └─> publish_tasks (1:N)
      ├─> metrics (1:N)
      └─> revenues (1:N)
```

## 索引策略
```sql
-- 项目关联查询
CREATE INDEX idx_materials_project ON materials(project_id);
CREATE INDEX idx_scripts_project ON scripts(project_id);

-- 分镜顺序
CREATE INDEX idx_scenes_script ON scenes(script_id);

-- 任务队列扫描
CREATE INDEX idx_generation_tasks_status ON generation_tasks(status);
CREATE INDEX idx_publish_tasks_status ON publish_tasks(status);

-- Agent类型分组
CREATE INDEX idx_agent_runs_type ON agent_runs(agent_type);
```

## 数据迁移
使用SQLx迁移：
```bash
sqlx migrate add initial
sqlx migrate run
```

## 后续扩展预留
- **权限字段**：后续加tenant_id, owner_id, visibility
- **审计日志**：后续加created_by, updated_by
- **软删除**：后续加deleted_at
- **版本控制**：scripts已有parent_id支持A/B测试

**Why**: 完整的数据库设计参考，开发时直接使用。

**How to apply**:
- 运行migrations/初始化脚本创建表
- 新增业务先看能否复用现有表
- JSONB字段先用，稳定后再拆成独立表
- [[project-tech-stack]] 配合使用
