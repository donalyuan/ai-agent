"""Project-scoped all-member preflight and per-Episode export orchestration."""

from __future__ import annotations

import hashlib
import json
from dataclasses import asdict, replace
from datetime import UTC, datetime
from typing import Any, cast

from video_agent_api.application.rendering import compile_render_plan
from video_agent_api.domain.errors import RevisionConflictError, ValidationDomainError
from video_agent_api.domain.exports import (
    EpisodeExportBatch,
    EpisodeExportSelection,
    ExportArtifact,
    ExportDiagnosticTarget,
    ExportDispatchOutbox,
    ExportExecutionSnapshot,
    ExportInputSnapshot,
    ExportJob,
    ExportSettings,
)
from video_agent_api.ports.contracts import ExportDownloadGrantPort, StoredObjectRef
from video_agent_api.ports.rendering import FfmpegRenderPort
from video_agent_api.resilience import OperationsResilienceCoordinator, admission_refs


class ExportService:
    def __init__(
        self,
        uow_factory: Any,
        renderer: FfmpegRenderPort,
        download_grants: ExportDownloadGrantPort | None = None,
        storage: Any | None = None,
        resilience: OperationsResilienceCoordinator | None = None,
        renderer_identity: dict[str, object] | None = None,
    ) -> None:
        self._uow_factory = uow_factory
        self._renderer = renderer
        self._download_grants = download_grants
        self._storage = storage
        self._resilience = resilience
        # Test renderers may omit identity; production subprocess renderers must
        # receive the complete composed identity and never get a fabricated one.
        if renderer_identity is not None:
            self._renderer_identity = dict(renderer_identity)
        elif renderer.__class__.__name__ == "MockFfmpegRenderAdapter":
            self._renderer_identity = {
                "profileId": "local-test-renderer",
                "profileRevision": 1,
                "capabilitySnapshotId": "local-test-capability",
                "capabilityRevision": 1,
            }
        else:
            self._renderer_identity = {}

    def configure_renderer(
        self, renderer: FfmpegRenderPort, renderer_identity: dict[str, object]
    ) -> None:
        """Install a renderer only after its durable catalog identity was resolved."""
        self._renderer = renderer
        self._renderer_identity = dict(renderer_identity)

    async def probe_renderer(self, project_id: str) -> dict[str, object]:
        async with self._uow_factory() as uow:
            if await uow.projects.get(project_id) is None:
                raise ValidationDomainError("export project not found")
        return asdict(self._renderer.probe())

    async def create_batch(
        self,
        project_id: str,
        selections: list[dict[str, object]],
        export_profile: str,
        idempotency_key: str,
        settings: dict[str, object] | None = None,
        expected_revision: int = 1,
        storage_profile_id: str | None = None,
        storage_profile_revision: int | None = None,
    ) -> EpisodeExportBatch:
        if not idempotency_key.strip() or expected_revision != 1:
            raise ValidationDomainError("export idempotency key or expectedRevision is invalid")
        parsed = tuple(self._selection(item) for item in selections)
        export_settings = _settings(settings)
        if self._storage is None:
            raise ValidationDomainError("export storage is unconfigured")
        if not storage_profile_id or storage_profile_revision is None:
            raise ValidationDomainError("export StorageProfile selection is required")
        frozen_admissions: dict[tuple[str, str], dict[str, object]] = {}
        if self._resilience is not None:
            for selection in parsed:
                operation_key = (
                    f"export:{project_id}:{idempotency_key}:{selection.episode_id}:"
                    f"{selection.timeline_version_id}"
                )
                admission = self._resilience.freeze(project_id, "export.render", operation_key)
                if not admission.allowed:
                    raise ValidationDomainError(
                        admission.diagnostic or "export_resource_admission_blocked"
                    )
                frozen_admissions[(selection.episode_id, selection.timeline_version_id)] = (
                    admission_refs(admission)
                )
        # Renderer capability is a prerequisite, never a post-submit background surprise.
        renderer_capability = self._renderer.probe()
        async with self._uow_factory() as uow:
            project = await uow.projects.get(project_id)
            if project is None:
                raise ValidationDomainError("export project not found")
            existing = next(
                (
                    batch
                    for batch in uow.export_batches.values()
                    if batch.project_id == project_id and batch.idempotency_key == idempotency_key
                ),
                None,
            )
            if existing is not None:
                if existing.selections != parsed or existing.settings != export_settings:
                    raise ValidationDomainError("export idempotency fingerprint conflict")
                return cast(EpisodeExportBatch, existing)

            # The complete explicit set is validated before the first mutation.
            prepared: list[tuple[EpisodeExportSelection, ExportExecutionSnapshot]] = []
            for selection in parsed:
                episode = await uow.episodes.get(selection.episode_id)
                version = uow.timeline_versions.get(selection.timeline_version_id)
                if episode is None or episode.project_id != project_id:
                    raise ValidationDomainError("export Episode is stale or foreign")
                if (
                    version is None
                    or version.project_id != project_id
                    or version.episode_id != selection.episode_id
                    or version.revision != selection.timeline_version_revision
                ):
                    raise ValidationDomainError("published TimelineVersion is stale or foreign")
                _preflight_snapshot(version.cut_snapshot)
                plan = compile_render_plan(version, export_settings)
                snapshot = await self._execution_snapshot(
                    uow,
                    project_id,
                    selection,
                    version,
                    plan,
                    storage_profile_id,
                    storage_profile_revision,
                    renderer_capability,
                    frozen_admissions.get(
                        (selection.episode_id, selection.timeline_version_id), {}
                    ),
                )
                prepared.append((selection, snapshot))

            batch = EpisodeExportBatch(
                project_id,
                parsed,
                export_profile=cast(Any, export_profile),
                idempotency_key=idempotency_key,
                settings=export_settings,
            )
            for selection, snapshot in prepared:
                job = ExportJob(
                    project_id,
                    selection.episode_id,
                    selection.timeline_version_id,
                    batch_id=batch.id,
                    logical_operation=(
                        f"export:{batch.id}:{selection.episode_id}:"
                        f"{selection.timeline_version_id}:initial"
                    ),
                    execution_snapshot=snapshot,
                )
                for artifact_type in ("mp4", "srt", "light_manifest"):
                    job.append_artifact(
                        ExportArtifact(
                            job.id,
                            cast(Any, artifact_type),
                            "pending",
                            operation_key=(f"export-upload:{project_id}:{job.id}:{artifact_type}"),
                        )
                    )
                batch.jobs.append(job)
                uow.export_jobs[job.id] = job
                dispatch = ExportDispatchOutbox(
                    project_id=project_id,
                    batch_id=batch.id,
                    job_id=job.id,
                    logical_operation=job.logical_operation,
                )
                uow.export_dispatch_outbox[dispatch.id] = dispatch
            uow.export_batches[batch.id] = batch
            uow.audit_events.append(
                {
                    "type": "export.batch.created",
                    "projectId": project_id,
                    "batchId": batch.id,
                    "memberCount": len(batch.jobs),
                }
            )
            uow.outbox_events.append(
                {
                    "type": "export.batch.created",
                    "batchId": batch.id,
                    "jobIds": [job.id for job in batch.jobs],
                }
            )
            await uow.commit()
            return batch

    async def transition_job(
        self, project_id: str, job_id: str, target: str, expected_revision: int
    ) -> ExportJob:
        async with self._uow_factory() as uow:
            job = self._job(uow, project_id, job_id)
            if job.revision != expected_revision:
                raise RevisionConflictError(job.id, expected_revision, job.revision)
            job.transition(cast(Any, target))
            batch = self._batch_for_job(uow, job)
            batch.summarize()
            await uow.commit()
            return job

    async def set_packaging_phase(
        self, project_id: str, job_id: str, phase: str, expected_revision: int
    ) -> ExportJob:
        async with self._uow_factory() as uow:
            job = self._job(uow, project_id, job_id)
            if job.revision != expected_revision:
                raise RevisionConflictError(job.id, expected_revision, job.revision)
            job.set_packaging_phase(cast(Any, phase))
            await uow.commit()
            return job

    async def start_rendering(
        self,
        project_id: str,
        job_id: str,
        expected_revision: int,
        render_plan_hash: str,
    ) -> ExportJob:
        if len(render_plan_hash) != 64:
            raise ValidationDomainError("render plan hash is invalid")
        async with self._uow_factory() as uow:
            job = self._job(uow, project_id, job_id)
            if job.revision != expected_revision:
                raise RevisionConflictError(job.id, expected_revision, job.revision)
            if job.render_plan_hash not in {None, render_plan_hash}:
                raise ValidationDomainError("export RenderPlan changed after preflight")
            job.render_plan_hash = render_plan_hash
            job.transition("rendering")
            await uow.commit()
            return job

    async def complete_rendering(
        self,
        project_id: str,
        job_id: str,
        expected_revision: int,
        render_plan_hash: str,
        raw_diagnostic: str,
    ) -> ExportJob:
        async with self._uow_factory() as uow:
            job = self._job(uow, project_id, job_id)
            if job.revision != expected_revision:
                raise RevisionConflictError(job.id, expected_revision, job.revision)
            if job.render_plan_hash != render_plan_hash:
                raise ValidationDomainError("renderer result does not match the RenderPlan")
            job.renderer_diagnostic = raw_diagnostic
            job.transition("packaging")
            await uow.commit()
            return job

    async def register_artifact(
        self,
        project_id: str,
        job_id: str,
        artifact_type: str,
        size_bytes: int,
        checksum: str,
        verified: bool,
        expected_revision: int,
        stored_object: StoredObjectRef | None = None,
        storage_profile_revision: int | None = None,
    ) -> ExportArtifact:
        mime_by_type = {
            "mp4": "video/mp4",
            "srt": "application/x-subrip",
            "light_manifest": "application/json",
        }
        if (
            artifact_type not in mime_by_type
            or size_bytes < 0
            or len(checksum) != 64
            or verified is not True
        ):
            raise ValidationDomainError("export artifact metadata is invalid")
        async with self._uow_factory() as uow:
            job = self._job(uow, project_id, job_id)
            if job.revision != expected_revision:
                raise RevisionConflictError(job.id, expected_revision, job.revision)
            if job.status != "packaging" or job.packaging_phase != "registering":
                raise ValidationDomainError("artifact registration requires registering subphase")
            operation_key = f"export-upload:{job.project_id}:{job.id}:{artifact_type}"
            existing = next(
                (item for item in job.artifacts if item.artifact_type == artifact_type), None
            )
            if existing is None:
                raise ValidationDomainError("export artifact reservation is missing")
            if existing.status == "verified":
                if existing.checksum != checksum or existing.size_bytes != size_bytes:
                    raise ValidationDomainError("export artifact fingerprint conflict")
                return existing
            if stored_object is None:
                raise ValidationDomainError("verified StoredObjectRef is required")
            if (
                not stored_object.verified
                or stored_object.project_id != job.project_id
                or stored_object.size_bytes != size_bytes
                or stored_object.checksum != checksum
                or stored_object.mime_type != mime_by_type[artifact_type]
                or stored_object.operation_key != operation_key
            ):
                raise ValidationDomainError("export stored object verification failed")
            artifact = replace(
                existing,
                status="verified",
                size_bytes=size_bytes,
                checksum=checksum,
                storage_object_ref=asdict(stored_object),
                operation_key=operation_key,
                storage_profile_revision=storage_profile_revision,
                mime_type=mime_by_type[artifact_type],
            )
            job.artifacts[job.artifacts.index(existing)] = artifact
            job.revision += 1
            await uow.commit()
            return artifact

    async def retry_failed_members(
        self,
        project_id: str,
        batch_id: str,
        episode_ids: list[str],
        logical_operation: str,
    ) -> list[ExportJob]:
        if not episode_ids or len(episode_ids) != len(set(episode_ids)) or not logical_operation:
            raise ValidationDomainError("retry member set or logical operation is invalid")
        self._renderer.probe()
        async with self._uow_factory() as uow:
            batch = uow.export_batches.get(batch_id)
            if batch is None or batch.project_id != project_id:
                raise ValidationDomainError("export batch not found")
            by_episode = {job.episode_id: job for job in batch.jobs}
            if any(
                episode_id not in by_episode or by_episode[episode_id].status != "failed"
                for episode_id in episode_ids
            ):
                raise ValidationDomainError("retry can only target explicit failed members")
            created: list[ExportJob] = []
            for episode_id in episode_ids:
                predecessor = by_episode[episode_id]
                job = ExportJob(
                    predecessor.project_id,
                    predecessor.episode_id,
                    predecessor.timeline_version_id,
                    batch_id=batch.id,
                    logical_operation=logical_operation,
                    execution_snapshot=predecessor.execution_snapshot,
                )
                for artifact_type in ("mp4", "srt", "light_manifest"):
                    job.append_artifact(
                        ExportArtifact(
                            job.id,
                            cast(Any, artifact_type),
                            "pending",
                            operation_key=(
                                f"export-upload:{job.project_id}:{job.id}:{artifact_type}"
                            ),
                        )
                    )
                batch.jobs.append(job)
                uow.export_jobs[job.id] = job
                dispatch = ExportDispatchOutbox(
                    project_id=project_id,
                    batch_id=batch.id,
                    job_id=job.id,
                    logical_operation=job.logical_operation,
                )
                uow.export_dispatch_outbox[dispatch.id] = dispatch
                created.append(job)
            await uow.commit()
            return created

    async def _execution_snapshot(
        self,
        uow: Any,
        project_id: str,
        selection: EpisodeExportSelection,
        version: Any,
        plan: Any,
        storage_profile_id: str,
        storage_profile_revision: int,
        renderer_capability: Any,
        frozen_admission_refs: dict[str, object],
    ) -> ExportExecutionSnapshot:
        storage = self._storage
        if storage is None:
            raise ValidationDomainError("export storage is unconfigured")
        capability = storage.capability(storage_profile_revision)
        if capability.profile_revision != storage_profile_revision:
            raise ValidationDomainError("storage capability revision is stale")
        if storage_profile_id == "local-test-offline":
            profile_payload: dict[str, object] = {
                "adapterKey": "local_workspace",
                "profileId": storage_profile_id,
                "projectId": project_id,
                "revision": storage_profile_revision,
                "bucket": "workspace",
                "endpoint": "workspace://local",
                "region": "local",
                "bucketBindingId": "local-workspace",
            }
        else:
            profile = uow.storage_profiles.get(storage_profile_id)
            if (
                profile is None
                or profile.project_id != project_id
                or profile.revision != storage_profile_revision
                or not profile.enabled
                or project_id not in profile.project_scope
                or not profile.private_bucket
                or not profile.bucket_binding_id
            ):
                raise ValidationDomainError("export StorageProfile is stale, disabled, or foreign")
            profile_payload = {
                "adapterKey": profile.adapter_key,
                "profileId": profile.id,
                "projectId": profile.project_id,
                "revision": profile.revision,
                "bucketBindingId": profile.bucket_binding_id,
                "bucket": profile.bucket,
                "endpoint": profile.endpoint,
                "region": profile.region,
                "credentialRef": profile.credential_ref,
            }
        capability_payload = {
            "profileRevision": capability.profile_revision,
            "minPartSizeBytes": capability.min_part_size_bytes,
            "maxPartSizeBytes": capability.max_part_size_bytes,
            "maxPartCount": capability.max_part_count,
            "maxObjectSizeBytes": capability.max_object_size_bytes,
        }
        profile_hash = _payload_hash({**profile_payload, "capability": capability_payload})
        renderer_payload = {
            "ffmpegVersion": renderer_capability.ffmpeg_version,
            "ffprobeVersion": renderer_capability.ffprobe_version,
            "h264Decoder": renderer_capability.h264_decoder,
            "h264Encoder": renderer_capability.h264_encoder,
            "aacDecoder": renderer_capability.aac_decoder,
            "aacEncoder": renderer_capability.aac_encoder,
            "yuv420p": renderer_capability.yuv420p,
            "mp4Muxer": renderer_capability.mp4_muxer,
            "mp4Demuxer": renderer_capability.mp4_demuxer,
        }
        renderer_base = {
            key: value
            for key, value in self._renderer_identity.items()
            if key not in {"snapshotId", "capability"}
        }
        renderer_identity = {
            **renderer_base,
            "snapshotId": _payload_hash({**renderer_base, "capability": renderer_payload}),
        }

        inputs: list[ExportInputSnapshot] = []
        assets: dict[str, Any] = {}
        for source in (*plan.clips, *plan.cues):
            version_id = str(source["assetVersionId"])
            asset_version = await uow.asset_versions.get(version_id)
            if (
                asset_version is None
                or asset_version.project_id != project_id
                or asset_version.revision != source["assetVersionRevision"]
                or asset_version.content_hash != source["assetVersionHash"]
            ):
                raise ValidationDomainError("export AssetVersion is stale or foreign")
            asset = await uow.assets.get(asset_version.asset_id)
            if asset is None or asset.authorization_status != "verified" or not asset.license:
                raise ValidationDomainError("export AssetVersion authorization is incomplete")
            assets[asset_version.id] = asset
            stored = asset_version.storage_object
            inputs.append(
                ExportInputSnapshot(
                    asset_version.id,
                    asset_version.revision,
                    str(asset_version.content_hash),
                    stored.object_key,
                    stored.mime_type,
                    stored.size_bytes,
                    stored.checksum,
                    stored.bucket,
                    stored.storage_provider,
                )
            )

        models: dict[tuple[str, str, str, str], dict[str, object]] = {}
        skills: dict[tuple[str, str], dict[str, object]] = {}
        calls: list[Any] = []
        for input_snapshot in inputs:
            candidate = next(
                (
                    item
                    for item in uow.video_take_candidates.values()
                    if item.asset_version_id == input_snapshot.asset_version_id
                    and item.asset_version_revision == input_snapshot.asset_version_revision
                    and item.asset_version_hash == input_snapshot.asset_version_hash
                    and item.status == "accepted"
                ),
                None,
            )
            asset = assets[input_snapshot.asset_version_id]
            if candidate is None:
                if asset.source_type == "provider_generated":
                    raise ValidationDomainError("generated export source provenance is incomplete")
                continue
            operation = uow.video_operations.get((candidate.run_id, candidate.logical_operation))
            call_id = uow.provider_call_keys.get((candidate.run_id, candidate.logical_operation))
            provider_call = None if call_id is None else uow.provider_calls.get(call_id)
            run = uow.workflow_runs.get(candidate.run_id)
            if (
                operation is None
                or provider_call is None
                or run is None
                or provider_call.status != "succeeded"
                or operation.provider_id != provider_call.provider_id
                or operation.profile_id != provider_call.profile_id
                or operation.model_id != provider_call.model_id
                or operation.capability_snapshot_id != provider_call.capability_snapshot_id
            ):
                raise ValidationDomainError("generated export source provenance is incomplete")
            model_key = (
                operation.provider_id,
                operation.profile_id,
                operation.model_id,
                operation.capability_snapshot_id,
            )
            models[model_key] = {
                "providerId": model_key[0],
                "profileId": model_key[1],
                "modelId": model_key[2],
                "capabilitySnapshotId": model_key[3],
            }
            revision_ids = run.selection_snapshot.get("skillRevisionIds")
            digests = run.selection_snapshot.get("skillDigests")
            if (
                not isinstance(revision_ids, list)
                or not isinstance(digests, list)
                or not revision_ids
                or len(revision_ids) != len(digests)
            ):
                raise ValidationDomainError("generated export Skill provenance is incomplete")
            for revision_id, digest in zip(revision_ids, digests, strict=True):
                if (
                    not isinstance(revision_id, str)
                    or not revision_id
                    or not isinstance(digest, str)
                    or len(digest) != 64
                ):
                    raise ValidationDomainError("generated export Skill provenance is incomplete")
                name, separator, version_value = revision_id.rpartition("@")
                skill = next(
                    (
                        item
                        for item in uow.skills
                        if item.name == name and item.version == version_value
                    ),
                    None,
                )
                if (
                    not separator
                    or skill is None
                    or skill.digest != digest
                    or skill.approval != "approved"
                    or not skill.enabled
                ):
                    raise ValidationDomainError(
                        "generated export Skill provenance is unapproved or stale"
                    )
                skills[(revision_id, digest)] = {
                    "id": revision_id,
                    "revision": 1,
                    "digest": digest,
                }
            calls.append(provider_call)

        if not models or not skills:
            raise ValidationDomainError("export generation provenance is required")

        if calls and any(call.native_usage is None for call in calls):
            usage = {
                "value": 0,
                "unit": "provider-native-units",
                "status": "unknown",
                "source": "provider-call-native-usage-unavailable",
            }
        else:
            usage_value = sum(
                float(value)
                for call in calls
                for value in (call.native_usage or {}).values()
                if isinstance(value, (int, float)) and not isinstance(value, bool)
            )
            usage = {
                "value": usage_value,
                "unit": "provider-native-units",
                "status": "measured" if calls else "unknown",
                "source": "provider-call-native-usage" if calls else "no-generated-source",
            }
        if calls and all(call.cost_status == "known" and call.cost_value for call in calls):
            cost_value: object = sum(float(call.cost_value) for call in calls)
            cost_status = "measured"
            cost_source = ",".join(sorted({str(call.cost_source) for call in calls}))
        else:
            cost_value = "unknown"
            cost_status = "unknown"
            cost_source = "provider-call-cost-unavailable" if calls else "no-generated-source"
        asset_ids = sorted(assets)
        audit_facts = {
            "episode": {
                "id": selection.episode_id,
                "revision": 1,
                "hash": _payload_hash(
                    {
                        "episodeId": selection.episode_id,
                        "timelineVersionId": version.id,
                    }
                ),
            },
            "authorization": {
                "status": "authorized",
                "source": "asset-owner",
                "recordId": _payload_hash(asset_ids),
            },
            "license": {
                "status": "approved",
                "source": "asset-owner",
                "recordId": _payload_hash([assets[item].license for item in asset_ids]),
            },
            "models": list(models.values()),
            "skillRevisions": list(skills.values()),
            "parameters": {
                "fps": plan.settings.fps,
                "codec": plan.settings.video_codec,
                "pixelFormat": plan.settings.pixel_format,
                "audioCodec": plan.settings.audio_codec,
                "sampleRate": plan.settings.sample_rate,
            },
            "usage": usage,
            "cost": {
                "value": cost_value,
                "currency": (calls[0].cost_currency or "CNY") if calls else "CNY",
                "status": cost_status,
                "source": cost_source,
            },
        }
        return ExportExecutionSnapshot(
            project_id,
            selection.episode_id,
            version.id,
            version.revision,
            _payload_hash(
                {
                    "id": version.id,
                    "revision": version.revision,
                    "schemaVersion": version.schema_version,
                    "snapshot": version.cut_snapshot,
                }
            ),
            plan.render_plan_hash,
            selection.output_base_name,
            storage_profile_id,
            storage_profile_revision,
            profile_payload,
            profile_hash,
            capability_payload,
            renderer_identity,
            renderer_payload,
            plan.canonical_payload(),
            tuple(inputs),
            audit_facts,
            admission_refs=dict(frozen_admission_refs),
        )

    async def diagnostic(self, target: ExportDiagnosticTarget) -> dict[str, object]:
        async with self._uow_factory() as uow:
            episode = await uow.episodes.get(target.episode_id)
            if episode is None or episode.project_id != target.project_id:
                raise ValidationDomainError("diagnostic target owner scope is invalid")
            version = None
            if target.timeline_version_id:
                version = uow.timeline_versions.get(target.timeline_version_id)
                if (
                    version is None
                    or version.project_id != target.project_id
                    or version.episode_id != target.episode_id
                ):
                    raise ValidationDomainError("diagnostic target owner scope is invalid")
            if target.target_type in {"renderer", "storage"}:
                if target.owner_id is not None or target.owner_revision is not None:
                    raise ValidationDomainError("settings diagnostic must not claim an owner fact")
            elif target.target_type == "timeline":
                cut = uow.timeline_cuts.get(target.episode_id)
                if target.owner_id is not None and (
                    cut is None
                    or cut.id != target.owner_id
                    or cut.revision != target.owner_revision
                ):
                    raise ValidationDomainError("timeline diagnostic owner is stale or foreign")
            elif target.target_type in {"clip", "caption", "sound_cue"}:
                if version is not None:
                    collection = {
                        "clip": "clips",
                        "caption": "captions",
                        "sound_cue": "soundCues",
                    }[target.target_type]
                    values = version.cut_snapshot.get(collection)
                    owner_revision = version.source_cut_revision
                else:
                    cut = uow.timeline_cuts.get(target.episode_id)
                    values = (
                        None
                        if cut is None
                        else {
                            "clip": cut.clips,
                            "caption": cut.captions,
                            "sound_cue": [asdict(item) for item in cut.cues],
                        }[target.target_type]
                    )
                    owner_revision = None if cut is None else cut.revision
                if (
                    not isinstance(values, list)
                    or not any(
                        isinstance(item, dict) and item.get("id") == target.owner_id
                        for item in values
                    )
                    or target.owner_revision != owner_revision
                ):
                    raise ValidationDomainError("diagnostic owner fact is stale or foreign")
            elif target.target_type == "asset_version":
                asset_version = await uow.asset_versions.get(str(target.owner_id or ""))
                if (
                    asset_version is None
                    or asset_version.project_id != target.project_id
                    or asset_version.revision != target.owner_revision
                ):
                    raise ValidationDomainError("asset diagnostic owner is stale or foreign")
            elif target.target_type == "artifact":
                artifact_job = next(
                    (
                        job
                        for job in uow.export_jobs.values()
                        if job.project_id == target.project_id
                        and job.episode_id == target.episode_id
                        and job.timeline_version_id == target.timeline_version_id
                        and any(item.id == target.owner_id for item in job.artifacts)
                    ),
                    None,
                )
                if artifact_job is None or artifact_job.revision != target.owner_revision:
                    raise ValidationDomainError("artifact diagnostic owner is stale or foreign")
            return asdict(target)

    async def projection(self, project_id: str, batch_id: str) -> dict[str, object]:
        async with self._uow_factory() as uow:
            batch = uow.export_batches.get(batch_id)
            if batch is None or batch.project_id != project_id:
                raise ValidationDomainError("export batch not found")
            return export_batch_projection(cast(EpisodeExportBatch, batch))

    async def get_job(self, project_id: str, job_id: str) -> ExportJob:
        async with self._uow_factory() as uow:
            return self._job(uow, project_id, job_id)

    async def download_grant(
        self,
        project_id: str,
        episode_id: str,
        timeline_version_id: str,
        job_id: str,
        artifact_id: str,
        actor_project_id: str,
        ttl_seconds: int,
    ) -> dict[str, object]:
        if self._download_grants is None:
            raise ValidationDomainError("export download grants are unconfigured")
        async with self._uow_factory() as uow:
            job = self._job(uow, project_id, job_id)
            version = uow.timeline_versions.get(timeline_version_id)
            artifact = next((item for item in job.artifacts if item.id == artifact_id), None)
            if (
                actor_project_id != project_id
                or job.episode_id != episode_id
                or job.timeline_version_id != timeline_version_id
                or version is None
                or version.project_id != project_id
                or version.episode_id != episode_id
                or artifact is None
                or not artifact.downloadable(datetime.now(UTC))
            ):
                raise ValidationDomainError("export artifact is unavailable")
            raw_ref = artifact.storage_object_ref
            if not isinstance(raw_ref, dict):
                raise ValidationDomainError("export artifact is unavailable")
            expected_ref_fields = {
                "project_id",
                "profile_id",
                "bucket",
                "object_key",
                "size_bytes",
                "checksum",
                "mime_type",
                "etag",
                "operation_key",
                "verified",
            }
            if set(raw_ref) != expected_ref_fields:
                raise ValidationDomainError("export artifact is unavailable")
            size_bytes = raw_ref["size_bytes"]
            if isinstance(size_bytes, bool) or not isinstance(size_bytes, int):
                raise ValidationDomainError("export artifact is unavailable")
            object_ref = StoredObjectRef(
                project_id=str(raw_ref["project_id"]),
                profile_id=str(raw_ref["profile_id"]),
                bucket=str(raw_ref["bucket"]),
                object_key=str(raw_ref["object_key"]),
                size_bytes=size_bytes,
                checksum=str(raw_ref["checksum"]),
                mime_type=str(raw_ref["mime_type"]),
                etag=str(raw_ref["etag"]) if raw_ref["etag"] is not None else None,
                operation_key=str(raw_ref["operation_key"]),
                verified=raw_ref["verified"] is True,
            )
            grant = self._download_grants.issue_read_grant(
                artifact.id, object_ref, actor_project_id, ttl_seconds
            )
            return {
                "schemaVersion": "1.0.0",
                "artifactId": grant.artifact_id,
                "expiresAt": grant.expires_at,
                "action": grant.action,
                "accessPath": f"/v1/asset-media-grants/{grant.token}",
            }

    async def record_job_failure(
        self,
        project_id: str,
        job_id: str,
        raw_diagnostic: str,
        target: ExportDiagnosticTarget,
    ) -> ExportJob:
        async with self._uow_factory() as uow:
            job = self._job(uow, project_id, job_id)
            if target.project_id != project_id or target.episode_id != job.episode_id:
                raise ValidationDomainError("export failure diagnostic owner scope is invalid")
            job.renderer_diagnostic = raw_diagnostic
            job.diagnostics.append(target)
            if job.status in {"preflighting", "rendering", "packaging"}:
                job.transition("failed")
            else:
                raise ValidationDomainError("export job cannot fail from its current state")
            self._batch_for_job(uow, job).summarize()
            await uow.commit()
            return job

    def _selection(self, item: dict[str, object]) -> EpisodeExportSelection:
        if set(item) != {
            "episodeId",
            "timelineVersionId",
            "timelineVersionRevision",
            "outputBaseName",
        }:
            raise ValidationDomainError("export selection fields are incomplete or aliased")
        revision = item["timelineVersionRevision"]
        if isinstance(revision, bool) or not isinstance(revision, int):
            raise ValidationDomainError("timelineVersionRevision must be an integer")
        return EpisodeExportSelection(
            str(item["episodeId"]),
            str(item["timelineVersionId"]),
            revision,
            str(item["outputBaseName"]),
        )

    def _job(self, uow: Any, project_id: str, job_id: str) -> ExportJob:
        job = uow.export_jobs.get(job_id)
        if job is None or job.project_id != project_id:
            raise ValidationDomainError("export job not found")
        return cast(ExportJob, job)

    def _batch_for_job(self, uow: Any, job: ExportJob) -> EpisodeExportBatch:
        batch = uow.export_batches.get(job.batch_id)
        if batch is None:
            raise ValidationDomainError("export batch owner is missing")
        return cast(EpisodeExportBatch, batch)


