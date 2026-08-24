from __future__ import annotations

from copy import deepcopy
from hashlib import sha256
from pathlib import Path

import pytest
import yaml

from video_agent_api.skills.registry import SkillRegistry


def _candidate() -> dict[str, object]:
    content = b"# Skill\n"
    return {
        "name": "drama",
        "version": "1.0.0",
        "sourceType": "git",
        "sourceIdentity": "https://example.test/drama@abc123",
        "upstreamDigest": "a" * 64,
        "runtimeDigest": sha256(content).hexdigest(),
        "provenance": "verified_snapshot",
        "approval": "approved",
        "license": "MIT",
        "licenseStatus": "verified",
        "enabled": True,
        "stages": ["text.generate"],
        "projectTypes": ["short_drama"],
        "capabilities": ["story_spec"],
        "targetModels": ["configured-model"],
        "inputSchema": {"type": "object"},
        "outputSchema": {"type": "object"},
        "allowedTools": ["text_model"],
        "access": {"network": False, "subprocess": False, "file": False, "secret": False},
        "scriptsAllowed": False,
        "priority": 10,
        "contentPath": "drama/1.0.0/SKILL.md",
    }


def _write_index(root: Path, candidates: list[dict[str, object]]) -> None:
    root.mkdir(parents=True, exist_ok=True)
    (root / "index.yaml").write_text(
        yaml.safe_dump({"schemaVersion": "1.0.0", "candidates": candidates}, sort_keys=False),
        encoding="utf-8",
    )
    for candidate in candidates:
        path = candidate.get("contentPath")
        if isinstance(path, str):
            target = root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text("# Skill\n", encoding="utf-8")


@pytest.mark.parametrize("missing_key", list(_candidate()))
def test_registry_rejects_every_missing_required_field(tmp_path: Path, missing_key: str) -> None:
    candidate = _candidate()
    del candidate[missing_key]
    _write_index(tmp_path, [candidate])
    registry = SkillRegistry(tmp_path)
    registry.load()
    assert registry.routable() == []
    assert any(missing_key in error for error in registry.errors)


@pytest.mark.parametrize(
    ("field", "invalid_value"),
    [
        ("version", "v1"),
        ("sourceType", "local"),
        ("enabled", "true"),
        ("stages", []),
        ("access", {"network": False}),
        ("scriptsAllowed", True),
        ("priority", True),
    ],
)
def test_registry_rejects_invalid_or_unsafe_metadata(
    tmp_path: Path, field: str, invalid_value: object
) -> None:
    candidate = _candidate()
    candidate[field] = invalid_value
    _write_index(tmp_path, [candidate])
    registry = SkillRegistry(tmp_path)
    registry.load()
    assert registry.routable() == []
    assert registry.errors


def test_pending_candidate_is_metadata_only_and_not_routable(tmp_path: Path) -> None:
    candidate = _candidate()
    candidate.update(
        {
            "provenance": "pending_provenance",
            "approval": "not_approved",
            "enabled": False,
            "upstreamDigest": None,
            "runtimeDigest": None,
        }
    )
    candidate.pop("contentPath")
    _write_index(tmp_path, [candidate])
    registry = SkillRegistry(tmp_path)
    registry.load()
    assert [record.name for record in registry.list()] == ["drama"]
    assert registry.routable() == []
    with pytest.raises(PermissionError, match="not approved"):
        registry.load_selected_content(
            "drama",
            "1.0.0",
            allowed_skills=frozenset({"drama"}),
            required_capabilities=frozenset({"story_spec"}),
            selection_mode="fixed",
        )


def test_selected_content_requires_policy_and_exact_digest(tmp_path: Path) -> None:
    candidate = _candidate()
    _write_index(tmp_path, [candidate])
    registry = SkillRegistry(tmp_path)
    registry.load()
    with pytest.raises(PermissionError, match="outside"):
        registry.load_selected_content(
            "drama",
            "1.0.0",
            allowed_skills=frozenset(),
            required_capabilities=frozenset(),
            selection_mode="fixed",
        )
    (tmp_path / "drama/1.0.0/SKILL.md").write_text("# Drift\n", encoding="utf-8")
    with pytest.raises(ValueError, match="digest drift"):
        registry.load_selected_content(
            "drama",
            "1.0.0",
            allowed_skills=frozenset({"drama"}),
            required_capabilities=frozenset({"story_spec"}),
            selection_mode="inherit",
        )


def test_duplicate_revision_is_rejected_without_overwrite(tmp_path: Path) -> None:
    first = _candidate()
    second = deepcopy(first)
    second["sourceIdentity"] = "https://example.test/drama@def456"
    _write_index(tmp_path, [first, second])
    registry = SkillRegistry(tmp_path)
    registry.load()
    assert len(registry.list()) == 1
    assert any("duplicate skill revision" in error for error in registry.errors)
