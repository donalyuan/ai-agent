"""AgentScope dependency and Skill Registry startup composition."""

from __future__ import annotations

import importlib.metadata
import os
from dataclasses import dataclass
from pathlib import Path

from video_agent_api.skills.registry import SkillRegistry


@dataclass(frozen=True, slots=True)
class AgentRuntimeComposition:
    agentscope_version: str
    registry_schema_version: str
    approved_skill_revisions: tuple[str, ...]
    content_loaded_at_startup: bool = False


def compose_agent_runtime(registry_root: Path | None = None) -> AgentRuntimeComposition:
    """Validate dependency lock and metadata without reading selected Skill content."""
    version = importlib.metadata.version("agentscope")
    if version.split(".", 1)[0] != "2":
        raise RuntimeError("agentscope_runtime_version_unsupported")
    root = registry_root or Path(
        os.environ.get(
            "SKILL_REGISTRY_ROOT",
            str(Path(__file__).resolve().parents[2] / "skill_registry"),
        )
    )
    registry = SkillRegistry(root)
    registry.load()
    if registry.errors:
        raise RuntimeError(f"skill_registry_invalid:{registry.errors[0]}")
    approved = tuple(f"{item.name}@{item.version}" for item in registry.routable())
    if set(approved) != {"drama-skills@1.0.0", "novel-writing@1.0.0"}:
        raise RuntimeError("default_skill_binding_unavailable")
    return AgentRuntimeComposition(version, "1.0.0", approved)
