# ProductionState 数据规格

## 数据库 Schema

### production_projects

制作项目主表。

```sql
CREATE TABLE production_projects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title VARCHAR(255) NOT NULL,
    description TEXT,
    project_type VARCHAR(50) NOT NULL, -- 'fast_lane' | 'full_crew'
    status VARCHAR(50) NOT NULL DEFAULT 'created',
    -- 状态流转: created -> briefing -> scripting -> directing -> 
    --          generating -> editing -> qc -> approved -> published
    user_id UUID NOT NULL,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE
);

CREATE INDEX idx_production_projects_user_id ON production_projects(user_id);
CREATE INDEX idx_production_projects_status ON production_projects(status);
CREATE INDEX idx_production_projects_type ON production_projects(project_type);
```

---

### creative_briefs

创意简报，制片人产出。

```sql
CREATE TABLE creative_briefs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    production_project_id UUID NOT NULL REFERENCES production_projects(id) ON DELETE CASCADE,
    version INT NOT NULL DEFAULT 1,
    status VARCHAR(50) NOT NULL DEFAULT 'draft', -- draft | approved | superseded
    content JSONB NOT NULL,
    -- content schema:
    -- {
    --   "target_audience": string,
    --   "tone": string[],
    --   "key_messages": string[],
    --   "constraints": {
    --     "budget"?: number,
    --     "duration_seconds"?: number,
    --     "platform"?: string[]
    --   },
    --   "success_criteria": string[]
    -- }
    created_by VARCHAR(50) NOT NULL DEFAULT 'producer',
    approved_by UUID,
    approved_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(production_project_id, version)
);

CREATE INDEX idx_creative_briefs_project ON creative_briefs(production_project_id);
```

---

### story_bibles

故事圣经，编剧产出。

```sql
CREATE TABLE story_bibles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    production_project_id UUID NOT NULL REFERENCES production_projects(id) ON DELETE CASCADE,
    version INT NOT NULL DEFAULT 1,
    status VARCHAR(50) NOT NULL DEFAULT 'draft',
    content JSONB NOT NULL,
    -- content schema:
    -- {
    --   "premise": string,
    --   "themes": string[],
    --   "world_rules": string[],
    --   "narrative_arc": {
    --     "setup": string,
    --     "conflict": string,
    --     "climax": string,
    --     "resolution": string
    --   }
    -- }
    created_by VARCHAR(50) NOT NULL DEFAULT 'screenwriter',
    approved_by UUID,
    approved_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(production_project_id, version)
);

CREATE INDEX idx_story_bibles_project ON story_bibles(production_project_id);
```

---

### character_bibles

角色圣经，编剧产出，可有多个角色。

```sql
CREATE TABLE character_bibles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    production_project_id UUID NOT NULL REFERENCES production_projects(id) ON DELETE CASCADE,
    character_id VARCHAR(100) NOT NULL, -- 角色唯一标识
    version INT NOT NULL DEFAULT 1,
    status VARCHAR(50) NOT NULL DEFAULT 'draft',
    content JSONB NOT NULL,
    -- content schema:
    -- {
    --   "name": string,
    --   "archetype": string,
    --   "personality_traits": string[],
    --   "background": string,
    --   "goals": string[],
    --   "fears": string[],
    --   "speech_patterns": string[],
    --   "visual_description": string
    -- }
    created_by VARCHAR(50) NOT NULL DEFAULT 'screenwriter',
    approved_by UUID,
    approved_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(production_project_id, character_id, version)
);

CREATE INDEX idx_character_bibles_project ON character_bibles(production_project_id);
CREATE INDEX idx_character_bibles_character ON character_bibles(production_project_id, character_id);
```

---

### script_drafts

剧本草稿，编剧产出。

```sql
CREATE TABLE script_drafts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    production_project_id UUID NOT NULL REFERENCES production_projects(id) ON DELETE CASCADE,
    version INT NOT NULL DEFAULT 1,
    status VARCHAR(50) NOT NULL DEFAULT 'draft',
    content JSONB NOT NULL,
    -- content schema:
    -- {
    --   "scenes": [
    --     {
    --       "scene_id": string,
    --       "location": string,
    --       "time_of_day": string,
    --       "description": string,
    --       "beats": [
    --         {
    --           "type": "action" | "dialogue",
    --           "character_id"?: string,
    --           "content": string
    --         }
    --       ]
    --     }
    --   ]
    -- }
    created_by VARCHAR(50) NOT NULL DEFAULT 'screenwriter',
    approved_by UUID,
    approved_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(production_project_id, version)
);

CREATE INDEX idx_script_drafts_project ON script_drafts(production_project_id);
```

