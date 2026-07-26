-- Provider context overflow invalidates the fixed tokenizer/profile evidence for the owner binding.
ALTER TABLE agent_run_bindings
    ADD COLUMN context_binding_status VARCHAR(40) NOT NULL DEFAULT 'executable',
    ADD CONSTRAINT agent_run_bindings_context_status_check CHECK (
        context_binding_status IN ('executable', 'tokenizer_profile_incompatible')
    );

COMMENT ON COLUMN agent_run_bindings.context_binding_status IS
    'Provider context overflow 后固定为 tokenizer_profile_incompatible；同一 Run 禁止缩短后重试。';

DROP TRIGGER agent_run_bindings_no_update ON agent_run_bindings;

CREATE OR REPLACE FUNCTION reject_agent_run_binding_update() RETURNS TRIGGER AS $$
BEGIN
    IF ROW(
        NEW.agent_run_id, NEW.agent_key, NEW.agent_version, NEW.agent_digest,
        NEW.prompt_bindings, NEW.context_policy_bindings, NEW.registry_digest,
        NEW.model_id, NEW.behavior_fingerprint, NEW.model_capabilities,
        NEW.tokenizer_profile_key, NEW.tokenizer_profile_version,
        NEW.tokenizer_profile_digest, NEW.legacy_partial_audit, NEW.created_at
    ) IS DISTINCT FROM ROW(
        OLD.agent_run_id, OLD.agent_key, OLD.agent_version, OLD.agent_digest,
        OLD.prompt_bindings, OLD.context_policy_bindings, OLD.registry_digest,
        OLD.model_id, OLD.behavior_fingerprint, OLD.model_capabilities,
        OLD.tokenizer_profile_key, OLD.tokenizer_profile_version,
        OLD.tokenizer_profile_digest, OLD.legacy_partial_audit, OLD.created_at
    ) THEN
        RAISE EXCEPTION 'agent run bindings are immutable';
    END IF;
    IF OLD.context_binding_status = NEW.context_binding_status
       OR (
           OLD.context_binding_status = 'executable'
           AND NEW.context_binding_status = 'tokenizer_profile_incompatible'
       ) THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'invalid agent run context binding status transition';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER agent_run_bindings_no_update
    BEFORE UPDATE ON agent_run_bindings
    FOR EACH ROW EXECUTE FUNCTION reject_agent_run_binding_update();
