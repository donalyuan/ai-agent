from __future__ import annotations

import pytest
from fastapi.testclient import TestClient

from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.app import create_app
from video_agent_api.application.asset_bible import (
    AcceptImpactCommand,
    ApplyInitialHandoffCommand,
    AssetBibleService,
    AssignContinuityCommand,
    CreateEntryCommand,
    CreateRelationshipCommand,
    DisableEntryCommand,
    InitialEntrySpec,
    OwnerProjectionResult,
    PreviewImpactCommand,
    UpdateEntryCommand,
)
from video_agent_api.application.projects_episodes import ProjectsEpisodesService
from video_agent_api.application.scenes import CreateSceneCommand, CreateShotCommand, ScenesService
from video_agent_api.application.text_generation import TextGenerationService
from video_agent_api.domain.asset_bible import (
    ContinuityAssignment,
    ContinuityImpactTarget,
    OwnerReference,
    canonical_hash,
    resolve_assignments,
)
from video_agent_api.domain.errors import RevisionConflictError, ValidationDomainError
from video_agent_api.domain.text_review import StructuredTextCandidate, TextOwnerHandoff
from video_agent_api.domain.timeline import TimelineCut

ACTOR_UUID = "00000000-0000-4000-8000-000000000101"


async def _owners() -> tuple[
    InMemoryUnitOfWork,
    ProjectsEpisodesService,
    ScenesService,
    AssetBibleService,
    object,
    object,
    object,
    object,
]:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    scenes = ScenesService(lambda: uow)
    bible = AssetBibleService(lambda: uow)
    project = await projects.create_project("P")
    episode = await projects.create_episode((project.id, "E", 1))
    scene = await scenes.create_scene(CreateSceneCommand(project.id, episode.id))
    shot = await scenes.create_shot(CreateShotCommand(project.id, episode.id, scene.id))
    return uow, projects, scenes, bible, project, episode, scene, shot


async def test_typed_relationships_and_reference_only_versions() -> None:
    uow, _, _, bible, project, *_ = await _owners()
    character = await bible.create_entry(CreateEntryCommand(project.id, "character"))
    look = await bible.create_entry(CreateEntryCommand(project.id, "look"))
    with pytest.raises(ValidationDomainError, match="requires characterRef"):
        await bible.update_entry(UpdateEntryCommand(project.id, look.id, {"name": "coat"}, 1))
    character_version = await bible.update_entry(
        UpdateEntryCommand(project.id, character.id, {"name": "Lin"}, 1)
    )
    look_version = await bible.update_entry(
        UpdateEntryCommand(
            project.id,
            look.id,
            {
                "name": "coat",
                "characterRef": {
                    "entryId": character.id,
                    "versionId": character_version.id,
                    "revision": character_version.revision,
                    "hash": character_version.content_hash,
                },
            },
            1,
        )
    )
    relationship = await bible.create_relationship(
        CreateRelationshipCommand(project.id, look.id, character.id, "character_look")
    )
    assert relationship.project_id == project.id and look_version.revision == 1
    current_look = uow.asset_bible_entries[look.id]
    before = (len(current_look.versions), len(uow.audit_events), len(uow.outbox_events))
    with pytest.raises(ValidationDomainError, match="copies owner data"):
        await bible.update_entry(
            UpdateEntryCommand(
                project.id,
                look.id,
                {"characterRef": {"entryId": character.id}, "objectKey": "secret/path"},
                current_look.revision,
            )
        )
    current_look = uow.asset_bible_entries[look.id]
    assert (len(current_look.versions), len(uow.audit_events), len(uow.outbox_events)) == before


async def test_four_level_override_is_deterministic_and_most_specific() -> None:
    _, _, _, bible, project, episode, scene, shot = await _owners()
    entry = await bible.create_entry(CreateEntryCommand(project.id, "prop"))
    versions = []
    for index in range(4):
        versions.append(
            await bible.update_entry(
                UpdateEntryCommand(project.id, entry.id, {"name": f"key-{index}"}, entry.revision)
            )
        )
    scopes = [
        ("project", project.id),
        ("episode", episode.id),
        ("scene", scene.id),
        ("shot", shot.id),
    ]
    for (level, target_id), version in zip(scopes, versions, strict=True):
        await bible.assign(
            AssignContinuityCommand(
                project.id,
                level,
                target_id,
                entry.id,
                version.id,
                version.revision,
                version.content_hash,
                {
                    "project": project.revision,
                    "episode": episode.revision,
                    "scene": scene.revision,
                    "shot": shot.revision,
                }[level],
            )
        )
    snapshot = await bible.resolve(
        project.id,
        shot.id,
        {
            "project": project.id,
            "episode": episode.id,
            "scene": scene.id,
            "shot": shot.id,
        },
    )
    repeat = await bible.resolve(
        project.id,
        shot.id,
        {
            "project": project.id,
            "episode": episode.id,
            "scene": scene.id,
            "shot": shot.id,
        },
        persist=False,
    )
    assert snapshot.refs[0].version_id == versions[-1].id
    assert [item.level for item in snapshot.override_chain] == list(
        ("project", "episode", "scene", "shot")
    )
    assert repeat.content_hash == snapshot.content_hash


