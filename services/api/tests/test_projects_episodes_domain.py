from __future__ import annotations

import pytest

from video_agent_api.domain.entities import Episode, Project
from video_agent_api.domain.errors import RevisionConflictError, ValidationDomainError


def test_domain_entities_validate_defaults_and_stable_identity() -> None:
    project = Project("  Demo  ")
    assert project.name == "Demo"
    assert project.status == "draft"
    assert project.schema_version == "1.0.0"
    assert project.revision == 1
    assert project.id

    episode = Episode(project.id, " Opening ", 1)
    assert episode.title == "Opening"
    assert episode.project_id == project.id
    assert episode.number == 1
    assert episode.revision == 1


@pytest.mark.parametrize("value", ["", " ", "\t"])
def test_domain_rejects_blank_names_and_titles(value: str) -> None:
    with pytest.raises(ValidationDomainError):
        Project(value)
    with pytest.raises(ValidationDomainError):
        Episode("project", value, 1)


@pytest.mark.parametrize("number", [0, -1, True, 1.0])
def test_domain_requires_positive_episode_number(number: object) -> None:
    with pytest.raises(ValidationDomainError):
        Episode("project", "Episode", number)  # type: ignore[arg-type]


def test_domain_update_requires_matching_revision_and_increments_atomically() -> None:
    project = Project("Demo")
    project.update(expected_revision=1, name="Updated")
    assert project.name == "Updated"
    assert project.revision == 2
    with pytest.raises(RevisionConflictError) as error:
        project.update(expected_revision=1, name="Stale")
    assert error.value.code == "revision_conflict"
    assert project.name == "Updated"
    assert project.revision == 2