---

### directorial_treatments

导演阐述，导演产出。

```sql
CREATE TABLE directorial_treatments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    production_project_id UUID NOT NULL REFERENCES production_projects(id) ON DELETE CASCADE,
    version INT NOT NULL DEFAULT 1,
    status VARCHAR(50) NOT NULL DEFAULT 'draft',
    content JSONB NOT NULL,
    -- content schema:
    -- {
    --   "visual_style": string,
    --   "color_palette": string[],
    --   "pacing": string,
    --   "mood": string,
    --   "reference_works": string[]
    -- }
    created_by VARCHAR(50) NOT NULL DEFAULT 'director',
    approved_by UUID,
    approved_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(production_project_id, version)
);

CREATE INDEX idx_directorial_treatments_project ON directorial_treatments(production_project_id);
```

---

### shot_contracts

镜头合约，导演产出，包含多个镜头。

```sql
CREATE TABLE shot_contracts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    production_project_id UUID NOT NULL REFERENCES production_projects(id) ON DELETE CASCADE,
    shot_id VARCHAR(100) NOT NULL, -- 镜头唯一标识
    scene_id VARCHAR(100) NOT NULL, -- 关联场景
    version INT NOT NULL DEFAULT 1,
    status VARCHAR(50) NOT NULL DEFAULT 'draft',
    content JSONB NOT NULL,
    -- content schema:
    -- {
    --   "shot_type": string, -- "wide" | "medium" | "close-up" | "extreme-close-up"
    --   "camera_movement": string, -- "static" | "pan" | "tilt" | "dolly" | "tracking"
    --   "composition": string,
    --   "lighting": string,
    --   "duration_seconds": number,
    --   "dialogue": string[],
    --   "action_beats": string[],
    --   "continuity_refs": string[] -- 引用其他镜头ID以保持连续性
    -- }
    created_by VARCHAR(50) NOT NULL DEFAULT 'director',
    approved_by UUID,
    approved_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(production_project_id, shot_id, version)
);

CREATE INDEX idx_shot_contracts_project ON shot_contracts(production_project_id);
CREATE INDEX idx_shot_contracts_scene ON shot_contracts(production_project_id, scene_id);
CREATE INDEX idx_shot_contracts_shot ON shot_contracts(production_project_id, shot_id);
```

---

### performance_briefs

表演简报，表演指导产出，按角色分。

```sql
CREATE TABLE performance_briefs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    production_project_id UUID NOT NULL REFERENCES production_projects(id) ON DELETE CASCADE,
    character_id VARCHAR(100) NOT NULL,
    version INT NOT NULL DEFAULT 1,
    status VARCHAR(50) NOT NULL DEFAULT 'draft',
    content JSONB NOT NULL,
    -- content schema:
    -- {
    --   "emotional_arc": [
    --     {
    --       "scene_id": string,
    --       "emotion": string,
    --       "intensity": number, -- 1-10
    --       "notes": string
    --     }
    --   ],
    --   "body_language": string,
    --   "vocal_direction": string,
    --   "key_moments": string[]
    -- }
    created_by VARCHAR(50) NOT NULL DEFAULT 'performance_director',
    approved_by UUID,
    approved_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(production_project_id, character_id, version)
);

CREATE INDEX idx_performance_briefs_project ON performance_briefs(production_project_id);
CREATE INDEX idx_performance_briefs_character ON performance_briefs(production_project_id, character_id);
```

---

### sound_plans

声音计划，声音指导产出。

```sql
CREATE TABLE sound_plans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    production_project_id UUID NOT NULL REFERENCES production_projects(id) ON DELETE CASCADE,
    version INT NOT NULL DEFAULT 1,
    status VARCHAR(50) NOT NULL DEFAULT 'draft',
    content JSONB NOT NULL,
    -- content schema:
    -- {
    --   "music_style": string,
    --   "music_cues": [
    --     {
    --       "scene_id": string,
    --       "timing": string,
    --       "mood": string,
    --       "description": string
    --     }
    --   ],
    --   "sound_effects": [
    --     {
    --       "scene_id": string,
    --       "description": string,
    --       "purpose": string
    --     }
    --   ],
    --   "dialogue_recording": {
    --     "style": string, -- "voiceover" | "sync-sound"
    --     "voice_direction": string
    --   }
    -- }
    created_by VARCHAR(50) NOT NULL DEFAULT 'sound_director',
    approved_by UUID,
    approved_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(production_project_id, version)
);

CREATE INDEX idx_sound_plans_project ON sound_plans(production_project_id);
```