async def test_impact_accept_is_atomic_idempotent_and_tasks_do_not_rewrite_shot() -> None:
    uow, _, scenes, bible, project, episode, scene, shot = await _owners()
    entry = await bible.create_entry(CreateEntryCommand(project.id, "character"))
    initial = await bible.update_entry(UpdateEntryCommand(project.id, entry.id, {"name": "Lin"}, 1))
    await bible.assign(
        AssignContinuityCommand(
            project.id,
            "shot",
            shot.id,
            entry.id,
            initial.id,
            initial.revision,
            initial.content_hash,
            shot.revision,
        )
    )
    snapshot = await bible.resolve(project.id, shot.id)
    await scenes.append_spec(
        project.id,
        episode.id,
        scene.id,
        {"continuitySnapshotId": snapshot.id, "continuitySnapshotHash": snapshot.content_hash},
        shot.id,
    )
    old_spec = shot.spec_ref
    analysis = await bible.preview_impact(
        PreviewImpactCommand(project.id, entry.id, entry.revision, {"name": "Lin successor"})
    )
    command = AcceptImpactCommand(
        project_id=project.id,
        entry_id=entry.id,
        analysis_id=analysis.id,
        expected_analysis_revision=analysis.revision,
        expected_entry_revision=entry.revision,
        expected_asset_bible_revision=uow.asset_bibles_by_project[project.id].revision,
        candidate_payload_hash=analysis.candidate_payload_hash,
        target_refs=analysis.target_refs,
        target_set_hash=analysis.target_set_hash,
        actor_uuid=ACTOR_UUID,
        correlation_id="correlation",
    )
    decision, successor, tasks = await bible.accept_impact(command)
    retry = await bible.accept_impact(command)
    assert retry[0].id == decision.id and retry[1].id == successor.id
    assert len(tasks) == 1 and tasks[0].status == "pending"
    assert shot.spec_ref == old_spec and shot.current_video is None
    before = (len(entry.versions), len(uow.asset_bible_tasks), len(uow.audit_events))
    stale = await bible.preview_impact(
        PreviewImpactCommand(project.id, entry.id, entry.revision, {"name": "third"})
    )
    with pytest.raises(RevisionConflictError):
        await bible.accept_impact(
            AcceptImpactCommand(
                project_id=project.id,
                entry_id=entry.id,
                analysis_id=stale.id,
                expected_analysis_revision=stale.revision,
                expected_entry_revision=entry.revision - 1,
                expected_asset_bible_revision=uow.asset_bibles_by_project[project.id].revision,
                candidate_payload_hash=stale.candidate_payload_hash,
                target_refs=stale.target_refs,
                target_set_hash=stale.target_set_hash,
                actor_uuid=ACTOR_UUID,
                correlation_id="stale",
            )
        )
    assert (len(entry.versions), len(uow.asset_bible_tasks), len(uow.audit_events)) == before


async def test_incomplete_impact_cannot_be_accepted() -> None:
    _, _, _, bible, project, *_ = await _owners()
    entry = await bible.create_entry(CreateEntryCommand(project.id, "visual_style"))
    await bible.update_entry(UpdateEntryCommand(project.id, entry.id, {"name": "noir"}, 1))
    analysis = await bible.preview_impact(
        PreviewImpactCommand(
            project.id,
            entry.id,
            entry.revision,
            {"name": "warm"},
            owner_projection_complete=False,
            diagnostic="owner_projection_incomplete",
        )
    )
    with pytest.raises(ValidationDomainError, match="stale or incomplete"):
        await bible.accept_impact(
            AcceptImpactCommand(
                project_id=project.id,
                entry_id=entry.id,
                analysis_id=analysis.id,
                expected_analysis_revision=1,
                expected_entry_revision=entry.revision,
                expected_asset_bible_revision=2,
                candidate_payload_hash=analysis.candidate_payload_hash,
                target_refs=analysis.target_refs,
                target_set_hash=analysis.target_set_hash,
                actor_uuid=ACTOR_UUID,
                correlation_id="correlation",
            )
        )


async def test_stable_bible_identity_immutable_versions_and_disable() -> None:
    uow, _, _, bible, project, *_ = await _owners()
    first = await bible.create_entry(CreateEntryCommand(project.id, "character"))
    second = await bible.create_entry(CreateEntryCommand(project.id, "prop"))
    assert first.asset_bible_id == second.asset_bible_id
    v1 = await bible.update_entry(
        UpdateEntryCommand(project.id, first.id, {"name": "Lin"}, 1, ACTOR_UUID)
    )
    original_payload = dict(v1.payload)
    v2 = await bible.update_entry(
        UpdateEntryCommand(project.id, first.id, {"name": "Lin 2"}, first.revision, ACTOR_UUID)
    )
    assert (v1.version_number, v1.revision, v2.version_number, v2.revision) == (1, 1, 2, 1)
    assert v1.payload == original_payload and v1.id != v2.id
    aggregate = uow.asset_bibles_by_project[project.id]
    assert aggregate.current_version_map[first.id] == v2.id
    await bible.disable_entry(DisableEntryCommand(project.id, first.id, first.revision))
    with pytest.raises(ValidationDomainError, match="disabled"):
        await bible.update_entry(
            UpdateEntryCommand(
                project.id, first.id, {"name": "forbidden"}, first.revision, ACTOR_UUID
            )
        )
    assert [item.id for item in first.versions] == [v1.id, v2.id]