def _settings(value: dict[str, object] | None) -> ExportSettings:
    if value is None:
        return ExportSettings()
    expected = {
        "aspectRatio",
        "width",
        "height",
        "fps",
        "container",
        "videoCodec",
        "pixelFormat",
        "audioCodec",
        "sampleRate",
        "subtitleEncoding",
    }
    if set(value) != expected:
        raise ValidationDomainError("export settings are incomplete or aliased")
    return ExportSettings(
        aspect_ratio=cast(Any, value["aspectRatio"]),
        width=cast(int, value["width"]),
        height=cast(int, value["height"]),
        fps=cast(int, value["fps"]),
        container=cast(Any, value["container"]),
        video_codec=cast(Any, value["videoCodec"]),
        pixel_format=cast(Any, value["pixelFormat"]),
        audio_codec=cast(Any, value["audioCodec"]),
        sample_rate=cast(int, value["sampleRate"]),
        subtitle_encoding=cast(Any, value["subtitleEncoding"]),
    )


def _payload_hash(value: object) -> str:
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":"), default=str).encode()
    ).hexdigest()


def _preflight_snapshot(snapshot: dict[str, object]) -> None:
    if snapshot.get("schema_version") != "1.0.0":
        raise ValidationDomainError("TimelineVersion schema_version is unsupported")
    clips = snapshot.get("clips")
    if not isinstance(clips, list) or not clips:
        raise ValidationDomainError("TimelineVersion preflight requires at least one Clip")
    for clip in clips:
        if not isinstance(clip, dict) or clip.get("derivativeStatus", "ready") != "ready":
            raise ValidationDomainError("TimelineVersion contains an unready Clip")


