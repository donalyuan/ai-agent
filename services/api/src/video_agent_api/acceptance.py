"""默认零网络的 credentialed sandbox 准入与脱敏验收证据。"""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Callable, Mapping
from dataclasses import dataclass
from typing import TypeVar
from urllib.parse import urlsplit

from video_agent_api.ports.contracts import AdapterNotConfiguredError

_T = TypeVar("_T")
_REQUIRED = (
    "MVP_A_CREDENTIAL_SANDBOX",
    "MVP_A_SANDBOX_PROFILE_ID",
    "MVP_A_SANDBOX_ALLOWLIST",
    "MVP_A_SANDBOX_CREDENTIAL_REF",
    "MVP_A_SANDBOX_PROVIDER_PROBE",
    "MVP_A_SANDBOX_TOS_PROBE",
    "MVP_A_SANDBOX_RENDERER_PROBE",
)
_FORBIDDEN_KEYS = {
    "authorization",
    "token",
    "apikey",
    "secret",
    "credential",
    "completeresponse",
    "mediabytes",
}
_FORBIDDEN_VALUES = re.compile(r"(?i)(?:bearer\s+\S+|(?:sk|ak)_[a-z0-9_-]{8,})")
_FIXED_STAGES = (
    ("S01", "项目/剧集", "项目/剧集 owner"),
    ("S02", "文本生成", "文本生成 owner"),
    ("S03", "文本审核", "文本审核 owner"),
    ("S04", "GPT Image 候选", "图片生成 owner"),
    ("S05", "图片审核/current", "场景/镜头 owner"),
    ("S06", "Agnes submit/poll/result/cancel", "视频生成 owner"),
    ("S07", "视频审核/current", "场景/镜头 owner"),
    ("S08", "MediaInspect/Derivative", "媒体 owner"),
    ("S08a", "普通上传/source inspection", "媒体 owner"),
    ("S08b", "音频 proxy", "媒体 owner"),
    ("S09", "时间线", "时间线 owner"),
    ("S10", "MP4/SRT/light render", "导出 owner"),
    ("S11", "artifact handoff", "导出/存储 owner"),
    ("R01", "Generation route/legacy drain", "Generation owner"),
    ("R02", "restart/reconcile replay", "Generation/媒体/导出 owner"),
)


@dataclass(frozen=True, slots=True)
class SandboxAssessment:
    result: str
    readiness: str
    missing: tuple[str, ...]
    allowlist_hosts: tuple[str, ...]

    @property
    def ready(self) -> bool:
        return self.result == "ready"


