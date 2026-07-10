from fastapi.testclient import TestClient

from video_worker.main import create_app


def test_health_returns_service_status():
    client = TestClient(create_app(enable_background_worker=False))

    response = client.get("/health")

    assert response.status_code == 200
    assert response.json() == {
        "service": "novex-video-worker",
        "status": "ok",
        "asset_generation_worker": "disabled",
    }


def test_process_next_endpoint_runs_single_image_task():
    calls: list[str] = []

    def process_next() -> bool:
        calls.append("called")
        return True

    client = TestClient(create_app(process_next_image_task=process_next))

    response = client.post("/asset-generation/process-next")

    assert response.status_code == 200
    assert response.json() == {"processed": True}
    assert calls == ["called"]
