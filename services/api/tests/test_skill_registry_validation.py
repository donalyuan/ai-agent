from __future__ import annotations

from copy import deepcopy
from pathlib import Path

import pytest
import yaml

from video_agent_api.skills.registry import SkillRegistry


def _valid_manifest() -> dict[str, object]:
    return {
        "name": "drama",
        "version": "1.0.0",
        "source_commit": "abc123",
        "license": "MIT",
        "enabled": True,
        "stages": ["script"],
        "project_types": ["short_drama"],
        "capabilities": ["scene_writing"],
        "target_models": ["configured-model"],
        "input_schema": {"type": "object"},
        "output_schema": {"type": "object"},
        "allowed_tools": ["text_model"],
        "priority": 10,
    }


def _write_skill(root: Path, folder: str, manifest: dict[str, object]) -> None:
    directory = root / folder
    directory.mkdir(parents=True)
    (directory / "manifest.yaml").write_text(
        yaml.safe_dump(manifest, sort_keys=False), encoding="utf-8"
    )
    (directory / "SKILL.md").write_text("# Skill\n", encoding="utf-8")


@pytest.mark.parametrize("missing_key", list(_valid_manifest()))
def test_registry_rejects_every_missing_required_field(tmp_path: Path, missing_key: str) -> None:
    manifest = _valid_manifest()
    del manifest[missing_key]
    _write_skill(tmp_path, "missing", manifest)

    registry = SkillRegistry(tmp_path)
    registry.load()

    assert registry.routable() == []
    assert any(f"missing required keys: {missing_key}" in error for error in registry.errors)


@pytest.mark.parametrize(
    ("field", "invalid_value"),
    [
        ("name", ""),
        ("version", "v1"),
        ("enabled", "true"),
        ("stages", "script"),
        ("project_types", []),
        ("capabilities", ["scene_writing", 1]),
        ("target_models", []),
        ("input_schema", []),
        ("output_schema", "object"),
        ("allowed_tools", []),
        ("priority", True),
    ],
)
def test_registry_rejects_empty_or_wrongly_typed_metadata(
    tmp_path: Path, field: str, invalid_value: object
) -> None:
    manifest = _valid_manifest()
    manifest[field] = invalid_value
    _write_skill(tmp_path, "invalid", manifest)

    registry = SkillRegistry(tmp_path)
    registry.load()

    assert registry.routable() == []
    assert any(field in error for error in registry.errors)


def test_registry_reports_disabled_skill_and_keeps_it_non_routable(tmp_path: Path) -> None:
    manifest = _valid_manifest()
    manifest["enabled"] = False
    _write_skill(tmp_path, "disabled", manifest)

    registry = SkillRegistry(tmp_path)
    registry.load()

    assert [record.name for record in registry.list()] == ["drama"]
    assert registry.routable() == []
    assert any("disabled" in error for error in registry.errors)


def test_registry_rejects_duplicate_name_and_version_without_overwrite(tmp_path: Path) -> None:
    first = _valid_manifest()
    second = deepcopy(first)
    second["source_commit"] = "def456"
    _write_skill(tmp_path, "first", first)
    _write_skill(tmp_path, "second", second)

    registry = SkillRegistry(tmp_path)
    registry.load()

    assert registry.list() == []
    assert registry.routable() == []
    assert any("duplicate skill revision: drama@1.0.0" in error for error in registry.errors)
