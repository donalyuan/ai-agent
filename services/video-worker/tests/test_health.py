from fastapi.testclient import TestClient

from video_worker.main import app


def test_health_returns_service_status():
    client = TestClient(app)

    response = client.get("/health")

    assert response.status_code == 200
    assert response.json() == {
        "service": "novex-video-worker",
        "status": "ok",
    }
