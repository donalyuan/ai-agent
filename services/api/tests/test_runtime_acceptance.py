from __future__ import annotations

import json
from hashlib import sha256
from pathlib import Path

import pytest

from video_agent_api.acceptance import (
    CredentialedSandboxHarness,
    assert_sanitized_evidence,
    canonical_evidence_json,
)
from video_agent_api.ports.contracts import AdapterNotConfiguredError


def _ready_values() -> dict[str, str]:
    return {
        "MVP_A_CREDENTIAL_SANDBOX": "enabled",
        "MVP_A_SANDBOX_PROFILE_ID": "sandbox-profile-opaque",
        "MVP_A_SANDBOX_ALLOWLIST": "https://provider.example.test",
        "MVP_A_SANDBOX_CREDENTIAL_REF": "sandbox-reference-opaque",
        "MVP_A_SANDBOX_PROVIDER_PROBE": "passed",
        "MVP_A_SANDBOX_TOS_PROBE": "passed",
        "MVP_A_SANDBOX_RENDERER_PROBE": "passed",
    }


def test_default_ci_sandbox_is_unconfigured_and_never_resolves_a_value() -> None:
    harness = CredentialedSandboxHarness({})
    assessment = harness.assess()

    assert assessment.result == "unconfigured"
    assert assessment.readiness == "not_ready"
    with pytest.raises(AdapterNotConfiguredError, match="credentialed_sandbox_unconfigured"):
        harness.invoke(
            "https://provider.example.test",
            lambda: pytest.fail("default CI must not resolve an injected value"),
            lambda _value: None,
        )


def test_sandbox_rejects_an_unallowlisted_endpoint_before_injection() -> None:
    harness = CredentialedSandboxHarness(_ready_values())

    with pytest.raises(AdapterNotConfiguredError, match="endpoint_not_allowlisted"):
        harness.invoke(
            "https://foreign.example.test",
            lambda: pytest.fail("unallowlisted endpoint must not resolve an injected value"),
            lambda _value: None,
        )


def test_sandbox_injection_is_ephemeral_and_evidence_is_sanitized() -> None:
    harness = CredentialedSandboxHarness(_ready_values())
    injected: list[str] = []

    length = harness.invoke(
        "https://provider.example.test",
        lambda: "test-only-injected-value",
        lambda value: (injected.append(value), len(value))[1],
    )

    assert length == len("test-only-injected-value")
    assert injected == ["test-only-injected-value"]
    evidence = harness.evidence()
    assert "test-only-injected-value" not in json.dumps(evidence)
    assert evidence["sandbox"] == {
        "profileId": "sandbox-profile-opaque",
        "allowlistDigest": sha256(b"provider.example.test").hexdigest(),
        "allowlistCount": 1,
        "injection": "ephemeral_only",
    }
    assert_sanitized_evidence(evidence)


def test_sanitized_evidence_rejects_sensitive_fields_and_values() -> None:
    with pytest.raises(ValueError, match="forbidden evidence field"):
        assert_sanitized_evidence({"authorization": "masked"})
    with pytest.raises(ValueError, match="forbidden evidence value"):
        assert_sanitized_evidence({"diagnostic": "Bearer actual-value"})


def test_versioned_runtime_gap_report_matches_default_unconfigured_harness() -> None:
    report_path = Path(__file__).parents[3] / "docs/evidence/E2E-MVPA-001-runtime-gaps.json"

    assert json.loads(report_path.read_text(encoding="utf-8")) == json.loads(
        canonical_evidence_json({})
    )
