from __future__ import annotations

import hashlib
from dataclasses import replace
from pathlib import Path

import pytest

from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.application.agent_edit import AgentEditService
from video_agent_api.application.assets import AssetsService, CreateAssetCommand
from video_agent_api.application.catalog import CatalogService, CreateModelCommand
from video_agent_api.application.creative import CreativeService, SaveCreativeBriefCommand
from video_agent_api.application.image_generation import (
    GenerateImageCommand,
    ImageGenerationService,
)
from video_agent_api.application.projects_episodes import (
    CreateEpisodeCommand,
    CreateProjectCommand,
    ProjectsEpisodesService,
)
from video_agent_api.application.text_generation import (
    GenerateTextBatchCommand,
    TextGenerationService,
)
from video_agent_api.application.video_generation import (
    AgnesVideoService,
    ReconcileVideoCommand,
    SubmitVideoCommand,
    video_request_fingerprint,
)
from video_agent_api.domain.agent_edit import AssetEditExecution, AssetVersionRef
from video_agent_api.domain.asset_bible import (
    AssetBibleEntry,
    ContinuityAssignment,
    ResolvedContinuitySnapshot,
)
from video_agent_api.domain.assets import Asset, AssetVersion, StorageObject
from video_agent_api.domain.conversation import AgentConversation
from video_agent_api.domain.errors import (
    ProjectAccessForbiddenError,
    RevisionConflictError,
    ValidationDomainError,
)
from video_agent_api.domain.exports import (
    EpisodeExportBatch,
    EpisodeExportSelection,
    ExportJob,
)
from video_agent_api.domain.provider_ops import ProviderOperationPolicy
from video_agent_api.domain.runs import BudgetGate, NodeRun, WorkflowRun
from video_agent_api.domain.scenes import AcceptedMediaEligibility, ImmutableOwnerRef, Shot
from video_agent_api.domain.source_material import SourceMaterial, SourceMaterialUploadIntent
from video_agent_api.domain.video_generation import VideoOperation
from video_agent_api.ports.contracts import AdapterNotConfiguredError, ModelSelection, PortResult
from video_agent_api.ports.credentials import (
    CredentialKeyring,
    CredentialMasterKeyUnavailable,
)
from video_agent_api.ports.mocks import DeterministicMockProvider
from video_agent_api.ports.storage import LocalWorkspaceAdapter
from video_agent_api.providers.agnes import AgnesVideoProvider, VideoSubmissionState
from video_agent_api.providers.gpt_image import GPTImageProvider
from video_agent_api.providers.media_inspect import LocalMediaInspector
from video_agent_api.providers.text import OpenAICompatibleTextModelAdapter


def test_credential_envelope_is_authenticated_and_owner_bound() -> None:
    ring = CredentialKeyring(b"k" * 32, "v2")
    envelope = ring.seal("secret", profile_id="profile", credential_id="credential")
    assert envelope.algorithm == "AES-256-GCM"
    assert ring.open(envelope, profile_id="profile", credential_id="credential") == "secret"
    with pytest.raises(ValueError, match="owner mismatch"):
        ring.open(envelope, profile_id="foreign", credential_id="credential")
    with pytest.raises(CredentialMasterKeyUnavailable):
        CredentialKeyring().seal("secret")


def test_source_material_inline_and_upload_keys_are_disjoint() -> None:
    source = SourceMaterial("project", "novel", "inline_text")
    inline = source.append(expected_revision=1, input_mode="inline_text", content=b"hello")
    assert inline.asset_version_id is None
    with pytest.raises(RevisionConflictError):
        source.append(expected_revision=1, input_mode="inline_text", content=b"retry")
    intent = SourceMaterialUploadIntent(
        "project",
        source.id,
        source.revision,
        "novel",
        "uploaded_file",
        hashlib.sha256(b"file").hexdigest(),
        f"source-material-upload:project:{source.id}:{source.revision}",
        "reservation",
    )
    assert intent.input_mode == "uploaded_file"