---

### continuity_ledgers

连续性台账，剪辑师维护。

```sql
CREATE TABLE continuity_ledgers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    production_project_id UUID NOT NULL REFERENCES production_projects(id) ON DELETE CASCADE,
    shot_id VARCHAR(100) NOT NULL,
    content JSONB NOT NULL,
    -- content schema:
    -- {
    --   "visual_facts": {
    --     "costumes": string[],
    --     "props": string[],
    --     "scene_details": string[],
    --     "lighting_state": string,
    --     "weather": string
    --   },
    --   "temporal_position": string, -- 时间线位置
    --   "continuity_constraints": string[] -- 后续镜头必须遵守的约束
    -- }
    created_by VARCHAR(50) NOT NULL DEFAULT 'editor',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(production_project_id, shot_id)
);

CREATE INDEX idx_continuity_ledgers_project ON continuity_ledgers(production_project_id);
CREATE INDEX idx_continuity_ledgers_shot ON continuity_ledgers(production_project_id, shot_id);
```

---

### take_reviews

镜头评审，QC 产出。

```sql
CREATE TABLE take_reviews (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    production_project_id UUID NOT NULL REFERENCES production_projects(id) ON DELETE CASCADE,
    shot_id VARCHAR(100) NOT NULL,
    take_number INT NOT NULL, -- 第几次生成
    status VARCHAR(50) NOT NULL, -- 'approved' | 'rejected' | 'needs_revision'
    content JSONB NOT NULL,
    -- content schema:
    -- {
    --   "contract_compliance": {
    --     "met": boolean,
    --     "issues": string[]
    --   },
    --   "continuity_compliance": {
    --     "met": boolean,
    --     "violations": string[]
    --   },
    --   "quality_assessment": {
    --     "visual": number, -- 1-10
    --     "narrative": number,
    --     "technical": number,
    --     "notes": string
    --   },
    --   "revision_notes": string[]
    -- }
    created_by VARCHAR(50) NOT NULL DEFAULT 'qc',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_take_reviews_project ON take_reviews(production_project_id);
CREATE INDEX idx_take_reviews_shot ON take_reviews(production_project_id, shot_id);
CREATE INDEX idx_take_reviews_status ON take_reviews(status);
```

---

### collaboration_suggestions

协作建议，角色间提出修改意见。

```sql
CREATE TABLE collaboration_suggestions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    production_project_id UUID NOT NULL REFERENCES production_projects(id) ON DELETE CASCADE,
    from_role VARCHAR(50) NOT NULL, -- 提出建议的角色
    to_role VARCHAR(50) NOT NULL, -- 目标角色
    artifact_type VARCHAR(50) NOT NULL, -- 产物类型
    artifact_id UUID NOT NULL, -- 产物ID
    suggestion_type VARCHAR(50) NOT NULL, -- 'revision' | 'addition' | 'deletion'
    content JSONB NOT NULL,
    -- content schema:
    -- {
    --   "reason": string,
    --   "specific_change": string,
    --   "priority": "low" | "medium" | "high"
    -- }
    status VARCHAR(50) NOT NULL DEFAULT 'pending', -- pending | accepted | rejected
    responded_by UUID,
    responded_at TIMESTAMP WITH TIME ZONE,
    response_note TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_collaboration_suggestions_project ON collaboration_suggestions(production_project_id);
CREATE INDEX idx_collaboration_suggestions_to_role ON collaboration_suggestions(to_role, status);
CREATE INDEX idx_collaboration_suggestions_artifact ON collaboration_suggestions(artifact_type, artifact_id);
```

---

## 数据生命周期

1. **版本管理**：每个产物表支持多版本，通过 `version` 字段递增
2. **状态流转**：`draft` → `approved` → `superseded`（当新版本被批准）
3. **软删除**：`production_projects` 支持 `deleted_at`，级联影响所有关联产物
4. **审计追溯**：所有表保留 `created_at`、`updated_at`，关键产物记录 `approved_by`、`approved_at`
5. **协作追溯**：`collaboration_suggestions` 完整记录角色间的交互历史

## 约束规则

1. 每个项目在同一产物类型下只能有一个 `approved` 版本
2. 新版本创建时，旧版本自动标记为 `superseded`
3. 删除项目时级联删除所有产物（ON DELETE CASCADE）
4. `shot_id`、`character_id` 在项目内唯一
5. 所有 JSONB 字段在应用层验证 schema，数据库层不强制
