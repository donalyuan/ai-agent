-- Historical execution data records only what can be proven; missing call snapshots are never fabricated.
ALTER TABLE agent_runs
    ADD COLUMN legacy_partial_audit BOOLEAN NOT NULL DEFAULT FALSE;

CREATE TABLE agent_history_migration_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type VARCHAR(24) NOT NULL,
    entity_id UUID NOT NULL,
    disposition VARCHAR(48) NOT NULL,
    evidence JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_history_migration_entity_check
        CHECK (entity_type IN ('conversation', 'agent_run')),
    CONSTRAINT agent_history_migration_unique UNIQUE (entity_type, entity_id)
);

COMMENT ON TABLE agent_history_migration_events IS
    '历史 Conversation/Run 的幂等迁移证据；只记录可证明映射和 partial 状态，不伪造 ModelCall。';

CREATE FUNCTION reject_agent_history_migration_event_mutation() RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'agent history migration events are immutable';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER agent_history_migration_events_immutable
    BEFORE UPDATE OR DELETE ON agent_history_migration_events
    FOR EACH ROW EXECUTE FUNCTION reject_agent_history_migration_event_mutation();
