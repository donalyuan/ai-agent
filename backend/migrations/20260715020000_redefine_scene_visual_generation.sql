-- Redefine asset generation as image-only scene visual preparation.
-- Historical per-scene video tasks and candidates stay in place for audit reads.

UPDATE video_workspace_menus
SET
    label = '画面生成',
    description = '按脚本分镜生成、复用和选择图片候选，为每个分镜确认唯一主画面。',
    icon = 'images',
    updated_at = NOW()
WHERE menu_key = 'asset-generation';

COMMENT ON TABLE asset_generation_tasks IS
    '画面生成任务；新任务只允许图片候选，历史 video_draft/video_generation 记录只读保留。';
COMMENT ON TABLE scene_asset_candidates IS
    '分镜画面候选；新候选只允许图片，历史 video/video_task 候选只读保留。';

DROP INDEX scene_asset_candidates_one_selected_per_scene;
CREATE UNIQUE INDEX scene_asset_candidates_one_selected_per_scene
    ON scene_asset_candidates(scene_id)
    WHERE status = 'selected' AND candidate_type = 'image';
COMMENT ON INDEX scene_asset_candidates_one_selected_per_scene IS
    '同一分镜最多一个已选主图片；历史已选视频候选不参与新画面唯一性约束。';

CREATE OR REPLACE FUNCTION freeze_legacy_asset_video_task_writes()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.task_type IN ('video_draft', 'video_generation') THEN
            RAISE EXCEPTION 'legacy per-scene video tasks are read-only'
                USING ERRCODE = 'check_violation';
        END IF;
        RETURN NEW;
    END IF;

    IF OLD.task_type IN ('video_draft', 'video_generation') THEN
        RAISE EXCEPTION 'legacy per-scene video tasks are read-only'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

COMMENT ON FUNCTION freeze_legacy_asset_video_task_writes() IS
    '禁止新增、修改或删除历史逐分镜视频任务；只允许查询既有审计记录。';

CREATE TRIGGER trigger_freeze_legacy_asset_video_tasks
    BEFORE INSERT OR UPDATE OR DELETE ON asset_generation_tasks
    FOR EACH ROW
    EXECUTE FUNCTION freeze_legacy_asset_video_task_writes();

CREATE OR REPLACE FUNCTION freeze_legacy_video_candidate_writes()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        IF NEW.candidate_type = 'video' OR NEW.source = 'video_task' THEN
            RAISE EXCEPTION 'legacy per-scene video candidates are read-only'
                USING ERRCODE = 'check_violation';
        END IF;
        RETURN NEW;
    END IF;

    IF OLD.candidate_type = 'video' OR OLD.source = 'video_task' THEN
        RAISE EXCEPTION 'legacy per-scene video candidates are read-only'
            USING ERRCODE = 'check_violation';
    END IF;

    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

COMMENT ON FUNCTION freeze_legacy_video_candidate_writes() IS
    '禁止新增、修改或删除历史逐分镜视频候选；只允许查询既有审计记录。';

CREATE TRIGGER trigger_freeze_legacy_video_candidates
    BEFORE INSERT OR UPDATE OR DELETE ON scene_asset_candidates
    FOR EACH ROW
    EXECUTE FUNCTION freeze_legacy_video_candidate_writes();