@pytest.mark.asyncio
async def test_structured_text_batch_is_reviewed_once_and_live_missing_is_explicit() -> None:
    uow = InMemoryUnitOfWork()
    project = await ProjectsEpisodesService(lambda: uow).create_project("Text project")
    brief = await CreativeService(lambda: uow).save_brief(
        SaveCreativeBriefCommand(
            project.id,
            "original",
            {
                "subject": "A",
                "genre": "Drama",
                "audience": "Adult",
                "characterPremise": "C",
                "style": "Real",
                "episodeDurationSeconds": 60,
                "episodeCount": 1,
                "scenesPerEpisode": 1,
                "shotsPerScene": 1,
            },
            project.revision,
        )
    )
    catalog = CatalogService(lambda: uow)
    await catalog.bootstrap()
    profile = next(iter(uow.profiles.values()))
    provider = uow.providers[profile.provider_id]
    model = next(item for item in uow.models.values() if item.profile_id == profile.id)
    capability = profile.capability_snapshots["text.generate"]
    selection_snapshot = {
        "providerId": provider.id,
        "profileId": profile.id,
        "modelId": model.id,
        "adapterKey": provider.adapter_key,
        "adapterIdentity": profile.adapter_identity,
        "profileRevision": profile.revision,
        "capabilitySnapshotId": capability.id,
        "capabilityRevision": capability.revision,
        "capabilityOperation": capability.operation,
        "capabilitySnapshots": {
            capability.operation: {"id": capability.id, "revision": capability.revision}
        },
    }
    selection = ModelSelection(provider.id, profile.id, model.id, provider.adapter_key)
    run = WorkflowRun(
        project.id, "workflow", status="running", selection_snapshot=selection_snapshot
    )
    run.nodes = [NodeRun(run.id, "text.generate", "running", logical_operation="text:1")]
    uow.workflow_runs[run.id] = run
    service = TextGenerationService(lambda: uow, DeterministicMockProvider())
    batch = await service.generate(
        GenerateTextBatchCommand(
            project.id,
            run.id,
            1,
            selection,
            brief,
            scope_ids=(project.id,),
        )
    )
    assert {item.kind for item in batch.candidates} == {
        "story_spec",
        "script_spec",
        "episode",
        "scene",
        "shot",
        "shot_spec",
        "asset_bible_spec",
    }
    accepted = await service.decide(batch.id, 1, "accept")
    assert accepted.status == "accepted"
    with pytest.raises(RevisionConflictError):
        await service.decide(batch.id, 1, "accept")
    live_run = WorkflowRun(
        project.id, "workflow", status="running", selection_snapshot=selection_snapshot
    )
    live_run.nodes = [NodeRun(live_run.id, "text.generate", "running", logical_operation="text:2")]
    uow.workflow_runs[live_run.id] = live_run
    with pytest.raises(AdapterNotConfiguredError, match="agentscope"):
        await TextGenerationService(lambda: uow).generate(
            GenerateTextBatchCommand(
                project.id,
                live_run.id,
                brief.revision,
                selection,
                brief,
                scope_ids=(project.id,),
            )
        )


def test_provider_policy_export_and_conversation_boundaries() -> None:
    policy = ProviderOperationPolicy("image.generate")
    policy.update(1, max_concurrency=2)
    with pytest.raises(RevisionConflictError):
        policy.update(1, max_concurrency=3)

    batch = EpisodeExportBatch(
        "project",
        (EpisodeExportSelection("episode", "timeline", 1, "episode-01"),),
    )
    job = ExportJob("project", "episode", "timeline")
    job.transition("preflighting")
    job.transition("rendering")
    job.transition("packaging")
    assert batch.export_profile == "light" and job.status == "packaging"
    with pytest.raises(ValidationDomainError, match="portable"):
        EpisodeExportBatch(
            "project",
            (EpisodeExportSelection("episode", "timeline", 1, "episode-01"),),
            export_profile="portable",
        )

    conversation = AgentConversation("project", "episode")
    turn = conversation.append_user_message("a" * 64, "correlation")
    assert turn.sequence == 1 and conversation.revision == 2


