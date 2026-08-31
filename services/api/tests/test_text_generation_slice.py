from __future__ import annotations

import ast
from dataclasses import dataclass, replace
from pathlib import Path

import httpx
import pytest
from fastapi.testclient import TestClient

from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.app import create_app
from video_agent_api.application.catalog import CatalogService
from video_agent_api.application.creative import (
    BindCreativeSourceCommand,
    CreativeService,
    SaveCreativeBriefCommand,
)
from video_agent_api.application.projects_episodes import ProjectsEpisodesService
from video_agent_api.application.source_material import (
    AppendSourceMaterialCommand,
    CreateSourceMaterialCommand,
    SourceMaterialService,
)
from video_agent_api.application.text_generation import (
    GenerateTextBatchCommand,
    RegenerateTextCandidateCommand,
    TextGenerationService,
)
from video_agent_api.domain.assets import Asset, AssetVersion, StorageObject
from video_agent_api.domain.creative import (
    CreativeBriefSourceBindingSnapshot,
    CreativeBriefVersion,
)
from video_agent_api.domain.errors import RevisionConflictError, ValidationDomainError
from video_agent_api.domain.runs import NodeRun, WorkflowRun
from video_agent_api.domain.text_review import StructuredTextCandidate
from video_agent_api.ports.contracts import ModelSelection, PortResult
from video_agent_api.ports.mocks import DeterministicMockProvider, build_mock_text_output
from video_agent_api.providers.text import OpenAICompatibleTextModelAdapter
from video_agent_api.resilience import (
    OperationsResilienceCoordinator,
    RuntimeResourceSnapshot,
    capacity_snapshot,
)


