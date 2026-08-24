"""Skill Registry metadata index and route-gated content loader."""

from __future__ import annotations

import builtins
import re
from dataclasses import dataclass
from hashlib import sha256
from pathlib import Path
from typing import Literal, cast

import yaml

_SEMANTIC_VERSION = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
_DIGEST = re.compile(r"^[a-f0-9]{64}$")
_FORBIDDEN_ACCESS = {"network", "subprocess", "file", "secret"}
_REQUIRED_FIELDS = {
    "name",
    "version",
    "sourceType",
    "sourceIdentity",
    "upstreamDigest",
    "runtimeDigest",
    "provenance",
    "approval",
    "license",
    "licenseStatus",
    "enabled",
    "stages",
    "projectTypes",
    "capabilities",
    "targetModels",
    "inputSchema",
    "outputSchema",
    "allowedTools",
    "access",
    "scriptsAllowed",
    "priority",
}


def _text(raw: dict[str, object], key: str) -> str:
    value = raw.get(key)
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{key} must be a non-empty string")
    return value.strip()


def _string_set(raw: dict[str, object], key: str) -> frozenset[str]:
    value = raw.get(key)
    if (
        not isinstance(value, list)
        or not value
        or any(not isinstance(item, str) or not item.strip() for item in value)
        or len(value) != len(set(cast(list[str], value)))
    ):
        raise ValueError(f"{key} must be a non-empty unique string list")
    return frozenset(cast(list[str], value))


def _schema(raw: dict[str, object], key: str) -> dict[str, object]:
    value = raw.get(key)
    if not isinstance(value, dict) or not value or any(not isinstance(item, str) for item in value):
        raise ValueError(f"{key} must be a non-empty object")
    return cast(dict[str, object], value)


def _access(raw: dict[str, object]) -> dict[str, bool]:
    value = raw.get("access")
    if not isinstance(value, dict) or set(value) != _FORBIDDEN_ACCESS:
        raise ValueError("access must explicitly declare network/subprocess/file/secret")
    if any(not isinstance(flag, bool) for flag in value.values()):
        raise ValueError("access values must be booleans")
    return cast(dict[str, bool], value)


@dataclass(frozen=True, slots=True)
class SkillRevision:
    name: str
    version: str
    source_type: Literal["git", "public_markdown"]
    source_identity: str
    upstream_digest: str | None
    runtime_digest: str | None
    provenance: Literal["verified_snapshot", "pending_provenance"]
    approval: Literal["approved", "not_approved"]
    license: str
    license_status: str
    enabled: bool
    stages: frozenset[str]
    project_types: frozenset[str]
    capabilities: frozenset[str]
    target_models: frozenset[str]
    input_schema: dict[str, object]
    output_schema: dict[str, object]
    allowed_tools: frozenset[str]
    access: dict[str, bool]
    scripts_allowed: bool
    priority: int
    content_path: Path | None
    registry_root: Path


