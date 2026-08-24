from __future__ import annotations

import hashlib
from typing import Any

from video_agent_api.application.assets import (
    AppendAssetVersionCommand,
    AssetsService,
    CreateAssetCommand,
)
from video_agent_api.application.catalog import CatalogService, CreateModelCommand
from video_agent_api.domain.assets import AssetVersion, StorageObject
from video_agent_api.domain.provider_ops import ProviderCall
from video_agent_api.domain.runs import WorkflowRun, WorkflowVersion
from video_agent_api.domain.video_generation import VideoOperation, VideoTakeCandidate


async def create_persisted_generated_export_asset(
    uow_factory: Any,
    storage: Any,
    project_id: str,
    episode_id: str,
    *,
    label: str,
) -> AssetVersion:
    """Create every normalized owner fact required by export provenance preflight."""
    catalog = CatalogService(uow_factory)
    await catalog.bootstrap()
    async with uow_factory() as uow:
        provider = next(iter(uow.providers.values()))
        profile = next(iter(uow.profiles.values()))
    model = await catalog.create_model(CreateModelCommand(profile.id, f"{label}-model"))
    capability = await catalog.snapshot(profile.id, "video.submit")

    content = b"source"
    checksum = hashlib.sha256(content).hexdigest()
    object_key = f"projects/{project_id}/{label}.mp4"
    storage.put(f"workspace://{object_key}", content, f"seed:{label}")
    assets = AssetsService(uow_factory)
    asset = await assets.create_asset(
        CreateAssetCommand(
            project_id,
            label,
            "video",
            source_type="provider_generated",
            authorization_status="verified",
            license_label="owned",
        )
    )
    version = await assets.append_version(
        AppendAssetVersionCommand(
            asset.id,
            StorageObject(
                "local",
                "workspace",
                object_key,
                "video/mp4",
                len(content),
                checksum,
            ),
            "b" * 64,
        )
    )

    logical_operation = f"video.submit:{label}"
    async with uow_factory() as uow:
        selected_skills = [
            item for item in uow.skills if item.name in {"novel-writing", "drama-skills"}
        ]
        workflow = WorkflowVersion(
            project_id,
            scope_ids=(project_id,),
            definition={"nodes": [{"key": "video.submit"}]},
        )
        run = WorkflowRun(
            project_id,
            workflow.id,
            selection_snapshot={
                "skillRevisionIds": [f"{item.name}@{item.version}" for item in selected_skills],
                "skillDigests": [item.digest for item in selected_skills],
            },
        )
        operation = VideoOperation(
            project_id,
            run.id,
            logical_operation,
            provider.id,
            profile.id,
            model.id,
            capability.id,
            "source-version",
            0,
            "c" * 64,
            "shot-spec",
            1,
            "d" * 64,
            1.0,
            "9:16",
            status="succeeded",
            episode_id=episode_id,
            target_id="shot",
            asset_id=asset.id,
        )
        provider_call = ProviderCall(
            project_id,
            run.id,
            None,
            logical_operation,
            "video.submit",
            provider.id,
            profile.id,
            model.id,
            capability.id,
            "e" * 64,
            "succeeded",
            cost_status="known",
            cost_value="1.0",
            cost_currency="CNY",
            cost_source="provider-billing",
            native_usage={"seconds": 1},
        )
        candidate = VideoTakeCandidate(
            project_id,
            episode_id,
            "shot",
            run.id,
            logical_operation,
            "source-version",
            0,
            "c" * 64,
            "shot-spec",
            1,
            "d" * 64,
            1.0,
            "9:16",
            version.id,
            version.revision,
            str(version.content_hash),
            "provider-request",
            status="accepted",
        )
        uow.workflow_by_project[project_id] = workflow
        uow.workflow_runs[run.id] = run
        uow.video_operations[(run.id, logical_operation)] = operation
        uow.provider_calls[provider_call.id] = provider_call
        uow.provider_call_keys[(run.id, logical_operation)] = provider_call.id
        uow.video_take_candidates[candidate.id] = candidate
        await uow.commit()
    return version
