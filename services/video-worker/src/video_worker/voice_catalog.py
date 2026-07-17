import hashlib
import hmac
import json
from dataclasses import dataclass
from datetime import UTC, datetime
from typing import Callable, Protocol
from urllib.parse import quote, urlparse
from urllib.request import Request, urlopen
from uuid import UUID

from psycopg import connect
from psycopg.types.json import Jsonb


CATALOG_ACTION = "ListSpeakers"
CATALOG_VERSION = "2025-05-20"
CATALOG_SERVICE = "speech_saas_prod"
CATALOG_REGION = "cn-beijing"
CATALOG_ENDPOINT = "https://open.volcengineapi.com/"


class VoiceCatalogSyncError(RuntimeError):
    def __init__(self, code: str, message: str, *, retryable: bool = False) -> None:
        super().__init__(message)
        self.code = code
        self.retryable = retryable


@dataclass(frozen=True)
class VoiceCatalogPage:
    page: int
    limit: int
    total: int
    speakers: list[dict]


@dataclass(frozen=True)
class VoiceCatalogSyncResult:
    page_count: int
    speaker_count: int


@dataclass(frozen=True)
class VoiceCatalogSyncJob:
    sync_id: UUID
    model_id: UUID
    resource_id: str
    page_limit: int
    access_key: str
    secret_key: str
    timeout_seconds: float


class VoiceCatalogProvider(Protocol):
    def list_speakers(self, resource_id: str, page: int, limit: int) -> VoiceCatalogPage: ...


class VoiceCatalogStore(Protocol):
    def replace_catalog(
        self,
        sync_id: UUID,
        model_id: UUID,
        speakers: list[dict],
    ) -> None: ...

    def fail_sync(self, sync_id: UUID, error_summary: str) -> None: ...


class VoiceCatalogSynchronizer:
    def __init__(self, store: VoiceCatalogStore, provider: VoiceCatalogProvider) -> None:
        self.store = store
        self.provider = provider

    def synchronize(
        self,
        *,
        sync_id: UUID,
        model_id: UUID,
        resource_id: str,
        page_limit: int = 30,
    ) -> VoiceCatalogSyncResult:
        speakers: list[dict] = []
        page_number = 1
        total = 0
        try:
            while page_number == 1 or len(speakers) < total:
                page = self.provider.list_speakers(resource_id, page_number, page_limit)
                self._validate_page(page, page_number, page_limit, resource_id)
                total = page.total
                speakers.extend(_normalize_catalog_speaker(speaker) for speaker in page.speakers)
                page_number += 1
                if not page.speakers and len(speakers) < total:
                    raise VoiceCatalogSyncError(
                        "invalid_catalog_page",
                        "供应商在目录完成前返回空分页",
                    )
            self._validate_complete_catalog(speakers, total)
            self.store.replace_catalog(sync_id, model_id, speakers)
        except Exception as error:
            message = _safe_sync_error(error)
            self.store.fail_sync(sync_id, message)
            if isinstance(error, VoiceCatalogSyncError):
                raise
            raise VoiceCatalogSyncError(
                "catalog_sync_failed",
                message,
                retryable=True,
            ) from error
        return VoiceCatalogSyncResult(
            page_count=page_number - 1,
            speaker_count=len(speakers),
        )

    @staticmethod
    def _validate_page(
        page: VoiceCatalogPage,
        expected_page: int,
        expected_limit: int,
        resource_id: str,
    ) -> None:
        if page.page != expected_page or page.limit != expected_limit or page.total < 0:
            raise VoiceCatalogSyncError("invalid_catalog_page", "音色目录分页元数据无效")
        for speaker in page.speakers:
            if not isinstance(speaker, dict):
                raise VoiceCatalogSyncError("invalid_catalog_entry", "音色目录条目无效")
            required = ("VoiceType", "Name", "ResourceID")
            if any(not str(speaker.get(field, "")).strip() for field in required):
                raise VoiceCatalogSyncError("invalid_catalog_entry", "音色目录缺少必要字段")
            if speaker["ResourceID"] != resource_id:
                raise VoiceCatalogSyncError("invalid_catalog_entry", "音色资源版本不匹配")

    @staticmethod
    def _validate_complete_catalog(speakers: list[dict], total: int) -> None:
        if len(speakers) != total:
            raise VoiceCatalogSyncError("incomplete_catalog", "音色目录数量与 Total 不一致")
        keys = [(item["ResourceID"], item["VoiceType"]) for item in speakers]
        if len(keys) != len(set(keys)):
            raise VoiceCatalogSyncError("duplicate_catalog_entry", "音色目录包含重复条目")


