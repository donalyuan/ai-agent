import os
import uuid

import pytest

from video_worker.work_generation import (
    FakeWorkProvider,
    InMemoryWorkGenerationStore,
    PostgresWorkGenerationStore,
    TemporaryWorkProviderError,
    UnknownSubmissionError,
    WorkStep,
    process_next_work_generation,
    WorkAttempt,
    WORK_GENERATION_CLAIM_SQL,
    _build_slideshow_command,
    _resolve_scene_images,
    RealWorkGenerationLimits,
    WorkGenerationConfigurationError,
    _work_artifact_identity,
    validate_work_generation_mode,
)


class TemporaryOnceProvider(FakeWorkProvider):
    def __init__(self):
        self.calls = 0

    def submit(self, step):
        self.calls += 1
        if self.calls <= 2:
            raise TemporaryWorkProviderError("temporary")
        return super().submit(step)


class UnknownProvider(FakeWorkProvider):
    def submit(self, step):
        raise UnknownSubmissionError("response lost")

    def submission_audit(self):
        return {"request_sha256": "safe"}

    def output_audit(self):
        return {"error_code": "speech_network_error", "retryable": True}


class DelayedProvider(FakeWorkProvider):
    def __init__(self, *, supports_cancel=False):
        self.supports_cancel = supports_cancel
        self.submissions = []
        self.queries = []
        self.cancellations = []

    def submit(self, step):
        self.submissions.append(step.step_id)
        return f"upstream-{step.step_id}"

    def query(self, upstream_task_id):
        self.queries.append(upstream_task_id)
        return "cancelled" if upstream_task_id in self.cancellations else "running"

    def cancel(self, upstream_task_id):
        self.cancellations.append(upstream_task_id)
        return "accepted"


class PersistAwareStore(InMemoryWorkGenerationStore):
    def __init__(self, steps):
        super().__init__(steps)
        self.persisted = []

    def persist_upstream_task(self, attempt):
        self.persisted.append(attempt.upstream_task_id)


class PersistAwareProvider(DelayedProvider):
    def query(self, upstream_task_id):
        assert upstream_task_id == "upstream-video"
        return "running"

    def submit(self, step):
        return "upstream-video"


def test_steps_follow_dependencies_and_successful_attempt_is_not_replaced():
    store = InMemoryWorkGenerationStore([
        WorkStep("plan", "run", "plan"),
        WorkStep("video", "run", "video_segment", depends_on=["plan"]),
    ])
    assert process_next_work_generation(store)
    assert store.steps[0].status == "succeeded"
    assert process_next_work_generation(store)
    assert store.steps[1].status == "succeeded"
    assert len(store.steps[0].attempts) == 1


def test_temporary_submission_retries_once_then_waits_for_manual_action():
    store = InMemoryWorkGenerationStore([WorkStep("step", "run", "tts")])
    provider = TemporaryOnceProvider()
    assert process_next_work_generation(store, provider)
    assert store.steps[0].status == "queued"
    assert process_next_work_generation(store, provider)
    assert store.steps[0].status == "waiting_manual"
    assert [attempt.status for attempt in store.steps[0].attempts] == ["failed", "waiting_manual"]


def test_unknown_submission_never_retries_automatically():
    store = InMemoryWorkGenerationStore([WorkStep("step", "run", "video_segment")])
    assert process_next_work_generation(store, UnknownProvider())
    assert store.steps[0].status == "waiting_manual"
    assert store.steps[0].attempts[0].error_code == "unknown_submission"
    assert store.steps[0].attempts[0].input_snapshot == {"request_sha256": "safe"}
    assert store.steps[0].attempts[0].output_snapshot["error_code"] == "speech_network_error"
    assert process_next_work_generation(store, UnknownProvider()) is False