@dataclass
class CountingTextProvider:
    calls: int = 0

    def generate_text(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult:
        self.calls += 1
        return PortResult("request", correlation_id, {"prompt": prompt})


@dataclass
class InvalidStructuredTextProvider:
    def generate_text(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult:
        return PortResult(
            "invalid-request",
            correlation_id,
            {"payload": {"candidates": [{"kind": "story_spec"}]}},
        )


@dataclass
class IncompleteStructuredTextProvider:
    def generate_text(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult:
        output = build_mock_text_output(prompt)
        candidates = output["candidates"]
        assert isinstance(candidates, list)
        candidates.pop()
        return PortResult("incomplete-request", correlation_id, {"payload": output})


@dataclass
class UsageTextProvider:
    calls: int = 0

    def generate_text(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult:
        self.calls += 1
        return PortResult(
            "usage-request",
            correlation_id,
            {
                "payload": build_mock_text_output(prompt),
                "usage": {"input_tokens": 11, "output_tokens": 22, "prompt": "secret"},
            },
        )


@dataclass
class AmbiguousTextProvider:
    calls: int = 0

    def generate_text(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult:
        del prompt, selection, correlation_id
        self.calls += 1
        raise RuntimeError("response lost after submit")


@dataclass
class CorrelatedTextProvider:
    correlation_ids: list[str]

    def generate_text(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult:
        del selection
        self.correlation_ids.append(correlation_id)
        return PortResult(
            "correlated-request", correlation_id, {"payload": build_mock_text_output(prompt)}
        )


def _configured_running_text_run(uow: InMemoryUnitOfWork, project_id: str) -> WorkflowRun:
    provider = next(item for item in uow.providers.values() if item.adapter_key == "mock")
    profile = next(item for item in uow.profiles.values() if item.provider_id == provider.id)
    model = next(item for item in uow.models.values() if item.profile_id == profile.id)
    capability = profile.capability_snapshots["text.generate"]
    run = WorkflowRun(
        project_id,
        "workflow-version",
        status="running",
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
    run.nodes = [
        NodeRun(
            run.id,
            "text.generate",
            status="running",
            logical_operation="text.generate:test",
        )
    ]
    uow.workflow_runs[run.id] = run
    return run


async def _running_text_run(uow: InMemoryUnitOfWork, project_id: str) -> WorkflowRun:
    await CatalogService(lambda: uow).bootstrap()
    return _configured_running_text_run(uow, project_id)


def _model_selection(uow: InMemoryUnitOfWork) -> ModelSelection:
    profile = next(iter(uow.profiles.values()))
    provider = uow.providers[profile.provider_id]
    model = next(item for item in uow.models.values() if item.profile_id == profile.id)
    return ModelSelection(provider.id, profile.id, model.id, provider.adapter_key)


async def _text_context() -> tuple[
    InMemoryUnitOfWork,
    ProjectsEpisodesService,
    TextGenerationService,
    str,
    str,
    CreativeBriefVersion,
]:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    project = await projects.create_project("Structured text")
    brief = await CreativeService(lambda: uow).save_brief(
        SaveCreativeBriefCommand(project.id, "original", _brief_fields(), project.revision)
    )
    run = await _running_text_run(uow, project.id)
    return (
        uow,
        projects,
        TextGenerationService(lambda: uow, DeterministicMockProvider()),
        project.id,
        run.id,
        brief,
    )


def _brief_fields() -> dict[str, object]:
    return {
        "subject": "A missing witness",
        "genre": "Drama",
        "audience": "Adult",
        "characterPremise": "A witness must choose whether to speak",
        "style": "Grounded",
        "episodeDurationSeconds": 60,
        "episodeCount": 1,
        "scenesPerEpisode": 1,
        "shotsPerScene": 1,
    }


async def test_output_allowlist_and_owner_scope_fail_before_provider() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    project = await projects.create_project("Structured text")
    brief = await CreativeService(lambda: uow).save_brief(
        SaveCreativeBriefCommand(project.id, "original", _brief_fields(), project.revision)
    )
    run = await _running_text_run(uow, project.id)
    provider = CountingTextProvider()
    service = TextGenerationService(lambda: uow, provider)

    for unsupported in ("novel_body", "chapter", "chapter_draft", "story", "script"):
        with pytest.raises(ValidationDomainError, match="unsupported_output_type"):
            await service.generate(
                GenerateTextBatchCommand(
                    project.id,
                    run.id,
                    1,
                    ModelSelection("mock", "local", "model", "mock"),
                    brief,
                    None,
                    (unsupported,),
                )
            )
    foreign = WorkflowRun("foreign-project", "workflow-version")
    uow.workflow_runs[foreign.id] = foreign
    with pytest.raises(ValidationDomainError, match="Project/Run scope"):
        await service.generate(
            GenerateTextBatchCommand(
                project.id,
                foreign.id,
                1,
                ModelSelection("mock", "local", "model", "mock"),
                brief,
                scope_ids=(project.id,),
            )
        )


@pytest.mark.asyncio
async def test_text_resource_admission_rejects_before_provider_call() -> None:
    uow, _projects, _service, project_id, run_id, brief = await _text_context()
    provider = CountingTextProvider()
    unavailable = RuntimeResourceSnapshot(
        cpu_count=4,
        available_concurrency=4,
        memory_available_bytes=1024,
        memory_limit_bytes=2048,
        disk_free_bytes=None,
        disk_total_bytes=None,
        captured_at="2026-08-26T00:00:00+00:00",
        error="resource probe unavailable",
    )
    service = TextGenerationService(
        lambda: uow,
        provider,
        CatalogService(lambda: uow),
        resilience=OperationsResilienceCoordinator(
            unavailable,
            capacity_snapshot(unavailable, project_id),
        ),
    )

    with pytest.raises(ValidationDomainError, match="resource_probe_unavailable"):
        await service.generate(
            GenerateTextBatchCommand(
                project_id,
                run_id,
                brief.revision,
                _model_selection(uow),
                brief,
                scope_ids=(project_id,),
            )
        )

    assert provider.calls == 0
    assert uow.provider_calls == {}
    assert uow.text_review_batches == {}
    assert provider.calls == 0
    assert uow.text_candidates == {}
    assert uow.text_review_batches == {}


async def test_generation_is_idempotent_and_builds_complete_source_closure() -> None:
    uow, _projects, service, project_id, run_id, brief = await _text_context()
    command = GenerateTextBatchCommand(
        project_id,
        run_id,
        brief.revision,
        _model_selection(uow),
        brief,
        scope_ids=(project_id,),
    )
    batch = await service.generate(command)
    retry = await service.generate(command)

    assert retry.id == batch.id
    assert len(uow.text_review_batches) == 1
    assert len(uow.text_candidates) == 12
    by_id = {candidate.id: candidate for candidate in batch.candidates}
    for candidate in batch.candidates[1:]:
        source = by_id[candidate.source_candidate_ids[0]]
        assert candidate.source_hashes == (source.payload_hash,)
        assert candidate.payload["schema_version"] == candidate.schema_version
    shot_specs = [item for item in batch.candidates if item.kind == "shot_spec"]
    assert shot_specs
    assert all(len(item.payload["assetBibleRefs"]) == 6 for item in shot_specs)


async def test_text_generation_records_success_and_sanitized_usage() -> None:
    uow, _projects, _service, project_id, run_id, brief = await _text_context()
    service = TextGenerationService(lambda: uow, UsageTextProvider(), CatalogService(lambda: uow))
    batch = await service.generate(
        GenerateTextBatchCommand(
            project_id,
            run_id,
            brief.revision,
            _model_selection(uow),
            brief,
            scope_ids=(project_id,),
        )
    )

    assert batch.status == "pending_review"
    call = next(iter(uow.provider_calls.values()))
    assert call.status == "succeeded"
    assert call.provider_request_id == "usage-request"
    assert call.native_usage == {"input_tokens": 11, "output_tokens": 22}
    assert call.capability_snapshot_id is not None


async def test_text_generation_transport_ambiguity_stays_unknown_without_candidates() -> None:
    uow, _projects, _service, project_id, run_id, brief = await _text_context()
    provider = AmbiguousTextProvider()
    service = TextGenerationService(lambda: uow, provider, CatalogService(lambda: uow))

    with pytest.raises(RuntimeError, match="response lost"):
        await service.generate(
            GenerateTextBatchCommand(
                project_id,
                run_id,
                brief.revision,
                _model_selection(uow),
                brief,
                scope_ids=(project_id,),
            )
        )

    call = next(iter(uow.provider_calls.values()))
    assert provider.calls == 1
    assert call.status == "unknown"
    assert uow.text_review_batches == {}
    assert uow.text_candidates == {}


async def test_text_generation_submit_uses_frozen_provider_call_correlation() -> None:
    uow, _projects, _service, project_id, run_id, brief = await _text_context()
    provider = CorrelatedTextProvider([])
    service = TextGenerationService(lambda: uow, provider, CatalogService(lambda: uow))
    command = GenerateTextBatchCommand(
        project_id,
        run_id,
        brief.revision,
        _model_selection(uow),
        brief,
        scope_ids=(project_id,),
        correlation_id="caller-controlled-correlation",
    )

    await service.generate(command)

    call = next(iter(uow.provider_calls.values()))
    assert provider.correlation_ids == [call.outbound_correlation]
    assert call.outbound_correlation != command.correlation_id


async def test_text_generation_retry_finalizes_existing_batch_without_resubmission() -> None:
    uow, _projects, _service, project_id, run_id, brief = await _text_context()
    provider = UsageTextProvider()
    service = TextGenerationService(lambda: uow, provider, CatalogService(lambda: uow))
    command = GenerateTextBatchCommand(
        project_id,
        run_id,
        brief.revision,
        _model_selection(uow),
        brief,
        scope_ids=(project_id,),
    )
    batch = await service.generate(command)
    call_id = next(iter(uow.provider_calls))
    call = uow.provider_calls[call_id]
    uow.provider_calls[call_id] = replace(
        call,
        status="unknown",
        revision=call.revision + 1,
    )

    retry = await service.generate(command)

    assert retry.id == batch.id
    assert provider.calls == 1
    assert uow.provider_calls[call_id].status == "succeeded"
    assert uow.provider_calls[call_id].native_usage == {
        "input_tokens": 11,
        "output_tokens": 22,
    }


async def test_text_generation_ambiguous_call_requires_reconciliation_without_resubmit() -> None:
    uow, _projects, _service, project_id, run_id, brief = await _text_context()
    provider = UsageTextProvider()
    service = TextGenerationService(lambda: uow, provider, CatalogService(lambda: uow))
    command = GenerateTextBatchCommand(
        project_id,
        run_id,
        brief.revision,
        _model_selection(uow),
        brief,
        scope_ids=(project_id,),
    )
    await service.generate(command)
    call_id = next(iter(uow.provider_calls))
    call = uow.provider_calls[call_id]
    uow.provider_calls[call_id] = replace(
        call,
        status="unknown",
        revision=call.revision + 1,
    )
    uow.text_review_batches.clear()
    uow.text_candidates.clear()

    with pytest.raises(ValidationDomainError, match="requires reconciliation"):
        await service.generate(command)

    assert provider.calls == 1
    assert uow.provider_calls[call_id].status == "unknown"


async def test_text_generation_records_sanitized_failure_without_candidates() -> None:
    uow, _projects, _service, project_id, run_id, brief = await _text_context()
    service = TextGenerationService(
        lambda: uow, InvalidStructuredTextProvider(), CatalogService(lambda: uow)
    )
    with pytest.raises(ValidationDomainError):
        await service.generate(
            GenerateTextBatchCommand(
                project_id,
                run_id,
                brief.revision,
                _model_selection(uow),
                brief,
                scope_ids=(project_id,),
            )
        )

    call = next(iter(uow.provider_calls.values()))
    assert call.status == "failed"
    assert call.provider_request_id == "invalid-request"
    assert call.failure_code == "ValidationDomainError"
    assert uow.text_review_batches == {}


async def test_invalid_provider_structured_output_creates_no_candidates() -> None:
    uow, _projects, _service, project_id, run_id, brief = await _text_context()
    service = TextGenerationService(lambda: uow, InvalidStructuredTextProvider())

    with pytest.raises(ValidationDomainError, match="structured.*output|candidate graph"):
        await service.generate(
            GenerateTextBatchCommand(
                project_id,
                run_id,
                brief.revision,
                _model_selection(uow),
                brief,
                scope_ids=(project_id,),
            )
        )

    assert uow.text_candidates == {}
    assert uow.text_review_batches == {}


async def test_incomplete_provider_candidate_graph_creates_no_candidates() -> None:
    uow, _projects, _service, project_id, run_id, brief = await _text_context()
    service = TextGenerationService(lambda: uow, IncompleteStructuredTextProvider())

    with pytest.raises(ValidationDomainError, match="candidate graph is incomplete"):
        await service.generate(
            GenerateTextBatchCommand(
                project_id,
                run_id,
                brief.revision,
                _model_selection(uow),
                brief,
                scope_ids=(project_id,),
            )
        )

    assert uow.text_candidates == {}
    assert uow.text_review_batches == {}


async def test_generation_requires_running_text_node_and_exact_frozen_model() -> None:
    uow, _projects, _service, project_id, run_id, brief = await _text_context()
    provider = CountingTextProvider()
    service = TextGenerationService(lambda: uow, provider)
    run = uow.workflow_runs[run_id]
    selection = _model_selection(uow)
    command = GenerateTextBatchCommand(
        project_id,
        run_id,
        brief.revision,
        selection,
        brief,
        scope_ids=(project_id,),
    )

    run.status = "succeeded"
    with pytest.raises(ValidationDomainError, match="running Run"):
        await service.generate(command)
    run = uow.workflow_runs[run_id]
    run.status = "running"
    run.nodes[0].status = "succeeded"
    with pytest.raises(ValidationDomainError, match="running text.generate"):
        await service.generate(command)
    run = uow.workflow_runs[run_id]
    run.status = "running"
    run.nodes[0].status = "running"
    with pytest.raises(ValidationDomainError, match="does not match the Run snapshot"):
        await service.generate(
            replace(command, selection=replace(selection, model_id="client-model"))
        )

    assert provider.calls == 0
    assert uow.text_candidates == {}
    assert uow.text_review_batches == {}


async def test_generation_expands_exact_two_by_two_by_three_counts() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    project = await projects.create_project("2x2x3")
    fields = {**_brief_fields(), "episodeCount": 2, "scenesPerEpisode": 2, "shotsPerScene": 3}
    brief = await CreativeService(lambda: uow).save_brief(
        SaveCreativeBriefCommand(project.id, "original", fields, project.revision)
    )
    run = await _running_text_run(uow, project.id)
    batch = await TextGenerationService(lambda: uow, DeterministicMockProvider()).generate(
        GenerateTextBatchCommand(
            project.id,
            run.id,
            brief.revision,
            _model_selection(uow),
            brief,
            scope_ids=(project.id,),
        )
    )
    counts = {
        kind: sum(item.kind == kind for item in batch.candidates)
        for kind in {
            "story_spec",
            "script_spec",
            "episode",
            "scene",
            "shot",
            "shot_spec",
            "asset_bible_spec",
        }
    }
    assert counts == {
        "story_spec": 1,
        "script_spec": 2,
        "episode": 2,
        "scene": 4,
        "shot": 12,
        "shot_spec": 12,
        "asset_bible_spec": 6,
    }
    assert len({item.scope_id for item in batch.candidates if item.kind == "shot"}) == 12


async def test_adaptation_generation_freezes_current_inline_source_binding() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    project = await projects.create_project("Adaptation")
    creative = CreativeService(lambda: uow)
    brief = await creative.save_brief(
        SaveCreativeBriefCommand(project.id, "adaptation", _brief_fields(), project.revision)
    )
    sources = SourceMaterialService(lambda: uow)
    source = await sources.create(
        CreateSourceMaterialCommand(project.id, "novel", "inline_text", project.id)
    )
    version = await sources.append(
        AppendSourceMaterialCommand(
            source.id, source.revision, "inline_text", content=b"source", project_scope=project.id
        )
    )
    snapshot = CreativeBriefSourceBindingSnapshot(
        project.id,
        source.id,
        source.revision,
        version.content_hash,
        brief.creative_brief_id,
        brief.revision,
        brief.payload_hash,
        version.parse_status,
        version.validation_status,
        "bound",
        "1",
    )
    current_project = await uow.projects.get(project.id)
    assert current_project is not None
    await creative.bind_source(
        BindCreativeSourceCommand(project.id, snapshot, current_project.revision, brief.revision)
    )
    run = await _running_text_run(uow, project.id)
    batch = await TextGenerationService(lambda: uow, DeterministicMockProvider()).generate(
        GenerateTextBatchCommand(
            project.id,
            run.id,
            brief.revision,
            _model_selection(uow),
            brief,
            snapshot,
            scope_ids=(project.id,),
        )
    )
    assert batch.input_snapshot["sourceBinding"] == {
        "projectId": project.id,
        "sourceMaterialId": source.id,
        "sourceMaterialRevision": source.revision,
        "sourceContentHash": version.content_hash,
        "creativeBriefId": brief.creative_brief_id,
        "creativeBriefRevision": brief.revision,
        "creativeBriefPayloadHash": brief.payload_hash,
        "parseStatus": "parsed",
        "validationStatus": "valid",
        "bindingStatus": "bound",
        "bindingVersion": "1",
        "schema_version": "1.0.0",
    }


async def test_accept_handoff_ack_and_media_gate_are_exact_and_idempotent() -> None:
    uow, _projects, service, project_id, run_id, brief = await _text_context()
    batch = await service.generate(
        GenerateTextBatchCommand(
            project_id,
            run_id,
            1,
            _model_selection(uow),
            brief,
            scope_ids=(project_id,),
        )
    )
    accepted = await service.decide(batch.id, batch.revision, "accept")
    assert accepted.status == "accepted"
    handoff = await service.handoff_for_batch(batch.id)
    assert handoff is not None
    assert (await service.media_gate(handoff.id))["status"] == "blocked"

    with pytest.raises(ValidationDomainError, match="correlation"):
        await service.ack_handoff(handoff.id, "projects", 1, "projects-fp", "wrong")
    for owner in handoff.required_owners:
        ack = await service.ack_handoff(handoff.id, owner, 1, f"{owner}-fp", handoff.correlation_id)
        retry = await service.ack_handoff(
            handoff.id, owner, 1, f"{owner}-fp", handoff.correlation_id
        )
        assert retry.id == ack.id
    assert (await service.media_gate(handoff.id))["status"] == "ready"

    with pytest.raises(RevisionConflictError):
        await service.decide(batch.id, batch.revision, "accept")
    with pytest.raises(ValidationDomainError):
        await service.decide(accepted.id, accepted.revision, "approve")


async def test_regenerate_creates_successor_and_preserves_old_batch() -> None:
    uow, _projects, service, project_id, run_id, brief = await _text_context()
    batch = await service.generate(
        GenerateTextBatchCommand(
            project_id,
            run_id,
            1,
            _model_selection(uow),
            brief,
            scope_ids=(project_id,),
        )
    )
    target = next(item for item in batch.candidates if item.kind == "script_spec")
    with pytest.raises(ValidationDomainError, match="incomplete"):
        await service.regenerate(
            RegenerateTextCandidateCommand(
                batch.id,
                target.id,
                batch.revision,
                target.revision,
                {**target.payload, "status": "revised"},
                (),
                (),
            )
        )

    successor = await service.regenerate(
        RegenerateTextCandidateCommand(
            batch.id,
            target.id,
            batch.revision,
            target.revision,
            {**target.payload, "status": "revised"},
            target.source_candidate_ids,
            target.source_hashes,
        )
    )
    assert successor.supersedes_batch_id == batch.id
    assert uow.text_review_batches[batch.id].status == "stale"
    assert successor.candidates[0].id == batch.candidates[0].id
    successor_target = next(
        item for item in successor.candidates if item.supersedes_id == target.id
    )
    assert successor_target.payload["status"] == "revised"
    assert any(item.supersedes_id is not None for item in successor.candidates[2:])
    assert target.payload["status"] == "generated"


async def test_source_material_inline_and_uploaded_asset_ownership() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    first = await projects.create_project("Adaptation")
    second = await projects.create_project("Foreign")
    await CreativeService(lambda: uow).save_brief(
        SaveCreativeBriefCommand(first.id, "adaptation", _brief_fields(), first.revision)
    )
    service = SourceMaterialService(lambda: uow)
    source = await service.create(
        CreateSourceMaterialCommand(first.id, "novel", "inline_text", first.id)
    )
    inline = await service.append(
        AppendSourceMaterialCommand(
            source.id, 1, "inline_text", content=b"source", project_scope=first.id
        )
    )
    assert inline.asset_version_id is None
    with pytest.raises(ValidationDomainError, match="immutable"):
        await service.append(
            AppendSourceMaterialCommand(
                source.id,
                source.revision,
                "uploaded_file",
                content_hash="0" * 64,
                asset_version_id="unverified",
                project_scope=first.id,
            )
        )

    original = await projects.create_project("Original")
    await CreativeService(lambda: uow).save_brief(
        SaveCreativeBriefCommand(original.id, "original", _brief_fields(), original.revision)
    )
    with pytest.raises(ValidationDomainError, match="adaptation"):
        await service.create(
            CreateSourceMaterialCommand(original.id, "novel", "inline_text", original.id)
        )

    foreign_asset = Asset(second.id, "video", "foreign")
    uow.state.assets[foreign_asset.id] = foreign_asset
    foreign_version = AssetVersion(
        foreign_asset.id,
        second.id,
        1,
        StorageObject(
            "workspace",
            "bucket",
            "foreign.bin",
            "application/octet-stream",
            1,
            "1" * 64,
        ),
        "0" * 64,
    )
    uow.state.asset_versions[foreign_version.id] = foreign_version
    upload_source = await service.create(
        CreateSourceMaterialCommand(first.id, "novel", "uploaded_file", first.id)
    )
    with pytest.raises(ValidationDomainError, match="unverified or foreign"):
        await service.append(
            AppendSourceMaterialCommand(
                upload_source.id,
                upload_source.revision,
                "uploaded_file",
                content_hash="0" * 64,
                asset_version_id=foreign_version.id,
                project_scope=first.id,
            )
        )


async def test_uploaded_source_derives_hash_and_verified_status_from_asset_version() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    project = await projects.create_project("Uploaded adaptation")
    await CreativeService(lambda: uow).save_brief(
        SaveCreativeBriefCommand(project.id, "adaptation", _brief_fields(), project.revision)
    )
    asset = Asset(project.id, "document", "source", authorization_status="verified")
    uow.state.assets[asset.id] = asset
    checksum = "a" * 64
    asset_version = AssetVersion(
        asset.id,
        project.id,
        1,
        StorageObject(
            "local_workspace",
            "workspace",
            f"projects/{project.id}/source.txt",
            "text/plain",
            10,
            checksum,
        ),
        checksum,
    )
    uow.state.asset_versions[asset_version.id] = asset_version
    service = SourceMaterialService(lambda: uow)
    source = await service.create(
        CreateSourceMaterialCommand(project.id, "novel", "uploaded_file", project.id)
    )

    version = await service.append(
        AppendSourceMaterialCommand(
            source.id,
            source.revision,
            "uploaded_file",
            asset_version_id=asset_version.id,
            project_scope=project.id,
        )
    )

    assert version.content_hash == checksum
    assert version.parse_status == "parsed"
    assert version.validation_status == "valid"


def test_text_http_camel_case_and_schema_conflict_have_no_write() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    app = create_app(readiness_probe=lambda: True, projects_episodes_service=projects)
    with TestClient(app) as client:
        project = client.post("/v1/projects", json={"name": "HTTP text"}).json()
        fields = _brief_fields()
        brief_response = client.put(
            f"/v1/projects/{project['id']}/creative/brief",
            headers={
                "If-Match": str(project["revision"]),
                "X-Project-Scope": project["id"],
            },
            json={
                "creationMode": "original",
                **fields,
                "expectedRevision": project["revision"],
                "schemaVersion": "1.0.0",
            },
        )
        assert brief_response.status_code == 200
        brief = brief_response.json()
        run = _configured_running_text_run(uow, project["id"])
        selection = _model_selection(uow)
        body = {
            "runId": run.id,
            "briefRevision": 1,
            "providerId": selection.provider_id,
            "profileId": selection.profile_id,
            "modelId": selection.model_id,
            "adapterKey": selection.adapter_key,
            "creativeBrief": {
                "creativeBriefId": brief["creative_brief_id"],
                **fields,
                "revision": brief["revision"],
                "schemaVersion": brief["schema_version"],
                "payloadHash": brief["payload_hash"],
            },
            "requestedKinds": [
                "story_spec",
                "script_spec",
                "episode",
                "scene",
                "shot",
                "shot_spec",
                "asset_bible_spec",
            ],
            "scopeIds": [project["id"]],
            "schemaVersion": "1.0.0",
            "schema_version": "2.0.0",
        }
        response = client.post(
            f"/v1/projects/{project['id']}/text-review-batches",
            json=body,
            headers={"X-Project-Scope": project["id"]},
        )
        assert response.status_code == 422
        assert uow.text_review_batches == {}
        assert uow.text_candidates == {}

        created = client.post(
            f"/v1/projects/{project['id']}/text-review-batches",
            json={key: value for key, value in body.items() if key != "schema_version"},
            headers={"X-Project-Scope": project["id"]},
        )
        assert created.status_code == 201, created.text
        batch_id = created.json()["id"]
        foreign = client.get(
            f"/v1/text-review-batches/{batch_id}",
            headers={"X-Project-Scope": "foreign"},
        )
        assert foreign.status_code == 403


def test_creative_and_text_routes_require_matching_project_scope() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    app = create_app(readiness_probe=lambda: True, projects_episodes_service=projects)
    with TestClient(app) as client:
        project = client.post("/v1/projects", json={"name": "Scoped text"}).json()
        creative_path = f"/v1/projects/{project['id']}/creative"
        text_path = f"/v1/projects/{project['id']}/text-review-batches"
        before = (len(uow.audit_events), len(uow.outbox_events))

        assert client.get(creative_path).status_code == 403
        assert client.get(text_path).status_code == 403
        assert client.get(creative_path, headers={"X-Project-Scope": "foreign"}).status_code == 403
        assert before == (len(uow.audit_events), len(uow.outbox_events))
        assert (
            client.get(creative_path, headers={"X-Project-Scope": project["id"]}).status_code == 200
        )


def test_skill_route_http_requires_explicit_current_selection() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    app = create_app(readiness_probe=lambda: True, projects_episodes_service=projects)
    with TestClient(app) as client:
        project = client.post("/v1/projects", json={"name": "Skill route"}).json()
        body = {
            "nodeKey": "text.generate",
            "launchId": "launch-1",
            "projectType": "short_drama",
            "stage": "text.generate",
            "targetModel": "configured-model",
            "query": "story spec",
            "allowedTools": ["text_model"],
            "allowedLicenses": ["MIT"],
            "allowedSkills": ["drama-skills"],
            "requiredCapabilities": ["story_spec"],
            "selectionMode": "fixed",
        }
        scope = {"X-Project-Scope": project["id"]}
        response = client.post(
            f"/v1/projects/{project['id']}/skill-route-decisions", json=body, headers=scope
        )
        assert response.status_code == 201
        decision = response.json()
        assert decision["needsManualSelection"] is True
        assert decision["candidates"][0]["name"] == "drama-skills"
        assert len(decision["candidates"][0]["digest"]) == 64
        selection_body = {
            "skillName": "drama-skills",
            "skillVersion": "1.0.0",
            "actorUuid": "00000000-0000-4000-8000-000000000111",
            "expectedRevision": decision["revision"],
        }
        foreign = client.post(
            f"/v1/skill-route-decisions/{decision['id']}/selection",
            json=selection_body,
            headers={"X-Project-Scope": "foreign"},
        )
        assert foreign.status_code == 403
        assert uow.skill_route_selections == {}
        selected = client.post(
            f"/v1/skill-route-decisions/{decision['id']}/selection",
            json=selection_body,
            headers=scope,
        )
        retry = client.post(
            f"/v1/skill-route-decisions/{decision['id']}/selection",
            json=selection_body,
            headers=scope,
        )
        assert selected.status_code == 201
        assert retry.json()["id"] == selected.json()["id"]

        conflict = client.post(
            f"/v1/projects/{project['id']}/skill-route-decisions",
            json={**body, "query": "different route"},
            headers=scope,
        )
        assert conflict.status_code == 409
        invalid = client.post(
            f"/v1/skill-route-decisions/{decision['id']}/selection",
            json={**selection_body, "skillName": "novel-writing"},
            headers=scope,
        )
        assert invalid.status_code == 409
    assert uow.workflow_runs == {}
    assert uow.provider_calls == {}


def test_openai_compatible_text_adapter_does_not_repeat_ambiguous_submit() -> None:
    calls: list[int] = []

    def retry_then_success(request: httpx.Request) -> httpx.Response:
        assert request.headers["authorization"] == "Bearer secret"
        calls.append(len(calls) + 1)
        if len(calls) == 1:
            return httpx.Response(503, request=request)
        return httpx.Response(
            200,
            headers={"x-request-id": "request-2"},
            json={
                "choices": [{"message": {"content": '{"story":"ok"}'}}],
                "usage": {"prompt_tokens": 4},
            },
            request=request,
        )

    selection = ModelSelection("provider", "profile", "model", "openai-compatible")
    adapter = OpenAICompatibleTextModelAdapter(
        "https://text.example.test",
        "secret",
        max_retries=2,
        transport=httpx.MockTransport(retry_then_success),
    )
    with pytest.raises(httpx.HTTPStatusError):
        adapter.generate_text("prompt", selection, "correlation")
    assert calls == [1]

    non_retry_calls = 0

    def invalid_response(request: httpx.Request) -> httpx.Response:
        nonlocal non_retry_calls
        non_retry_calls += 1
        return httpx.Response(
            200,
            json={"choices": [{"message": {"content": "not-json"}}]},
            request=request,
        )

    invalid = OpenAICompatibleTextModelAdapter(
        "https://text.example.test",
        "secret",
        max_retries=2,
        transport=httpx.MockTransport(invalid_response),
    )
    with pytest.raises(ValueError):
        invalid.generate_text("prompt", selection, "correlation")
    assert non_retry_calls == 1


def test_text_slice_imports_no_foreign_owner_application_or_repository() -> None:
    source = (
        Path(__file__).parents[1] / "src/video_agent_api/application/text_generation.py"
    ).read_text(encoding="utf-8")
    imported = {
        node.module
        for node in ast.walk(ast.parse(source))
        if isinstance(node, ast.ImportFrom) and node.module is not None
    }
    assert imported.isdisjoint(
        {
            "video_agent_api.application.runs",
            "video_agent_api.application.catalog",
            "video_agent_api.application.assets",
            "video_agent_api.application.scenes",
            "video_agent_api.application.timeline",
            "video_agent_api.adapters.sqlalchemy_models",
        }
    )


async def test_text_failures_and_success_do_not_mutate_foreign_owner_facts() -> None:
    uow, _projects, service, project_id, run_id, brief = await _text_context()
    project = await uow.projects.get(project_id)
    assert project is not None
    owner_revision = project.revision
    before_run_events = dict(uow.run_events)
    before_provider_calls = dict(uow.provider_calls)
    before_scenes = dict(uow.scenes)
    before_asset_bible = dict(uow.asset_bible_entries)
    batch = await service.generate(
        GenerateTextBatchCommand(
            project_id,
            run_id,
            brief.revision,
            _model_selection(uow),
            brief,
            scope_ids=(project_id,),
        )
    )
    with pytest.raises(ValidationDomainError, match="terminal or action"):
        await service.decide(batch.id, batch.revision, "approve")
    assert project.revision == owner_revision
    assert uow.run_events == before_run_events
    assert uow.provider_calls == before_provider_calls
    assert uow.scenes == before_scenes
    assert uow.asset_bible_entries == before_asset_bible


def test_structured_candidate_rejects_schema_scope_and_source_mismatch() -> None:
    base = {
        "kind": "story_spec",
        "status": "generated",
        "scopeId": "project",
        "schema_version": "1.0.0",
    }
    with pytest.raises(ValidationDomainError, match="schema version"):
        StructuredTextCandidate(
            "project",
            "story_spec",
            "project",
            {**base, "schema_version": "2.0.0"},
        )
    with pytest.raises(ValidationDomainError, match="payload scope"):
        StructuredTextCandidate(
            "project",
            "story_spec",
            "project",
            {**base, "scopeId": "foreign"},
        )
    with pytest.raises(ValidationDomainError, match="not aligned"):
        StructuredTextCandidate(
            "project",
            "story_spec",
            "project",
            base,
            ("source",),
            (),
        )