class CredentialedSandboxHarness:
    """Live sandbox 只在显式配置、allowlist 和 probe 全部满足后允许临时注入。"""

    def __init__(self, values: Mapping[str, str]) -> None:
        self._values = values

    def assess(self) -> SandboxAssessment:
        missing = tuple(key for key in _REQUIRED if not self._values.get(key))
        if self._values.get("MVP_A_CREDENTIAL_SANDBOX") not in {None, "enabled"}:
            missing = (*missing, "MVP_A_CREDENTIAL_SANDBOX=enabled")
        hosts = self._allowlist_hosts()
        if not hosts and "MVP_A_SANDBOX_ALLOWLIST" not in missing:
            missing = (*missing, "MVP_A_SANDBOX_ALLOWLIST=https allowlist")
        probe_keys = _REQUIRED[-3:]
        probes_pending = any(self._values.get(key) != "passed" for key in probe_keys)
        if missing:
            return SandboxAssessment("unconfigured", "not_ready", missing, hosts)
        if probes_pending:
            return SandboxAssessment("not_ready", "not_ready", probe_keys, hosts)
        return SandboxAssessment("ready", "ready", (), hosts)

    def invoke[R](
        self,
        endpoint: str,
        credential_resolver: Callable[[], str],
        operation: Callable[[str], R],
    ) -> R:
        """仅向 allowlisted endpoint 的调用者临时交付注入值，不产生日志或证据。"""
        assessment = self.assess()
        if not assessment.ready:
            raise AdapterNotConfiguredError(f"credentialed_sandbox_{assessment.result}")
        host = _https_host(endpoint)
        if host not in assessment.allowlist_hosts:
            raise AdapterNotConfiguredError("credentialed_sandbox_endpoint_not_allowlisted")
        credential = credential_resolver()
        if not credential:
            raise AdapterNotConfiguredError("credentialed_sandbox_injection_unavailable")
        return operation(credential)

    def evidence(self) -> dict[str, object]:
        assessment = self.assess()
        allowlist_digest = hashlib.sha256(",".join(assessment.allowlist_hosts).encode()).hexdigest()
        stages = [
            {
                "id": identifier,
                "name": name,
                "owner": owner,
                "status": "not_executed" if not assessment.ready else "pending_execution",
                "record": "not_captured",
            }
            for identifier, name, owner in _FIXED_STAGES
        ]
        evidence: dict[str, object] = {
            "schemaVersion": "1.1.0",
            "reportId": "E2E-MVPA-001",
            "scope": "close-phase-one-mvp-a-runtime-gaps/7.x",
            "result": assessment.result,
            "readiness": assessment.readiness,
            "sandbox": {
                "profileId": self._values.get("MVP_A_SANDBOX_PROFILE_ID") or None,
                "allowlistDigest": allowlist_digest,
                "allowlistCount": len(assessment.allowlist_hosts),
                "injection": "not_attempted" if not assessment.ready else "ephemeral_only",
            },
            "prerequisites": {"missing": list(assessment.missing)},
            "defaultCi": {"provider": "mock", "storage": "local_workspace", "network": "disabled"},
            "fixedLoop": [identifier for identifier, _, _ in _FIXED_STAGES],
            "stageRecordContract": {
                "admission": {
                    "catalog": _identity(),
                    "policy": _identity(),
                    "resource": _identity(),
                    "capacity": _identity(),
                },
                "executionRoute": None,
                "operation": {"runId": None, "logicalOperation": None},
                "outbound": {
                    "correlation": None,
                    "lookupOutcome": "not_attempted",
                    "externalAcceptCount": 0,
                },
                "ownerHandoff": [],
                "artifacts": [],
                "restartReconcile": {"outcome": "not_attempted"},
                "retention": "no_gc_not_evaluated",
            },
            "contractEvidence": [
                "services/api/tests/test_runtime_composition.py",
                "services/api/tests/test_live_provider_transports.py",
                "services/api/tests/test_generation_worker.py",
                "services/api/tests/test_media_export_worker.py",
                "services/api/tests/test_resilience_observability.py",
                "services/api/tests/test_phase_one_contracts.py",
            ],
            "stages": stages,
            "secretScan": {"result": "passed", "forbiddenFieldCount": 0},
        }
        assert_sanitized_evidence(evidence)
        return evidence

    def _allowlist_hosts(self) -> tuple[str, ...]:
        raw = self._values.get("MVP_A_SANDBOX_ALLOWLIST", "")
        hosts: set[str] = set()
        for value in raw.split(","):
            if not value.strip():
                continue
            try:
                hosts.add(_https_host(value.strip()))
            except ValueError:
                return ()
        return tuple(sorted(hosts))


def assert_sanitized_evidence(value: object) -> None:
    """拒绝高风险字段和值，避免报告将 transport 私密内容当作证据。"""
    _scan_evidence(value)


def _scan_evidence(value: object) -> None:
    if isinstance(value, Mapping):
        for key, nested in value.items():
            if not isinstance(key, str):
                raise ValueError("evidence field name must be text")
            if key.casefold() in _FORBIDDEN_KEYS:
                raise ValueError(f"forbidden evidence field: {key}")
            _scan_evidence(nested)
    elif isinstance(value, list | tuple):
        for nested in value:
            _scan_evidence(nested)
    elif isinstance(value, str) and _FORBIDDEN_VALUES.search(value):
        raise ValueError("forbidden evidence value")


def canonical_evidence_json(values: Mapping[str, str]) -> str:
    """返回版本化、稳定排序的报告正文，供版本控制中的脱敏证据复验。"""
    return (
        json.dumps(
            CredentialedSandboxHarness(values).evidence(),
            ensure_ascii=False,
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )


def _https_host(value: str) -> str:
    parsed = urlsplit(value)
    if parsed.scheme != "https" or not parsed.hostname or parsed.username or parsed.password:
        raise ValueError("sandbox allowlist requires an https origin")
    if parsed.path not in {"", "/"} or parsed.query or parsed.fragment:
        raise ValueError("sandbox allowlist must not include a path, query, or fragment")
    return parsed.hostname.casefold()


def _identity() -> dict[str, None]:
    return {"id": None, "revision": None, "hash": None}
