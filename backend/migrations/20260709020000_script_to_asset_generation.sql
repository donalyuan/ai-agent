-- Add script-to-asset generation planning and per-scene candidate selection state.
-- This migration is append-only because earlier migrations may already be applied in
-- development databases.

CREATE TABLE asset_generation_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    script_id UUID REFERENCES scripts(id) ON DELETE SET NULL,
    scene_id UUID REFERENCES scenes(id) ON DELETE SET NULL,
    provider VARCHAR(40) NOT NULL,
    task_type VARCHAR(40) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    candidate_count INT NOT NULL DEFAULT 0,
    reference_material_ids UUID[] NOT NULL DEFAULT ARRAY[]::UUID[],
    idempotency_key TEXT NOT NULL DEFAULT '',
    params JSONB NOT NULL DEFAULT '{}'::jsonb,
    result JSONB NOT NULL DEFAULT '{}'::jsonb,
    error_message TEXT,
    retry_count INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT asset_generation_tasks_provider_check CHECK (provider IN ('gpt-image-2', 'jimeng')),
    CONSTRAINT asset_generation_tasks_type_check CHECK (task_type IN ('image_candidates', 'video_draft', 'video_generation')),
    CONSTRAINT asset_generation_tasks_status_check CHECK (status IN ('draft', 'pending', 'processing', 'completed', 'failed')),
    CONSTRAINT asset_generation_tasks_candidate_count_check CHECK (candidate_count >= 0 AND candidate_count <= 48),
    CONSTRAINT asset_generation_tasks_retry_count_check CHECK (retry_count >= 0)
);

COMMENT ON TABLE asset_generation_tasks IS '脚本到素材生成任务，记录图片候选自动生成和 AI 视频二次确认任务。';
COMMENT ON COLUMN asset_generation_tasks.provider IS '图片/视频生成供应商，第一版支持 gpt-image-2 和 jimeng。';
COMMENT ON COLUMN asset_generation_tasks.reference_material_ids IS '作为 AI 参考图或风格参考的已登记素材 ID，用于审计复用来源。';
COMMENT ON COLUMN asset_generation_tasks.idempotency_key IS '同一脚本同一生成配置的任务幂等键，防止重复点击创建重复任务。';
COMMENT ON COLUMN asset_generation_tasks.params IS '生成参数和成本控制输入，例如 prompt、候选数量和参考图开关。';
COMMENT ON COLUMN asset_generation_tasks.result IS 'Worker 回写的生成结果摘要，不保存供应商临时 URL 作为最终素材地址。';

CREATE INDEX idx_asset_generation_tasks_project_created
    ON asset_generation_tasks(project_id, created_at DESC);
CREATE INDEX idx_asset_generation_tasks_script
    ON asset_generation_tasks(script_id, created_at DESC)
    WHERE script_id IS NOT NULL;
CREATE INDEX idx_asset_generation_tasks_scene
    ON asset_generation_tasks(scene_id, created_at DESC)
    WHERE scene_id IS NOT NULL;
CREATE INDEX idx_asset_generation_tasks_status
    ON asset_generation_tasks(status, created_at ASC);
CREATE UNIQUE INDEX asset_generation_tasks_idempotency_unique
    ON asset_generation_tasks(idempotency_key)
    WHERE idempotency_key <> '';

CREATE TRIGGER trigger_asset_generation_tasks_updated_at
    BEFORE UPDATE ON asset_generation_tasks
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

ALTER TABLE materials
    ADD CONSTRAINT materials_id_project_unique UNIQUE(id, project_id);

ALTER TABLE scripts
    ADD CONSTRAINT scripts_id_project_unique UNIQUE(id, project_id);

ALTER TABLE scenes
    ADD CONSTRAINT scenes_id_script_unique UNIQUE(id, script_id);

CREATE TABLE scene_asset_candidates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    script_id UUID NOT NULL REFERENCES scripts(id) ON DELETE CASCADE,
    scene_id UUID NOT NULL REFERENCES scenes(id) ON DELETE CASCADE,
    material_id UUID REFERENCES materials(id) ON DELETE SET NULL,
    candidate_type VARCHAR(20) NOT NULL,
    source VARCHAR(30) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'candidate',
    rank INT NOT NULL DEFAULT 0,
    generation_task_id UUID REFERENCES asset_generation_tasks(id) ON DELETE SET NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT scene_asset_candidates_type_check CHECK (candidate_type IN ('image', 'video')),
    CONSTRAINT scene_asset_candidates_source_check CHECK (source IN ('existing_material', 'ai_generated', 'video_task')),
    CONSTRAINT scene_asset_candidates_status_check CHECK (status IN ('candidate', 'selected', 'rejected', 'failed')),
    CONSTRAINT scene_asset_candidates_rank_check CHECK (rank >= 0),
    CONSTRAINT scene_asset_candidates_script_project_fk
        FOREIGN KEY (script_id, project_id) REFERENCES scripts(id, project_id) ON DELETE CASCADE,
    CONSTRAINT scene_asset_candidates_scene_script_fk
        FOREIGN KEY (scene_id, script_id) REFERENCES scenes(id, script_id) ON DELETE CASCADE,
    CONSTRAINT scene_asset_candidates_material_project_fk
        FOREIGN KEY (material_id, project_id) REFERENCES materials(id, project_id)
);

COMMENT ON TABLE scene_asset_candidates IS '分镜素材候选，允许旧素材复用、AI 图片候选和 AI 视频任务作为候选来源。';
COMMENT ON COLUMN scene_asset_candidates.status IS 'candidate 候选，selected 已选为该分镜主素材，rejected 人工排除，failed 生成失败。';
COMMENT ON COLUMN scene_asset_candidates.metadata IS '候选解释、未选候选标记、来源分镜、存储和供应商生成审计信息。';

CREATE UNIQUE INDEX scene_asset_candidates_one_selected_per_scene
    ON scene_asset_candidates(scene_id)
    WHERE status = 'selected';

CREATE INDEX idx_scene_asset_candidates_script_scene_rank
    ON scene_asset_candidates(script_id, scene_id, rank ASC, created_at ASC);
CREATE INDEX idx_scene_asset_candidates_material
    ON scene_asset_candidates(material_id)
    WHERE material_id IS NOT NULL;
CREATE INDEX idx_scene_asset_candidates_task
    ON scene_asset_candidates(generation_task_id)
    WHERE generation_task_id IS NOT NULL;
CREATE UNIQUE INDEX scene_asset_candidates_existing_material_unique
    ON scene_asset_candidates(scene_id, material_id)
    WHERE source = 'existing_material' AND material_id IS NOT NULL;
CREATE UNIQUE INDEX scene_asset_candidates_video_task_unique
    ON scene_asset_candidates(generation_task_id)
    WHERE source = 'video_task' AND generation_task_id IS NOT NULL;

CREATE TRIGGER trigger_scene_asset_candidates_updated_at
    BEFORE UPDATE ON scene_asset_candidates
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();