def test_provider_boundaries_keep_unconfigured_and_reconcile_unknown() -> None:
    with pytest.raises(AdapterNotConfiguredError, match="gpt_image"):
        GPTImageProvider().generate_image("prompt", ModelSelection("p", "q", "m", "mock"), "c")
    with pytest.raises(AdapterNotConfiguredError, match="agentscope"):
        OpenAICompatibleTextModelAdapter().generate_text(
            "prompt", ModelSelection("p", "q", "m", "live"), "c"
        )
    state = VideoSubmissionState("run:video:1")
    state.mark_unknown()
    assert state.status == "submission_unknown"
    assert state.reconcile(None, None) == "submission_unknown"
    assert state.reconcile("provider-job", "succeeded") == "succeeded"


@pytest.mark.asyncio
async def test_asset_edit_plan_is_image_video_only_and_execute_is_post_commit_intent() -> None:
    uow = InMemoryUnitOfWork()
    project = "project-edit"
    asset = Asset(project, "image", "Base")
    uow.state.assets[asset.id] = asset
    version = AssetVersion(
        asset.id,
        project,
        1,
        StorageObject("local", "workspace", "edit/base.png", "image/png", 4, "a" * 64),
    )
    uow.state.asset_versions[version.id] = version
    service = AgentEditService(lambda: uow)
    ref = AssetVersionRef(version.id, version.revision, version.content_hash or "", "image")
    plan = await service.create_plan(project, "episode-edit", "image", ref, (), "edit", "turn-1")
    execution = await service.execute(
        plan.id, plan.revision, "run-edit", "node-edit", "edit:1", "corr", "f" * 64
    )
    assert execution.status == "queued"
    assert [event["type"] for event in uow.outbox_events] == ["asset-edit.execute"]
    with pytest.raises(ValidationDomainError, match="unsupported_feature"):
        await service.create_plan(
            project, "episode-edit", "image", ref, (), "edit", "turn-2", mask="x"
        )
    assert len(uow.asset_edit_executions) == 1
    assert len(uow.outbox_events) == 1


def test_asset_edit_execution_state_is_monotonic_and_unknown_reconciles() -> None:
    execution = AssetEditExecution("plan", 1, "run", "node", "asset-edit", "corr", "a" * 64)
    execution.transition("running")
    execution.transition("submission_unknown")
    execution.transition("succeeded")
    with pytest.raises(ValidationDomainError, match="transition"):
        execution.transition("running")


def test_conversation_agent_reply_requires_pending_turn_and_is_idempotent_by_revision() -> None:
    conversation = AgentConversation("project", "episode")
    turn = conversation.append_user_message("a" * 64, "corr")
    reply = conversation.append_agent_reply(turn.id, "b" * 64, "corr-agent", 1)
    assert reply.role == "agent" and turn.status == "complete"
    with pytest.raises(RevisionConflictError):
        conversation.append_agent_reply(turn.id, "c" * 64, "corr-agent-2", 1)


def test_gpt_image_reference_and_payload_limits() -> None:
    provider = GPTImageProvider(allowed_hosts=frozenset({"images.example.test"}))
    provider.validate_reference_urls(["https://images.example.test/a.png"])
    with pytest.raises(ValueError, match="not_allowed"):
        provider.validate_reference_urls(["https://other.example.test/a.png"])
    with pytest.raises(ValueError, match="private"):
        GPTImageProvider(allowed_hosts=frozenset({"127.0.0.1"})).validate_reference_urls(
            ["https://127.0.0.1/a.png"]
        )