async def test_relationship_cycle_cross_project_and_type_mismatch_are_atomic() -> None:
    uow, projects, _, bible, project, *_ = await _owners()
    entries = [await bible.create_entry(CreateEntryCommand(project.id, "prop")) for _ in range(3)]
    await bible.create_relationship(
        CreateRelationshipCommand(project.id, entries[0].id, entries[1].id, "related")
    )
    await bible.create_relationship(
        CreateRelationshipCommand(project.id, entries[1].id, entries[2].id, "related")
    )
    before = (len(uow.asset_bible_relationships), len(uow.audit_events), len(uow.outbox_events))
    with pytest.raises(ValidationDomainError, match="cycle"):
        await bible.create_relationship(
            CreateRelationshipCommand(project.id, entries[2].id, entries[0].id, "related")
        )
    assert (
        len(uow.asset_bible_relationships),
        len(uow.audit_events),
        len(uow.outbox_events),
    ) == before
    foreign_project = await projects.create_project("foreign")
    foreign = await bible.create_entry(CreateEntryCommand(foreign_project.id, "character"))
    before = (len(uow.asset_bible_relationships), len(uow.audit_events), len(uow.outbox_events))
    with pytest.raises(ValidationDomainError, match="scope"):
        await bible.create_relationship(
            CreateRelationshipCommand(project.id, entries[0].id, foreign.id, "related")
        )
    with pytest.raises(ValidationDomainError, match="type mismatch"):
        await bible.create_relationship(
            CreateRelationshipCommand(project.id, entries[0].id, entries[1].id, "character_look")
        )
    assert (
        len(uow.asset_bible_relationships),
        len(uow.audit_events),
        len(uow.outbox_events),
    ) == before


async def test_owner_references_are_reference_only_and_handoff_is_idempotent() -> None:
    uow, _, _, bible, project, *_ = await _owners()
    entry = await bible.create_entry(CreateEntryCommand(project.id, "prop"))
    asset_ref = OwnerReference("00000000-0000-4000-8000-000000000201", 0, "a" * 64, "appearance")
    generation_ref = OwnerReference("00000000-0000-4000-8000-000000000202", 1, "b" * 64, "image")
    version = await bible.update_entry(
        UpdateEntryCommand(
            project.id,
            entry.id,
            {"name": "key"},
            entry.revision,
            ACTOR_UUID,
            (asset_ref,),
            (generation_ref,),
        )
    )
    assert version.reference_asset_version_refs == (asset_ref,)
    with pytest.raises(ValidationDomainError, match="sha256"):
        OwnerReference(asset_ref.owner_id, 0, "bad", "appearance")
    spec_id = "00000000-0000-4000-8000-000000000203"
    payload = {"name": "handoff prop"}
    command = ApplyInitialHandoffCommand(
        "handoff-1",
        project.id,
        "c" * 64,
        (InitialEntrySpec(spec_id, "prop", payload, canonical_hash(payload)),),
        ACTOR_UUID,
        "corr-1",
    )
    ack = await bible.apply_initial_handoff(command)
    retry = await bible.apply_initial_handoff(command)
    assert retry.id == ack.id and len(uow.asset_bible_entries[spec_id].versions) == 1


def test_resolver_rejects_ambiguous_or_foreign_assignments() -> None:
    base = ContinuityAssignment("project", "shot", "shot", "entry", "v1", 1, "a" * 64)
    conflict = ContinuityAssignment("project", "shot", "shot", "entry", "v2", 1, "b" * 64)
    with pytest.raises(ValidationDomainError, match="ambiguous"):
        resolve_assignments("project", "shot", [base, conflict])
    foreign = ContinuityAssignment("foreign", "shot", "shot", "entry", "v1", 1, "a" * 64)
    with pytest.raises(ValidationDomainError, match="foreign"):
        resolve_assignments("project", "shot", [foreign])


async def test_impact_projection_covers_episode_scene_shot_and_preview_has_no_side_effects() -> (
    None
):
    uow, _, _, bible, project, episode, scene, shot = await _owners()
    entry = await bible.create_entry(CreateEntryCommand(project.id, "prop"))
    version = await bible.update_entry(
        UpdateEntryCommand(project.id, entry.id, {"name": "key"}, entry.revision, ACTOR_UUID)
    )
    scopes = [
        ("episode", episode.id, episode.revision, {"project": project.id, "episode": episode.id}),
        (
            "scene",
            scene.id,
            scene.revision,
            {"project": project.id, "episode": episode.id, "scene": scene.id},
        ),
        (
            "shot",
            shot.id,
            shot.revision,
            {
                "project": project.id,
                "episode": episode.id,
                "scene": scene.id,
                "shot": shot.id,
            },
        ),
    ]
    for level, target_id, revision, scope_ids in scopes:
        await bible.assign(
            AssignContinuityCommand(
                project.id,
                level,
                target_id,
                entry.id,
                version.id,
                version.revision,
                version.content_hash,
                revision,
            )
        )
        await bible.resolve(project.id, target_id, scope_ids)
    before = {
        "versions": len(entry.versions),
        "tasks": len(uow.asset_bible_tasks),
        "audit": len(uow.audit_events),
        "outbox": len(uow.outbox_events),
        "providerCalls": len(uow.provider_calls),
    }
    first = await bible.preview_impact(
        PreviewImpactCommand(project.id, entry.id, entry.revision, {"name": "new key"}, ACTOR_UUID)
    )
    second = await bible.preview_impact(
        PreviewImpactCommand(project.id, entry.id, entry.revision, {"name": "new key"}, ACTOR_UUID)
    )
    assert [item.target_type for item in first.target_refs] == ["episode", "scene", "shot"]
    assert first.target_set_hash == second.target_set_hash
    assert before == {
        "versions": len(entry.versions),
        "tasks": len(uow.asset_bible_tasks),
        "audit": len(uow.audit_events),
        "outbox": len(uow.outbox_events),
        "providerCalls": len(uow.provider_calls),
    }


