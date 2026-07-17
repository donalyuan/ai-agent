import json
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from uuid import UUID, uuid4

import pytest

from video_worker.voice_catalog import (
    VoiceCatalogPage,
    VoiceCatalogSyncError,
    VoiceCatalogSyncJob,
    VoiceCatalogSynchronizer,
    VolcengineVoiceCatalogProvider,
    run_next_voice_catalog_sync,
)


@dataclass
class FakeProvider:
    pages: list[VoiceCatalogPage]
    fail_on_page: int | None = None

    def list_speakers(self, resource_id: str, page: int, limit: int) -> VoiceCatalogPage:
        assert resource_id == "seed-tts-2.0"
        assert limit == 30
        if page == self.fail_on_page:
            raise VoiceCatalogSyncError("temporary_provider_error", "fixture page failed")
        return self.pages[page - 1]


class FakeStore:
    def __init__(self) -> None:
        self.catalog = [{"voice_type": "old_voice", "is_available": True}]
        self.replacements: list[tuple[UUID, UUID, list[dict]]] = []
        self.failed: list[tuple[UUID, str]] = []

    def replace_catalog(
        self,
        sync_id: UUID,
        model_id: UUID,
        speakers: list[dict],
    ) -> None:
        self.replacements.append((sync_id, model_id, speakers))
        seen = {speaker["VoiceType"] for speaker in speakers}
        updated = [
            {"voice_type": item["voice_type"], "is_available": item["voice_type"] in seen}
            for item in self.catalog
        ]
        for speaker in speakers:
            if speaker["VoiceType"] not in {item["voice_type"] for item in updated}:
                updated.append({"voice_type": speaker["VoiceType"], "is_available": True})
        self.catalog = updated

    def fail_sync(self, sync_id: UUID, error_summary: str) -> None:
        self.failed.append((sync_id, error_summary))


def page(page_number: int, total: int, voices: list[str]) -> VoiceCatalogPage:
    return VoiceCatalogPage(
        page=page_number,
        limit=30,
        total=total,
        speakers=[
            {
                "VoiceType": voice,
                "Name": voice,
                "ResourceID": "seed-tts-2.0",
                "Languages": [],
                "Emotions": [],
            }
            for voice in voices
        ],
    )


def test_paginated_failure_keeps_previous_complete_catalog() -> None:
    model_id = uuid4()
    sync_id = uuid4()
    store = FakeStore()
    provider = FakeProvider(
        pages=[page(1, 2, ["new_voice"]), page(2, 2, ["last_voice"])],
        fail_on_page=2,
    )

    with pytest.raises(VoiceCatalogSyncError, match="fixture page failed"):
        VoiceCatalogSynchronizer(store, provider).synchronize(
            sync_id=sync_id,
            model_id=model_id,
            resource_id="seed-tts-2.0",
        )

    assert store.catalog == [{"voice_type": "old_voice", "is_available": True}]
    assert store.replacements == []
    assert store.failed == [(sync_id, "fixture page failed")]


def test_unexpected_catalog_failure_persists_only_safe_error_type() -> None:
    model_id = uuid4()
    sync_id = uuid4()
    store = FakeStore()
    provider = FakeProvider(pages=[page(1, 1, ["new_voice"])])

    def fail_replace(*_args) -> None:
        raise RuntimeError("https://provider.example/private-row?signature=secret")

    store.replace_catalog = fail_replace  # type: ignore[method-assign]

    with pytest.raises(VoiceCatalogSyncError, match="RuntimeError"):
        VoiceCatalogSynchronizer(store, provider).synchronize(
            sync_id=sync_id,
            model_id=model_id,
            resource_id="seed-tts-2.0",
        )

    assert store.failed == [(sync_id, "音色目录同步失败: RuntimeError")]


