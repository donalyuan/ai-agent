from __future__ import annotations

import binascii
from base64 import b64decode
from collections.abc import Callable, Iterable
from dataclasses import dataclass
from ipaddress import ip_address
from urllib.parse import urlparse

from video_agent_api.ports.contracts import (
    AdapterNotConfiguredError,
    ImageGenerationPort,
    ModelSelection,
    PortResult,
)


@dataclass(slots=True)
class GPTImageProvider(ImageGenerationPort):
    configured: bool = False
    allowed_hosts: frozenset[str] = frozenset()
    transport: Callable[[str, dict[str, object], ModelSelection, str], PortResult] | None = None
    max_references: int = 8
    max_reference_bytes: int = 32 * 1024 * 1024
    max_dimension: int = 8192

    def _unconfigured(self) -> None:
        if not self.configured:
            raise AdapterNotConfiguredError("gpt_image_provider_unconfigured")

    def generate_image(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult:
        self._unconfigured()
        return self._request(
            "generate",
            {"prompt": prompt, "parameters": dict(selection.default_parameters)},
            selection,
            correlation_id,
        )

    def edit_image(self, prompt: str, selection: ModelSelection, correlation_id: str) -> PortResult:
        self._unconfigured()
        return self._request(
            "edit",
            {"prompt": prompt, "parameters": dict(selection.default_parameters)},
            selection,
            correlation_id,
        )

    def _request(
        self,
        operation: str,
        payload: dict[str, object],
        selection: ModelSelection,
        correlation_id: str,
    ) -> PortResult:
        if self.transport is None:
            raise AdapterNotConfiguredError("gpt_image_transport_unconfigured")
        result = self.transport(operation, payload, selection, correlation_id)
        if not isinstance(result, PortResult):
            raise ValueError("gpt_image_result_invalid")
        return result

    def validate_reference_urls(self, urls: Iterable[str]) -> None:
        values = tuple(urls)
        if len(values) > self.max_references:
            raise ValueError("image_reference_limit_exceeded")
        if not self.allowed_hosts:
            raise AdapterNotConfiguredError("gpt_image_url_allowlist_unconfigured")
        for raw in values:
            parsed = urlparse(raw)
            host = (parsed.hostname or "").lower()
            if (
                parsed.scheme != "https"
                or host not in {item.lower() for item in self.allowed_hosts}
                or parsed.port not in {None, 443}
            ):
                raise ValueError("image_reference_url_not_allowed")
            if parsed.username or parsed.password or parsed.query or parsed.fragment:
                raise ValueError("image_reference_url_unsafe")
            self.validate_resolved_addresses(host, ())

    def validate_resolved_addresses(self, host: str, addresses: Iterable[str]) -> None:
        """Reject unsafe literals and every unsafe DNS answer, including rebinding."""
        normalized_host = host.lower().rstrip(".")
        if normalized_host in {
            "metadata.google.internal",
            "metadata",
            "instance-data.ec2.internal",
        }:
            raise ValueError("image_reference_metadata_address")
        candidates = tuple(addresses) or (normalized_host,)
        for raw in candidates:
            try:
                address = ip_address(raw)
            except ValueError:
                continue
            if (
                address.is_private
                or address.is_loopback
                or address.is_link_local
                or address.is_reserved
                or address.is_unspecified
                or address.is_multicast
                or str(address) in {"169.254.169.254", "100.100.100.200"}
            ):
                raise ValueError("image_reference_private_address")

    @staticmethod
    def reject_redirect(status_code: int) -> None:
        if 300 <= status_code < 400:
            raise ValueError("image_reference_redirect_rejected")

    @staticmethod
    def _observed_mime(value: bytes) -> str:
        if value.startswith(b"\x89PNG\r\n\x1a\n"):
            return "image/png"
        if value.startswith(b"\xff\xd8\xff"):
            return "image/jpeg"
        if value.startswith(b"RIFF") and value[8:12] == b"WEBP":
            return "image/webp"
        raise ValueError("image_mime_unrecognized")

    @classmethod
    def validate_media_bytes(
        cls,
        value: bytes,
        declared_mime: str,
        *,
        width: int,
        height: int,
        max_bytes: int = 32 * 1024 * 1024,
        max_dimension: int = 8192,
        mask: bool = False,
    ) -> tuple[str, int]:
        observed = cls._observed_mime(value)
        allowed = {"image/png"} if mask else {"image/png", "image/jpeg", "image/webp"}
        if declared_mime not in allowed or observed != declared_mime:
            raise ValueError("image_mime_mismatch")
        if len(value) > max_bytes:
            raise ValueError("image_reference_size_exceeded")
        if width < 1 or height < 1 or max(width, height) > max_dimension:
            raise ValueError("image_dimension_exceeded")
        return observed, len(value)

    def validate_base64_image(
        self, value: str, mime_type: str, size_bytes: int, width: int, height: int
    ) -> bytes:
        if mime_type not in {"image/png", "image/jpeg", "image/webp"}:
            raise ValueError("image_mime_unsupported")
        if size_bytes < 0 or size_bytes > self.max_reference_bytes:
            raise ValueError("image_reference_size_exceeded")
        if width < 1 or height < 1 or max(width, height) > self.max_dimension:
            raise ValueError("image_dimension_exceeded")
        try:
            decoded = b64decode(value, validate=True)
        except (ValueError, binascii.Error) as error:
            raise ValueError("image_base64_invalid") from error
        if len(decoded) != size_bytes:
            raise ValueError("image_size_mismatch")
        cls = type(self)
        cls.validate_media_bytes(
            decoded,
            mime_type,
            width=width,
            height=height,
            max_bytes=self.max_reference_bytes,
            max_dimension=self.max_dimension,
        )
        return decoded