class _IncompleteOwnerProjection:
    def __init__(self, diagnostic: str) -> None:
        self.diagnostic = diagnostic

    async def find_references(
        self, project_id: str, entry_id: str, version_id: str
    ) -> OwnerProjectionResult:
        del project_id, entry_id, version_id
        return OwnerProjectionResult((), False, self.diagnostic)


@pytest.mark.parametrize(
    "diagnostic",
    ["owner_unavailable", "owner_projection_page_incomplete", "owner_projection_revision_drift"],
)
async def test_incomplete_owner_projection_preserves_diagnostic(diagnostic: str) -> None:
    uow, _, _, _, project, *_ = await _owners()
    bible = AssetBibleService(lambda: uow, _IncompleteOwnerProjection(diagnostic))
    entry = await bible.create_entry(CreateEntryCommand(project.id, "prop"))
    await bible.update_entry(
        UpdateEntryCommand(project.id, entry.id, {"name": "key"}, entry.revision, ACTOR_UUID)
    )
    analysis = await bible.preview_impact(
        PreviewImpactCommand(project.id, entry.id, entry.revision, {"name": "new"}, ACTOR_UUID)
    )
    assert analysis.status == "incomplete" and analysis.diagnostic == diagnostic


async def test_target_revision_drift_blocks_accept_without_mutation() -> None:
    uow, _, scenes, bible, project, episode, scene, shot = await _owners()
    entry = await bible.create_entry(CreateEntryCommand(project.id, "character"))
    version = await bible.update_entry(
        UpdateEntryCommand(project.id, entry.id, {"name": "Lin"}, entry.revision, ACTOR_UUID)
    )
    await bible.assign(
        AssignContinuityCommand(
            project.id,
            "shot",
            shot.id,
            entry.id,
            version.id,
            version.revision,
            version.content_hash,
            shot.revision,
        )
    )
    await bible.resolve(project.id, shot.id)
    analysis = await bible.preview_impact(
        PreviewImpactCommand(project.id, entry.id, entry.revision, {"name": "Lin 2"}, ACTOR_UUID)
    )
    await scenes.append_spec(project.id, episode.id, scene.id, {"summary": "drift"}, shot.id)
    before = (len(entry.versions), len(uow.asset_bible_tasks), len(uow.audit_events))
    with pytest.raises(ValidationDomainError, match="target set is stale"):
        await bible.accept_impact(
            AcceptImpactCommand(
                project_id=project.id,
                entry_id=entry.id,
                analysis_id=analysis.id,
                expected_analysis_revision=analysis.revision,
                expected_entry_revision=entry.revision,
                expected_asset_bible_revision=uow.asset_bibles_by_project[project.id].revision,
                candidate_payload_hash=analysis.candidate_payload_hash,
                target_refs=analysis.target_refs,
                target_set_hash=analysis.target_set_hash,
                actor_uuid=ACTOR_UUID,
                correlation_id="drift",
            )
        )
    assert (len(entry.versions), len(uow.asset_bible_tasks), len(uow.audit_events)) == before


async def test_accept_rejects_stale_bible_duplicate_and_foreign_targets_atomically() -> None:
    uow, _, _, bible, project, episode, _, shot = await _owners()
    entry = await bible.create_entry(CreateEntryCommand(project.id, "prop"))
    version = await bible.update_entry(
        UpdateEntryCommand(project.id, entry.id, {"name": "key"}, entry.revision, ACTOR_UUID)
    )
    await bible.assign(
        AssignContinuityCommand(
            project.id,
            "shot",
            shot.id,
            entry.id,
            version.id,
            version.revision,
            version.content_hash,
            shot.revision,
        )
    )
    snapshot = await bible.resolve(project.id, shot.id)
    cut = TimelineCut(episode.id)
    cut.clips.append({"id": "clip-1", "continuitySnapshotId": snapshot.id})
    uow.timeline_cuts[episode.id] = cut
    analysis = await bible.preview_impact(
        PreviewImpactCommand(project.id, entry.id, entry.revision, {"name": "key 2"}, ACTOR_UUID)
    )
    aggregate_revision = uow.asset_bibles_by_project[project.id].revision

    def command_with(**changes: object) -> AcceptImpactCommand:
        values: dict[str, object] = {
            "project_id": project.id,
            "entry_id": entry.id,
            "analysis_id": analysis.id,
            "expected_analysis_revision": analysis.revision,
            "expected_entry_revision": entry.revision,
            "expected_asset_bible_revision": aggregate_revision,
            "candidate_payload_hash": analysis.candidate_payload_hash,
            "target_refs": analysis.target_refs,
            "target_set_hash": analysis.target_set_hash,
            "actor_uuid": ACTOR_UUID,
            "correlation_id": "atomic-reject",
        }
        values.update(changes)
        return AcceptImpactCommand(**values)  # type: ignore[arg-type]

    foreign = ContinuityImpactTarget(
        "shot",
        "foreign-shot",
        shot.revision,
        "resolved_version_reference",
        snapshot.id,
        snapshot.content_hash,
    )
    before = {
        "versions": tuple(item.id for item in entry.versions),
        "current": entry.current.id if entry.current else None,
        "tasks": tuple(uow.asset_bible_tasks),
        "decisions": tuple(uow.asset_bible_decisions),
        "audit": len(uow.audit_events),
        "outbox": len(uow.outbox_events),
        "timeline": tuple(dict(item) for item in cut.clips),
        "shotSpec": shot.spec_ref,
        "shotVideo": shot.current_video,
    }
    invalid_commands = (
        command_with(expected_asset_bible_revision=aggregate_revision - 1),
        command_with(target_refs=analysis.target_refs + analysis.target_refs),
        command_with(target_refs=(foreign,)),
    )
    for command in invalid_commands:
        with pytest.raises((RevisionConflictError, ValidationDomainError)):
            await bible.accept_impact(command)
        assert before == {
            "versions": tuple(item.id for item in entry.versions),
            "current": entry.current.id if entry.current else None,
            "tasks": tuple(uow.asset_bible_tasks),
            "decisions": tuple(uow.asset_bible_decisions),
            "audit": len(uow.audit_events),
            "outbox": len(uow.outbox_events),
            "timeline": tuple(dict(item) for item in cut.clips),
            "shotSpec": shot.spec_ref,
            "shotVideo": shot.current_video,
        }


