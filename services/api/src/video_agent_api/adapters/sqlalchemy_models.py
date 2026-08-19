"""阶段 0 的最小关系模型；媒体正文始终在对象存储之外。"""

from __future__ import annotations

from datetime import datetime
from uuid import uuid4

from sqlalchemy import (
    JSON,
    CheckConstraint,
    DateTime,
    ForeignKey,
    ForeignKeyConstraint,
    Integer,
    String,
    Text,
    UniqueConstraint,
    func,
)
from sqlalchemy.dialects.postgresql import JSONB
from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column


def new_id() -> str:
    return str(uuid4())


class Base(DeclarativeBase):
    pass


# SQLite tests use JSON while PostgreSQL persists document facts as JSONB.
JSON_DOCUMENT = JSON().with_variant(JSONB, "postgresql")
STATUS_VALUES = (
    "draft",
    "generated",
    "pending_review",
    "approved",
    "rejected",
    "superseded",
    "archived",
)
STATUS_CHECK = "status IN (" + ", ".join(f"'{value}'" for value in STATUS_VALUES) + ")"


def hex64_check(column: str) -> str:
    """Build a portable PostgreSQL/SQLite check for a 64-character hex value."""
    stripped = f"lower({column})"
    for character in "0123456789abcdef":
        stripped = f"replace({stripped}, '{character}', '')"
    return f"length({column}) = 64 AND length({stripped}) = 0"


class IdentityRevisionMixin:
    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), server_default=func.now())
    updated_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), server_default=func.now(), onupdate=func.now()
    )


class Project(IdentityRevisionMixin, Base):
    __tablename__ = "projects"

    name: Mapped[str] = mapped_column(String(255), nullable=False)
    status: Mapped[str] = mapped_column(
        String(32), CheckConstraint(STATUS_CHECK), nullable=False, default="draft"
    )


class Episode(IdentityRevisionMixin, Base):
    __tablename__ = "episodes"
    __table_args__ = (
        CheckConstraint("display_number > 0", name="ck_episodes_display_number_positive"),
        UniqueConstraint("project_id", "display_number", name="uq_episode_project_display_number"),
    )

    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    display_number: Mapped[int] = mapped_column(Integer, nullable=False)
    # 0003 backfills legacy rows while preserving the display_number column name.
    title: Mapped[str] = mapped_column(String(255), nullable=False, default="Untitled Episode")
    status: Mapped[str] = mapped_column(
        String(32), CheckConstraint(STATUS_CHECK), nullable=False, default="draft"
    )


class Scene(IdentityRevisionMixin, Base):
    __tablename__ = "scenes"

    episode_id: Mapped[str] = mapped_column(ForeignKey("episodes.id"), nullable=False, index=True)
    display_number: Mapped[int] = mapped_column(Integer, nullable=False)
    status: Mapped[str] = mapped_column(
        String(32), CheckConstraint(STATUS_CHECK), nullable=False, default="draft"
    )


class Shot(IdentityRevisionMixin, Base):
    __tablename__ = "shots"

    scene_id: Mapped[str] = mapped_column(ForeignKey("scenes.id"), nullable=False, index=True)
    display_number: Mapped[int] = mapped_column(Integer, nullable=False)
    status: Mapped[str] = mapped_column(
        String(32), CheckConstraint(STATUS_CHECK), nullable=False, default="draft"
    )


class Asset(IdentityRevisionMixin, Base):
    __tablename__ = "assets"
    __table_args__ = (
        UniqueConstraint("id", "project_id", name="uq_assets_id_project_id"),
        CheckConstraint(
            "kind IN ('image', 'video', 'audio', 'text', 'document')", name="ck_assets_kind"
        ),
        CheckConstraint("length(trim(name)) > 0", name="ck_assets_name_nonblank"),
    )

    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    kind: Mapped[str] = mapped_column(String(64), nullable=False)
    name: Mapped[str] = mapped_column(String(255), nullable=False, default="Untitled Asset")
    status: Mapped[str] = mapped_column(
        String(32), CheckConstraint(STATUS_CHECK), nullable=False, default="draft"
    )


class AssetVersion(Base):
    __tablename__ = "asset_versions"
    __table_args__ = (
        ForeignKeyConstraint(
            ["asset_id", "project_id"],
            ["assets.id", "assets.project_id"],
            name="fk_asset_versions_asset_project",
        ),
        UniqueConstraint("asset_id", "version_number", name="uq_asset_version_number"),
        CheckConstraint("version_number > 0", name="ck_asset_versions_version_positive"),
        CheckConstraint("revision >= 0", name="ck_asset_versions_revision_nonnegative"),
        CheckConstraint(STATUS_CHECK, name="ck_asset_versions_status"),
        CheckConstraint(hex64_check("checksum"), name="ck_asset_versions_checksum_hex64"),
        CheckConstraint(hex64_check("content_hash"), name="ck_asset_versions_content_hash_hex64"),
        CheckConstraint(
            "length(trim(object_key)) > 0", name="ck_asset_versions_object_key_nonblank"
        ),
        CheckConstraint("length(trim(mime_type)) > 0", name="ck_asset_versions_mime_type_nonblank"),
        CheckConstraint("size_bytes >= 0", name="ck_asset_versions_size_nonnegative"),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    asset_id: Mapped[str] = mapped_column(ForeignKey("assets.id"), nullable=False, index=True)
    project_id: Mapped[str] = mapped_column(
        ForeignKey("projects.id", name="fk_asset_versions_project_id"), nullable=False, index=True
    )
    version_number: Mapped[int] = mapped_column(Integer, nullable=False)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=0)
    status: Mapped[str] = mapped_column(String(32), nullable=False, default="draft")
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    storage_ref: Mapped[str] = mapped_column(String(1024), nullable=False)
    checksum: Mapped[str] = mapped_column(String(128), nullable=False)
    content_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    storage_provider: Mapped[str] = mapped_column(String(128), nullable=False, default="legacy")
    bucket: Mapped[str] = mapped_column(String(255), nullable=False, default="legacy")
    region: Mapped[str | None] = mapped_column(String(255))
    object_key: Mapped[str] = mapped_column(String(1024), nullable=False, default="legacy")
    e_tag: Mapped[str | None] = mapped_column(String(255))
    mime_type: Mapped[str] = mapped_column(
        String(255), nullable=False, default="application/octet-stream"
    )
    size_bytes: Mapped[int] = mapped_column(Integer, nullable=False, default=0)
    media_metadata: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)
    metadata_json: Mapped[dict[str, object]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=dict
    )
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), server_default=func.now())


