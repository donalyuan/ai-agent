-- Initial MVP schema for video-agent.
-- The first version intentionally has no tenant/user/RBAC columns; project memory
-- defines this repository as a single-user MVP until the core production loop works.

CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TABLE projects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(120) NOT NULL,
    positioning TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT projects_status_check CHECK (status IN ('active', 'archived'))
);

COMMENT ON TABLE projects IS '内容项目，承载一个账号方向或内容生产主题。';
COMMENT ON COLUMN projects.positioning IS '账号定位文本，是选题 Agent 和脚本 Agent 的主要上下文。';

CREATE TRIGGER trigger_projects_updated_at
    BEFORE UPDATE ON projects
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TABLE accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    platform VARCHAR(30) NOT NULL,
    display_name VARCHAR(120) NOT NULL,
    credentials JSONB NOT NULL DEFAULT '{}'::jsonb,
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT accounts_platform_check CHECK (platform IN ('douyin', 'xiaohongshu')),
    CONSTRAINT accounts_status_check CHECK (status IN ('active', 'disabled'))
);

COMMENT ON TABLE accounts IS '发布平台账号；credentials 保存 Cookie/Token 等平台凭据。';
COMMENT ON COLUMN accounts.credentials IS 'MVP 使用 JSONB 承载平台凭据，后续稳定后再拆密钥管理。';

CREATE INDEX idx_accounts_project ON accounts(project_id);

CREATE TRIGGER trigger_accounts_updated_at
    BEFORE UPDATE ON accounts
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TABLE materials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    material_type VARCHAR(20) NOT NULL,
    file_url TEXT NOT NULL,
    file_name VARCHAR(255) NOT NULL,
    tags TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    usage_count INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT materials_type_check CHECK (material_type IN ('video', 'image', 'audio')),
    CONSTRAINT materials_usage_count_check CHECK (usage_count >= 0)
);

COMMENT ON TABLE materials IS '素材库主表，保存视频、图片和音频素材的基础信息。';
COMMENT ON COLUMN materials.metadata IS '素材尺寸、时长、版权标记等暂存 JSONB。';

CREATE INDEX idx_materials_project ON materials(project_id);
CREATE INDEX idx_materials_type ON materials(material_type);
CREATE INDEX idx_materials_tags ON materials USING GIN(tags);

CREATE TRIGGER trigger_materials_updated_at
    BEFORE UPDATE ON materials
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TABLE material_embeddings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    material_id UUID NOT NULL REFERENCES materials(id) ON DELETE CASCADE,
    collection_name VARCHAR(120) NOT NULL,
    vector_id VARCHAR(160) NOT NULL,
    embedding_model VARCHAR(120) NOT NULL,
    embedding_dim INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT material_embeddings_dim_check CHECK (embedding_dim > 0),
    CONSTRAINT material_embeddings_vector_unique UNIQUE(collection_name, vector_id)
);

COMMENT ON TABLE material_embeddings IS '素材向量索引映射表，实际向量存储在 Milvus。';

CREATE INDEX idx_material_embeddings_material ON material_embeddings(material_id);

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
    CONSTRAINT scripts_status_check CHECK (status IN ('draft', 'approved', 'archived'))
);

COMMENT ON TABLE scripts IS '视频脚本聚合根，脚本 Agent 生成后先以 draft 状态保存。';
COMMENT ON COLUMN scripts.content IS '保存选题、风格、总时长和 LLM 生成元数据等完整脚本上下文。';
COMMENT ON COLUMN scripts.parent_id IS 'A/B 版本父脚本引用；删除父脚本时保留子脚本并置空。';

CREATE INDEX idx_scripts_project ON scripts(project_id);
CREATE INDEX idx_scripts_status ON scripts(status);
CREATE INDEX idx_scripts_parent ON scripts(parent_id) WHERE parent_id IS NOT NULL;
CREATE INDEX idx_scripts_created ON scripts(created_at DESC);

CREATE TRIGGER trigger_scripts_updated_at
    BEFORE UPDATE ON scripts
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TABLE scenes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    script_id UUID NOT NULL REFERENCES scripts(id) ON DELETE CASCADE,
    sequence INT NOT NULL,
    narration TEXT NOT NULL,
    visual_description TEXT NOT NULL,
    emotion VARCHAR(50) NOT NULL,
    duration_sec INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT scenes_sequence_check CHECK (sequence > 0 AND sequence <= 20),
    CONSTRAINT scenes_duration_check CHECK (duration_sec > 0 AND duration_sec <= 30),
    CONSTRAINT scenes_script_sequence_unique UNIQUE(script_id, sequence)
);