def test_worker_recovery_queries_existing_upstream_task_without_resubmitting():
    step = WorkStep("step", "run", "video_segment", status="running", attempts=[WorkAttempt("attempt", "step", 1, "running", "upstream-1", lease_expired=True)])
    store = InMemoryWorkGenerationStore([step])
    provider = FakeWorkProvider()
    assert process_next_work_generation(store, provider)
    assert step.status == "succeeded"
    assert len(step.attempts) == 1


def test_parallel_segments_are_independently_claimable_while_another_is_running():
    store = InMemoryWorkGenerationStore([
        WorkStep("video-1", "run", "video_segment"),
        WorkStep("video-2", "run", "video_segment"),
    ])
    provider = DelayedProvider()

    assert process_next_work_generation(store, provider)
    assert process_next_work_generation(store, provider)

    assert provider.submissions == ["video-1", "video-2"]
    assert [step.status for step in store.steps] == ["running", "running"]


def test_manual_retry_uses_precreated_attempt_and_preserves_unrelated_success():
    successful = WorkStep(
        "video-ok",
        "run",
        "video_segment",
        status="succeeded",
        attempts=[WorkAttempt("ok-1", "video-ok", 1, "succeeded", "upstream-ok")],
    )
    retry = WorkStep(
        "video-retry",
        "run",
        "video_segment",
        status="queued",
        attempts=[
            WorkAttempt("failed-1", "video-retry", 1, "failed", "upstream-failed"),
            WorkAttempt("retry-2", "video-retry", 2, "queued"),
        ],
    )
    store = InMemoryWorkGenerationStore([successful, retry])

    assert process_next_work_generation(store, FakeWorkProvider())

    assert retry.status == "succeeded"
    assert [attempt.attempt_id for attempt in retry.attempts] == ["failed-1", "retry-2"]
    assert successful.status == "succeeded"
    assert len(successful.attempts) == 1


def test_running_provider_status_remains_running_instead_of_becoming_failed():
    step = WorkStep("video", "run", "video_segment")
    store = InMemoryWorkGenerationStore([step])

    assert process_next_work_generation(store, DelayedProvider())

    assert step.status == "running"
    assert step.attempts[0].status == "running"


def test_upstream_task_id_is_persisted_before_first_query():
    store = PersistAwareStore([WorkStep("video", "run", "video_segment")])

    assert process_next_work_generation(store, PersistAwareProvider())

    assert store.persisted == ["upstream-video"]


def test_real_cost_limits_accept_only_approved_single_run():
    limits = RealWorkGenerationLimits(
        allowed_run_ids={"approved-run"},
        max_video_tasks=1,
        max_video_seconds=15,
        max_tts_characters=398,
        max_asr_tasks=0,
        max_concurrency=1,
        submit_retries=0,
    )
    step = WorkStep(
        "video",
        "approved-run",
        "video_segment",
        run_resource_usage={
            "video_task_count": 1,
            "video_seconds": 15,
            "tts_characters": 398,
            "asr_seconds": 0,
        },
        asr_step_count=0,
    )

    limits.validate_step(step)


@pytest.mark.parametrize(
    ("overrides", "message"),
    [
        ({"run_id": "other-run"}, "allowlist"),
        ({"run_resource_usage": {"video_task_count": 2}}, "视频任务数"),
        ({"run_resource_usage": {"video_task_count": 1, "video_seconds": 16}}, "视频总时长"),
        ({"run_resource_usage": {"video_task_count": 1, "video_seconds": 15, "tts_characters": 399}}, "TTS 字符数"),
        ({"asr_step_count": 1}, "ASR"),
    ],
)
def test_real_cost_limits_reject_out_of_scope_run(overrides, message):
    limits = RealWorkGenerationLimits({"approved-run"})
    values = {
        "step_id": "video",
        "run_id": "approved-run",
        "step_type": "video_segment",
        "run_resource_usage": {
            "video_task_count": 1,
            "video_seconds": 15,
            "tts_characters": 398,
            "asr_seconds": 0,
        },
        "asr_step_count": 0,
    }
    values.update(overrides)

    with pytest.raises(WorkGenerationConfigurationError, match=message):
        limits.validate_step(WorkStep(**values))


