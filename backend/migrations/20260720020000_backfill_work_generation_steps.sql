-- 为迁移前已经确认但尚未写入 DAG 步骤的运行补齐可审计步骤。
DO $$
DECLARE
    run_row RECORD;
    segment_row JSONB;
    plan_id UUID;
    tts_id UUID;
    asr_id UUID;
    subtitle_id UUID;
    mix_id UUID;
    video_id UUID;
    video_ids UUID[];
    step_no INT;
    audio_mode TEXT;
    tts_required BOOLEAN;
    asr_required BOOLEAN;
BEGIN
    FOR run_row IN
        SELECT r.id, r.model_snapshot, r.prompt_snapshot, r.timeline_snapshot
        FROM work_generation_runs r
        WHERE NOT EXISTS (SELECT 1 FROM work_generation_steps s WHERE s.run_id = r.id)
    LOOP
        audio_mode := COALESCE(run_row.timeline_snapshot->>'audio_mode', 'independent_tts');
        tts_required := audio_mode <> 'seedance_original';
        asr_required := audio_mode = 'seedance_original';
        step_no := 1;
        INSERT INTO work_generation_steps (run_id, step_no, step_type, status, is_required, depends_on, input_snapshot, model_snapshot)
        VALUES (run_row.id, step_no, 'plan', 'succeeded', TRUE, '[]'::jsonb, run_row.prompt_snapshot, run_row.model_snapshot)
        RETURNING id INTO plan_id;
        step_no := step_no + 1;
        INSERT INTO work_generation_steps (run_id, step_no, step_type, status, is_required, depends_on, input_snapshot, model_snapshot)
        VALUES (run_row.id, step_no, 'tts', CASE WHEN tts_required THEN 'queued' ELSE 'blocked' END, tts_required, jsonb_build_array(plan_id), jsonb_build_object('voice_snapshot', run_row.timeline_snapshot->'voice_snapshot'), run_row.model_snapshot)
        RETURNING id INTO tts_id;
        video_ids := ARRAY[]::UUID[];
        FOR segment_row IN SELECT value FROM jsonb_array_elements(COALESCE(run_row.prompt_snapshot->'segments', '[]'::jsonb))
        LOOP
            step_no := step_no + 1;
            INSERT INTO work_generation_steps (run_id, step_no, step_type, status, is_required, depends_on, input_snapshot, model_snapshot, resource_usage)
            VALUES (run_row.id, step_no, 'video_segment', 'queued', TRUE, '[]'::jsonb, segment_row, run_row.model_snapshot, jsonb_build_object('video_seconds', COALESCE((segment_row->>'duration_seconds')::INT, 0)))
            RETURNING id INTO video_id;
            video_ids := array_append(video_ids, video_id);
        END LOOP;
        step_no := step_no + 1;
        INSERT INTO work_generation_steps (run_id, step_no, step_type, status, is_required, depends_on, input_snapshot, model_snapshot)
        VALUES (run_row.id, step_no, 'asr', CASE WHEN asr_required THEN 'queued' ELSE 'blocked' END, asr_required, to_jsonb(video_ids), '{}'::jsonb, run_row.model_snapshot)
        RETURNING id INTO asr_id;
        step_no := step_no + 1;
        INSERT INTO work_generation_steps (run_id, step_no, step_type, status, is_required, depends_on, input_snapshot, model_snapshot)
        VALUES (run_row.id, step_no, 'subtitle', 'queued', TRUE, CASE WHEN asr_required THEN jsonb_build_array(asr_id) ELSE jsonb_build_array(tts_id) END, jsonb_build_object('source', run_row.timeline_snapshot->'subtitle_source'), run_row.model_snapshot)
        RETURNING id INTO subtitle_id;
        step_no := step_no + 1;
        INSERT INTO work_generation_steps (run_id, step_no, step_type, status, is_required, depends_on, input_snapshot, model_snapshot)
        VALUES (run_row.id, step_no, 'mix', 'queued', TRUE, jsonb_build_array(subtitle_id), '{}'::jsonb, jsonb_build_object('tool', 'ffmpeg'))
        RETURNING id INTO mix_id;
        step_no := step_no + 1;
        INSERT INTO work_generation_steps (run_id, step_no, step_type, status, is_required, depends_on, input_snapshot, model_snapshot)
        VALUES (run_row.id, step_no, 'compose', 'queued', TRUE, jsonb_build_array(mix_id), '{}'::jsonb, jsonb_build_object('tool', 'ffmpeg', 'format', 'mp4_h264_aac'));
    END LOOP;
END $$;