COMMENT ON TABLE scenes IS '脚本分镜，生命周期归属于 scripts。';
COMMENT ON COLUMN scenes.sequence IS '同一脚本内从 1 开始递增的分镜顺序。';

CREATE INDEX idx_scenes_script ON scenes(script_id, sequence);

CREATE TABLE generation_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    script_id UUID REFERENCES scripts(id) ON DELETE SET NULL,
    provider VARCHAR(30) NOT NULL,
    task_type VARCHAR(30) NOT NULL,
    params JSONB NOT NULL DEFAULT '{}'::jsonb,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    external_task_id VARCHAR(160),
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT generation_tasks_provider_check CHECK (provider IN ('runway', 'kling')),
    CONSTRAINT generation_tasks_type_check CHECK (task_type IN ('text_to_video', 'image_to_video')),
    CONSTRAINT generation_tasks_status_check CHECK (status IN ('pending', 'processing', 'completed', 'failed'))
);

COMMENT ON TABLE generation_tasks IS '视频生成异步任务，Python Worker 领取并对接 Runway/可灵。';
COMMENT ON COLUMN generation_tasks.params IS '不同视频平台的生成参数，MVP 阶段用 JSONB 保持适配弹性。';

CREATE INDEX idx_generation_tasks_project ON generation_tasks(project_id);
CREATE INDEX idx_generation_tasks_status ON generation_tasks(status);
CREATE INDEX idx_generation_tasks_script ON generation_tasks(script_id) WHERE script_id IS NOT NULL;

CREATE TRIGGER trigger_generation_tasks_updated_at
    BEFORE UPDATE ON generation_tasks
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TABLE videos (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    script_id UUID REFERENCES scripts(id) ON DELETE SET NULL,
    generation_task_id UUID REFERENCES generation_tasks(id) ON DELETE SET NULL,
    title VARCHAR(200) NOT NULL,
    video_url TEXT NOT NULL,
    cover_url TEXT,
    status VARCHAR(20) NOT NULL DEFAULT 'draft',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT videos_status_check CHECK (status IN ('draft', 'ready', 'published'))
);

COMMENT ON TABLE videos IS '最终视频作品，连接生成任务、发布任务和数据回流。';

CREATE INDEX idx_videos_project ON videos(project_id);
CREATE INDEX idx_videos_script ON videos(script_id) WHERE script_id IS NOT NULL;
CREATE INDEX idx_videos_status ON videos(status);

CREATE TRIGGER trigger_videos_updated_at
    BEFORE UPDATE ON videos
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TABLE publish_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    video_id UUID NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
    account_id UUID REFERENCES accounts(id) ON DELETE SET NULL,
    platform VARCHAR(30) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    scheduled_at TIMESTAMPTZ,
    published_at TIMESTAMPTZ,
    external_publish_id VARCHAR(160),
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT publish_tasks_platform_check CHECK (platform IN ('douyin', 'xiaohongshu')),
    CONSTRAINT publish_tasks_status_check CHECK (status IN ('pending', 'scheduled', 'published', 'failed'))
);

COMMENT ON TABLE publish_tasks IS '发布任务，MVP 支持手动发布和基础定时发布。';

CREATE INDEX idx_publish_tasks_video ON publish_tasks(video_id);
CREATE INDEX idx_publish_tasks_account ON publish_tasks(account_id) WHERE account_id IS NOT NULL;
CREATE INDEX idx_publish_tasks_status ON publish_tasks(status);
CREATE INDEX idx_publish_tasks_scheduled ON publish_tasks(scheduled_at) WHERE scheduled_at IS NOT NULL;

CREATE TRIGGER trigger_publish_tasks_updated_at
    BEFORE UPDATE ON publish_tasks
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TABLE metrics (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    video_id UUID NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
    publish_task_id UUID REFERENCES publish_tasks(id) ON DELETE SET NULL,
    platform VARCHAR(30) NOT NULL,
    plays BIGINT NOT NULL DEFAULT 0,
    likes BIGINT NOT NULL DEFAULT 0,
    comments BIGINT NOT NULL DEFAULT 0,
    shares BIGINT NOT NULL DEFAULT 0,
    completion_rate NUMERIC(5, 4),
    captured_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT metrics_platform_check CHECK (platform IN ('douyin', 'xiaohongshu')),
    CONSTRAINT metrics_non_negative_check CHECK (
        plays >= 0 AND likes >= 0 AND comments >= 0 AND shares >= 0
    ),
    CONSTRAINT metrics_completion_rate_check CHECK (
        completion_rate IS NULL OR (completion_rate >= 0 AND completion_rate <= 1)
    )
);