async def test_task_state_machine_and_consumer_projection_are_owner_safe() -> None:
    _, _, _, bible, project, _, _, shot = await _owners()
    entry = await bible.create_entry(CreateEntryCommand(project.id, "prop"))
    asset_ref = OwnerReference("00000000-0000-4000-8000-000000000301", 0, "a" * 64, "appearance")
    version = await bible.update_entry(
        UpdateEntryCommand(
            project.id,
            entry.id,
            {"name": "key"},
            entry.revision,
            ACTOR_UUID,
            (asset_ref,),
        )
    )
    await bible.assign(
        AssignContinuityCommand(
            project.id,
            "shot",
            shot.id,
            entry.id,
            version.id,
            version.revision,
            version.content_hash,
            shot.revision,
        )
    )
    snapshot = await bible.resolve(project.id, shot.id)
    projection = await bible.consumer_projection(project.id, snapshot.id)
    assert projection["snapshotRef"] == {
        "id": snapshot.id,
        "revision": snapshot.revision,
        "hash": snapshot.content_hash,
    }
    assert projection["assetVersionRefs"] == [asset_ref]
    assert "payload" not in projection and "name" not in str(projection)
    analysis = await bible.preview_impact(
        PreviewImpactCommand(project.id, entry.id, entry.revision, {"name": "key 2"}, ACTOR_UUID)
    )
    _, _, tasks = await bible.accept_impact(
        AcceptImpactCommand(
            project_id=project.id,
            entry_id=entry.id,
            analysis_id=analysis.id,
            expected_analysis_revision=analysis.revision,
            expected_entry_revision=entry.revision,
            expected_asset_bible_revision=2,
            candidate_payload_hash=analysis.candidate_payload_hash,
            target_refs=analysis.target_refs,
            target_set_hash=analysis.target_set_hash,
            actor_uuid=ACTOR_UUID,
            correlation_id="task-state",
        )
    )
    task = tasks[0]
    await bible.transition_task(project.id, task.id, "acknowledged", 1)
    await bible.transition_task(project.id, task.id, "resolved", 2)
    with pytest.raises(ValidationDomainError, match="transition"):
        await bible.transition_task(project.id, task.id, "pending", 3)


async def test_text_handoff_applies_only_referenced_specs_and_media_gate_waits_for_all_owners() -> (
    None
):
    uow, _, _, bible, project, *_ = await _owners()
    text = TextGenerationService(lambda: uow)
    spec_id = "00000000-0000-4000-8000-000000000401"
    payload = {
        "kind": "asset_bible_spec",
        "schema_version": "1.0.0",
        "scopeId": project.id,
        "entryType": "prop",
        "stableId": spec_id,
        "attributes": {"name": "key"},
    }
    candidate = StructuredTextCandidate(
        project.id,
        "asset_bible_spec",
        project.id,
        payload,
        status="accepted",
    )
    uow.text_candidates[candidate.id] = candidate
    handoff = TextOwnerHandoff(
        "batch",
        2,
        project.id,
        "run",
        (
            {
                "candidateId": candidate.id,
                "candidateRevision": candidate.revision,
                "kind": candidate.kind,
                "scopeId": candidate.scope_id,
                "payloadHash": candidate.payload_hash,
            },
        ),
        "d" * 64,
        "corr-text",
        ("projects", "episodes", "scenes", "asset_bible"),
    )
    uow.text_handoffs[handoff.id] = handoff
    first = await text.apply_asset_bible_handoff(handoff.id, bible, ACTOR_UUID)
    retry = await text.apply_asset_bible_handoff(handoff.id, bible, ACTOR_UUID)
    assert retry.id == first.id and len(uow.asset_bible_entries[spec_id].versions) == 1
    blocked = await text.media_gate(handoff.id)
    assert blocked == {
        "status": "blocked",
        "handoffId": handoff.id,
        "missingOwners": ["episodes", "projects", "scenes"],
    }
    for owner in ("projects", "episodes", "scenes"):
        await text.ack_handoff(handoff.id, owner, 1, f"{owner}-fingerprint", "corr-text")
    assert (await text.media_gate(handoff.id))["status"] == "ready"


