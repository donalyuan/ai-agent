-- Link Context compilation failures without changing existing Run/Step/Conversation status flows.
ALTER TABLE agent_conversations
    ADD COLUMN last_context_compile_attempt_id UUID
        REFERENCES context_compile_attempts(id) ON DELETE SET NULL;

ALTER TABLE agent_runs
    ADD COLUMN context_compile_attempt_id UUID
        REFERENCES context_compile_attempts(id) ON DELETE SET NULL;

ALTER TABLE agent_steps
    ADD COLUMN context_compile_attempt_id UUID
        REFERENCES context_compile_attempts(id) ON DELETE SET NULL;

CREATE INDEX idx_agent_conversations_context_attempt
    ON agent_conversations(last_context_compile_attempt_id)
    WHERE last_context_compile_attempt_id IS NOT NULL;
CREATE INDEX idx_agent_runs_context_attempt
    ON agent_runs(context_compile_attempt_id)
    WHERE context_compile_attempt_id IS NOT NULL;
CREATE INDEX idx_agent_steps_context_attempt
    ON agent_steps(context_compile_attempt_id)
    WHERE context_compile_attempt_id IS NOT NULL;

COMMENT ON COLUMN agent_conversations.last_context_compile_attempt_id IS
    'Conversation 最近一次 Context 编译失败证据；不改变 active/archived 状态。';
COMMENT ON COLUMN agent_runs.context_compile_attempt_id IS
    'Run 失败收尾关联的 ContextCompileAttempt；既有 status/error_message/ended_at 语义保持不变。';
COMMENT ON COLUMN agent_steps.context_compile_attempt_id IS
    '失败模型步骤关联的 ContextCompileAttempt；必须与 Step 属于同一 AgentRun。';

CREATE FUNCTION validate_context_failure_link() RETURNS TRIGGER AS $$
DECLARE
    linked BOOLEAN;
BEGIN
    IF TG_TABLE_NAME = 'agent_conversations' THEN
        IF NEW.last_context_compile_attempt_id IS NULL THEN
            RETURN NEW;
        END IF;
        SELECT EXISTS (
            SELECT 1
            FROM context_compile_attempts attempt
            LEFT JOIN agent_runs run ON run.id = attempt.agent_run_id
            WHERE attempt.id = NEW.last_context_compile_attempt_id
              AND (
                  attempt.conversation_id = NEW.id
                  OR run.input->>'conversation_id' = NEW.id::text
              )
        ) INTO linked;
    ELSIF TG_TABLE_NAME = 'agent_runs' THEN
        IF NEW.context_compile_attempt_id IS NULL THEN
            RETURN NEW;
        END IF;
        SELECT EXISTS (
            SELECT 1 FROM context_compile_attempts attempt
            WHERE attempt.id = NEW.context_compile_attempt_id
              AND attempt.agent_run_id = NEW.id
        ) INTO linked;
    ELSIF TG_TABLE_NAME = 'agent_steps' THEN
        IF NEW.context_compile_attempt_id IS NULL THEN
            RETURN NEW;
        END IF;
        SELECT EXISTS (
            SELECT 1 FROM context_compile_attempts attempt
            WHERE attempt.id = NEW.context_compile_attempt_id
              AND attempt.agent_run_id = NEW.agent_run_id
        ) INTO linked;
    ELSE
        RAISE EXCEPTION 'unsupported Context failure link table';
    END IF;

    IF NOT linked THEN
        RAISE EXCEPTION 'ContextCompileAttempt owner does not match failure record';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER agent_conversations_context_failure_guard
    BEFORE INSERT OR UPDATE OF last_context_compile_attempt_id ON agent_conversations
    FOR EACH ROW EXECUTE FUNCTION validate_context_failure_link();

CREATE TRIGGER agent_runs_context_failure_guard
    BEFORE INSERT OR UPDATE OF context_compile_attempt_id ON agent_runs
    FOR EACH ROW EXECUTE FUNCTION validate_context_failure_link();

CREATE TRIGGER agent_steps_context_failure_guard
    BEFORE INSERT OR UPDATE OF context_compile_attempt_id ON agent_steps
    FOR EACH ROW EXECUTE FUNCTION validate_context_failure_link();
