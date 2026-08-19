from __future__ import annotations

from fastapi.testclient import TestClient

from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.app import create_app
from video_agent_api.application.projects_episodes import ProjectsEpisodesService


def _client() -> TestClient:
    uow = InMemoryUnitOfWork()
    app = create_app(
        readiness_probe=lambda: True,
        projects_episodes_service=ProjectsEpisodesService(lambda: uow),
    )
    return TestClient(app)


def test_projects_episodes_http_contract_and_if_match() -> None:
    client = _client()
    response = client.post("/v1/projects", json={"name": "Demo"})
    assert response.status_code == 201
    project = response.json()
    assert project["status"] == "draft"
    assert project["schemaVersion"] == "1.0.0"
    assert project["revision"] == 1

    episode_response = client.post(
        f"/v1/projects/{project['id']}/episodes", json={"number": 1, "title": "Opening"}
    )
    assert episode_response.status_code == 201
    episode = episode_response.json()
    assert episode["projectId"] == project["id"]
    assert client.get(f"/v1/projects/{project['id']}/episodes").json()[0]["title"] == "Opening"

    updated = client.patch(
        f"/v1/projects/{project['id']}", headers={"If-Match": "1"}, json={"name": "Changed"}
    )
    assert updated.status_code == 200
    assert updated.json()["revision"] == 2
    conflict = client.patch(
        f"/v1/projects/{project['id']}", headers={"If-Match": "1"}, json={"name": "Stale"}
    )
    assert conflict.status_code == 409
    assert conflict.json()["detail"]["type"] == "revision_conflict"


def test_http_validation_not_found_and_database_unavailable(monkeypatch) -> None:
    client = _client()
    assert client.post("/v1/projects", json={"name": " "}).status_code == 422
    missing = client.post("/v1/projects/missing/episodes", json={"number": 1, "title": "Opening"})
    assert missing.status_code == 404
    assert missing.json()["detail"]["type"] == "project_not_found"

    unavailable = TestClient(create_app(readiness_probe=lambda: True))
    response = unavailable.get("/v1/projects")
    assert response.status_code == 503
    assert response.json()["detail"]["type"] == "database_unavailable"

    monkeypatch.setenv("DATABASE_URL", "postgresql+asyncpg://x:x@127.0.0.1:1/unavailable")
    unreachable = TestClient(create_app(readiness_probe=lambda: True))
    response = unreachable.get("/v1/projects")
    assert response.status_code == 503
    assert response.json()["detail"]["type"] == "database_unavailable"