def test_asset_bible_http_cas_alias_and_read_projection() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    client = TestClient(
        create_app(readiness_probe=lambda: True, projects_episodes_service=projects)
    )
    project = client.post("/v1/projects", json={"name": "P"}).json()
    scope = {"X-Project-Scope": project["id"]}
    assert client.get(f"/v1/projects/{project['id']}/asset-bible/entries").status_code == 403
    entry_response = client.post(
        f"/v1/projects/{project['id']}/asset-bible/entries",
        headers=scope,
        json={"entryType": "prop", "schemaVersion": "1.0.0"},
    )
    assert entry_response.status_code == 201
    entry = entry_response.json()
    assert entry["schemaVersion"] == "1.0.0" and "schema_version" not in entry
    conflict = client.post(
        f"/v1/projects/{project['id']}/asset-bible/entries/{entry['id']}/versions",
        headers={**scope, "If-Match": "2"},
        json={
            "payload": {"name": "key"},
            "expectedRevision": 1,
            "actorUuid": ACTOR_UUID,
            "schemaVersion": "1.0.0",
        },
    )
    assert conflict.status_code == 409
    assert len(uow.asset_bible_entries[entry["id"]].versions) == 0
    missing_schema = client.post(
        f"/v1/projects/{project['id']}/asset-bible/entries/{entry['id']}/versions",
        headers={**scope, "If-Match": "1"},
        json={
            "payload": {"name": "key"},
            "expectedRevision": 1,
            "actorUuid": ACTOR_UUID,
        },
    )
    assert missing_schema.status_code == 422
    missing_precondition = client.post(
        f"/v1/projects/{project['id']}/asset-bible/entries/{entry['id']}/versions",
        headers=scope,
        json={
            "payload": {"name": "key"},
            "expectedRevision": 1,
            "actorUuid": ACTOR_UUID,
            "schemaVersion": "1.0.0",
        },
    )
    assert missing_precondition.status_code == 409
    version = client.post(
        f"/v1/projects/{project['id']}/asset-bible/entries/{entry['id']}/versions",
        headers={**scope, "If-Match": "1"},
        json={
            "payload": {"name": "key"},
            "expectedRevision": 1,
            "actorUuid": ACTOR_UUID,
            "schemaVersion": "1.0.0",
        },
    )
    assert version.status_code == 201
    assert version.json()["contentHash"]
    before = (len(uow.audit_events), len(uow.outbox_events), len(uow.provider_calls))
    listed = client.get(f"/v1/projects/{project['id']}/asset-bible/entries", headers=scope)
    detail = client.get(
        f"/v1/projects/{project['id']}/asset-bible/entries/{entry['id']}",
        headers=scope,
    )
    assert listed.status_code == detail.status_code == 200
    assert listed.json()[0]["id"] == detail.json()["id"] == entry["id"]
    assert before == (len(uow.audit_events), len(uow.outbox_events), len(uow.provider_calls))
    forbidden = client.get(
        f"/v1/projects/{project['id']}/asset-bible/entries",
        headers={"X-Project-Scope": "foreign-project"},
    )
    assert forbidden.status_code == 403


def test_asset_bible_http_returns_unconfigured_service_as_503() -> None:
    client = TestClient(create_app(readiness_probe=lambda: True))
    response = client.get("/v1/projects/unconfigured/asset-bible/entries")
    assert response.status_code == 503
    assert response.json()["detail"]["type"] == "database_unavailable"