def test_worker_resource_limit_blocks_media_side_effect_before_submit():
    class LimitCheckedProvider(FakeWorkProvider):
        def __init__(self):
            self.limits = RealWorkGenerationLimits({"approved-run"})
            self.submit_calls = 0

        def bind_step(self, step):
            self.limits.validate_step(step)

        def submit(self, step):
            self.submit_calls += 1
            return super().submit(step)

    step = WorkStep(
        "video",
        "approved-run",
        "video_segment",
        run_resource_usage={
            "video_task_count": 2,
            "video_seconds": 15,
            "tts_characters": 0,
            "asr_seconds": 0,
        },
    )
    store = InMemoryWorkGenerationStore([step])
    provider = LimitCheckedProvider()

    assert process_next_work_generation(store, provider)

    assert provider.submit_calls == 0
    assert step.status == "failed"
    assert step.attempts[0].error_summary == "视频任务数超过真实生成上限"


def test_fake_and_real_modes_are_mutually_exclusive():
    with pytest.raises(WorkGenerationConfigurationError, match="不能同时启用"):
        validate_work_generation_mode(fake_enabled=True, real_enabled=True, worker_enabled=True)


@pytest.mark.parametrize(
    ("artifact_role", "file_name", "expected"),
    [
        ("video_segment", "segment.mp4", ("reusable_intermediate", "video/mp4")),
        ("tts_audio", "voice.mp3", ("audio_track", "audio/mpeg")),
        ("subtitle", "subtitle.srt", ("subtitle", "application/x-subrip")),
        ("final_video", "final.mp4", ("final_video", "video/mp4")),
    ],
)
def test_generated_material_roles_have_formal_work_artifact_identity(
    artifact_role, file_name, expected
):
    assert _work_artifact_identity(artifact_role, file_name) == expected


def test_unknown_generated_role_cannot_bypass_work_artifact_registration():
    with pytest.raises(WorkGenerationConfigurationError, match="不支持登记"):
        _work_artifact_identity("thumbnail", "thumbnail.jpg")


def test_running_cancellation_is_forwarded_only_to_capable_provider():
    attempt = WorkAttempt("attempt", "video", 1, "running", "upstream-video")
    step = WorkStep(
        "video",
        "run",
        "video_segment",
        status="running",
        attempts=[attempt],
        cancel_requested=True,
    )
    store = InMemoryWorkGenerationStore([step])
    provider = DelayedProvider(supports_cancel=True)

    assert process_next_work_generation(store, provider)

    assert provider.cancellations == ["upstream-video"]
    assert attempt.cancel_requested_at is not None
    assert attempt.cancel_response == "accepted"
    assert step.status == "cancelled"


def test_postgres_claim_query_matches_migrated_schema():
    import psycopg

    database_url = os.getenv(
        "DATABASE_URL",
        "postgres://postgres:postgres@biga-postgres:5432/video_agent",
    )
    with psycopg.connect(database_url) as connection:
        connection.execute(f"EXPLAIN {WORK_GENERATION_CLAIM_SQL}").fetchall()


