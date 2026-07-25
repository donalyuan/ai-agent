-- Definition evidence is immutable. Conversation model evidence may only be filled once.
ALTER TABLE agent_conversation_bindings
    ADD CONSTRAINT agent_conversation_bindings_prompt_object_check
    CHECK (jsonb_typeof(prompt_bindings) = 'object' AND prompt_bindings <> '{}'::jsonb),
    ADD CONSTRAINT agent_conversation_bindings_capabilities_object_check
    CHECK (model_capabilities IS NULL OR jsonb_typeof(model_capabilities) = 'object');

ALTER TABLE agent_run_bindings
    ADD CONSTRAINT agent_run_bindings_prompt_object_check
    CHECK (jsonb_typeof(prompt_bindings) = 'object' AND prompt_bindings <> '{}'::jsonb),
    ADD CONSTRAINT agent_run_bindings_capabilities_object_check
    CHECK (jsonb_typeof(model_capabilities) = 'object');

CREATE FUNCTION enforce_conversation_binding_update() RETURNS TRIGGER AS $$
BEGIN
    IF ROW(
        NEW.conversation_id, NEW.agent_key, NEW.agent_version, NEW.agent_digest,
        NEW.prompt_bindings, NEW.registry_digest, NEW.migration_source,
        NEW.parent_conversation_id, NEW.created_at
    ) IS DISTINCT FROM ROW(
        OLD.conversation_id, OLD.agent_key, OLD.agent_version, OLD.agent_digest,
        OLD.prompt_bindings, OLD.registry_digest, OLD.migration_source,
        OLD.parent_conversation_id, OLD.created_at
    ) THEN
        RAISE EXCEPTION 'conversation definition binding is immutable';
    END IF;

    IF OLD.model_id IS NULL
       AND OLD.behavior_fingerprint IS NULL
       AND OLD.model_capabilities IS NULL
       AND OLD.binding_status = 'definition_bound'
       AND NEW.model_id IS NOT NULL
       AND NEW.behavior_fingerprint IS NOT NULL
       AND NEW.model_capabilities IS NOT NULL
       AND NEW.binding_status = 'executable' THEN
        RETURN NEW;
    END IF;

    IF ROW(NEW.model_id, NEW.behavior_fingerprint, NEW.model_capabilities)
       IS DISTINCT FROM ROW(OLD.model_id, OLD.behavior_fingerprint, OLD.model_capabilities) THEN
        RAISE EXCEPTION 'conversation model binding is immutable';
    END IF;

    IF OLD.binding_status = NEW.binding_status
       OR (OLD.binding_status = 'definition_bound' AND NEW.binding_status = 'read_only')
       OR (OLD.binding_status = 'executable' AND NEW.binding_status = 'model_rebind_required') THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'invalid conversation binding status transition';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER agent_conversation_bindings_update_guard
    BEFORE UPDATE ON agent_conversation_bindings
    FOR EACH ROW EXECUTE FUNCTION enforce_conversation_binding_update();

CREATE FUNCTION reject_agent_run_binding_update() RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'agent run bindings are immutable';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER agent_run_bindings_no_update
    BEFORE UPDATE ON agent_run_bindings
    FOR EACH ROW EXECUTE FUNCTION reject_agent_run_binding_update();