def test_complete_pages_are_committed_once_and_missing_voice_becomes_unavailable() -> None:
    model_id = uuid4()
    sync_id = uuid4()
    store = FakeStore()
    provider = FakeProvider(
        pages=[page(1, 2, ["new_voice"]), page(2, 2, ["last_voice"])]
    )

    result = VoiceCatalogSynchronizer(store, provider).synchronize(
        sync_id=sync_id,
        model_id=model_id,
        resource_id="seed-tts-2.0",
    )

    assert result.page_count == 2
    assert result.speaker_count == 2
    assert len(store.replacements) == 1
    assert store.catalog == [
        {"voice_type": "old_voice", "is_available": False},
        {"voice_type": "new_voice", "is_available": True},
        {"voice_type": "last_voice", "is_available": True},
    ]


def test_optional_catalog_collections_normalize_null_to_empty_lists() -> None:
    model_id = uuid4()
    sync_id = uuid4()
    store = FakeStore()
    speaker = {
        "VoiceType": "nullable_voice",
        "Name": "Nullable voice",
        "ResourceID": "seed-tts-2.0",
        "Categories": None,
        "NormalLabels": None,
        "SpecialLabels": None,
        "Languages": None,
        "Emotions": None,
    }
    provider = FakeProvider(
        pages=[VoiceCatalogPage(page=1, limit=30, total=1, speakers=[speaker])]
    )

    VoiceCatalogSynchronizer(store, provider).synchronize(
        sync_id=sync_id,
        model_id=model_id,
        resource_id="seed-tts-2.0",
    )

    committed = store.replacements[0][2][0]
    assert committed["Categories"] == []
    assert committed["NormalLabels"] == []
    assert committed["SpecialLabels"] == []
    assert committed["Languages"] == []
    assert committed["Emotions"] == []


def test_list_speakers_request_uses_signed_official_contract_without_secret_leak() -> None:
    fixture = (
        Path(__file__).parent / "fixtures" / "speech" / "list_speakers_page_1.json"
    ).read_bytes()
    captured = {}

    def transport(request, timeout):
        captured["url"] = request.full_url
        captured["headers"] = dict(request.header_items())
        captured["body"] = request.data
        captured["timeout"] = timeout
        return 200, {}, fixture

    provider = VolcengineVoiceCatalogProvider(
        access_key="fixture-access-key",
        secret_key="fixture-secret-key",
        timeout_seconds=12,
        transport=transport,
        now=lambda: datetime(2026, 7, 15, 8, 0, tzinfo=UTC),
    )

    result = provider.list_speakers("seed-tts-2.0", 1, 30)

    assert result.total == 1
    assert "Action=ListSpeakers" in captured["url"]
    assert "Version=2025-05-20" in captured["url"]
    assert json.loads(captured["body"]) == {
        "ResourceIDs": ["seed-tts-2.0"],
        "VoiceTypes": [],
        "Page": 1,
        "Limit": 30,
    }
    headers = {key.lower(): value for key, value in captured["headers"].items()}
    assert headers["x-date"] == "20260715T080000Z"
    assert headers["authorization"].startswith("HMAC-SHA256 Credential=")
    assert "fixture-secret-key" not in json.dumps(captured, default=str)


def test_worker_claims_and_completes_one_catalog_sync() -> None:
    model_id = uuid4()
    sync_id = uuid4()
    store = FakeStore()
    store.enqueued = 0
    store.job = VoiceCatalogSyncJob(
        sync_id=sync_id,
        model_id=model_id,
        resource_id="seed-tts-2.0",
        page_limit=30,
        access_key="fixture-access",
        secret_key="fixture-secret",
        timeout_seconds=10,
    )
    store.enqueue_due_syncs = lambda: 0
    store.claim_next_sync = lambda: store.job
    provider = FakeProvider(pages=[page(1, 1, ["new_voice"])])

    processed = run_next_voice_catalog_sync(store, lambda job: provider)

    assert processed is True
    assert store.replacements[0][0] == sync_id
    assert store.catalog[-1] == {"voice_type": "new_voice", "is_available": True}