COMMENT ON TABLE metrics IS '视频效果数据回流；MVP 先支持手动录入基础指标。';

CREATE INDEX idx_metrics_video ON metrics(video_id, captured_at DESC);
CREATE INDEX idx_metrics_publish_task ON metrics(publish_task_id) WHERE publish_task_id IS NOT NULL;

CREATE TABLE revenues (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    video_id UUID NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
    amount NUMERIC(12, 2) NOT NULL DEFAULT 0,
    currency CHAR(3) NOT NULL DEFAULT 'CNY',
    source VARCHAR(60) NOT NULL DEFAULT 'manual',
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT revenues_amount_check CHECK (amount >= 0)
);

COMMENT ON TABLE revenues IS '收益记录，MVP 阶段保留表结构但不作为 P0 主线。';

CREATE INDEX idx_revenues_video ON revenues(video_id, recorded_at DESC);

CREATE TABLE agent_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID REFERENCES projects(id) ON DELETE SET NULL,
    agent_type VARCHAR(30) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    input JSONB NOT NULL DEFAULT '{}'::jsonb,
    output JSONB,
    error_message TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ended_at TIMESTAMPTZ,
    CONSTRAINT agent_runs_type_check CHECK (
        agent_type IN ('topic', 'script', 'material', 'video', 'publish', 'optimization')
    ),
    CONSTRAINT agent_runs_status_check CHECK (status IN ('pending', 'running', 'succeeded', 'failed'))
);

COMMENT ON TABLE agent_runs IS 'Agent 单次运行记录，用于基础日志、调试和后续评测回放。';
COMMENT ON COLUMN agent_runs.input IS 'Agent 输入快照，避免后续上下文变化导致不可复现。';
COMMENT ON COLUMN agent_runs.output IS 'Agent 输出快照；失败时可为空并写入 error_message。';

CREATE INDEX idx_agent_runs_project ON agent_runs(project_id) WHERE project_id IS NOT NULL;
CREATE INDEX idx_agent_runs_type ON agent_runs(agent_type);
CREATE INDEX idx_agent_runs_status ON agent_runs(status);
CREATE INDEX idx_agent_runs_started ON agent_runs(started_at DESC);

CREATE TABLE agent_steps (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_run_id UUID NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
    step_order INT NOT NULL,
    step_type VARCHAR(60) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'succeeded',
    input JSONB NOT NULL DEFAULT '{}'::jsonb,
    output JSONB,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_steps_order_check CHECK (step_order > 0),
    CONSTRAINT agent_steps_status_check CHECK (status IN ('succeeded', 'failed')),
    CONSTRAINT agent_steps_run_order_unique UNIQUE(agent_run_id, step_order)
);

COMMENT ON TABLE agent_steps IS 'Agent 运行步骤日志，MVP 只做基础可追踪，不做完整 Trace 可视化。';

CREATE INDEX idx_agent_steps_run ON agent_steps(agent_run_id, step_order);

CREATE TABLE viral_videos (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    platform VARCHAR(30) NOT NULL,
    title VARCHAR(200) NOT NULL,
    url TEXT NOT NULL,
    author VARCHAR(120) NOT NULL DEFAULT '',
    metrics JSONB NOT NULL DEFAULT '{}'::jsonb,
    analysis JSONB NOT NULL DEFAULT '{}'::jsonb,
    captured_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT viral_videos_platform_check CHECK (platform IN ('douyin', 'xiaohongshu'))
);

COMMENT ON TABLE viral_videos IS '爆款视频库，Month 4 再进入主线；当前只预留结构。';
COMMENT ON COLUMN viral_videos.analysis IS 'LLM 对爆款模式的分析结果。';

CREATE INDEX idx_viral_videos_platform ON viral_videos(platform, captured_at DESC);

CREATE TABLE content_strategies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID REFERENCES projects(id) ON DELETE CASCADE,
    source_run_id UUID REFERENCES agent_runs(id) ON DELETE SET NULL,
    title VARCHAR(160) NOT NULL,
    strategy JSONB NOT NULL,
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    status VARCHAR(20) NOT NULL DEFAULT 'draft',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT content_strategies_status_check CHECK (status IN ('draft', 'active', 'archived'))
);

COMMENT ON TABLE content_strategies IS '内容策略建议，供后续优化 Agent 和选题 Agent 使用。';

CREATE INDEX idx_content_strategies_project ON content_strategies(project_id) WHERE project_id IS NOT NULL;
CREATE INDEX idx_content_strategies_status ON content_strategies(status);

CREATE TRIGGER trigger_content_strategies_updated_at
    BEFORE UPDATE ON content_strategies
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();
