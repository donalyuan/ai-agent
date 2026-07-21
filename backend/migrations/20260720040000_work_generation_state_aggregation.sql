-- 作品生成运行状态由锁定 DAG 与 attempt 事实统一聚合，禁止各客户端自行推算。
ALTER TABLE work_generation_runs
    ADD COLUMN IF NOT EXISTS cancel_requested_at TIMESTAMPTZ;

ALTER TABLE work_generation_attempts
    ADD COLUMN IF NOT EXISTS provider_cancel_supported BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS cancel_requested_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS cancel_response TEXT;

CREATE OR REPLACE FUNCTION refresh_work_generation_run_state(target_run_id UUID)
RETURNS VOID AS $$
DECLARE
    required_count BIGINT;
    succeeded_required BIGINT;
    running_required BIGINT;
    failed_required BIGINT;
    waiting_required BIGINT;
    cancelled_required BIGINT;
    has_attempt BOOLEAN;
    cancellation_requested BOOLEAN;
    next_status VARCHAR(24);
    next_stage VARCHAR(32);
    next_error_category VARCHAR(64);
    next_error_summary TEXT;
BEGIN
    SELECT
        COUNT(*) FILTER (WHERE is_required),
        COUNT(*) FILTER (WHERE is_required AND status = 'succeeded'),
        COUNT(*) FILTER (WHERE is_required AND status = 'running'),
        COUNT(*) FILTER (WHERE is_required AND status = 'failed'),
        COUNT(*) FILTER (WHERE is_required AND status = 'waiting_manual'),
        COUNT(*) FILTER (WHERE is_required AND status = 'cancelled')
    INTO required_count, succeeded_required, running_required, failed_required,
         waiting_required, cancelled_required
    FROM work_generation_steps
    WHERE run_id = target_run_id;

    IF required_count = 0 THEN
        RETURN;
    END IF;

    SELECT EXISTS (
        SELECT 1
        FROM work_generation_attempts a
        JOIN work_generation_steps s ON s.id = a.step_id
        WHERE s.run_id = target_run_id
    ) INTO has_attempt;

    SELECT cancel_requested_at IS NOT NULL
    INTO cancellation_requested
    FROM work_generation_runs
    WHERE id = target_run_id;

    IF waiting_required > 0 THEN
        next_status := 'waiting_manual';
        SELECT step_type INTO next_stage
        FROM work_generation_steps
        WHERE run_id = target_run_id AND is_required AND status = 'waiting_manual'
        ORDER BY step_no LIMIT 1;
    ELSIF failed_required > 0 AND running_required = 0 THEN
        next_status := 'failed';
        SELECT step_type INTO next_stage
        FROM work_generation_steps
        WHERE run_id = target_run_id AND is_required AND status = 'failed'
        ORDER BY step_no LIMIT 1;
    ELSIF cancellation_requested AND running_required > 0 THEN
        next_status := 'cancelling';
        next_stage := 'cancelling';
    ELSIF succeeded_required = required_count THEN
        next_status := 'succeeded';
        next_stage := 'completed';
    ELSIF cancelled_required > 0 AND running_required = 0 THEN
        next_status := 'cancelled';
        next_stage := 'cancelled';
    ELSE
        next_status := CASE WHEN has_attempt OR running_required > 0 THEN 'running' ELSE 'queued' END;
        SELECT step_type INTO next_stage
        FROM work_generation_steps
        WHERE run_id = target_run_id
          AND is_required
          AND status IN ('running', 'queued', 'blocked', 'failed')
        ORDER BY
            CASE status WHEN 'running' THEN 0 WHEN 'failed' THEN 1 WHEN 'queued' THEN 2 ELSE 3 END,
            step_no
        LIMIT 1;
        next_stage := COALESCE(next_stage, next_status);
    END IF;

    SELECT error_category, error_summary
    INTO next_error_category, next_error_summary
    FROM work_generation_steps
    WHERE run_id = target_run_id
      AND is_required
      AND status IN ('failed', 'waiting_manual')
    ORDER BY step_no
    LIMIT 1;

    UPDATE work_generation_runs
    SET status = next_status,
        current_stage = next_stage,
        progress_percent = (100 * succeeded_required / required_count)::INT,
        error_category = next_error_category,
        error_summary = next_error_summary,
        started_at = CASE WHEN has_attempt THEN COALESCE(started_at, NOW()) ELSE started_at END,
        completed_at = CASE
            WHEN next_status IN ('succeeded', 'failed', 'cancelled', 'waiting_manual')
                THEN COALESCE(completed_at, NOW())
            ELSE NULL
        END,
        updated_at = NOW()
    WHERE id = target_run_id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION trigger_refresh_work_generation_run_from_step_fn()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM refresh_work_generation_run_state(NEW.run_id);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trigger_refresh_work_generation_run_from_step ON work_generation_steps;
CREATE TRIGGER trigger_refresh_work_generation_run_from_step
AFTER INSERT OR UPDATE OF status, is_required ON work_generation_steps
FOR EACH ROW EXECUTE FUNCTION trigger_refresh_work_generation_run_from_step_fn();

CREATE OR REPLACE FUNCTION trigger_refresh_work_generation_run_from_attempt_fn()
RETURNS TRIGGER AS $$
DECLARE
    target_run_id UUID;
BEGIN
    SELECT run_id INTO target_run_id
    FROM work_generation_steps
    WHERE id = NEW.step_id;
    PERFORM refresh_work_generation_run_state(target_run_id);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trigger_refresh_work_generation_run_from_attempt ON work_generation_attempts;
CREATE TRIGGER trigger_refresh_work_generation_run_from_attempt
AFTER INSERT OR UPDATE OF status ON work_generation_attempts
FOR EACH ROW EXECUTE FUNCTION trigger_refresh_work_generation_run_from_attempt_fn();

CREATE OR REPLACE FUNCTION trigger_block_work_generation_dependents_fn()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.is_required
       AND NEW.status IN ('failed', 'waiting_manual')
       AND OLD.status IS DISTINCT FROM NEW.status THEN
        WITH RECURSIVE downstream AS (
            SELECT child.id
            FROM work_generation_steps child
            WHERE child.run_id = NEW.run_id
              AND child.is_required
              AND child.depends_on ? NEW.id::TEXT
            UNION
            SELECT child.id
            FROM work_generation_steps child
            JOIN downstream parent ON child.depends_on ? parent.id::TEXT
            WHERE child.run_id = NEW.run_id AND child.is_required
        )
        UPDATE work_generation_steps
        SET status = 'blocked',
            error_category = 'dependency',
            error_summary = '前置步骤失败，等待失败节点处理',
            updated_at = NOW()
        WHERE id IN (SELECT id FROM downstream)
          AND status IN ('queued', 'blocked');
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trigger_block_work_generation_dependents ON work_generation_steps;
CREATE TRIGGER trigger_block_work_generation_dependents
AFTER UPDATE OF status ON work_generation_steps
FOR EACH ROW EXECUTE FUNCTION trigger_block_work_generation_dependents_fn();

DO $$
DECLARE
    run_id UUID;
BEGIN
    FOR run_id IN SELECT id FROM work_generation_runs LOOP
        PERFORM refresh_work_generation_run_state(run_id);
    END LOOP;
END $$;
