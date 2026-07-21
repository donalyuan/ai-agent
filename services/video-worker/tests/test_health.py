from threading import Event

from fastapi.testclient import TestClient

from video_worker.main import create_app


def test_health_returns_service_status():
    client = TestClient(
        create_app(
            enable_background_worker=False,
            enable_voice_catalog_worker=False,
            enable_speech_worker=False,
            enable_tos_tool_worker=False,
            enable_work_generation_worker=False,
        )
    )

    response = client.get("/health")

    assert response.status_code == 200
    assert response.json() == {
        "service": "novex-video-worker",
        "status": "ok",
        "asset_generation_worker": "disabled",
        "voice_catalog_worker": "disabled",
        "speech_generation_worker": "disabled",
        "tos_tool_worker": "disabled",
        "work_generation_worker": "disabled",
    }


def test_voice_catalog_background_worker_is_independent_from_speech_generation():
    catalog_processed = Event()
    speech_calls: list[str] = []

    def process_catalog() -> bool:
        catalog_processed.set()
        return True

    def process_speech() -> bool:
        speech_calls.append("called")
        return True

    app = create_app(
        process_next_voice_catalog=process_catalog,
        process_next_speech_work=process_speech,
        enable_background_worker=False,
        enable_voice_catalog_worker=True,
        enable_speech_worker=False,
        enable_tos_tool_worker=False,
        enable_work_generation_worker=False,
    )

    with TestClient(app) as client:
        assert catalog_processed.wait(timeout=1)
        assert client.get("/health").json()["voice_catalog_worker"] == "enabled"
        assert client.get("/health").json()["speech_generation_worker"] == "disabled"

    assert speech_calls == []


def test_work_generation_background_worker_runs_when_explicitly_enabled():
    work_processed = Event()

    def process_work_generation() -> bool:
        work_processed.set()
        return True

    app = create_app(
        process_next_work_generation=process_work_generation,
        enable_background_worker=False,
        enable_voice_catalog_worker=False,
        enable_speech_worker=False,
        enable_tos_tool_worker=False,
        enable_work_generation_worker=True,
    )

    with TestClient(app) as client:
        assert work_processed.wait(timeout=1)
        assert client.get("/health").json()["work_generation_worker"] == "enabled"


def test_process_next_endpoint_runs_single_image_task():
    calls: list[str] = []

    def process_next() -> bool:
        calls.append("called")
        return True

    client = TestClient(
        create_app(
            process_next_image_task=process_next,
            enable_background_worker=False,
            enable_voice_catalog_worker=False,
            enable_speech_worker=False,
            enable_tos_tool_worker=False,
            enable_work_generation_worker=False,
        )
    )

    response = client.post("/asset-generation/process-next")

    assert response.status_code == 200
    assert response.json() == {"processed": True}
    assert calls == ["called"]


def test_process_next_voice_catalog_endpoint_runs_single_sync():
    calls: list[str] = []

    def process_next() -> bool:
        calls.append("called")
        return True

    client = TestClient(
        create_app(
            process_next_voice_catalog=process_next,
            enable_background_worker=False,
            enable_voice_catalog_worker=False,
            enable_speech_worker=False,
            enable_tos_tool_worker=False,
            enable_work_generation_worker=False,
        )
    )

    response = client.post("/speech/voice-catalog/process-next")

    assert response.status_code == 200
    assert response.json() == {"processed": True}
    assert calls == ["called"]


def test_process_next_speech_endpoint_runs_inspection_generation_and_cleanup_cycle():
    calls: list[str] = []

    def process_next() -> bool:
        calls.append("called")
        return True

    client = TestClient(
        create_app(
            process_next_speech_work=process_next,
            enable_background_worker=False,
            enable_voice_catalog_worker=False,
            enable_speech_worker=False,
            enable_tos_tool_worker=False,
            enable_work_generation_worker=False,
        )
    )

    response = client.post("/speech/process-next")

    assert response.status_code == 200
    assert response.json() == {"processed": True}
    assert calls == ["called"]


def test_process_next_tos_tool_endpoint_is_independent_from_speech_generation():
    calls: list[str] = []

    def process_next() -> bool:
        calls.append("called")
        return True

    client = TestClient(
        create_app(
            process_next_tos_tool_work=process_next,
            enable_background_worker=False,
            enable_voice_catalog_worker=False,
            enable_speech_worker=False,
            enable_tos_tool_worker=False,
            enable_work_generation_worker=False,
        )
    )

    response = client.post("/tools/tos-staging/process-next")

    assert response.status_code == 200
    assert response.json() == {"processed": True}
    assert calls == ["called"]


def test_process_next_work_generation_endpoint_runs_one_controlled_cycle():
    calls: list[str] = []

    client = TestClient(
        create_app(
            process_next_work_generation=lambda: calls.append("called") is None,
            enable_background_worker=False,
            enable_voice_catalog_worker=False,
            enable_speech_worker=False,
            enable_tos_tool_worker=False,
            enable_work_generation_worker=False,
        )
    )

    response = client.post("/work-generation/process-next")

    assert response.status_code == 200
    assert response.json() == {"processed": True}
    assert calls == ["called"]
