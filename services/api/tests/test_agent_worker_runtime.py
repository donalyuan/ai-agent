from __future__ import annotations

from pathlib import Path

import pytest
import yaml

from video_agent_api.agent_runtime import compose_agent_runtime


def test_agent_worker_loads_only_locked_runtime_and_registry_metadata() -> None:
    composition = compose_agent_runtime(Path(__file__).parents[1] / "skill_registry")
    assert composition.agentscope_version.startswith("2.")
    assert set(composition.approved_skill_revisions) == {
        "drama-skills@1.0.0",
        "novel-writing@1.0.0",
    }
    assert composition.content_loaded_at_startup is False


def test_agent_worker_rejects_unapproved_default_binding(tmp_path: Path) -> None:
    source = Path(__file__).parents[1] / "skill_registry/index.yaml"
    index = yaml.safe_load(source.read_text(encoding="utf-8"))
    for candidate in index["candidates"]:
        candidate.pop("contentPath", None)
        if candidate["name"] == "drama-skills":
            candidate.update(
                {
                    "provenance": "pending_provenance",
                    "approval": "not_approved",
                    "enabled": False,
                    "upstreamDigest": None,
                    "runtimeDigest": None,
                }
            )
    (tmp_path / "index.yaml").write_text(yaml.safe_dump(index, sort_keys=False), encoding="utf-8")
    with pytest.raises(RuntimeError, match="registry_invalid|binding_unavailable"):
        compose_agent_runtime(tmp_path)