def test_generated_material_and_work_artifact_are_registered_atomically(tmp_path):
    import psycopg

    database_url = os.getenv(
        "DATABASE_URL",
        "postgres://postgres:postgres@biga-postgres:5432/video_agent",
    )
    key = uuid.uuid4().hex
    with psycopg.connect(database_url) as connection:
        project_id = connection.execute(
            "INSERT INTO projects (name) VALUES (%s) RETURNING id",
            (f"work-artifact-{key}",),
        ).fetchone()[0]
        script_id = connection.execute(
            "INSERT INTO scripts (project_id,title,hook,content,status) VALUES (%s,%s,'hook','{}','approved') RETURNING id",
            (project_id, f"script-{key}"),
        ).fetchone()[0]
        work_id = connection.execute(
            "INSERT INTO works (project_id,script_id,title,status) VALUES (%s,%s,%s,'running') RETURNING id",
            (project_id, script_id, f"work-{key}"),
        ).fetchone()[0]
        version_id = connection.execute(
            "INSERT INTO work_versions (work_id,version_no,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot,status) VALUES (%s,1,%s,'{}','{}','{}','running') RETURNING id",
            (work_id, key),
        ).fetchone()[0]
        plan_id = connection.execute(
            "INSERT INTO work_plans (work_id,work_version_id,plan_version,status,input_fingerprint,capability_snapshot,output_snapshot,prompt_snapshot,timeline_snapshot) VALUES (%s,%s,1,'confirmed',%s,'{}','{}','{}','{}') RETURNING id",
            (work_id, version_id, "a" * 64),
        ).fetchone()[0]
        run_id = connection.execute(
            "INSERT INTO work_generation_runs (work_id,work_version_id,work_plan_id,idempotency_key,status,model_snapshot,capability_snapshot,prompt_snapshot,timeline_snapshot,parameter_snapshot) VALUES (%s,%s,%s,%s,'running','{}','{}','{}','{}','{}') RETURNING id",
            (work_id, version_id, plan_id, key),
        ).fetchone()[0]
        step_id = connection.execute(
            "INSERT INTO work_generation_steps (run_id,step_no,step_type,status) VALUES (%s,1,'video_segment','running') RETURNING id",
            (run_id,),
        ).fetchone()[0]
        attempt_id = connection.execute(
            "INSERT INTO work_generation_attempts (step_id,attempt_no,status,model_snapshot,resource_usage) VALUES (%s,1,'running','{}','{}') RETURNING id",
            (step_id,),
        ).fetchone()[0]
        connection.commit()

    output = tmp_path / f"{key}.mp4"
    output.write_bytes(b"deterministic-video-fixture")
    material_id = uuid.uuid4()
    step = WorkStep(
        str(step_id),
        str(run_id),
        "video_segment",
        attempts=[WorkAttempt(str(attempt_id), str(step_id), 1, "running")],
        work_id=str(work_id),
        work_version_id=str(version_id),
        project_id=str(project_id),
    )
    store = PostgresWorkGenerationStore(database_url)
    try:
        store.register_generated_material(
            step,
            material_id=material_id,
            material_type="video",
            artifact_role="video_segment",
            file_url=f"/assets/{key}.mp4",
            file_name="视频片段.mp4",
            file_path=output,
            media_metadata={"duration_ms": 4000},
            tags=["测试"],
        )
        with psycopg.connect(database_url) as connection:
            artifact = connection.execute(
                "SELECT material_id,generation_step_id,role,sha256,metadata FROM work_artifacts WHERE work_version_id=%s",
                (version_id,),
            ).fetchone()
            assert artifact[0] == material_id
            assert artifact[1] == step_id
            assert artifact[2] == "reusable_intermediate"
            assert artifact[3] == step.output_snapshot["sha256"]
            assert artifact[4]["generation_attempt_id"] == str(attempt_id)
    finally:
        with psycopg.connect(database_url) as connection:
            connection.execute("DELETE FROM work_artifacts WHERE work_version_id=%s", (version_id,))
            connection.execute("DELETE FROM materials WHERE id=%s", (material_id,))
            connection.execute("DELETE FROM projects WHERE id=%s", (project_id,))
            connection.commit()


def test_fake_compose_prefers_locked_scene_images(tmp_path):
    image = tmp_path / "generated" / "images" / "scene-1.jpg"
    image.parent.mkdir(parents=True)
    image.write_bytes(b"image")
    snapshot = {
        "scenes": [{
            "image_url": "/assets/generated/images/scene-1.jpg",
            "duration_seconds": 8,
        }]
    }

    images = _resolve_scene_images(snapshot, str(tmp_path))
    command = _build_slideshow_command(images, "/tmp/output.mp4", 1920, 1080, 15)

    assert images == [(str(image), 8)]
    assert "testsrc2" not in " ".join(command)
    assert "concat=n=1:v=1:a=0" in " ".join(command)
