"""只读取本地 manifest；不会下载、更新或执行第三方 Skill。"""

from __future__ import annotations

import builtins
import re
from dataclasses import dataclass
from pathlib import Path

import yaml

_SEMANTIC_VERSION = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
_REQUIRED_FIELDS = {
    "name",
    "version",
    "source_commit",
    "license",
    "enabled",
    "stages",
    "project_types",
    "capabilities",
    "target_models",
    "input_schema",
    "output_schema",
    "allowed_tools",
    "priority",
}


def _required_text(raw: dict[str, object], key: str) -> str:
    value = raw[key]
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{key} must be a non-empty string")
    return value


def _required_string_list(raw: dict[str, object], key: str) -> frozenset[str]:
    value = raw[key]
    if (
        not isinstance(value, list)
        or not value
        or any(not isinstance(item, str) or not item.strip() for item in value)
    ):
        raise ValueError(f"{key} must be a non-empty list of non-empty strings")
    if len(value) != len(set(value)):
        raise ValueError(f"{key} must not contain duplicates")
    return frozenset(value)


def _required_schema(raw: dict[str, object], key: str) -> dict[str, object]:
    value = raw[key]
    if not isinstance(value, dict) or not value or any(not isinstance(item, str) for item in value):
        raise ValueError(f"{key} must be a non-empty object with string keys")
    return dict(value)


@dataclass(frozen=True, slots=True)
class SkillRevision:
    name: str
    version: str
    source_commit: str
    license: str
    enabled: bool
    stages: frozenset[str]
    project_types: frozenset[str]
    capabilities: frozenset[str]
    target_models: frozenset[str]
    input_schema: dict[str, object]
    output_schema: dict[str, object]
    allowed_tools: frozenset[str]
    priority: int
    directory: Path


class SkillRegistry:
    def __init__(self, root: Path) -> None:
        self._root = root
        self._records: dict[tuple[str, str], SkillRevision] = {}
        self._conflicts: set[tuple[str, str]] = set()
        self.errors: list[str] = []

    def load(self) -> None:
        self._records.clear()
        self._conflicts.clear()
        self.errors.clear()
        for manifest_path in sorted(self._root.rglob("manifest.yaml")):
            self._load_manifest(manifest_path)

    def _load_manifest(self, path: Path) -> None:
        try:
            loaded = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
            if not isinstance(loaded, dict) or any(not isinstance(key, str) for key in loaded):
                raise ValueError("manifest must be an object with string keys")
            raw: dict[str, object] = dict(loaded)
            missing = sorted(_REQUIRED_FIELDS - raw.keys())
            if missing:
                raise ValueError(f"missing required keys: {', '.join(missing)}")
            name = _required_text(raw, "name")
            version = _required_text(raw, "version")
            if not _SEMANTIC_VERSION.fullmatch(version):
                raise ValueError("version must use semantic x.y.z form")
            enabled = raw["enabled"]
            if not isinstance(enabled, bool):
                raise ValueError("enabled must be a boolean")
            priority = raw["priority"]
            if isinstance(priority, bool) or not isinstance(priority, int):
                raise ValueError("priority must be an integer")
            if not (path.parent / "SKILL.md").is_file():
                raise ValueError("SKILL.md is required beside manifest.yaml")
            revision = SkillRevision(
                name=name,
                version=version,
                source_commit=_required_text(raw, "source_commit"),
                license=_required_text(raw, "license"),
                enabled=enabled,
                stages=_required_string_list(raw, "stages"),
                project_types=_required_string_list(raw, "project_types"),
                capabilities=_required_string_list(raw, "capabilities"),
                target_models=_required_string_list(raw, "target_models"),
                input_schema=_required_schema(raw, "input_schema"),
                output_schema=_required_schema(raw, "output_schema"),
                allowed_tools=_required_string_list(raw, "allowed_tools"),
                priority=priority,
                directory=path.parent,
            )
            key = (revision.name, revision.version)
            if key in self._records or key in self._conflicts:
                self._records.pop(key, None)
                self._conflicts.add(key)
                raise ValueError(f"duplicate skill revision: {revision.name}@{revision.version}")
            self._records[key] = revision
            if not revision.enabled:
                self.errors.append(f"{path}: skill disabled: {revision.name}@{revision.version}")
        except (OSError, TypeError, ValueError, yaml.YAMLError) as error:
            self.errors.append(f"{path}: {error}")

    def list(self) -> builtins.list[SkillRevision]:
        return sorted(self._records.values(), key=lambda item: (item.name, item.version))

    def search(self, query: str) -> builtins.list[SkillRevision]:
        needle = query.lower()
        return [
            item
            for item in self.list()
            if needle in " ".join((item.name, *item.capabilities)).lower()
        ]

    def read(self, name: str, version: str) -> str:
        record = self.resolve(name, version)
        return (record.directory / "SKILL.md").read_text(encoding="utf-8")

    def resolve(self, name: str, version: str) -> SkillRevision:
        record = self._records.get((name, version))
        if record is None:
            raise LookupError(f"skill revision is not registered: {name}@{version}")
        return record

    def routable(self) -> builtins.list[SkillRevision]:
        return [record for record in self.list() if record.enabled]
