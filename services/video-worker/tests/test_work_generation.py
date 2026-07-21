import os

import pytest

from video_worker.work_generation import (
    FakeWorkProvider,
    InMemoryWorkGenerationStore,
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


def test_fake_and_real_modes_are_mutually_exclusive():
    with pytest.raises(WorkGenerationConfigurationError, match="不能同时启用"):
        validate_work_generation_mode(fake_enabled=True, real_enabled=True, worker_enabled=True)


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