class WorkflowDraft(IdentityRevisionMixin, Base):
    __tablename__ = "workflow_drafts"
    __table_args__ = (
        CheckConstraint("scope_type IN ('project', 'episode', 'scene', 'shot')"),
        CheckConstraint("revision >= 0"),
        CheckConstraint(STATUS_CHECK),
    )

    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    scope_type: Mapped[str] = mapped_column(String(64), nullable=False)
    scope_ids: Mapped[list[str]] = mapped_column(JSON_DOCUMENT, nullable=False)
    definition: Mapped[dict[str, object]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=dict
    )
    status: Mapped[str] = mapped_column(
        String(32), CheckConstraint(STATUS_CHECK), nullable=False, default="draft"
    )


class WorkflowVersion(Base):
    __tablename__ = "workflow_versions"
    __table_args__ = (
        UniqueConstraint("workflow_draft_id", "version_number", name="uq_workflow_version_number"),
        CheckConstraint("revision >= 0", name="ck_workflow_versions_revision_nonnegative"),
        CheckConstraint(STATUS_CHECK, name="ck_workflow_versions_status"),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    workflow_draft_id: Mapped[str] = mapped_column(
        ForeignKey("workflow_drafts.id"), nullable=False, index=True
    )
    version_number: Mapped[int] = mapped_column(Integer, nullable=False)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=0)
    status: Mapped[str] = mapped_column(String(32), nullable=False, default="draft")
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    definition: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)
    content_hash: Mapped[str] = mapped_column(String(128), nullable=False)
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), server_default=func.now())


class TimelineDocument(IdentityRevisionMixin, Base):
    __tablename__ = "timeline_documents"

    episode_id: Mapped[str] = mapped_column(ForeignKey("episodes.id"), nullable=False, index=True)
    name: Mapped[str] = mapped_column(String(255), nullable=False)
    document: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False, default=dict)
    status: Mapped[str] = mapped_column(
        String(32), CheckConstraint(STATUS_CHECK), nullable=False, default="draft"
    )


class Provider(Base):
    __tablename__ = "providers"

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    name: Mapped[str] = mapped_column(String(255), unique=True, nullable=False)
    adapter_key: Mapped[str] = mapped_column(String(128), nullable=False)
    enabled: Mapped[bool] = mapped_column(nullable=False, default=True)
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), server_default=func.now())


class ProviderProfile(Base):
    __tablename__ = "provider_profiles"

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    provider_id: Mapped[str] = mapped_column(ForeignKey("providers.id"), nullable=False, index=True)
    name: Mapped[str] = mapped_column(String(255), nullable=False)
    enabled: Mapped[bool] = mapped_column(nullable=False, default=True)
    base_url: Mapped[str | None] = mapped_column(String(1024))
    endpoint: Mapped[str | None] = mapped_column(String(1024))
    bucket: Mapped[str | None] = mapped_column(String(255))
    region: Mapped[str | None] = mapped_column(String(255))
    settings: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False, default=dict)
    auth: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False, default=dict)
    credential_metadata_id: Mapped[str | None] = mapped_column(ForeignKey("credential_metadata.id"))
    timeout_ms: Mapped[int] = mapped_column(Integer, nullable=False, default=30_000)


class Model(Base):
    __tablename__ = "models"

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    profile_id: Mapped[str] = mapped_column(
        ForeignKey("provider_profiles.id"), nullable=False, index=True
    )
    model_key: Mapped[str] = mapped_column(String(255), nullable=False)
    enabled: Mapped[bool] = mapped_column(nullable=False, default=True)
    default_parameters: Mapped[dict[str, object]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=dict
    )
    parameter_schema: Mapped[dict[str, object]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=dict
    )


class CredentialMetadata(Base):
    __tablename__ = "credential_metadata"

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    provider_id: Mapped[str] = mapped_column(ForeignKey("providers.id"), nullable=False, index=True)
    key_version: Mapped[str] = mapped_column(String(64), nullable=False)
    masked_prefix: Mapped[str | None] = mapped_column(String(32))
    last4: Mapped[str | None] = mapped_column(String(4))
    ciphertext: Mapped[str] = mapped_column(Text, nullable=False)
    nonce: Mapped[str] = mapped_column(String(128), nullable=False)
    tag: Mapped[str] = mapped_column(String(128), nullable=False)
