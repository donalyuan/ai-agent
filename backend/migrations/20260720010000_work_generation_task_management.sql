-- 作品生成任务管理：保留运行、步骤和每次 provider attempt 的可恢复审计。
ALTER TABLE work_generation_runs
    ADD COLUMN IF NOT EXISTS current_stage VARCHAR(32) NOT NULL DEFAULT 'queued',
    ADD COLUMN IF NOT EXISTS progress_percent INT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS error_category VARCHAR(64),
    ADD COLUMN IF NOT EXISTS last_provider_request_id VARCHAR(200),
    ADD COLUMN IF NOT EXISTS started_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS completed_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS dismissed_at TIMESTAMPTZ;

ALTER TABLE work_generation_runs DROP CONSTRAINT IF EXISTS work_generation_runs_status_check;
ALTER TABLE work_generation_runs
    ADD CONSTRAINT work_generation_runs_status_check
    CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'cancelled', 'waiting_manual', 'cancelling'));
ALTER TABLE work_generation_runs
    ADD CONSTRAINT work_generation_runs_progress_check CHECK (progress_percent BETWEEN 0 AND 100);
CREATE INDEX IF NOT EXISTS idx_work_generation_runs_visible_updated
    ON work_generation_runs(status, updated_at DESC)
    WHERE dismissed_at IS NULL;

ALTER TABLE work_generation_steps
    ADD COLUMN IF NOT EXISTS is_required BOOLEAN NOT NULL DEFAULT true,
    ADD COLUMN IF NOT EXISTS depends_on JSONB NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN IF NOT EXISTS model_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS resource_usage JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS result_material_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN IF NOT EXISTS error_category VARCHAR(64),
    ADD COLUMN IF NOT EXISTS error_code VARCHAR(120);

ALTER TABLE work_generation_steps DROP CONSTRAINT IF EXISTS work_generation_steps_status_check;
ALTER TABLE work_generation_steps
    ADD CONSTRAINT work_generation_steps_status_check
    CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'cancelled', 'blocked', 'waiting_manual'));

CREATE TABLE IF NOT EXISTS work_generation_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    step_id UUID NOT NULL REFERENCES work_generation_steps(id) ON DELETE CASCADE,
    attempt_no INT NOT NULL,
    status VARCHAR(24) NOT NULL DEFAULT 'queued',
    model_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    input_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    output_snapshot JSONB,
    resource_usage JSONB NOT NULL DEFAULT '{}'::jsonb,
    error_category VARCHAR(64),
    error_code VARCHAR(120),
    error_summary TEXT,
    request_trace_id VARCHAR(200),
    upstream_task_id VARCHAR(200),
    lease_owner VARCHAR(120),
    lease_expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT work_generation_attempts_status_check
      CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'cancelled', 'waiting_manual')),
    CONSTRAINT work_generation_attempts_no CHECK (attempt_no > 0),
    UNIQUE (step_id, attempt_no)
);
CREATE UNIQUE INDEX IF NOT EXISTS work_generation_attempts_one_in_flight
    ON work_generation_attempts(step_id)
    WHERE status IN ('queued', 'running');
CREATE INDEX IF NOT EXISTS idx_work_generation_attempts_upstream
    ON work_generation_attempts(upstream_task_id)
    WHERE upstream_task_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS work_generation_retry_idempotency (
    idempotency_key VARCHAR(200) PRIMARY KEY,
    attempt_id UUID NOT NULL REFERENCES work_generation_attempts(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE OR REPLACE FUNCTION trigger_work_generation_attempts_updated_at_fn()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trigger_work_generation_attempts_updated_at ON work_generation_attempts;
CREATE TRIGGER trigger_work_generation_attempts_updated_at
BEFORE UPDATE ON work_generation_attempts
FOR EACH ROW EXECUTE FUNCTION trigger_work_generation_attempts_updated_at_fn();

-- 作品生产任务是独立二级模块，不混入一级“工作流任务”。
INSERT INTO video_workspace_menus (
    id, parent_id, menu_key, label, description, route_path, icon, menu_type, module_key,
    agent_key, sort_order, is_enabled, is_visible, status, metadata
)
SELECT '30000000-0000-4000-8000-000000000004', parent.id, 'work-generation-task', '生成任务',
       '查看作品生成运行、步骤进度、失败审计和重试。', '/production/tasks', 'list-checks',
       'page', 'production.work-generation-task', 'work', 20, true, true, 'active', '{"phase":4}'::jsonb
FROM video_workspace_menus parent WHERE parent.menu_key = 'production'
ON CONFLICT (menu_key) DO UPDATE SET parent_id = EXCLUDED.parent_id, label = EXCLUDED.label,
    description = EXCLUDED.description, route_path = EXCLUDED.route_path, icon = EXCLUDED.icon,
    module_key = EXCLUDED.module_key, agent_key = EXCLUDED.agent_key, sort_order = EXCLUDED.sort_order,
    is_enabled = EXCLUDED.is_enabled, is_visible = EXCLUDED.is_visible, status = EXCLUDED.status,
    metadata = EXCLUDED.metadata, updated_at = NOW();
