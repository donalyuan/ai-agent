"""凭据加密边界。

主密钥只接受 32-byte Docker Secret，明文只在该模块的 adapter boundary 短暂存在。
Envelope 可序列化但不包含明文；AAD 绑定 profile/credential，避免密文跨 owner 重放。
"""

from __future__ import annotations

import base64
import hashlib
import os
from collections.abc import Mapping
from dataclasses import asdict, dataclass

from cryptography.exceptions import InvalidTag
from cryptography.hazmat.primitives.ciphers.aead import AESGCM


@dataclass(frozen=True, slots=True)
class CredentialEnvelope:
    algorithm: str
    ciphertext: str
    nonce: str
    auth_tag: str
    key_version: str
    aad_version: str
    profile_id: str
    credential_id: str
    masked_prefix: str
    last4: str

    @property
    def tag(self) -> str:
        """兼容旧调用方的只读别名；canonical 字段为 auth_tag。"""
        return self.auth_tag

    def to_dict(self) -> dict[str, str]:
        result = asdict(self)
        result["authTag"] = result.pop("auth_tag")
        result["keyVersion"] = result.pop("key_version")
        result["aadVersion"] = result.pop("aad_version")
        result["profileId"] = result.pop("profile_id")
        result["credentialId"] = result.pop("credential_id")
        return result


class CredentialMasterKeyUnavailable(RuntimeError):
    code = "credential_master_key_unavailable"


def _b64(value: bytes) -> str:
    return base64.urlsafe_b64encode(value).decode("ascii").rstrip("=")


def _unb64(value: str) -> bytes:
    return base64.urlsafe_b64decode(value + "=" * (-len(value) % 4))


class CredentialKeyring:
    def __init__(self, master_key: bytes | None = None, version: str = "local-v1") -> None:
        self._master_key = master_key
        self.version = version

    def _key(self) -> bytes:
        if not self._master_key or len(self._master_key) != 32:
            raise CredentialMasterKeyUnavailable(CredentialMasterKeyUnavailable.code)
        return self._master_key

    @staticmethod
    def _aad(profile_id: str, credential_id: str, aad_version: str) -> bytes:
        return f"video-agent:{aad_version}:{profile_id}:{credential_id}".encode()

    def seal(
        self,
        value: str,
        *,
        profile_id: str = "unbound",
        credential_id: str = "unbound",
        aad_version: str = "v1",
    ) -> CredentialEnvelope:
        key = self._key()
        if not isinstance(value, str) or not value:
            raise ValueError("credential value must not be blank")
        nonce = os.urandom(12)
        aad = self._aad(profile_id, credential_id, aad_version)
        encrypted = AESGCM(key).encrypt(nonce, value.encode("utf-8"), aad)
        ciphertext, tag = encrypted[:-16], encrypted[-16:]
        return CredentialEnvelope(
            "AES-256-GCM",
            _b64(ciphertext),
            _b64(nonce),
            _b64(tag),
            self.version,
            aad_version,
            profile_id,
            credential_id,
            value[:4],
            value[-4:],
        )

    def open(self, envelope: CredentialEnvelope, *, profile_id: str, credential_id: str) -> str:
        key = self._key()
        if envelope.algorithm != "AES-256-GCM":
            raise ValueError("unsupported credential envelope algorithm")
        if envelope.profile_id != profile_id or envelope.credential_id != credential_id:
            raise ValueError("credential envelope owner mismatch")
        aad = self._aad(profile_id, credential_id, envelope.aad_version)
        try:
            value = AESGCM(key).decrypt(
                _unb64(envelope.nonce), _unb64(envelope.ciphertext) + _unb64(envelope.auth_tag), aad
            )
        except (InvalidTag, ValueError) as exc:
            raise ValueError("credential envelope authentication failed") from exc
        return value.decode("utf-8")

    def reencrypt(
        self,
        envelope: CredentialEnvelope,
        *,
        profile_id: str,
        credential_id: str,
        target: CredentialKeyring,
    ) -> CredentialEnvelope:
        return target.seal(
            self.open(envelope, profile_id=profile_id, credential_id=credential_id),
            profile_id=profile_id,
            credential_id=credential_id,
            aad_version=envelope.aad_version,
        )

    @staticmethod
    def fingerprint(envelope: CredentialEnvelope) -> str:
        payload = "|".join(
            (
                envelope.algorithm,
                envelope.key_version,
                envelope.nonce,
                envelope.ciphertext,
                envelope.auth_tag,
                envelope.profile_id,
                envelope.credential_id,
            )
        ).encode("utf-8")
        return hashlib.sha256(payload).hexdigest()


def masked_credential_status(envelope: CredentialEnvelope | None) -> Mapping[str, str]:
    if envelope is None:
        return {"status": "unconfigured"}
    return {
        "status": "configured",
        "maskedPrefix": envelope.masked_prefix,
        "last4": envelope.last4,
        "keyVersion": envelope.key_version,
    }


class CatalogCredentialResolver:
    """只解析 catalog owner 已持有的 envelope，不复制或持久化 keyring 状态。"""

    def __init__(
        self,
        keyring: CredentialKeyring,
        envelopes: Mapping[str, CredentialEnvelope],
    ) -> None:
        self._keyring = keyring
        self._envelopes = envelopes

    def resolve(self, credential_ref: str, profile_id: str) -> str:
        envelope = self._envelopes.get(profile_id)
        if envelope is None or envelope.credential_id != credential_ref:
            raise ValueError("credential reference is unavailable")
        return self._keyring.open(envelope, profile_id=profile_id, credential_id=credential_ref)