def export_batch_projection(batch: EpisodeExportBatch) -> dict[str, object]:
    """Return the public batch contract without storage-internal object references."""
    jobs_by_episode: dict[str, ExportJob] = {}
    for job in batch.jobs:
        jobs_by_episode[job.episode_id] = job
    return {
        "id": batch.id,
        "schemaVersion": batch.schema_version,
        "revision": batch.revision,
        "projectId": batch.project_id,
        "exportProfile": batch.export_profile,
        "settings": {
            "aspectRatio": batch.settings.aspect_ratio,
            "width": batch.settings.width,
            "height": batch.settings.height,
            "fps": batch.settings.fps,
            "container": batch.settings.container,
            "videoCodec": batch.settings.video_codec,
            "pixelFormat": batch.settings.pixel_format,
            "audioCodec": batch.settings.audio_codec,
            "sampleRate": batch.settings.sample_rate,
            "subtitleEncoding": batch.settings.subtitle_encoding,
        },
        "status": batch.status,
        "jobs": [export_job_projection(job) for job in batch.jobs],
        "members": [
            {
                "episodeId": selection.episode_id,
                "timelineVersionId": selection.timeline_version_id,
                "timelineVersionRevision": selection.timeline_version_revision,
                "outputBaseName": selection.output_base_name,
                "exportJobId": jobs_by_episode[selection.episode_id].id,
                "status": jobs_by_episode[selection.episode_id].status,
            }
            for selection in batch.selections
        ],
    }


def export_job_projection(job: ExportJob) -> dict[str, object]:
    return {
        "id": job.id,
        "projectId": job.project_id,
        "episodeId": job.episode_id,
        "timelineVersionId": job.timeline_version_id,
        "batchId": job.batch_id,
        "revision": job.revision,
        "status": job.status,
        "packagingPhase": job.packaging_phase,
        "logicalOperation": job.logical_operation,
        "renderPlanHash": job.render_plan_hash,
        "rendererDiagnostic": job.renderer_diagnostic,
        "diagnostics": [asdict(item) for item in job.diagnostics],
        "artifacts": [
            {
                "id": artifact.id,
                "artifactType": artifact.artifact_type,
                "status": artifact.status,
                "sizeBytes": artifact.size_bytes,
                "checksum": artifact.checksum,
                "mimeType": artifact.mime_type,
                "hold": artifact.hold,
                "licenseStatus": artifact.license_status,
                "expiresAt": artifact.expires_at,
            }
            for artifact in job.artifacts
        ],
    }
