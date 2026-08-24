"""Generate deterministic local resilience evidence without live provider calls."""

from __future__ import annotations

import json
from dataclasses import asdict
from pathlib import Path

from video_agent_api.resilience import (
    CapacitySnapshot,
    RuntimeResourceSnapshot,
    admit,
    aggregate_capacity,
    check_recovery_requirements,
    cleanup_allowed,
    verify_restore,
)


def main() -> None:
    snapshot = RuntimeResourceSnapshot(
        4,
        4,
        8_000,
        16_000,
        5_000,
        10_000,
        "2026-08-23T00:00:00+00:00",
        source="local_workspace",
        config_revision=1,
        revision=1,
    )
    soft = admit(snapshot, required_bytes=4_100, operation_key="upload:soft")
    hard = admit(
        snapshot.__class__(4, 4, 8_000, 16_000, 400, 10_000, snapshot.captured_at),
        operation_key="export:hard",
    )
    passed = verify_restore("restore:fixture:1", "a" * 64, "a" * 64, "etag", "etag", 1)
    failed = verify_restore("restore:fixture:1", "a" * 64, "b" * 64, "etag", "drift", 1)
    aggregate = aggregate_capacity(
        (
            CapacitySnapshot("workspace", 5_000, 10_000, 5_000, snapshot.captured_at, "local"),
            CapacitySnapshot("worker", 2_000, 5_000, 3_000, snapshot.captured_at, "worker"),
        )
    )
    report = {
        "schemaVersion": "1.0.0",
        "profile": {"provider": "mock", "adapterIdentity": "local_workspace"},
        "admission": {
            "soft": asdict(soft),
            "hard": asdict(hard),
        },
        "capacity": {
            "observedBytes": aggregate.observed_bytes,
            "limitBytes": aggregate.limit_bytes,
        },
        "recovery": {
            "requirements": asdict(
                check_recovery_requirements(
                    required=("postgres", "manifest", "compose"), available=("postgres", "manifest")
                )
            )
        },
        "restore": {"passed": asdict(passed), "failed": asdict(failed)},
        "noGc": {
            "referencedAssetVersionPreserved": not cleanup_allowed(
                referenced=True, hold=False, retention_expired=True
            ),
            "unreferencedTemporaryAllowed": cleanup_allowed(
                referenced=False, hold=False, retention_expired=True
            ),
        },
        "liveProvider": "unconfigured",
        "liveTos": "unconfigured",
        "liveRenderer": "unconfigured",
    }
    target = Path("/tmp/phase-one-resilience-evidence.json")
    target.write_text(json.dumps(report, ensure_ascii=True, indent=2, default=str) + "\n")
    print(target)


if __name__ == "__main__":
    main()