def test_gpt_image_media_and_network_boundaries() -> None:
    provider = GPTImageProvider(allowed_hosts=frozenset({"images.example.test"}))
    provider.validate_resolved_addresses("images.example.test", ["93.184.216.34"])
    with pytest.raises(ValueError, match="private"):
        provider.validate_resolved_addresses("images.example.test", ["10.0.0.8"])
    with pytest.raises(ValueError, match="metadata"):
        provider.validate_resolved_addresses("metadata.google.internal", [])
    with pytest.raises(ValueError, match="redirect"):
        provider.reject_redirect(302)
    png = (
        b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01"
        b"\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\x0dIDAT\x08\xd7"
        b"c\xf8\xcf\xc0\xf0\x1f\x00\x05\x00\x01\xff\x89\x99=\x1d\x00\x00\x00"
        b"\x00IEND\xaeB`\x82"
    )
    assert provider.validate_media_bytes(png, "image/png", width=1, height=1)[0] == "image/png"
    with pytest.raises(ValueError, match="mime_mismatch"):
        provider.validate_media_bytes(png, "image/jpeg", width=1, height=1)


def test_agnes_state_is_monotonic_and_preview_is_unconfigured() -> None:
    state = VideoSubmissionState("run:video:1")
    state.observe("running", "poll-1", "provider-1")
    state.observe("submitted", "poll-0", "provider-1")
    assert state.status == "running"
    state.mark_unknown()
    assert state.status == "submission_unknown"
    assert state.reconcile(None, None) == "submission_unknown"
    state.request_cancel()
    assert state.status == "cancelled"
    assert state.reconcile("provider-1", "succeeded") == "cancelled"
    with pytest.raises(AdapterNotConfiguredError, match="preview"):
        AgnesVideoProvider(
            configured=True,
            operations={"submit": {"version": "2.5", "preview": True}},
        ).validate_capability("submit", {})
    provider = AgnesVideoProvider(configured=True)
    selected = provider.probe_capabilities(
        [
            {"id": "preview", "operation": "submit", "version": "2.5", "preview": True},
            {"id": "stable-v2", "operation": "submit", "version": "2.0"},
        ]
    )
    assert selected["id"] == "stable-v2"
    with pytest.raises(AdapterNotConfiguredError, match="capability"):
        AgnesVideoProvider(configured=True).probe_capabilities(
            [{"id": "preview", "operation": "submit", "version": "2.5", "preview": True}]
        )
    media = LocalMediaInspector()
    stored = type(
        "Stored", (), {"object_ref": "workspace://video.mp4", "checksum": "c" * 64, "size_bytes": 4}
    )()
    metadata = media.inspect(stored, "run-1")
    derivatives = media.derive(stored, metadata, "run-1")
    assert {item["kind"] for item in derivatives} == {
        "normalized_metadata",
        "proxy",
        "thumbnail",
        "keyframe_index",
        "waveform",
    }
    assert all(item["status"] == "pending" for item in derivatives)


def test_video_operation_rejects_backward_terminal_transition() -> None:
    operation = VideoOperation(
        "project",
        "run",
        "video.submit:1",
        "provider",
        "profile",
        "model",
        "snapshot",
        "asset-version",
        0,
        "a" * 64,
        "shot-spec",
        1,
        "b" * 64,
        4.0,
        "16:9",
    )
    operation.transition("succeeded")
    operation.transition("running")
    assert operation.status == "succeeded"


