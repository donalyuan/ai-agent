# 数据库设计

## 概述

脚本Agent需要两张核心表：`scripts`（脚本主表）和`scenes`（分镜表）。

## 表结构

### scripts表

存储视频脚本的核心信息。

```sql
CREATE TABLE scripts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title VARCHAR(200) NOT NULL,
    hook TEXT NOT NULL,
    content JSONB NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'draft',
    parent_id UUID REFERENCES scripts(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT check_status CHECK (status IN ('draft', 'approved', 'archived'))
);

-- 索引
CREATE INDEX idx_scripts_project ON scripts(project_id);
CREATE INDEX idx_scripts_status ON scripts(status);
CREATE INDEX idx_scripts_parent ON scripts(parent_id) WHERE parent_id IS NOT NULL;
CREATE INDEX idx_scripts_created ON scripts(created_at DESC);

-- 触发器：自动更新updated_at
CREATE OR REPLACE FUNCTION update_scripts_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_scripts_updated_at
    BEFORE UPDATE ON scripts
    FOR EACH ROW
    EXECUTE FUNCTION update_scripts_updated_at();
```

**字段说明**：

| 字段 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | UUID | PK | 脚本ID |
| `project_id` | UUID | FK, NOT NULL | 所属项目ID |
| `title` | VARCHAR(200) | NOT NULL | 视频标题 |
| `hook` | TEXT | NOT NULL | 前3秒吸引点 |
| `content` | JSONB | NOT NULL | 完整脚本内容（元数据） |
| `status` | VARCHAR(20) | NOT NULL | 状态：draft/approved/archived |
| `parent_id` | UUID | FK, NULLABLE | 父脚本ID（A/B测试） |
| `created_at` | TIMESTAMPTZ | NOT NULL | 创建时间 |
| `updated_at` | TIMESTAMPTZ | NOT NULL | 更新时间 |

**content字段结构**（JSONB）：

```json
{
  "topic": "原始选题文本",
  "style": "knowledge",
  "total_duration_sec": 58,
  "metadata": {
    "llm_model": "gpt-4-turbo",
    "generation_time_ms": 3245,
    "retry_count": 0
  }
}
```

**索引说明**：
- `idx_scripts_project`: 按项目查询脚本列表
- `idx_scripts_status`: 按状态过滤（如只看approved）
- `idx_scripts_parent`: 查询A/B测试版本链
- `idx_scripts_created`: 按时间倒序分页

---

### scenes表

存储脚本的分镜详情。

```sql
CREATE TABLE scenes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    script_id UUID NOT NULL REFERENCES scripts(id) ON DELETE CASCADE,
    sequence INT NOT NULL,
    narration TEXT NOT NULL,
    visual_description TEXT NOT NULL,
    emotion VARCHAR(50) NOT NULL,
    duration_sec INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT check_sequence CHECK (sequence > 0 AND sequence <= 20),
    CONSTRAINT check_duration CHECK (duration_sec > 0 AND duration_sec <= 30),
    UNIQUE(script_id, sequence)
);

-- 索引
CREATE INDEX idx_scenes_script ON scenes(script_id, sequence);
```

**字段说明**：

| 字段 | 类型 | 约束 | 说明 |
|------|------|------|------|
| `id` | UUID | PK | 分镜ID |
| `script_id` | UUID | FK, NOT NULL | 所属脚本ID |
| `sequence` | INT | NOT NULL, 1-20 | 分镜顺序 |
| `narration` | TEXT | NOT NULL | 旁白文本 |
| `visual_description` | TEXT | NOT NULL | 视觉描述 |
| `emotion` | VARCHAR(50) | NOT NULL | 情绪标签 |
| `duration_sec` | INT | NOT NULL, 1-30 | 时长（秒） |
| `created_at` | TIMESTAMPTZ | NOT NULL | 创建时间 |

**约束说明**：
- `UNIQUE(script_id, sequence)`: 同一脚本的分镜序号不能重复
- `check_sequence`: 分镜序号必须在1-20之间
- `check_duration`: 单个分镜时长不超过30秒

**emotion枚举值建议**：
- 兴奋 (excited)
- 焦虑 (anxious)
- 好奇 (curious)
- 惊喜 (surprised)
- 平静 (calm)
- 紧张 (tense)
- 感动 (touched)
- 幽默 (humorous)

---

## 数据关系

```
projects (1) ──────< (N) scripts
                      │
                      ├─ parent_id (self-reference)
                      │
scripts (1) ──────< (N) scenes
```

**级联删除**：
- 删除project → 级联删除所有scripts → 级联删除所有scenes
- 删除script → 级联删除所有scenes
- 删除父script → 子script的parent_id设置为NULL

---

## 查询模式

### 1. 生成并保存脚本

```sql
-- 插入脚本
INSERT INTO scripts (project_id, title, hook, content, status)
VALUES ($1, $2, $3, $4, 'draft')
RETURNING id, created_at;

-- 批量插入分镜
INSERT INTO scenes (script_id, sequence, narration, visual_description, emotion, duration_sec)
VALUES 
    ($1, 1, $2, $3, $4, $5),
    ($1, 2, $6, $7, $8, $9),
    ...
RETURNING id;
```

### 2. 查询脚本及分镜

```sql
-- 查询脚本基本信息
SELECT 
    s.id, s.project_id, s.title, s.hook, s.content, s.status, 
    s.parent_id, s.created_at, s.updated_at,
    COUNT(sc.id) as scene_count
FROM scripts s
LEFT JOIN scenes sc ON s.id = sc.script_id
WHERE s.id = $1
GROUP BY s.id;

-- 查询脚本的所有分镜（按顺序）
SELECT 
    id, sequence, narration, visual_description, emotion, duration_sec
FROM scenes
WHERE script_id = $1
ORDER BY sequence ASC;
```