HttpTransport = Callable[[Request, float], tuple[int, dict[str, str], bytes]]


class VolcengineVoiceCatalogProvider:
    def __init__(
        self,
        *,
        access_key: str,
        secret_key: str,
        timeout_seconds: float,
        transport: HttpTransport | None = None,
        now: Callable[[], datetime] | None = None,
    ) -> None:
        if not access_key or not secret_key:
            raise VoiceCatalogSyncError("catalog_credentials_missing", "音色目录凭据未配置")
        self.access_key = access_key
        self.secret_key = secret_key
        self.timeout_seconds = timeout_seconds
        self.transport = transport or _default_transport
        self.now = now or (lambda: datetime.now(UTC))

    def list_speakers(self, resource_id: str, page: int, limit: int) -> VoiceCatalogPage:
        body = json.dumps(
            {
                "ResourceIDs": [resource_id],
                "VoiceTypes": [],
                "Page": page,
                "Limit": limit,
            },
            ensure_ascii=False,
            separators=(",", ":"),
        ).encode("utf-8")
        request = self._signed_request(body)
        try:
            status, _, response_body = self.transport(request, self.timeout_seconds)
        except VoiceCatalogSyncError:
            raise
        except Exception as error:
            raise VoiceCatalogSyncError(
                "catalog_transport_error",
                f"音色目录请求失败: {error.__class__.__name__}",
                retryable=True,
            ) from error
        if status >= 500 or status == 429:
            raise VoiceCatalogSyncError(
                "catalog_temporary_error",
                f"音色目录服务暂时不可用: HTTP {status}",
                retryable=True,
            )
        if status < 200 or status >= 300:
            raise VoiceCatalogSyncError(
                "catalog_permanent_error",
                f"音色目录请求被拒绝: HTTP {status}",
            )
        try:
            payload = json.loads(response_body)
            result = payload["Result"]
            speakers = result["Speakers"]
            total = int(result["Total"])
        except (KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
            raise VoiceCatalogSyncError(
                "invalid_catalog_response",
                "音色目录响应结构无效",
            ) from error
        if not isinstance(speakers, list):
            raise VoiceCatalogSyncError("invalid_catalog_response", "音色目录 Speakers 无效")
        return VoiceCatalogPage(page=page, limit=limit, total=total, speakers=speakers)

    def _signed_request(self, body: bytes) -> Request:
        timestamp = self.now().astimezone(UTC).strftime("%Y%m%dT%H%M%SZ")
        date = timestamp[:8]
        parsed = urlparse(CATALOG_ENDPOINT)
        query = f"Action={quote(CATALOG_ACTION)}&Version={quote(CATALOG_VERSION)}"
        content_hash = hashlib.sha256(body).hexdigest()
        canonical_headers = (
            "content-type:application/json; charset=UTF-8\n"
            f"host:{parsed.netloc}\n"
            f"x-content-sha256:{content_hash}\n"
            f"x-date:{timestamp}\n"
        )
        signed_headers = "content-type;host;x-content-sha256;x-date"
        canonical_request = "\n".join(
            ["POST", "/", query, canonical_headers, signed_headers, content_hash]
        )
        credential_scope = f"{date}/{CATALOG_REGION}/{CATALOG_SERVICE}/request"
        string_to_sign = "\n".join(
            [
                "HMAC-SHA256",
                timestamp,
                credential_scope,
                hashlib.sha256(canonical_request.encode("utf-8")).hexdigest(),
            ]
        )
        signing_key = _signature_key(self.secret_key, date, CATALOG_REGION, CATALOG_SERVICE)
        signature = hmac.new(signing_key, string_to_sign.encode("utf-8"), hashlib.sha256).hexdigest()
        authorization = (
            f"HMAC-SHA256 Credential={self.access_key}/{credential_scope}, "
            f"SignedHeaders={signed_headers}, Signature={signature}"
        )
        return Request(
            f"{CATALOG_ENDPOINT}?{query}",
            data=body,
            method="POST",
            headers={
                "Content-Type": "application/json; charset=UTF-8",
                "Host": parsed.netloc,
                "X-Content-Sha256": content_hash,
                "X-Date": timestamp,
                "Authorization": authorization,
            },
        )


def _signature_key(secret_key: str, date: str, region: str, service: str) -> bytes:
    date_key = hmac.new(secret_key.encode("utf-8"), date.encode("utf-8"), hashlib.sha256).digest()
    region_key = hmac.new(date_key, region.encode("utf-8"), hashlib.sha256).digest()
    service_key = hmac.new(region_key, service.encode("utf-8"), hashlib.sha256).digest()
    return hmac.new(service_key, b"request", hashlib.sha256).digest()


def _default_transport(request: Request, timeout: float) -> tuple[int, dict[str, str], bytes]:
    with urlopen(request, timeout=timeout) as response:  # noqa: S310 - fixed HTTPS endpoint.
        return response.status, dict(response.headers.items()), response.read()


class PostgresVoiceCatalogStore:
    def __init__(self, database_url: str) -> None:
        self.database_url = database_url

    def enqueue_due_syncs(self) -> int:
        with connect(self.database_url) as connection:
            result = connection.execute(
                """
                INSERT INTO voice_catalog_syncs (model_id, trigger_source)
                SELECT model.id, 'scheduled'
                FROM ai_models model
                WHERE model.model_type = 'speech'
                  AND model.api_protocol = 'volcengine_tts_v3'
                  AND model.status = 'enabled'
                  AND model.deleted_at IS NULL
                  AND model.catalog_access_key IS NOT NULL
                  AND model.catalog_secret_key IS NOT NULL
                  AND COALESCE((model.settings ->> 'catalog_sync_interval_minutes')::int, 0) > 0
                  AND NOT EXISTS (
                      SELECT 1 FROM voice_catalog_syncs active_sync
                      WHERE active_sync.model_id = model.id
                        AND active_sync.status IN ('queued', 'running')
                  )
                  AND COALESCE((
                      SELECT MAX(completed_at)
                      FROM voice_catalog_syncs completed_sync
                      WHERE completed_sync.model_id = model.id
                        AND completed_sync.status IN ('succeeded', 'failed')
                  ), '-infinity'::timestamptz) <= NOW() - make_interval(
                      mins => (model.settings ->> 'catalog_sync_interval_minutes')::int
                  )
                ON CONFLICT DO NOTHING
                """
            )
            return result.rowcount

    def claim_next_sync(self) -> VoiceCatalogSyncJob | None:
        with connect(self.database_url) as connection, connection.transaction():
            row = connection.execute(
                """
                WITH next_sync AS (
                    SELECT sync.id
                    FROM voice_catalog_syncs sync
                    JOIN ai_models model ON model.id = sync.model_id
                    WHERE sync.status = 'queued'
                      AND model.status = 'enabled'
                      AND model.deleted_at IS NULL
                      AND model.model_type = 'speech'
                      AND model.api_protocol = 'volcengine_tts_v3'
                    ORDER BY sync.created_at, sync.id
                    FOR UPDATE OF sync SKIP LOCKED
                    LIMIT 1
                )
                UPDATE voice_catalog_syncs sync
                SET status = 'running', started_at = NOW(), updated_at = NOW()
                FROM next_sync
                WHERE sync.id = next_sync.id
                RETURNING sync.id, sync.model_id, sync.page_limit
                """
            ).fetchone()
            if row is None:
                return None
            sync_id, model_id, page_limit = row
            model = connection.execute(
                """
                SELECT settings ->> 'resource_id', catalog_access_key,
                       catalog_secret_key, timeout_seconds
                FROM ai_models
                WHERE id = %s
                """,
                (model_id,),
            ).fetchone()
            if model is None or not all(model[:3]):
                raise VoiceCatalogSyncError(
                    "catalog_model_config_invalid",
                    "音色目录模型配置无效",
                )
            return VoiceCatalogSyncJob(
                sync_id=sync_id,
                model_id=model_id,
                resource_id=str(model[0]),
                page_limit=int(page_limit),
                access_key=str(model[1]),
                secret_key=str(model[2]),
                timeout_seconds=float(model[3]),
            )

    def replace_catalog(
        self,
        sync_id: UUID,
        model_id: UUID,
        speakers: list[dict],
    ) -> None:
        with connect(self.database_url) as connection, connection.transaction():
            sync = connection.execute(
                """
                SELECT status FROM voice_catalog_syncs
                WHERE id = %s AND model_id = %s
                FOR UPDATE
                """,
                (sync_id, model_id),
            ).fetchone()
            if sync is None or sync[0] not in {"queued", "running"}:
                raise VoiceCatalogSyncError("invalid_sync_state", "音色目录同步状态无效")
            connection.execute(
                """
                UPDATE voice_catalog_entries
                SET is_available = FALSE, updated_at = NOW()
                WHERE model_id = %s AND is_available = TRUE
                """,
                (model_id,),
            )
            for speaker in speakers:
                speaker = _normalize_catalog_speaker(speaker)
                connection.execute(
                    """
                    INSERT INTO voice_catalog_entries (
                        model_id, voice_type, resource_id, name, avatar_url, gender, age,
                        categories, normal_labels, special_labels, trial_url, short_trial_url,
                        languages, emotions, description, is_available,
                        first_seen_sync_id, last_seen_sync_id, catalog_version
                    )
                    VALUES (
                        %s, %s, %s, %s, %s, %s, %s,
                        %s, %s, %s, %s, %s, %s, %s, %s, TRUE, %s, %s, 1
                    )
                    ON CONFLICT (model_id, resource_id, voice_type) DO UPDATE SET
                        name = EXCLUDED.name,
                        avatar_url = EXCLUDED.avatar_url,
                        gender = EXCLUDED.gender,
                        age = EXCLUDED.age,
                        categories = EXCLUDED.categories,
                        normal_labels = EXCLUDED.normal_labels,
                        special_labels = EXCLUDED.special_labels,
                        trial_url = EXCLUDED.trial_url,
                        short_trial_url = EXCLUDED.short_trial_url,
                        languages = EXCLUDED.languages,
                        emotions = EXCLUDED.emotions,
                        description = EXCLUDED.description,
                        is_available = TRUE,
                        last_seen_sync_id = EXCLUDED.last_seen_sync_id,
                        catalog_version = voice_catalog_entries.catalog_version + 1,
                        updated_at = NOW()
                    """,
                    (
                        model_id,
                        speaker["VoiceType"],
                        speaker["ResourceID"],
                        speaker["Name"],
                        _text_or_none(speaker.get("Avatar")),
                        _text_or_none(speaker.get("Gender")),
                        _text_or_none(speaker.get("Age")),
                        Jsonb(speaker["Categories"]),
                        speaker["NormalLabels"],
                        speaker["SpecialLabels"],
                        _text_or_none(speaker.get("TrialURL")),
                        _text_or_none(speaker.get("ShortTrialURL")),
                        Jsonb(speaker["Languages"]),
                        Jsonb(speaker["Emotions"]),
                        str(speaker.get("Description", "")),
                        sync_id,
                        sync_id,
                    ),
                )
            connection.execute(
                """
                UPDATE voice_catalog_syncs
                SET status = 'succeeded', completed_at = NOW(),
                    page_count = CEIL(%s::numeric / NULLIF(page_limit, 0))::int,
                    speaker_count = %s, error_summary = NULL, updated_at = NOW()
                WHERE id = %s
                """,
                (len(speakers), len(speakers), sync_id),
            )

    def fail_sync(self, sync_id: UUID, error_summary: str) -> None:
        with connect(self.database_url) as connection:
            connection.execute(
                """
                UPDATE voice_catalog_syncs
                SET status = 'failed', completed_at = NOW(), error_summary = %s,
                    updated_at = NOW()
                WHERE id = %s AND status IN ('queued', 'running')
                """,
                (error_summary[:1000], sync_id),
            )


def _text_or_none(value: object) -> str | None:
    text = str(value or "").strip()
    return text or None


def _safe_sync_error(error: Exception) -> str:
    if isinstance(error, VoiceCatalogSyncError):
        return (str(error) or error.code)[:1000]
    return f"音色目录同步失败: {error.__class__.__name__}"


def _normalize_catalog_speaker(speaker: dict) -> dict:
    normalized = dict(speaker)
    for field in ("Categories", "Languages", "Emotions"):
        value = normalized.get(field)
        normalized[field] = value if isinstance(value, list) else []
    for field in ("NormalLabels", "SpecialLabels"):
        value = normalized.get(field)
        normalized[field] = (
            [str(item) for item in value if item is not None]
            if isinstance(value, list)
            else []
        )
    return normalized


def run_next_voice_catalog_sync(
    store: PostgresVoiceCatalogStore,
    provider_factory: Callable[[VoiceCatalogSyncJob], VoiceCatalogProvider] | None = None,
) -> bool:
    store.enqueue_due_syncs()
    job = store.claim_next_sync()
    if job is None:
        return False
    factory = provider_factory or (
        lambda current: VolcengineVoiceCatalogProvider(
            access_key=current.access_key,
            secret_key=current.secret_key,
            timeout_seconds=current.timeout_seconds,
        )
    )
    try:
        provider = factory(job)
        VoiceCatalogSynchronizer(store, provider).synchronize(
            sync_id=job.sync_id,
            model_id=job.model_id,
            resource_id=job.resource_id,
            page_limit=job.page_limit,
        )
    except VoiceCatalogSyncError:
        return True
    return True