def test_asset_bible_http_assignment_preview_accept_and_atomic_conflicts() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    client = TestClient(
        create_app(readiness_probe=lambda: True, projects_episodes_service=projects)
    )
    project = client.post("/v1/projects", json={"name": "P"}).json()
    scope = {"X-Project-Scope": project["id"]}
    episode = client.post(
        f"/v1/projects/{project['id']}/episodes", json={"number": 1, "title": "E"}
    ).json()
    scene = client.post(
        f"/v1/projects/{project['id']}/episodes/{episode['id']}/scenes",
        headers=scope,
        json={"schemaVersion": "1.0.0"},
    ).json()
    shot = client.post(
        f"/v1/projects/{project['id']}/episodes/{episode['id']}/scenes/{scene['id']}/shots",
        headers=scope,
        json={"schemaVersion": "1.0.0"},
    ).json()
    entry = client.post(
        f"/v1/projects/{project['id']}/asset-bible/entries",
        headers=scope,
        json={"entryType": "prop", "schemaVersion": "1.0.0"},
    ).json()
    version = client.post(
        f"/v1/projects/{project['id']}/asset-bible/entries/{entry['id']}/versions",
        headers={**scope, "If-Match": "1"},
        json={
            "payload": {"name": "key"},
            "expectedRevision": 1,
            "actorUuid": ACTOR_UUID,
            "schemaVersion": "1.0.0",
        },
    ).json()
    assignment = client.post(
        f"/v1/projects/{project['id']}/asset-bible/assignments",
        headers={**scope, "If-Match": str(shot["revision"])},
        json={
            "level": "shot",
            "targetId": shot["id"],
            "entryId": entry["id"],
            "versionId": version["id"],
            "versionRevision": version["revision"],
            "contentHash": version["contentHash"],
            "expectedRevision": shot["revision"],
            "schemaVersion": "1.0.0",
        },
    )
    assert assignment.status_code == 201
    snapshot = client.post(
        f"/v1/projects/{project['id']}/asset-bible/resolutions",
        headers=scope,
        json={
            "targetId": shot["id"],
            "scopeIds": {"shot": shot["id"]},
            "persist": True,
        },
    ).json()
    preview = client.post(
        f"/v1/projects/{project['id']}/asset-bible/entries/{entry['id']}/impact-previews",
        headers={**scope, "If-Match": "2"},
        json={
            "expectedRevision": 2,
            "payload": {"name": "key 2"},
            "actorUuid": ACTOR_UUID,
            "schemaVersion": "1.0.0",
        },
    )
    assert preview.status_code == 200
    analysis = preview.json()
    assert analysis["targetRefs"][0]["snapshotId"] == snapshot["id"]
    body = {
        "analysisId": analysis["id"],
        "expectedAnalysisRevision": analysis["revision"],
        "expectedEntryRevision": 2,
        "expectedAssetBibleRevision": 2,
        "candidatePayloadHash": analysis["candidatePayloadHash"],
        "targetRefs": analysis["targetRefs"],
        "targetSetHash": analysis["targetSetHash"],
        "actorUuid": ACTOR_UUID,
        "correlationId": "http-accept",
        "schemaVersion": "1.0.0",
    }
    before_accept = {
        "versions": len(uow.asset_bible_entries[entry["id"]].versions),
        "tasks": len(uow.asset_bible_tasks),
        "audit": len(uow.audit_events),
        "outbox": len(uow.outbox_events),
    }
    mismatch = client.post(
        f"/v1/projects/{project['id']}/asset-bible/entries/{entry['id']}/impact-accepts",
        headers={**scope, "If-Match": "2"},
        json={**body, "targetRefs": analysis["targetRefs"] * 2},
    )
    assert mismatch.status_code == 422
    assert before_accept == {
        "versions": len(uow.asset_bible_entries[entry["id"]].versions),
        "tasks": len(uow.asset_bible_tasks),
        "audit": len(uow.audit_events),
        "outbox": len(uow.outbox_events),
    }
    accepted = client.post(
        f"/v1/projects/{project['id']}/asset-bible/entries/{entry['id']}/impact-accepts",
        headers={**scope, "If-Match": "2"},
        json=body,
    )
    assert accepted.status_code == 200
    accepted_body = accepted.json()
    assert accepted_body["tasks"][0]["status"] == "pending"
    before = {
        "versions": len(uow.asset_bible_entries[entry["id"]].versions),
        "tasks": len(uow.asset_bible_tasks),
        "audit": len(uow.audit_events),
        "outbox": len(uow.outbox_events),
    }
    retry = client.post(
        f"/v1/projects/{project['id']}/asset-bible/entries/{entry['id']}/impact-accepts",
        headers={**scope, "If-Match": "2"},
        json=body,
    )
    assert retry.status_code == 200
    assert retry.json()["decision"]["id"] == accepted_body["decision"]["id"]
    concurrent = client.post(
        f"/v1/projects/{project['id']}/asset-bible/entries/{entry['id']}/impact-accepts",
        headers={**scope, "If-Match": "2"},
        json={**body, "correlationId": "competing-accept"},
    )
    assert concurrent.status_code == 409
    assert before == {
        "versions": len(uow.asset_bible_entries[entry["id"]].versions),
        "tasks": len(uow.asset_bible_tasks),
        "audit": len(uow.audit_events),
        "outbox": len(uow.outbox_events),
    }
    tasks = client.get(f"/v1/projects/{project['id']}/asset-bible/tasks", headers=scope)
    projection = client.get(
        f"/v1/projects/{project['id']}/asset-bible/snapshots/{snapshot['id']}/consumer-projection",
        headers=scope,
    )
    assert tasks.status_code == projection.status_code == 200
    assert projection.json()["snapshotRef"]["id"] == snapshot["id"]


