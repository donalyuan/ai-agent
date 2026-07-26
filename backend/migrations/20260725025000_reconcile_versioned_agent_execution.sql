-- Reconcile databases that applied an early 20260724010000 draft before its schema was finalized.
ALTER TABLE model_calls
    DROP CONSTRAINT model_calls_owner_check,
    ADD CONSTRAINT model_calls_owner_check
        CHECK (num_nonnulls(conversation_id, agent_run_id) = 1);

CREATE UNIQUE INDEX IF NOT EXISTS idx_model_calls_step
    ON model_calls(agent_step_id)
    WHERE agent_step_id IS NOT NULL;

CREATE OR REPLACE FUNCTION enforce_model_call_transition() RETURNS TRIGGER AS $$
BEGIN
    IF NEW.status = OLD.status
       AND OLD.agent_step_id IS NULL
       AND NEW.agent_step_id IS NOT NULL
       AND ROW(
           NEW.output_snapshot, NEW.usage_snapshot, NEW.error_snapshot,
           NEW.structured_parse_status, NEW.completed_at
       ) IS NOT DISTINCT FROM ROW(
           OLD.output_snapshot, OLD.usage_snapshot, OLD.error_snapshot,
           OLD.structured_parse_status, OLD.completed_at
       )
       AND ROW(
           NEW.schema_version, NEW.source_runtime, NEW.conversation_id, NEW.agent_run_id,
           NEW.root_call_id, NEW.parent_call_id, NEW.node_key, NEW.attempt,
           NEW.agent_key, NEW.agent_version, NEW.prompt_key, NEW.prompt_version,
           NEW.registry_digest, NEW.prompt_snapshot, NEW.context_sources, NEW.memory_sources,
           NEW.tool_schema, NEW.model_id, NEW.behavior_fingerprint, NEW.model_snapshot,
           NEW.parameters, NEW.asset_references, NEW.prepared_at
       ) IS NOT DISTINCT FROM ROW(
           OLD.schema_version, OLD.source_runtime, OLD.conversation_id, OLD.agent_run_id,
           OLD.root_call_id, OLD.parent_call_id, OLD.node_key, OLD.attempt,
           OLD.agent_key, OLD.agent_version, OLD.prompt_key, OLD.prompt_version,
           OLD.registry_digest, OLD.prompt_snapshot, OLD.context_sources, OLD.memory_sources,
           OLD.tool_schema, OLD.model_id, OLD.behavior_fingerprint, OLD.model_snapshot,
           OLD.parameters, OLD.asset_references, OLD.prepared_at
       ) THEN
        RETURN NEW;
    END IF;
    IF OLD.status <> 'prepared' THEN
        RAISE EXCEPTION 'model call already has terminal status';
    END IF;
    IF NEW.status NOT IN ('succeeded', 'failed', 'aborted') THEN
        RAISE EXCEPTION 'model call may only transition from prepared to a terminal status';
    END IF;
    IF ROW(
        NEW.schema_version, NEW.source_runtime, NEW.conversation_id, NEW.agent_run_id,
        NEW.root_call_id, NEW.parent_call_id, NEW.node_key, NEW.attempt,
        NEW.agent_key, NEW.agent_version, NEW.prompt_key, NEW.prompt_version,
        NEW.registry_digest, NEW.prompt_snapshot, NEW.context_sources, NEW.memory_sources,
        NEW.tool_schema, NEW.model_id, NEW.behavior_fingerprint, NEW.model_snapshot,
        NEW.parameters, NEW.asset_references, NEW.prepared_at
    ) IS DISTINCT FROM ROW(
        OLD.schema_version, OLD.source_runtime, OLD.conversation_id, OLD.agent_run_id,
        OLD.root_call_id, OLD.parent_call_id, OLD.node_key, OLD.attempt,
        OLD.agent_key, OLD.agent_version, OLD.prompt_key, OLD.prompt_version,
        OLD.registry_digest, OLD.prompt_snapshot, OLD.context_sources, OLD.memory_sources,
        OLD.tool_schema, OLD.model_id, OLD.behavior_fingerprint, OLD.model_snapshot,
        OLD.parameters, OLD.asset_references, OLD.prepared_at
    ) THEN
        RAISE EXCEPTION 'model call prepared input is immutable';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

COMMENT ON CONSTRAINT model_calls_owner_check ON model_calls IS
    '每个 ModelCall 必须且只能属于一个 Conversation 或 Agent Run owner。';
