CREATE OR REPLACE FUNCTION reject_publication_event_mutation()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'publication_events are append-only';
END;
$$;

CREATE TRIGGER trigger_publication_events_immutable
    BEFORE UPDATE OR DELETE ON publication_events
    FOR EACH ROW EXECUTE FUNCTION reject_publication_event_mutation();

CREATE OR REPLACE FUNCTION validate_publication_target_cover()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.cover_artifact_id IS NOT NULL AND NOT EXISTS (
        SELECT 1
        FROM publication_plans plan
        JOIN publication_handoffs handoff ON handoff.id = plan.handoff_id
        JOIN work_artifacts artifact ON artifact.id = NEW.cover_artifact_id
        WHERE plan.id = NEW.publication_plan_id
          AND artifact.work_version_id = handoff.work_version_id
    ) THEN
        RAISE EXCEPTION 'cover artifact must belong to the handed-off work version';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trigger_publication_target_cover_version
    BEFORE INSERT OR UPDATE OF cover_artifact_id, publication_plan_id ON publication_targets
    FOR EACH ROW EXECUTE FUNCTION validate_publication_target_cover();

ALTER TABLE publication_packages
    ADD CONSTRAINT publication_packages_relative_path_check
        CHECK (package_storage_path !~ '^/' AND package_storage_path !~ '(^|/)\.\.(/|$)');

COMMENT ON TRIGGER trigger_publication_events_immutable ON publication_events IS
    '发布事件只允许追加，修正结果必须插入 result_corrected 事件。';