async def test_e2e_mvpa_001_asset_bible_2x2x3_continuity_evidence() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    scenes = ScenesService(lambda: uow)
    bible = AssetBibleService(lambda: uow)
    project = await projects.create_project("E2E-MVPA-001")
    character_id = "00000000-0000-4000-8000-000000000801"
    look_id = "00000000-0000-4000-8000-000000000802"
    prop_id = "00000000-0000-4000-8000-000000000803"
    specs = (
        InitialEntrySpec(
            character_id, "character", {"name": "Lin"}, canonical_hash({"name": "Lin"})
        ),
        InitialEntrySpec(
            look_id,
            "look",
            {"name": "coat", "characterEntryId": character_id},
            canonical_hash({"name": "coat", "characterEntryId": character_id}),
        ),
        InitialEntrySpec(prop_id, "prop", {"name": "key"}, canonical_hash({"name": "key"})),
    )
    ack = await bible.apply_initial_handoff(
        ApplyInitialHandoffCommand(
            "e2e-mvpa-001-asset-bible",
            project.id,
            "e" * 64,
            specs,
            ACTOR_UUID,
            "e2e-s04a",
        )
    )
    assert ack.entry_version_refs == tuple(sorted(ack.entry_version_refs))
    assert {item[0] for item in ack.entry_version_refs} == {
        character_id,
        look_id,
        prop_id,
    }
    entries = [uow.asset_bible_entries[item] for item in (character_id, look_id, prop_id)]
    for entry in entries:
        assert entry.current is not None
        await bible.assign(
            AssignContinuityCommand(
                project.id,
                "project",
                project.id,
                entry.id,
                entry.current.id,
                entry.current.revision,
                entry.current.content_hash,
                project.revision,
            )
        )

    episodes = []
    all_scenes = []
    all_shots = []
    for episode_number in range(1, 3):
        episode = await projects.create_episode(
            (project.id, f"Episode {episode_number}", episode_number)
        )
        episodes.append(episode)
        await bible.resolve(
            project.id,
            episode.id,
            {"project": project.id, "episode": episode.id},
        )
        for _scene_number in range(2):
            scene = await scenes.create_scene(CreateSceneCommand(project.id, episode.id))
            all_scenes.append(scene)
            await bible.resolve(
                project.id,
                scene.id,
                {"project": project.id, "episode": episode.id, "scene": scene.id},
            )
            for _shot_number in range(3):
                shot = await scenes.create_shot(CreateShotCommand(project.id, episode.id, scene.id))
                all_shots.append(shot)
                await bible.resolve(
                    project.id,
                    shot.id,
                    {
                        "project": project.id,
                        "episode": episode.id,
                        "scene": scene.id,
                        "shot": shot.id,
                    },
                )
    assert (len(episodes), len(all_scenes), len(all_shots)) == (2, 4, 12)
    prop = uow.asset_bible_entries[prop_id]
    analysis = await bible.preview_impact(
        PreviewImpactCommand(
            project.id,
            prop.id,
            prop.revision,
            {"name": "brass key"},
            ACTOR_UUID,
        )
    )
    expected_targets = {
        *(("episode", item.id) for item in episodes),
        *(("scene", item.id) for item in all_scenes),
        *(("shot", item.id) for item in all_shots),
    }
    assert {(item.target_type, item.target_id) for item in analysis.target_refs} == expected_targets
    assert len(analysis.target_refs) == 18
    assert analysis.target_set_hash == canonical_hash(
        [item.canonical_value() for item in analysis.target_refs]
    )
    old_snapshot_refs = {
        shot.id: (
            shot.spec_ref,
            shot.current_image,
            shot.current_video,
        )
        for shot in all_shots
    }
    before_provider_calls = len(uow.provider_calls)
    decision, successor, tasks = await bible.accept_impact(
        AcceptImpactCommand(
            project_id=project.id,
            entry_id=prop.id,
            analysis_id=analysis.id,
            expected_analysis_revision=analysis.revision,
            expected_entry_revision=prop.revision,
            expected_asset_bible_revision=uow.asset_bibles_by_project[project.id].revision,
            candidate_payload_hash=analysis.candidate_payload_hash,
            target_refs=analysis.target_refs,
            target_set_hash=analysis.target_set_hash,
            actor_uuid=ACTOR_UUID,
            correlation_id="e2e-s04a-accept",
        )
    )
    assert decision.new_version_id == successor.id
    assert len(tasks) == 18 and {item.status for item in tasks} == {"pending"}
    assert {(item.target_type, item.target_id) for item in tasks} == expected_targets
    assert old_snapshot_refs == {
        shot.id: (shot.spec_ref, shot.current_image, shot.current_video) for shot in all_shots
    }
    assert len(uow.provider_calls) == before_provider_calls
    with pytest.raises(RevisionConflictError):
        await bible.accept_impact(
            AcceptImpactCommand(
                project_id=project.id,
                entry_id=prop.id,
                analysis_id=analysis.id,
                expected_analysis_revision=analysis.revision,
                expected_entry_revision=prop.revision - 1,
                expected_asset_bible_revision=uow.asset_bibles_by_project[project.id].revision - 1,
                candidate_payload_hash=analysis.candidate_payload_hash,
                target_refs=analysis.target_refs,
                target_set_hash=analysis.target_set_hash,
                actor_uuid=ACTOR_UUID,
                correlation_id="e2e-s04a-focused-conflict",
            )
        )
    first_task = tasks[0]
    await bible.transition_task(project.id, first_task.id, "acknowledged", 1)
    await bible.transition_task(project.id, first_task.id, "resolved", 2)
    current_tasks = await bible.list_tasks(project.id)
    evidence = {
        "stage": "S04a asset bible continuity",
        "ownerAckId": ack.id,
        "targetSetHash": analysis.target_set_hash,
        "targetCount": len(analysis.target_refs),
        "decisionId": decision.id,
        "pendingTaskCount": sum(item.status == "pending" for item in current_tasks),
        "resolvedTaskId": first_task.id,
        "focusedFailure": "F04a asset_bible_impact_or_snapshot_conflict",
        "noAutoRegeneration": len(uow.provider_calls) == before_provider_calls,
    }
    assert evidence["targetCount"] == 18
    assert evidence["pendingTaskCount"] == 17
    assert evidence["noAutoRegeneration"] is True