@pytest.mark.asyncio
async def test_agnes_submit_preflight_and_unknown_submission() -> None:
    uow = InMemoryUnitOfWork()
    catalog = CatalogService(lambda: uow)
    await catalog.bootstrap()
    provider = next(iter(uow.providers.values()))
    provider.adapter_key = "agnes"
    provider.approval = "approved"
    provider.adapter_installed = True
    profile = next(iter(uow.profiles.values()))
    profile.adapter_identity = "agnes"
    profile.explicit_live_opt_in = True
    profile.credential_status = "configured"
    model = next(item for item in uow.models.values() if item.profile_id == profile.id)
    capability = profile.capability_snapshots["video.submit"]
    project = "project-video"
    episode = "episode-video"
    scene = "scene-video"
    shot_id = "shot-video"
    source_hash = "a" * 64
    spec_hash = "b" * 64
    source = AcceptedMediaEligibility(
        "image-candidate",
        1,
        project,
        episode,
        shot_id,
        "image-version",
        0,
        source_hash,
        "media_review",
        "image",
    )
    shot = Shot(scene, project, episode, 1, id=shot_id)
    shot.current_image = source
    shot.spec_ref = ImmutableOwnerRef("shot-spec", 1, spec_hash)
    uow.shots[shot.id] = shot
    uow.scenes[scene] = type("Scene", (), {"project_id": project, "episode_id": episode})()
    uow.episodes._state.episodes[episode] = type("Episode", (), {"project_id": project})()
    uow.asset_versions._state.asset_versions["image-version"] = type(
        "Version", (), {"project_id": project}
    )()

    def transport(*_args: object, **_kwargs: object) -> PortResult:
        raise RuntimeError("transport uncertain")

    service = AgnesVideoService(
        lambda: uow,
        AgnesVideoProvider(
            configured=True,
            operations={"submit": {"version": "2.0"}},
            transport=transport,
        ),
        catalog,
    )
    command = SubmitVideoCommand(
        project,
        episode,
        scene,
        shot_id,
        "run-video",
        "video.submit:1",
        provider.id,
        profile.id,
        model.id,
        capability.id,
        capability.revision,
        "image-version",
        0,
        source_hash,
        "shot-spec",
        1,
        spec_hash,
        4.0,
        "16:9",
        {},
        "cinematic room",
    )
    with pytest.raises(ValidationDomainError, match="run_scope_or_node_invalid"):
        await service.submit(command)
    assert uow.video_operations == {} and uow.provider_calls == {}
    run = WorkflowRun(
        project,
        "workflow-video",
        status="running",
        id=command.run_id,
        selection_snapshot={
            "providerId": provider.id,
            "profileId": profile.id,
            "modelId": model.id,
            "adapterKey": provider.adapter_key,
            "adapterIdentity": profile.adapter_identity,
            "profileRevision": profile.revision,
            "capabilitySnapshotId": capability.id,
            "capabilityRevision": capability.revision,
            "capabilityOperation": capability.operation,
            "capabilitySnapshots": {
                capability.operation: {"id": capability.id, "revision": capability.revision}
            },
        },
    )
    node = NodeRun(
        run.id,
        "media.generate.video",
        "running",
        logical_operation=command.logical_operation,
    )
    run.nodes = [node]
    uow.workflow_runs[run.id] = run
    run.selection_snapshot["modelId"] = "client-model"
    with pytest.raises(ValidationDomainError, match="run_selection_mismatch"):
        await service.submit(command)
    run = uow.workflow_runs[command.run_id]
    run.selection_snapshot["modelId"] = model.id
    with pytest.raises(ValidationDomainError, match="budget_gate_unconfirmed_or_stale"):
        await service.submit(command)
    assert uow.video_operations == {} and uow.provider_calls == {}
    uow.budget_gates[f"{run.id}:{node.logical_operation}"] = BudgetGate(
        project,
        run.id,
        node.id,
        node.logical_operation,
        video_request_fingerprint(command),
        "video.submit",
        1,
        "unknown",
        None,
        None,
        None,
        None,
        status="confirmed",
        confirmation_id="confirmed",
        user_uuid="11111111-1111-4111-8111-111111111111",
    )
    with pytest.raises(RuntimeError, match="uncertain"):
        await service.submit(command)
    operation = uow.video_operations[("run-video", "video.submit:1")]
    assert operation.status == "submission_unknown"
    assert len(uow.provider_calls) == 1
    assert next(iter(uow.provider_calls.values())).capability_snapshot_id == capability.id
    assert not uow.run_events
    operation.status = "submitted"
    with pytest.raises(ValidationDomainError, match="requires_submission_unknown"):
        await service.reconcile(
            ReconcileVideoCommand(command.run_id, command.logical_operation, None)
        )
    operation = uow.video_operations[(command.run_id, command.logical_operation)]
    operation.status = "submission_unknown"
    with pytest.raises(ProjectAccessForbiddenError):
        await service.reconcile(
            ReconcileVideoCommand(command.run_id, command.logical_operation, None),
            project_scope="foreign-project",
        )
    with pytest.raises(ValidationDomainError, match="image_candidate_unaccepted"):
        shot = uow.shots[shot_id]
        shot.current_image = None
        run = uow.workflow_runs[command.run_id]
        second = replace(command, logical_operation="video.submit:2")
        second_node = NodeRun(
            run.id,
            "media.generate.video",
            "running",
            logical_operation=second.logical_operation,
        )
        run.nodes.append(second_node)
        uow.budget_gates[f"{run.id}:{second_node.logical_operation}"] = BudgetGate(
            project,
            run.id,
            second_node.id,
            second_node.logical_operation,
            video_request_fingerprint(second),
            "video.submit",
            1,
            "unknown",
            None,
            None,
            None,
            None,
            status="confirmed",
            confirmation_id="confirmed-2",
            user_uuid="11111111-1111-4111-8111-111111111111",
        )
        await service.submit(second)