class SkillRegistry:
    """Load only metadata at startup; content is route-gated and digest checked."""

    def __init__(self, root: Path) -> None:
        self._root = root.resolve()
        self._records: dict[tuple[str, str], SkillRevision] = {}
        self.errors: list[str] = []

    def load(self) -> None:
        self._records.clear()
        self.errors.clear()
        index_path = self._root / "index.yaml"
        try:
            loaded = yaml.safe_load(index_path.read_text(encoding="utf-8"))
            if not isinstance(loaded, dict) or loaded.get("schemaVersion") != "1.0.0":
                raise ValueError("registry index schemaVersion must be 1.0.0")
            candidates = loaded.get("candidates")
            if not isinstance(candidates, list) or not candidates:
                raise ValueError("registry index candidates must be a non-empty list")
            for raw in candidates:
                self._load_metadata(raw)
        except (OSError, TypeError, ValueError, yaml.YAMLError) as error:
            self.errors.append(f"{index_path}: {error}")

    def _load_metadata(self, value: object) -> None:
        try:
            if not isinstance(value, dict) or any(not isinstance(key, str) for key in value):
                raise ValueError("candidate metadata must be an object")
            raw = cast(dict[str, object], value)
            missing = sorted(_REQUIRED_FIELDS - raw.keys())
            if missing:
                raise ValueError(f"missing required keys: {', '.join(missing)}")
            name = _text(raw, "name")
            version = _text(raw, "version")
            if not _SEMANTIC_VERSION.fullmatch(version):
                raise ValueError("version must use semantic x.y.z form")
            source_type = _text(raw, "sourceType")
            provenance = _text(raw, "provenance")
            approval = _text(raw, "approval")
            if source_type not in {"git", "public_markdown"}:
                raise ValueError("sourceType is invalid")
            if provenance not in {"verified_snapshot", "pending_provenance"}:
                raise ValueError("provenance is invalid")
            if approval not in {"approved", "not_approved"}:
                raise ValueError("approval is invalid")
            enabled = raw.get("enabled")
            scripts_allowed = raw.get("scriptsAllowed")
            priority = raw.get("priority")
            if not isinstance(enabled, bool) or not isinstance(scripts_allowed, bool):
                raise ValueError("enabled and scriptsAllowed must be booleans")
            if isinstance(priority, bool) or not isinstance(priority, int):
                raise ValueError("priority must be an integer")
            upstream_digest = raw.get("upstreamDigest")
            runtime_digest = raw.get("runtimeDigest")
            content_path_value = raw.get("contentPath")
            content_path = (
                (self._root / content_path_value).resolve()
                if isinstance(content_path_value, str) and content_path_value
                else None
            )
            if provenance == "verified_snapshot":
                if not isinstance(upstream_digest, str) or not _DIGEST.fullmatch(upstream_digest):
                    raise ValueError("verified upstreamDigest must be sha256")
                if not isinstance(runtime_digest, str) or not _DIGEST.fullmatch(runtime_digest):
                    raise ValueError("verified runtimeDigest must be sha256")
                if approval != "approved" or not enabled or content_path is None:
                    raise ValueError(
                        "verified candidate must be approved, enabled and have contentPath"
                    )
            elif approval != "not_approved" or enabled or content_path is not None:
                raise ValueError(
                    "pending candidate must be not_approved, disabled and metadata-only"
                )
            access = _access(raw)
            if scripts_allowed or any(access.values()):
                raise ValueError("MVP-A skill scripts and external access must be disabled")
            revision = SkillRevision(
                name,
                version,
                cast(Literal["git", "public_markdown"], source_type),
                _text(raw, "sourceIdentity"),
                cast(str | None, upstream_digest),
                cast(str | None, runtime_digest),
                cast(Literal["verified_snapshot", "pending_provenance"], provenance),
                cast(Literal["approved", "not_approved"], approval),
                _text(raw, "license"),
                _text(raw, "licenseStatus"),
                enabled,
                _string_set(raw, "stages"),
                _string_set(raw, "projectTypes"),
                _string_set(raw, "capabilities"),
                _string_set(raw, "targetModels"),
                _schema(raw, "inputSchema"),
                _schema(raw, "outputSchema"),
                _string_set(raw, "allowedTools"),
                access,
                scripts_allowed,
                cast(int, priority),
                content_path,
                self._root,
            )
            key = (revision.name, revision.version)
            if key in self._records:
                raise ValueError(f"duplicate skill revision: {revision.name}@{revision.version}")
            self._records[key] = revision
        except (TypeError, ValueError) as error:
            self.errors.append(f"candidate: {error}")

    def list(self) -> builtins.list[SkillRevision]:
        return sorted(self._records.values(), key=lambda item: (item.name, item.version))

    def search(self, query: str) -> builtins.list[SkillRevision]:
        needle = query.lower()
        return [
            item
            for item in self.list()
            if needle in " ".join((item.name, *item.capabilities)).lower()
        ]

    def resolve(self, name: str, version: str) -> SkillRevision:
        record = self._records.get((name, version))
        if record is None:
            raise LookupError(f"skill revision is not registered: {name}@{version}")
        return record

    def routable(self) -> builtins.list[SkillRevision]:
        return [
            record
            for record in self.list()
            if record.enabled
            and record.approval == "approved"
            and record.provenance == "verified_snapshot"
        ]

    def load_selected_content(
        self,
        name: str,
        version: str,
        *,
        allowed_skills: frozenset[str],
        required_capabilities: frozenset[str],
        selection_mode: Literal["fixed", "inherit"],
    ) -> str:
        record = self.resolve(name, version)
        if name not in allowed_skills or selection_mode not in {"fixed", "inherit"}:
            raise PermissionError("skill is outside the selected node policy")
        if not required_capabilities.issubset(record.capabilities):
            raise PermissionError("skill capabilities do not satisfy the selected node")
        if record not in self.routable() or record.content_path is None:
            raise PermissionError("skill revision is not approved and routable")
        if self._root not in record.content_path.parents or record.content_path.name != "SKILL.md":
            raise PermissionError("skill content path escapes the registry snapshot")
        content = record.content_path.read_bytes()
        if sha256(content).hexdigest() != record.runtime_digest:
            raise ValueError("skill runtime snapshot digest drift")
        return content.decode("utf-8")
