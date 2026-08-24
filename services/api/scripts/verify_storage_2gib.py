"""Generate and verify the phase-one actual-byte Local storage fixture.

The fixture is a valid 1x1 PNG whose exact 2 GiB size comes from legal ancillary chunks.
Media bytes live only in a temporary directory and are removed after the JSON evidence is written.
"""

from __future__ import annotations

import asyncio
import hashlib
import json
import shutil
import struct
import tempfile
import zlib
from dataclasses import asdict
from pathlib import Path
from typing import cast

from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.application.assets import (
    AssetsService,
    CreateAssetCommand,
    CreateReservationCommand,
)
from video_agent_api.application.ports import AssetsUnitOfWorkFactory, UnitOfWorkFactory
from video_agent_api.application.projects_episodes import ProjectsEpisodesService
from video_agent_api.application.storage_handoffs import (
    AssetUploadCoordinator,
    asset_upload_intent,
)
from video_agent_api.ports.contracts import PartReceipt
from video_agent_api.ports.storage import LocalWorkspaceAdapter
from video_agent_api.providers.media_inspect import LocalMediaInspector

FIXTURE_SIZE = 2_147_483_648
PART_SIZE = 64 * 1024 * 1024
REPORT_PATH = Path("/tmp/phase-one-storage-2gib-evidence.json")
ZERO_BLOCK = b"\0" * (1024 * 1024)


def _png_chunk(kind: bytes, payload: bytes) -> bytes:
    checksum = zlib.crc32(kind)
    checksum = zlib.crc32(payload, checksum)
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", checksum)


def _write_valid_png(path: Path) -> str:
    signature = b"\x89PNG\r\n\x1a\n"
    ihdr = _png_chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0))
    idat = _png_chunk(b"IDAT", zlib.compress(b"\0\0\0\0\0"))
    iend = _png_chunk(b"IEND", b"")
    fixed_size = len(signature) + len(ihdr) + len(idat) + len(iend)
    filler_total = FIXTURE_SIZE - fixed_size - 3 * 12
    base, remainder = divmod(filler_total, 3)
    filler_sizes = [base + (1 if index < remainder else 0) for index in range(3)]
    if max(filler_sizes) > 2_147_483_647:
        raise RuntimeError("PNG ancillary chunk exceeds the specification limit")

    digest = hashlib.sha256()
    with path.open("wb") as output:

        def write(value: bytes) -> None:
            output.write(value)
            digest.update(value)

        write(signature)
        write(ihdr)
        write(idat)
        for payload_size in filler_sizes:
            kind = b"vpAg"
            write(struct.pack(">I", payload_size))
            write(kind)
            crc = zlib.crc32(kind)
            remaining = payload_size
            while remaining:
                block = ZERO_BLOCK[: min(len(ZERO_BLOCK), remaining)]
                output.write(block)
                digest.update(block)
                crc = zlib.crc32(block, crc)
                remaining -= len(block)
            write(struct.pack(">I", crc))
        write(iend)
    if path.stat().st_size != FIXTURE_SIZE:
        raise RuntimeError("fixture size mismatch")
    return digest.hexdigest()


async def verify() -> dict[str, object]:
    temporary_root = Path(tempfile.mkdtemp(prefix="video-agent-storage-2gib-"))
    source = temporary_root / "fixture.png"
    workspace = temporary_root / "workspace"
    source_checksum = _write_valid_png(source)

    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(cast(UnitOfWorkFactory, lambda: uow))
    project = await projects.create_project("2 GiB evidence")
    assets = AssetsService(cast(AssetsUnitOfWorkFactory, lambda: uow))
    asset = await assets.create_asset(CreateAssetCommand(project.id, "2 GiB PNG", "image"))
    reservation = await assets.create_reservation(
        CreateReservationCommand(
            project.id,
            asset.id,
            source_checksum,
            asset.revision,
            "image",
            "image/png",
            FIXTURE_SIZE,
            source_checksum,
            "local-2gib",
            1,
            source_checksum,
        )
    )
    intent = asset_upload_intent(
        reservation,
        "local-2gib",
        f"projects/{project.id}/assets/{asset.id}/{reservation.id}/original.png",
        expected_size_bytes=FIXTURE_SIZE,
        expected_checksum=source_checksum,
        expected_mime_type="image/png",
    )
    storage = LocalWorkspaceAdapter(workspace)
    storage.admit_upload(FIXTURE_SIZE, PART_SIZE, profile_revision=1)
    session = storage.create_multipart(intent, "phase-one-2gib-create")
    manifest: list[PartReceipt] = []
    restarted = False
    with source.open("rb") as fixture:
        part_number = 1
        while content := fixture.read(PART_SIZE):
            checksum = hashlib.sha256(content).hexdigest()
            receipt = PartReceipt(part_number, checksum, checksum, len(content))
            storage.upload_part(session, receipt, content, "phase-one-2gib-part")
            manifest.append(receipt)
            if part_number == 1:
                storage = LocalWorkspaceAdapter(workspace)
                session = storage.resume_multipart(intent, "phase-one-2gib-resume")
                restarted = True
            part_number += 1

    coordinator = AssetUploadCoordinator(storage, assets)
    version = await coordinator.complete_and_register(
        reservation.id, session, tuple(manifest), "phase-one-2gib-complete"
    )
    duplicate = await coordinator.complete_and_register(
        reservation.id, session, tuple(manifest), "phase-one-2gib-retry"
    )
    observed = storage.stat(f"workspace://{intent.object_key}")
    inspector = LocalMediaInspector("python-png-stream-1", "1.0.0", workspace)
    metadata = inspector.inspect(observed, "phase-one-2gib-inspect")
    derivatives = inspector.derive(observed, metadata, "phase-one-2gib-derive")

    report: dict[str, object] = {
        "status": "passed",
        "adapterIdentity": "local_workspace",
        "profileRevision": 1,
        "actualBytes": observed.size_bytes,
        "expectedBytes": FIXTURE_SIZE,
        "mimeType": "image/png",
        "checksum": observed.checksum,
        "sourceChecksum": source_checksum,
        "etag": observed.etag,
        "partSizeBytes": PART_SIZE,
        "partCount": len(manifest),
        "partManifest": [asdict(item) for item in manifest],
        "interruptedAfterPart": 1,
        "resumedAfterAdapterRestart": restarted,
        "operationKey": reservation.operation_key,
        "sessionId": session.session_id,
        "assetVersionId": version.id,
        "duplicateRegistrationReused": duplicate.id == version.id,
        "assetVersionCount": len(uow.state.asset_versions),
        "inspection": metadata,
        "derivatives": list(derivatives),
        "temporaryMediaRetained": False,
        "liveTosStatus": "unconfigured",
    }
    if not (
        observed.size_bytes == FIXTURE_SIZE
        and observed.checksum == source_checksum
        and version.id == duplicate.id
        and metadata.get("status") == "inspected"
        and any(
            item.get("kind") == "proxy" and item.get("status") == "succeeded"
            for item in derivatives
        )
    ):
        raise RuntimeError("2 GiB storage evidence did not satisfy the exit contract")
    REPORT_PATH.write_text(json.dumps(report, indent=2, sort_keys=True), encoding="utf-8")
    shutil.rmtree(temporary_root)
    return report


if __name__ == "__main__":
    print(json.dumps(asyncio.run(verify()), indent=2, sort_keys=True))
