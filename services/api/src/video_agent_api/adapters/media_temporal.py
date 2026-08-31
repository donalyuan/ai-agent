"""Temporal contracts for Media inspection/derivative/render/storage activities.

Activities are intentionally thin: owner services validate immutable handoffs and adapters
perform bounded I/O.  No activity writes AssetVersion, Timeline or ExportArtifact directly.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import timedelta
from hashlib import sha256
from pathlib import Path
from typing import Any, cast

from temporalio import activity, workflow
from temporalio.exceptions import WorkflowAlreadyStartedError

from video_agent_api.application.assets import CompleteReservationCommand
from video_agent_api.application.media import (
    MEDIA_ROUTE,
    MEDIA_SCHEMA_VERSION,
    MEDIA_TASK_QUEUE,
    MEDIA_WORKFLOW_TYPE,
    MediaOwnerService,
    RecordDerivativeCommand,
    RecordInspectionCommand,
)
from video_agent_api.domain.assets import StorageObject
from video_agent_api.domain.errors import RevisionConflictError, ValidationDomainError
from video_agent_api.ports.contracts import StoredObject, StoredObjectRef


def _file_checksum(path: Path) -> str:
    digest = sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


class MediaActivityDependencies:
    def __init__(
        self,
        owner: MediaOwnerService,
        storage: Any,
        inspector: Any,
        renderer: Any = None,
        *,
        assets: Any = None,
        exports: Any = None,
    ) -> None:
        self.owner = owner
        self.storage = storage
        self.inspector = inspector
        self.renderer = renderer
        self.assets = assets
        self.exports = exports


_dependencies: MediaActivityDependencies | None = None


@dataclass(frozen=True, slots=True)
class TemporalMediaLaunch:
    workflow_id: str
    payload: dict[str, object]


class TemporalMediaStarter:
    def __init__(self, client: Any, task_queue: str = "media-tasks") -> None:
        self._client = client
        self._task_queue = task_queue

    async def start(self, payload: dict[str, object]) -> str:
        workflow_id = str(payload.get("workflowId", ""))
        if not workflow_id:
            raise ValueError("media workflow identity is required")
        if (
            payload.get("executionRoute") != MEDIA_ROUTE
            or payload.get("workflowType") != MEDIA_WORKFLOW_TYPE
            or payload.get("taskQueue") != self._task_queue
        ):
            raise ValidationDomainError("media launch route is stale")
        if payload.get("schemaVersion") != MEDIA_SCHEMA_VERSION:
            raise ValidationDomainError("media launch schemaVersion is unsupported")
        try:
            await self._client.start_workflow(
                "media_operation",
                payload,
                id=workflow_id,
                task_queue=self._task_queue,
            )
        except WorkflowAlreadyStartedError:
            return "already_started"
        return "started"


def configure_media_activities(dependencies: MediaActivityDependencies) -> None:
    global _dependencies
    _dependencies = dependencies


def _require_dependencies() -> MediaActivityDependencies:
    if _dependencies is None:
        raise RuntimeError("media activities are unconfigured")
    return _dependencies


def _ref(payload: dict[str, object]) -> StoredObjectRef:
    return StoredObjectRef(
        project_id=str(payload["projectId"]),
        profile_id=str(payload["profileId"]),
        bucket=str(payload["bucket"]),
        object_key=str(payload["objectKey"]),
        size_bytes=int(cast(Any, payload["sizeBytes"])),
        checksum=str(payload["checksum"]),
        mime_type=str(payload["mimeType"]),
        etag=str(payload["etag"]) if payload.get("etag") is not None else None,
        operation_key=str(payload["operationKey"]),
        verified=bool(payload.get("verified", True)),
    )


def _validate_storage_identity(
    storage: Any, ref: StoredObjectRef, payload: dict[str, object]
) -> None:
    """Require the running adapter to match the frozen object identity before I/O."""
    expected = {
        "profileId": ref.profile_id,
        "bucket": ref.bucket,
    }
    for attr, key in (("profile_id", "profileId"), ("bucket", "bucket")):
        actual = getattr(storage, attr, None)
        # ``local`` was the pre-profile alias used by old in-memory fixtures;
        # normalize it only for the explicit local adapter identity.
        if key == "profileId" and actual == "local-test-offline" and expected[key] == "local":
            continue
        if actual is None or actual != expected[key]:
            raise ValidationDomainError("media storage identity is stale or incomplete")
    identity = payload.get("storageIdentity")
    if identity is not None:
        if not isinstance(identity, dict):
            raise ValidationDomainError("media storage identity is invalid")
        required = {
            "profileId",
            "profileRevision",
            "bucketBindingId",
            "bucket",
            "endpoint",
            "region",
        }
        if set(identity) != required:
            raise ValidationDomainError("media storage identity is incomplete")
        for key, attr in (
            ("profileId", "profile_id"),
            ("profileRevision", "profile_revision"),
            ("bucketBindingId", "bucket_binding_id"),
            ("bucket", "bucket"),
            ("endpoint", "endpoint"),
            ("region", "region"),
        ):
            if getattr(storage, attr, None) != identity[key]:
                raise ValidationDomainError("media storage identity is stale")


def _derivative_output(
    storage: Any,
    source: StoredObjectRef,
    output: dict[str, object],
    operation_key: str,
) -> tuple[str, dict[str, object] | None, str | None, int | None, str | None]:
    """Verify a local derivative object before the owner may expose it as ready."""
    requested_status = str(output.get("status", "pending"))
    if requested_status not in {"ready", "succeeded"}:
        return (
            "failed" if requested_status == "failed" else "pending",
            None,
            None,
            None,
            str(output.get("diagnostic")) if output.get("diagnostic") else None,
        )
    raw_ref = output.get("objectRef")
    if not isinstance(raw_ref, str) or not raw_ref.startswith("workspace://"):
        return "failed", None, None, None, "derivative output reference is invalid"
    try:
        observed = storage.stat(raw_ref)
    except Exception as error:
        return (
            "failed",
            None,
            None,
            None,
            f"derivative output verification failed: {type(error).__name__}",
        )
    expected_checksum = output.get("checksum")
    expected_size = output.get("sizeBytes")
    if (
        not isinstance(expected_checksum, str)
        or expected_checksum != observed.checksum
        or isinstance(expected_size, bool)
        or not isinstance(expected_size, int)
        or expected_size != observed.size_bytes
    ):
        return "failed", None, None, None, "derivative output claimed-vs-observed mismatch"
    object_key = raw_ref.removeprefix("workspace://")
    return (
        "ready",
        {
            "profileId": source.profile_id,
            "objectKey": object_key,
            "operationKey": operation_key,
        },
        observed.checksum,
        observed.size_bytes,
        None,
    )


@activity.defn(name="media_inspect")
async def media_inspect(payload: dict[str, object]) -> dict[str, object]:
    deps = _require_dependencies()
    if deps.storage is None:
        raise ValidationDomainError("storage_unconfigured")
    project_id = str(payload["projectId"])
    operation_key = str(payload["operationKey"])
    activity_admission = deps.owner.admit_activity(
        project_id,
        "media.inspect",
        operation_key,
        cast(dict[str, object], payload["resourceAdmission"])
        if isinstance(payload.get("resourceAdmission"), dict)
        else None,
    )
    ref = _ref(cast(dict[str, object], payload["storedObjectRef"]))
    _validate_storage_identity(deps.storage, ref, payload)
    observed = deps.storage.stat(f"workspace://{ref.object_key}")
    metadata = deps.inspector.inspect(
        StoredObject(
            f"workspace://{ref.object_key}",
            observed.size_bytes,
            observed.checksum,
            observed.etag,
        ),
        str(payload.get("correlationId", "media-inspect")),
    )
    claimed_checksum = metadata.get("checksum", metadata.get("sourceChecksum"))
    if claimed_checksum is not None and claimed_checksum != observed.checksum:
        raise ValueError("ffprobe claimed-vs-observed checksum mismatch")
    # Canonical owner metadata is deliberately explicit even for an adapter that only knows
    # a subset; unknown technical fields remain zero/empty and cannot be treated as verified.
    canonical = {
        "mimeType": str(metadata.get("mimeType", metadata.get("mediaType", ref.mime_type))),
        "sizeBytes": observed.size_bytes,
        "checksum": observed.checksum,
        "durationFrames": int(metadata.get("durationFrames", 0)),
        "timebase": str(metadata.get("timebase", "1/30")),
        "fpsNumerator": int(metadata.get("fpsNumerator", 30)),
        "fpsDenominator": int(metadata.get("fpsDenominator", 1)),
        "frameCount": int(metadata.get("frameCount", 0)),
        "width": int(metadata.get("width", 1)),
        "height": int(metadata.get("height", 1)),
        "videoCodec": str(metadata.get("videoCodec", "unknown")),
        "pixelFormat": str(metadata.get("pixelFormat", "unknown")),
        "audioTracks": int(metadata.get("audioTracks", 0)),
        "sampleRate": int(metadata.get("sampleRate", 0)),
        "channels": int(metadata.get("channels", 0)),
    }
    adapter_status = str(metadata.get("status", "pending"))
    status = "ready" if adapter_status in {"inspected", "ready", "succeeded"} else adapter_status
    if status not in {"pending", "ready", "failed", "stale"}:
        status = "failed"
    inspection = await deps.owner.record_inspection(
        RecordInspectionCommand(
            project_id=project_id,
            asset_version_id=str(payload["assetVersionId"]),
            asset_version_revision=int(cast(Any, payload["assetVersionRevision"])),
            asset_version_hash=str(payload["assetVersionHash"]),
            operation_key=str(payload["operationKey"]),
            metadata=canonical,
            tool="ffprobe",
            tool_version=str(metadata.get("toolVersion", "unknown")),
            status=status,
            raw_diagnostic=(str(metadata["diagnostic"]) if metadata.get("diagnostic") else None),
            admission_refs=activity_admission,
        )
    )
    return {"inspectionId": inspection.id, "status": inspection.status, "metadata": canonical}


@activity.defn(name="media_derivative")
async def media_derivative(payload: dict[str, object]) -> dict[str, object]:
    deps = _require_dependencies()
    if deps.storage is None:
        raise ValidationDomainError("storage_unconfigured")
    inspection_id = str(payload["inspectionId"])
    async with deps.owner._uow_factory() as uow:  # owner service remains the sole writer
        inspection = uow.media_inspections.get(inspection_id)
    if inspection is None:
        raise ValueError("media derivative inspection is unknown")
    project_id = inspection.project_id
    root_operation_key = str(payload["operationKey"])
    dispatch_refs = (
        cast(dict[str, object], payload["resourceAdmission"])
        if isinstance(payload.get("resourceAdmission"), dict)
        else None
    )
    # Revalidate before reading source bytes or invoking the inspector.
    deps.owner.admit_activity(project_id, "media.derivative", root_operation_key, dispatch_refs)
    ref = _ref(cast(dict[str, object], payload["storedObjectRef"]))
    _validate_storage_identity(deps.storage, ref, payload)
    observed = deps.storage.stat(f"workspace://{ref.object_key}")
    outputs = deps.inspector.derive(
        StoredObject(
            f"workspace://{ref.object_key}",
            observed.size_bytes,
            observed.checksum,
            observed.etag,
        ),
        inspection.metadata,
        str(payload.get("correlationId", "media-derivative")),
    )
    recorded: list[str] = []
    for output in outputs:
        kind = str(output.get("kind", ""))
        if kind not in {"proxy", "thumbnail", "keyframe_index", "waveform"}:
            continue
        operation_key = f"{payload['operationKey']}:{kind}"
        derivative_admission = deps.owner.admit_activity(
            project_id, "media.derivative", operation_key, dispatch_refs
        )
        status, object_ref, checksum, size_bytes, diagnostic = _derivative_output(
            deps.storage, ref, output, operation_key
        )
        derivative = await deps.owner.record_derivative(
            RecordDerivativeCommand(
                project_id=project_id,
                inspection_id=inspection.id,
                kind=kind,
                status=status,
                parameters=(
                    dict(output.get("metadata", {}))
                    if isinstance(output.get("metadata"), dict)
                    else {}
                ),
                operation_key=operation_key,
                tool="ffmpeg",
                tool_version=str(output.get("toolVersion", "unknown")),
                derivative_schema_version=str(output.get("derivativeSchemaVersion", "1.0.0")),
                object_ref=(object_ref),
                checksum=checksum,
                size_bytes=size_bytes,
                raw_diagnostic=diagnostic,
                admission_refs=derivative_admission,
                retention_policy=str(output.get("retentionPolicy", "phase-one")),
                retention_version=str(output.get("retentionVersion", "1")),
                license_status=str(output.get("licenseStatus", "approved")),
                hold=bool(output.get("hold", False)),
            )
        )
        recorded.append(derivative.id)
    return {"derivativeIds": recorded}


@activity.defn(name="media_render")
async def media_render(payload: dict[str, object]) -> dict[str, object]:
    deps = _require_dependencies()
    if deps.renderer is None:
        return {
            "status": "renderer_unconfigured",
            "diagnostic": "renderer activity handoff is unconfigured",
        }
    request = payload.get("renderRequest")
    workspace = payload.get("workspace")
    if not isinstance(request, dict) or not isinstance(workspace, str) or not workspace:
        raise ValueError("media render requires a typed renderRequest and workspace")
    project_id = payload.get("projectId")
    operation_key = payload.get("operationKey")
    if isinstance(project_id, str) and isinstance(operation_key, str):
        deps.owner.admit_activity(
            project_id,
            "media.render",
            f"{operation_key}:render",
            cast(dict[str, object], payload["resourceAdmission"])
            if isinstance(payload.get("resourceAdmission"), dict)
            else None,
        )
    expected = payload.get("expectedOutput")
    if not isinstance(expected, dict):
        return {"status": "failed", "diagnostic": "render_output_expectation_required"}
    required = {"durationSeconds", "container", "videoCodec", "audioCodec"}
    if set(expected).intersection(required) != required:
        return {"status": "failed", "diagnostic": "render_output_expectation_incomplete"}
    try:
        from video_agent_api.ports.rendering import RenderRequest

        raw_inputs = request.get("inputPaths")
        output = request.get("outputPath")
        if not isinstance(raw_inputs, list) or not all(
            isinstance(item, str) for item in raw_inputs
        ):
            raise TypeError("inputPaths")
        if not isinstance(output, str):
            raise TypeError("outputPath")
        result = deps.renderer.render(
            RenderRequest(
                tuple(Path(item) for item in raw_inputs),
                Path(output),
                str(request.get("filterGraph", "")),
                int(request.get("width", 0)),
                int(request.get("height", 0)),
                int(request.get("fps", 30)),
                str(request.get("videoMap", "[vout]")),
                str(request["audioMap"]) if request.get("audioMap") is not None else None,
            ),
            Path(workspace),
        )
        inspection = deps.renderer.inspect_output(result.output_path, Path(workspace))
    except Exception as error:
        if type(error).__name__ in {
            "RendererUnconfiguredError",
            "RendererCapabilityUnsupportedError",
        }:
            return {"status": "renderer_unconfigured", "diagnostic": str(error)}
        return {"status": "failed", "diagnostic": f"media render failed: {type(error).__name__}"}
    output_path = result.output_path
    workspace_path = Path(workspace).resolve()
    if not output_path.resolve().is_relative_to(workspace_path):
        return {"status": "failed", "diagnostic": "render_output_invalid:path"}
    if not output_path.is_file() or output_path.stat().st_size <= 0:
        return {"status": "failed", "diagnostic": "render_output_invalid:size"}
    observed_size = output_path.stat().st_size
    observed_hash = _file_checksum(output_path)
    if expected.get("container") != inspection.container:
        return {"status": "failed", "diagnostic": "render_output_invalid:container"}
    if expected.get("videoCodec") != inspection.video_codec:
        return {"status": "failed", "diagnostic": "render_output_invalid:video_codec"}
    if expected.get("audioCodec") != inspection.audio_codec:
        return {"status": "failed", "diagnostic": "render_output_invalid:audio_codec"}
    if float(expected["durationSeconds"]) != inspection.duration_seconds:
        return {"status": "failed", "diagnostic": "render_output_invalid:duration"}
    if expected.get("sizeBytes") is not None and int(expected["sizeBytes"]) != observed_size:
        return {"status": "failed", "diagnostic": "render_output_invalid:size"}
    if expected.get("checksum") and expected.get("checksum") != observed_hash:
        return {"status": "failed", "diagnostic": "render_output_invalid:hash"}
    return {
        "status": "succeeded",
        "outputPath": str(result.output_path),
        "mimeType": "video/mp4",
        "sizeBytes": observed_size,
        "checksum": observed_hash,
        "returnCode": result.return_code,
        "diagnostic": result.stderr[-2000:],
    }


@activity.defn(name="media_storage_upload")
async def media_storage_upload(payload: dict[str, object]) -> dict[str, object]:
    deps = _require_dependencies()
    if deps.storage is None:
        return {
            "status": "storage_unconfigured",
            "diagnostic": "storage activity handoff is unconfigured",
        }
    project_id = payload.get("projectId")
    operation_key = payload.get("operationKey")
    if isinstance(project_id, str) and isinstance(operation_key, str):
        # Storage reads/writes start only after the original dispatch is still
        # admitted and this activity has its own operation-specific identity.
        deps.owner.admit_activity(
            project_id,
            "media.storage",
            f"{operation_key}:storage",
            cast(dict[str, object], payload["resourceAdmission"])
            if isinstance(payload.get("resourceAdmission"), dict)
            else None,
        )
    raw_ref = payload.get("storedObjectRef")
    output_path = payload.get("outputPath")
    if isinstance(output_path, str) and output_path:
        # Consume renderer output through the bounded multipart port, then verify
        # the returned immutable reference before any Export handoff.
        try:
            from pathlib import Path

            from video_agent_api.ports.contracts import PartReceipt, StorageWriteIntent

            path = Path(output_path).resolve()
            if not path.is_file():
                raise ValueError("render output is missing")
            data_size = path.stat().st_size
            digest_state = sha256()
            with path.open("rb") as source:
                while chunk := source.read(8 * 1024 * 1024):
                    digest_state.update(chunk)
            digest = digest_state.hexdigest()
            project_id = str(payload["projectId"])
            profile_id = str(payload.get("profileId", "local"))
            identity_ref = (
                _ref(cast(dict[str, object], raw_ref)) if isinstance(raw_ref, dict) else None
            )
            if identity_ref is not None:
                _validate_storage_identity(deps.storage, identity_ref, payload)
            default_key = f"exports/{payload.get('operationKey', 'render')}.mp4"
            object_key = str(payload.get("objectKey", default_key))
            operation_key = str(payload.get("operationKey", "")) + ":storage"
            mime_type = str(payload.get("mimeType", "video/mp4"))
            intent = StorageWriteIntent(
                operation_key,
                project_id,
                profile_id,
                object_key,
                data_size,
                digest,
                mime_type,
            )
            correlation_id = str(payload.get("correlationId", operation_key))
            session = deps.storage.create_multipart(intent, correlation_id)
            chunk_size = 8 * 1024 * 1024
            receipts = []
            with path.open("rb") as source:
                part_number = 1
                while chunk := source.read(chunk_size):
                    checksum = sha256(chunk).hexdigest()
                    receipt = PartReceipt(part_number, checksum, checksum, len(chunk))
                    deps.storage.upload_part(session, receipt, chunk, correlation_id)
                    receipts.append(receipt)
                    part_number += 1
            verified = deps.storage.complete_multipart(session, tuple(receipts), correlation_id)
            if verified.checksum != digest or verified.size_bytes != data_size:
                return {"status": "failed", "diagnostic": "storage_output_invalid:hash_or_size"}
            return {
                "status": "verified",
                "objectRef": {
                    "projectId": verified.project_id,
                    "profileId": verified.profile_id,
                    "bucket": verified.bucket,
                    "objectKey": verified.object_key,
                    "sizeBytes": verified.size_bytes,
                    "checksum": verified.checksum,
                    "mimeType": verified.mime_type,
                    "etag": verified.etag,
                    "operationKey": verified.operation_key,
                    "verified": True,
                },
            }
        except Exception as error:
            return {
                "status": "failed",
                "diagnostic": f"storage multipart failed: {type(error).__name__}",
            }
    if not isinstance(raw_ref, dict):
        raise ValueError("media storage upload requires a typed storedObjectRef")
    try:
        ref = _ref(raw_ref)
        observed = deps.storage.stat(f"workspace://{ref.object_key}")
    except Exception as error:
        return {
            "status": "failed",
            "diagnostic": f"storage verification failed: {type(error).__name__}",
        }
    if observed.checksum != ref.checksum or observed.size_bytes != ref.size_bytes:
        return {"status": "failed", "diagnostic": "storage claimed-vs-observed mismatch"}
    return {
        "status": "verified",
        "objectRef": {
            "projectId": ref.project_id,
            "profileId": ref.profile_id,
            "bucket": ref.bucket,
            "objectKey": ref.object_key,
            "sizeBytes": observed.size_bytes,
            "checksum": observed.checksum,
            "etag": observed.etag,
            "operationKey": ref.operation_key,
            "mimeType": ref.mime_type,
            "verified": True,
        },
    }


@activity.defn(name="media_storage_terminal_handoff")
async def media_storage_terminal_handoff(payload: dict[str, object]) -> dict[str, object]:
    """Pass a verified storage result to exactly one aggregate owner.

    Temporal activities never append AssetVersion/ExportArtifact themselves. The explicit
    target is part of the frozen operation payload so a missing or stale target fails closed.
    """
    deps = _require_dependencies()
    if payload.get("status") != "verified":
        raise ValidationDomainError("media storage terminal result is not verified")
    target = payload.get("ownerHandoff")
    raw_ref = payload.get("objectRef")
    project_id = payload.get("projectId")
    operation_key = payload.get("operationKey")
    if (
        not isinstance(target, dict)
        or not isinstance(raw_ref, dict)
        or not isinstance(project_id, str)
        or not isinstance(operation_key, str)
        or not operation_key
    ):
        raise ValidationDomainError("media storage owner handoff target is required")
    ref = _ref(raw_ref)
    if (
        not ref.verified
        or ref.project_id != project_id
        or ref.operation_key not in {operation_key, f"{operation_key}:storage"}
    ):
        raise ValidationDomainError("media storage owner handoff reference is invalid")
    owner = target.get("owner")
    target_project = target.get("projectId", project_id)
    target_operation = target.get("operationKey")
    if target_project != project_id or target_operation != operation_key:
        raise ValidationDomainError("media storage owner handoff scope or operation is stale")
    if owner == "assets":
        if deps.assets is None:
            raise ValidationDomainError("assets owner handoff is unconfigured")
        reservation_id = target.get("reservationId")
        content_hash = target.get("contentHash")
        reservation_revision = target.get("reservationRevision")
        if (
            not isinstance(reservation_id, str)
            or not isinstance(content_hash, str)
            or content_hash != ref.checksum
            or isinstance(reservation_revision, bool)
            or not isinstance(reservation_revision, int)
        ):
            raise ValidationDomainError("assets owner handoff metadata is invalid")
        reservation = await deps.assets.get_reservation(project_id, reservation_id)
        if (
            reservation.revision != reservation_revision
            or reservation.operation_key != operation_key
            or reservation.storage_profile_id != ref.profile_id
        ):
            raise ValidationDomainError("assets owner handoff reservation is stale")
        version = await deps.assets.complete_reservation(
            CompleteReservationCommand(
                reservation_id,
                StorageObject(
                    ref.profile_id,
                    ref.bucket,
                    ref.object_key,
                    ref.mime_type,
                    ref.size_bytes,
                    ref.checksum,
                    e_tag=ref.etag,
                ),
                content_hash,
            )
        )
        return {"status": "registered", "owner": "assets", "assetVersionId": version.id}
    if owner == "export":
        if deps.exports is None:
            raise ValidationDomainError("export owner handoff is unconfigured")
        job_id = target.get("jobId")
        artifact_type = target.get("artifactType")
        expected_revision = target.get("expectedRevision")
        storage_profile_revision = target.get("storageProfileRevision")
        if (
            not isinstance(job_id, str)
            or artifact_type not in {"mp4", "srt", "light_manifest"}
            or isinstance(expected_revision, bool)
            or not isinstance(expected_revision, int)
            or isinstance(storage_profile_revision, bool)
            or not isinstance(storage_profile_revision, int)
            or target.get("packagingPhase") != "registering"
        ):
            raise ValidationDomainError("export owner handoff metadata is invalid")
        try:
            artifact = await deps.exports.register_artifact(
                project_id=project_id,
                job_id=job_id,
                artifact_type=artifact_type,
                size_bytes=ref.size_bytes,
                checksum=ref.checksum,
                verified=True,
                expected_revision=expected_revision,
                stored_object=ref,
                storage_profile_revision=storage_profile_revision,
            )
        except RevisionConflictError:
            # A response lost after a successful owner commit is reconciled by reading the
            # owner fact.  A stale revision never authorizes accepting a different artifact.
            get_job = getattr(deps.exports, "get_job", None)
            if get_job is None:
                raise
            job = await get_job(project_id, job_id)
            artifact = next(
                (item for item in job.artifacts if item.artifact_type == artifact_type), None
            )
            if (
                artifact is None
                or artifact.status != "verified"
                or artifact.size_bytes != ref.size_bytes
                or artifact.checksum != ref.checksum
                or artifact.operation_key != operation_key
                or artifact.mime_type != ref.mime_type
                or artifact.storage_object_ref is None
                or artifact.storage_object_ref.get("project_id") != ref.project_id
                or artifact.storage_object_ref.get("profile_id") != ref.profile_id
                or artifact.storage_object_ref.get("bucket") != ref.bucket
                or artifact.storage_object_ref.get("object_key") != ref.object_key
                or artifact.storage_object_ref.get("size_bytes") != ref.size_bytes
                or artifact.storage_object_ref.get("checksum") != ref.checksum
                or artifact.storage_object_ref.get("mime_type") != ref.mime_type
                or artifact.storage_object_ref.get("operation_key") != ref.operation_key
                or artifact.storage_object_ref.get("verified") is not True
            ):
                raise
        return {
            "status": "registered",
            "owner": "export",
            "artifactId": artifact.id,
            "artifactStatus": artifact.status,
        }
    raise ValidationDomainError("media storage owner handoff owner is unsupported")


@workflow.defn(name="media_operation")
class MediaOperationWorkflow:
    @workflow.run
    async def run(self, payload: dict[str, object]) -> dict[str, object]:
        _validate_media_workflow_payload(payload)
        operation = cast(str, payload["operation"])
        if operation == "pipeline":
            # A pipeline is explicit in the outbox payload. Each step receives
            # the previous activity result, preserving the frozen operation key.
            technical_input = payload.get("technicalInput")
            steps = payload.get("steps")
            if steps is None and isinstance(technical_input, dict):
                steps = technical_input.get("steps")
            if steps is None:
                steps = ["inspect", "derivative", "render", "storage"]
            if not isinstance(steps, list) or not all(isinstance(step, str) for step in steps):
                raise ValueError("media pipeline steps are invalid")
            current: dict[str, object] = dict(payload)
            result: dict[str, object] = {}
            for step in steps:
                if step not in {"inspect", "derivative", "render", "storage"}:
                    raise ValueError("unknown media pipeline operation")
                activity_name = {
                    "inspect": media_inspect,
                    "derivative": media_derivative,
                    "render": media_render,
                    "storage": media_storage_upload,
                }[step]
                result = await workflow.execute_activity(
                    activity_name,
                    current,
                    activity_id=f"media:{payload.get('operationKey', 'unknown')}:{step}",
                    start_to_close_timeout=timedelta(hours=6),
                )
                current.update(result)
                activity_status = result.get("status")
                if activity_status in {
                    "failed",
                    "renderer_unconfigured",
                    "storage_unconfigured",
                    "stale",
                }:
                    # A failed activity must stop the pipeline before any later
                    # storage or owner handoff can create side effects.
                    return result
                if step == "storage":
                    result = await workflow.execute_activity(
                        media_storage_terminal_handoff,
                        current,
                        activity_id=f"media:{payload.get('operationKey', 'unknown')}:handoff",
                        start_to_close_timeout=timedelta(hours=6),
                    )
                    current.update(result)
            return result
        activity_name = cast(
            Any,
            {
                "inspect": media_inspect,
                "derivative": media_derivative,
                "render": media_render,
                "storage": media_storage_upload,
            }.get(operation),
        )
        if activity_name is None:
            raise ValueError("unknown media operation")
        result = cast(
            dict[str, object],
            await workflow.execute_activity(
                activity_name,
                payload,
                activity_id=f"media:{payload.get('operationKey', 'unknown')}:{operation}",
                start_to_close_timeout=timedelta(hours=6),
            ),
        )
        if operation == "storage":
            result = await workflow.execute_activity(
                media_storage_terminal_handoff,
                {**payload, **result},
                activity_id=f"media:{payload.get('operationKey', 'unknown')}:handoff",
                start_to_close_timeout=timedelta(hours=6),
            )
        return result


MEDIA_WORKFLOWS = (MediaOperationWorkflow,)
MEDIA_ACTIVITIES = (
    media_inspect,
    media_derivative,
    media_render,
    media_storage_upload,
    media_storage_terminal_handoff,
)


def _validate_media_workflow_payload(payload: dict[str, object]) -> None:
    """Reject incomplete owner admission before Temporal can schedule any activity."""
    if (
        payload.get("executionRoute") != MEDIA_ROUTE
        or payload.get("workflowType") != MEDIA_WORKFLOW_TYPE
        or payload.get("taskQueue") != MEDIA_TASK_QUEUE
        or payload.get("schemaVersion") != MEDIA_SCHEMA_VERSION
    ):
        raise ValueError("media workflow route is stale or unsupported")
    if not all(
        isinstance(payload.get(key), str) and payload[key] for key in ("projectId", "operationKey")
    ):
        raise ValueError("media workflow owner identity is incomplete")
    if not isinstance(payload.get("storedObjectRef"), dict) or not isinstance(
        payload.get("resourceAdmission"), dict
    ):
        raise ValueError("media workflow admission is incomplete")
