-- Virtual Production Crew Schema
-- 虚拟制作团队数据库结构

-- ============================================================================
-- 1. Production Projects (制作项目主表)
-- ============================================================================

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
    deleted_at TIMESTAMP WITH TIME ZONE,

    CONSTRAINT production_projects_project_type_check
        CHECK (project_type IN ('fast_lane', 'full_crew')),
    CONSTRAINT production_projects_status_check
        CHECK (status IN ('created', 'briefing', 'scripting', 'directing',
                         'generating', 'editing', 'qc', 'approved', 'published', 'failed'))
);

CREATE INDEX idx_production_projects_user_id ON production_projects(user_id);
CREATE INDEX idx_production_projects_status ON production_projects(status);
CREATE INDEX idx_production_projects_type ON production_projects(project_type);
CREATE INDEX idx_production_projects_deleted_at ON production_projects(deleted_at) WHERE deleted_at IS NULL;

COMMENT ON TABLE production_projects IS '虚拟制作团队项目主表';
COMMENT ON COLUMN production_projects.project_type IS '项目类型：fast_lane(快速通道) | full_crew(完整团队)';
COMMENT ON COLUMN production_projects.status IS '项目状态流转';

-- ============================================================================
-- 2. Creative Briefs (创意简报 - 制片人产出)
-- ============================================================================

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

    CONSTRAINT creative_briefs_status_check CHECK (status IN ('draft', 'approved', 'superseded')),
    UNIQUE(production_project_id, version)
);

CREATE INDEX idx_creative_briefs_project ON creative_briefs(production_project_id);
CREATE INDEX idx_creative_briefs_status ON creative_briefs(status);

COMMENT ON TABLE creative_briefs IS '创意简报，制片人产出';

-- ============================================================================
-- 3. Story Bibles (故事圣经 - 编剧产出)
-- ============================================================================

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

    CONSTRAINT story_bibles_status_check CHECK (status IN ('draft', 'approved', 'superseded')),
    UNIQUE(production_project_id, version)
);

CREATE INDEX idx_story_bibles_project ON story_bibles(production_project_id);
CREATE INDEX idx_story_bibles_status ON story_bibles(status);

COMMENT ON TABLE story_bibles IS '故事圣经，编剧产出';

-- ============================================================================
-- 4. Character Bibles (角色圣经 - 编剧产出)
-- ============================================================================

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

    CONSTRAINT character_bibles_status_check CHECK (status IN ('draft', 'approved', 'superseded')),
    UNIQUE(production_project_id, character_id, version)
);

CREATE INDEX idx_character_bibles_project ON character_bibles(production_project_id);
CREATE INDEX idx_character_bibles_character ON character_bibles(production_project_id, character_id);
CREATE INDEX idx_character_bibles_status ON character_bibles(status);

COMMENT ON TABLE character_bibles IS '角色圣经，编剧产出，可包含多个角色';

-- ============================================================================
-- 5. Script Drafts (剧本草稿 - 编剧产出)
-- ============================================================================

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

    CONSTRAINT script_drafts_status_check CHECK (status IN ('draft', 'approved', 'superseded')),
    UNIQUE(production_project_id, version)
);

CREATE INDEX idx_script_drafts_project ON script_drafts(production_project_id);
CREATE INDEX idx_script_drafts_status ON script_drafts(status);

COMMENT ON TABLE script_drafts IS '剧本草稿，编剧产出';

-- ============================================================================
-- 6. Directorial Treatments (导演阐述 - 导演产出)
-- ============================================================================

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

    CONSTRAINT directorial_treatments_status_check CHECK (status IN ('draft', 'approved', 'superseded')),
    UNIQUE(production_project_id, version)
);

CREATE INDEX idx_directorial_treatments_project ON directorial_treatments(production_project_id);
CREATE INDEX idx_directorial_treatments_status ON directorial_treatments(status);

COMMENT ON TABLE directorial_treatments IS '导演阐述，导演产出';

-- ============================================================================
-- 7. Shot Contracts (镜头合约 - 导演产出)
-- ============================================================================

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

    CONSTRAINT shot_contracts_status_check CHECK (status IN ('draft', 'approved', 'superseded')),
    UNIQUE(production_project_id, shot_id, version)
);

CREATE INDEX idx_shot_contracts_project ON shot_contracts(production_project_id);
CREATE INDEX idx_shot_contracts_scene ON shot_contracts(production_project_id, scene_id);
CREATE INDEX idx_shot_contracts_shot ON shot_contracts(production_project_id, shot_id);
CREATE INDEX idx_shot_contracts_status ON shot_contracts(status);

COMMENT ON TABLE shot_contracts IS '镜头合约，导演产出，包含多个镜头';

-- ============================================================================
-- 8. Performance Briefs (表演简报 - 表演指导产出)
-- ============================================================================

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

    CONSTRAINT performance_briefs_status_check CHECK (status IN ('draft', 'approved', 'superseded')),
    UNIQUE(production_project_id, character_id, version)
);

CREATE INDEX idx_performance_briefs_project ON performance_briefs(production_project_id);
CREATE INDEX idx_performance_briefs_character ON performance_briefs(production_project_id, character_id);
CREATE INDEX idx_performance_briefs_status ON performance_briefs(status);

COMMENT ON TABLE performance_briefs IS '表演简报，表演指导产出，按角色分';

-- ============================================================================
-- 9. Sound Plans (声音计划 - 声音指导产出)
-- ============================================================================

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

    CONSTRAINT sound_plans_status_check CHECK (status IN ('draft', 'approved', 'superseded')),
    UNIQUE(production_project_id, version)
);

CREATE INDEX idx_sound_plans_project ON sound_plans(production_project_id);
CREATE INDEX idx_sound_plans_status ON sound_plans(status);

COMMENT ON TABLE sound_plans IS '声音计划，声音指导产出';

-- ============================================================================
-- 10. Continuity Ledgers (连续性台账 - 剪辑师维护)
-- ============================================================================

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

COMMENT ON TABLE continuity_ledgers IS '连续性台账，剪辑师维护';

-- ============================================================================
-- 11. Take Reviews (镜头评审 - QC 产出)
-- ============================================================================

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
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CONSTRAINT take_reviews_status_check CHECK (status IN ('approved', 'rejected', 'needs_revision'))
);

CREATE INDEX idx_take_reviews_project ON take_reviews(production_project_id);
CREATE INDEX idx_take_reviews_shot ON take_reviews(production_project_id, shot_id);
CREATE INDEX idx_take_reviews_status ON take_reviews(status);

COMMENT ON TABLE take_reviews IS '镜头评审，QC 产出';

-- ============================================================================
-- 12. Collaboration Suggestions (协作建议)
-- ============================================================================

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
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CONSTRAINT collaboration_suggestions_type_check
        CHECK (suggestion_type IN ('revision', 'addition', 'deletion')),
    CONSTRAINT collaboration_suggestions_status_check
        CHECK (status IN ('pending', 'accepted', 'rejected'))
);

CREATE INDEX idx_collaboration_suggestions_project ON collaboration_suggestions(production_project_id);
CREATE INDEX idx_collaboration_suggestions_to_role ON collaboration_suggestions(to_role, status);
CREATE INDEX idx_collaboration_suggestions_artifact ON collaboration_suggestions(artifact_type, artifact_id);

COMMENT ON TABLE collaboration_suggestions IS '协作建议，角色间提出修改意见';