### 3. 列出项目的脚本

```sql
SELECT 
    s.id, s.title, s.status, s.parent_id, s.created_at,
    COUNT(sc.id) as scene_count,
    SUM(sc.duration_sec) as total_duration
FROM scripts s
LEFT JOIN scenes sc ON s.id = sc.script_id
WHERE s.project_id = $1
  AND ($2::VARCHAR IS NULL OR s.status = $2)  -- 可选的状态过滤
GROUP BY s.id
ORDER BY s.created_at DESC
LIMIT $3 OFFSET $4;
```

### 4. 查询A/B测试版本

```sql
-- 查询某个脚本的所有变体版本
SELECT 
    id, title, status, created_at
FROM scripts
WHERE parent_id = $1
ORDER BY created_at ASC;

-- 查询某个脚本所属的版本树（递归）
WITH RECURSIVE version_tree AS (
    -- 找到根节点
    SELECT id, parent_id, title, 0 as level
    FROM scripts
    WHERE id = $1 AND parent_id IS NULL
    
    UNION
    
    SELECT s.id, s.parent_id, s.title, vt.level + 1
    FROM scripts s
    INNER JOIN version_tree vt ON s.parent_id = vt.id
)
SELECT * FROM version_tree ORDER BY level, created_at;
```

### 5. 更新脚本状态

```sql
UPDATE scripts
SET status = $2
WHERE id = $1
RETURNING id, status, updated_at;
```

### 6. 删除脚本

```sql
-- 检查是否被视频引用（假设未来有videos表）
-- SELECT EXISTS(SELECT 1 FROM videos WHERE script_id = $1);

-- 删除脚本（会级联删除scenes）
DELETE FROM scripts WHERE id = $1;
```

---

## Migration脚本

### 001_create_scripts_and_scenes.sql

```sql
-- Create scripts table
CREATE TABLE scripts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title VARCHAR(200) NOT NULL,
    hook TEXT NOT NULL,
    content JSONB NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'draft',
    parent_id UUID REFERENCES scripts(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT check_status CHECK (status IN ('draft', 'approved', 'archived'))
);

-- Create scenes table
CREATE TABLE scenes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    script_id UUID NOT NULL REFERENCES scripts(id) ON DELETE CASCADE,
    sequence INT NOT NULL,
    narration TEXT NOT NULL,
    visual_description TEXT NOT NULL,
    emotion VARCHAR(50) NOT NULL,
    duration_sec INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT check_sequence CHECK (sequence > 0 AND sequence <= 20),
    CONSTRAINT check_duration CHECK (duration_sec > 0 AND duration_sec <= 30),
    UNIQUE(script_id, sequence)
);

-- Indexes for scripts
CREATE INDEX idx_scripts_project ON scripts(project_id);
CREATE INDEX idx_scripts_status ON scripts(status);
CREATE INDEX idx_scripts_parent ON scripts(parent_id) WHERE parent_id IS NOT NULL;
CREATE INDEX idx_scripts_created ON scripts(created_at DESC);

-- Indexes for scenes
CREATE INDEX idx_scenes_script ON scenes(script_id, sequence);

-- Trigger for updated_at
CREATE OR REPLACE FUNCTION update_scripts_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_scripts_updated_at
    BEFORE UPDATE ON scripts
    FOR EACH ROW
    EXECUTE FUNCTION update_scripts_updated_at();
```

### Rollback脚本

```sql
DROP TRIGGER IF EXISTS trigger_scripts_updated_at ON scripts;
DROP FUNCTION IF EXISTS update_scripts_updated_at();
DROP TABLE IF EXISTS scenes;
DROP TABLE IF EXISTS scripts;
```

---

## 数据量估算

**假设**：
- 每个项目平均10个脚本
- 每个脚本6个分镜
- 100个活跃项目

**估算**：
- scripts表：100 × 10 = 1,000行
- scenes表：1,000 × 6 = 6,000行

**存储估算**（单行）：
- scripts：~500 bytes（UUID + 文本 + JSONB）
- scenes：~400 bytes（UUID + 文本）

**总存储**：
- scripts：1,000 × 500 bytes ≈ 0.5 MB
- scenes：6,000 × 400 bytes ≈ 2.4 MB
- 索引：~1 MB

**总计**：~4 MB（MVP规模可忽略不计）

---

## 性能优化

### 查询优化
1. 使用复合索引 `(script_id, sequence)` 加速分镜排序查询
2. `parent_id` 使用部分索引（WHERE parent_id IS NOT NULL）减少索引空间
3. `created_at DESC` 索引支持分页查询

### 写入优化
1. 批量插入scenes使用单条SQL
2. 使用事务确保script和scenes原子性

### JSONB优化
1. `content` 字段使用GIN索引（如需按JSONB字段查询）
2. 避免在JSONB中存储大量数据（当前只存元数据）

---

## 扩展预留

### 未来可能新增的字段

**scripts表**：
- `language`: 脚本语言（zh-CN/en-US）
- `target_platform`: 目标平台（douyin/xiaohongshu）
- `tone`: 语气风格（professional/casual/humorous）
- `version`: 版本号（用于更精细的版本管理）

**scenes表**：
- `material_id`: 关联的素材ID（预先指定素材）
- `transition`: 转场效果（fade/cut/slide）
- `background_music`: 背景音乐标签

### 未来可能新增的表
- `script_feedback`: 脚本反馈表（用户评分、修改建议）
- `script_analytics`: 脚本分析表（A/B测试结果统计）