@pytest.mark.asyncio
async def test_image_generation_continuity_gate_and_unreferenced_candidate(tmp_path: Path) -> None:
    uow = InMemoryUnitOfWork()
    catalog = CatalogService(lambda: uow)
    await catalog.bootstrap()
    projects = ProjectsEpisodesService(lambda: uow)
    project = await projects.create_project(CreateProjectCommand("Image project"))
    episode = await projects.create_episode(CreateEpisodeCommand(project.id, "Episode", 1))
    assets = AssetsService(lambda: uow)
    asset = await assets.create_asset(CreateAssetCommand(project.id, "Generated", "image"))
    profile = next(iter(uow.profiles.values()))
    model = await catalog.create_model(CreateModelCommand(profile.id, "configured-image"))
    model.enabled = True
    capability = await catalog.snapshot(profile.id, "image.generate")
    entry = AssetBibleEntry(project.id, "bible-1", "character", id="entry-1")
    entry_version = entry.successor(
        {"name": "Room"},
        1,
        "00000000-0000-4000-8000-000000000001",
    )
    uow.asset_bible_entries[entry.id] = entry
    assignment = ContinuityAssignment(
        project.id,
        "shot",
        "shot-1",
        entry.id,
        entry_version.id,
        entry_version.revision,
        entry_version.content_hash,
    )
    continuity = ResolvedContinuitySnapshot(
        project.id,
        "shot-1",
        (assignment,),
        ((assignment.id, assignment.revision),),
        target_type="shot",
        target_revision=1,
    )
    uow.asset_bible_snapshots[continuity.id] = continuity
    service = ImageGenerationService(
        lambda: uow,
        DeterministicMockProvider(),
        LocalWorkspaceAdapter(tmp_path / "workspace"),
        catalog,
        AssetsService(lambda: uow),
    )
    command = GenerateImageCommand(
        project.id,
        episode.id,
        "shot-1",
        asset.id,
        "run-image",
        "image.generate:1",
        "generate",
        "a quiet room",
        next(iter(uow.providers.values())).id,
        profile.id,
        model.id,
        capability.id,
        capability.revision,
        continuity.id,
        continuity.revision,
        continuity.content_hash,
        continuity.target_revision,
        {},
    )
    candidate = await service.execute(command)
    assert candidate.status == "unreferenced"
    assert candidate.asset_version_id in uow.state.asset_versions
    assert uow.provider_calls
    assert not uow.run_events
    assert (await service.execute(command)).id == candidate.id

    uow.asset_bible_tasks["pending"] = type(
        "PendingTask", (), {"project_id": project.id, "target_id": "shot-1", "status": "pending"}
    )()
    blocked = replace(command, logical_operation="image.generate:2")
    with pytest.raises(ValidationDomainError, match="continuity_revision_pending"):
        await service.execute(blocked)
