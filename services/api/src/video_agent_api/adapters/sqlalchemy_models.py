"""阶段 0 的最小关系模型；媒体正文始终在对象存储之外。"""

from __future__ import annotations

from datetime import datetime
from uuid import uuid4

from sqlalchemy import (
    JSON,
    BigInteger,
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
    creation_mode: Mapped[str | None] = mapped_column(String(32))
    creative_brief_current: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)
    creative_brief_history: Mapped[list[dict[str, object]]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=list
    )
    creative_settings_current: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)
    creative_settings_history: Mapped[list[dict[str, object]]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=list
    )
    source_binding_current: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)
    source_binding_history: Mapped[list[dict[str, object]]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=list
    )
    story_spec_ref: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)
    story_spec_history: Mapped[list[dict[str, object]]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=list
    )
    source_materials: Mapped[list[dict[str, object]]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=list
    )


class Episode(IdentityRevisionMixin, Base):
    __tablename__ = "episodes"
    __table_args__ = (
        UniqueConstraint("id", "project_id", name="uq_episodes_id_project"),
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
    script_spec_ref: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)
    script_spec_history: Mapped[list[dict[str, object]]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=list
    )


class Scene(IdentityRevisionMixin, Base):
    __tablename__ = "scenes"
    __table_args__ = (
        ForeignKeyConstraint(
            ["episode_id", "project_id"],
            ["episodes.id", "episodes.project_id"],
            name="fk_scenes_episode_project",
        ),
        UniqueConstraint("id", "project_id", "episode_id", name="uq_scenes_id_project_episode"),
        UniqueConstraint("episode_id", "display_number", name="uq_scene_episode_number"),
        CheckConstraint("display_number > 0", name="ck_scenes_display_number_positive"),
        CheckConstraint("length(trim(title)) > 0", name="ck_scenes_title_nonblank"),
    )

    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    episode_id: Mapped[str] = mapped_column(ForeignKey("episodes.id"), nullable=False, index=True)
    display_number: Mapped[int] = mapped_column(Integer, nullable=False)
    title: Mapped[str] = mapped_column(String(255), nullable=False, default="Untitled Scene")
    status: Mapped[str] = mapped_column(
        String(32), CheckConstraint(STATUS_CHECK), nullable=False, default="draft"
    )
    spec_ref: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)
    spec_versions: Mapped[list[dict[str, object]]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=list
    )


class Shot(IdentityRevisionMixin, Base):
    __tablename__ = "shots"
    __table_args__ = (
        ForeignKeyConstraint(
            ["scene_id", "project_id", "episode_id"],
            ["scenes.id", "scenes.project_id", "scenes.episode_id"],
            name="fk_shots_scene_project_episode",
        ),
        UniqueConstraint("scene_id", "display_number", name="uq_shot_scene_number"),
        CheckConstraint("display_number > 0", name="ck_shots_display_number_positive"),
    )

    scene_id: Mapped[str] = mapped_column(ForeignKey("scenes.id"), nullable=False, index=True)
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    episode_id: Mapped[str] = mapped_column(ForeignKey("episodes.id"), nullable=False, index=True)
    display_number: Mapped[int] = mapped_column(Integer, nullable=False)
    status: Mapped[str] = mapped_column(
        String(32), CheckConstraint(STATUS_CHECK), nullable=False, default="draft"
    )
    spec_ref: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)
    spec_versions: Mapped[list[dict[str, object]]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=list
    )
    continuity_snapshot: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)
    continuity_task_refs: Mapped[list[dict[str, object]]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=list
    )
    current_image: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)
    current_video: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)


class SceneOrderState(Base):
    __tablename__ = "scene_order_states"

    episode_id: Mapped[str] = mapped_column(ForeignKey("episodes.id"), primary_key=True)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)


class SceneShotHandoffAck(Base):
    __tablename__ = "scene_shot_handoff_acks"
    __table_args__ = (UniqueConstraint("handoff_id", name="uq_scene_shot_handoff_ack_handoff"),)

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    handoff_id: Mapped[str] = mapped_column(String(255), nullable=False)
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    episode_id: Mapped[str] = mapped_column(ForeignKey("episodes.id"), nullable=False, index=True)
    payload_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    correlation_id: Mapped[str] = mapped_column(String(255), nullable=False)
    scene_ids: Mapped[list[str]] = mapped_column(JSON_DOCUMENT, nullable=False)
    shot_ids: Mapped[list[str]] = mapped_column(JSON_DOCUMENT, nullable=False)
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), server_default=func.now())


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
    source_type: Mapped[str] = mapped_column(String(32), nullable=False, default="imported")
    catalog_role: Mapped[str | None] = mapped_column(String(32))
    tags: Mapped[list[str]] = mapped_column(JSON_DOCUMENT, nullable=False, default=list)
    authorization_status: Mapped[str] = mapped_column(String(32), nullable=False, default="unknown")
    copyright_owner: Mapped[str | None] = mapped_column(String(255))
    license_label: Mapped[str | None] = mapped_column(String(255))
    license_reference: Mapped[str | None] = mapped_column(String(1024))


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


class AssetVersionReservation(Base):
    __tablename__ = "asset_version_reservations"
    __table_args__ = (
        ForeignKeyConstraint(
            ["asset_id", "project_id"],
            ["assets.id", "assets.project_id"],
            name="fk_asset_reservation_asset_project",
        ),
        UniqueConstraint("operation_key", name="uq_asset_reservation_operation_key"),
        UniqueConstraint("asset_id", "fingerprint", name="uq_asset_reservation_fingerprint"),
        CheckConstraint(
            "status IN ('reserved','registered','cancelled','failed')",
            name="ck_asset_reservation_status",
        ),
        CheckConstraint("revision >= 1", name="ck_asset_reservation_revision"),
        CheckConstraint("declared_size_bytes >= 0", name="ck_asset_reservation_declared_size"),
        CheckConstraint(
            hex64_check("declared_checksum"), name="ck_asset_reservation_checksum_hex64"
        ),
        CheckConstraint(hex64_check("fingerprint"), name="ck_asset_reservation_fingerprint_hex64"),
        CheckConstraint(
            hex64_check("storage_profile_snapshot_hash"),
            name="ck_asset_reservation_profile_hash_hex64",
        ),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    asset_id: Mapped[str] = mapped_column(ForeignKey("assets.id"), nullable=False, index=True)
    operation_key: Mapped[str] = mapped_column(String(512), nullable=False)
    fingerprint: Mapped[str] = mapped_column(String(64), nullable=False)
    status: Mapped[str] = mapped_column(String(32), nullable=False, default="reserved")
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    registered_version_id: Mapped[str | None] = mapped_column(ForeignKey("asset_versions.id"))
    expected_asset_revision: Mapped[int] = mapped_column(Integer, nullable=False)
    declared_kind: Mapped[str] = mapped_column(String(32), nullable=False)
    declared_mime_type: Mapped[str] = mapped_column(String(255), nullable=False)
    declared_size_bytes: Mapped[int] = mapped_column(BigInteger, nullable=False)
    declared_checksum: Mapped[str] = mapped_column(String(64), nullable=False)
    storage_profile_id: Mapped[str] = mapped_column(String(255), nullable=False)
    storage_profile_revision: Mapped[int] = mapped_column(Integer, nullable=False)
    storage_profile_snapshot_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    admission_refs: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)
    upload_key: Mapped[str] = mapped_column(String(1024), nullable=False)
    diagnostic: Mapped[str | None] = mapped_column(String(255))
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
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


class PublishedWorkflowVersionModel(IdentityRevisionMixin, Base):
    __tablename__ = "published_workflow_versions"
    __table_args__ = (
        UniqueConstraint(
            "project_id", "template_key", "version_number", name="uq_published_workflow_source"
        ),
        UniqueConstraint("id", "project_id", name="uq_published_workflow_id_project"),
        CheckConstraint("revision >= 1", name="ck_published_workflow_revision_positive"),
        CheckConstraint("version_number >= 1", name="ck_published_workflow_version_positive"),
        CheckConstraint("status = 'published'", name="ck_published_workflow_status"),
        CheckConstraint("scope_type = 'project'", name="ck_published_workflow_scope_type"),
        CheckConstraint(hex64_check("content_hash"), name="ck_published_workflow_content_hash"),
    )

    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    template_key: Mapped[str] = mapped_column(String(128), nullable=False)
    version_number: Mapped[int] = mapped_column(Integer, nullable=False)
    status: Mapped[str] = mapped_column(String(32), nullable=False)
    scope_type: Mapped[str] = mapped_column(String(32), nullable=False)
    scope_ids: Mapped[list[str]] = mapped_column(JSON_DOCUMENT, nullable=False)
    definition: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)
    content_hash: Mapped[str] = mapped_column(String(64), nullable=False)


class ProjectDefaultWorkflowBindingModel(IdentityRevisionMixin, Base):
    __tablename__ = "project_default_workflow_bindings"
    __table_args__ = (
        ForeignKeyConstraint(
            ["workflow_version_id", "project_id"],
            ["published_workflow_versions.id", "published_workflow_versions.project_id"],
            name="fk_default_workflow_binding_source_project",
        ),
        UniqueConstraint("project_id", name="uq_default_workflow_binding_project"),
        CheckConstraint("revision >= 1", name="ck_default_workflow_binding_revision"),
        CheckConstraint(
            "template_key = 'drama-mvp-a-default'", name="ck_default_workflow_template_key"
        ),
        CheckConstraint(
            hex64_check("workflow_content_hash"), name="ck_default_workflow_content_hash"
        ),
    )

    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    workflow_version_id: Mapped[str] = mapped_column(String(36), nullable=False)
    workflow_content_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    template_key: Mapped[str] = mapped_column(String(128), nullable=False)


class WorkflowRunModel(IdentityRevisionMixin, Base):
    __tablename__ = "workflow_runs"
    __table_args__ = (
        CheckConstraint(
            "status IN ('queued', 'running', 'waiting_review', 'succeeded', 'failed', "
            "'cancel_requested', 'cancelled')",
            name="ck_workflow_runs_status",
        ),
        CheckConstraint("revision >= 1", name="ck_workflow_runs_revision_positive"),
    )

    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    status: Mapped[str] = mapped_column(String(32), nullable=False)
    document: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False, default=dict)
    workflow_version_id: Mapped[str | None] = mapped_column(
        ForeignKey("published_workflow_versions.id"), nullable=True, index=True
    )
    rerun_of_run_id: Mapped[str | None] = mapped_column(ForeignKey("workflow_runs.id"))
    predecessor_run_id: Mapped[str | None] = mapped_column(ForeignKey("workflow_runs.id"))
    input_snapshot: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)
    selection_snapshot: Mapped[dict[str, object]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=dict
    )
    source_snapshot: Mapped[dict[str, object]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=dict
    )


class WorkflowNodeRunModel(IdentityRevisionMixin, Base):
    __tablename__ = "workflow_node_runs"
    __table_args__ = (
        UniqueConstraint("run_id", "logical_operation", name="uq_node_run_logical_operation"),
        UniqueConstraint("id", "run_id", name="uq_node_run_id_run"),
        CheckConstraint(
            "status IN ('pending', 'running', 'waiting_review', 'succeeded', 'failed', "
            "'cancel_requested', 'cancelled', 'skipped')",
            name="ck_node_run_status",
        ),
        CheckConstraint("revision >= 1", name="ck_node_run_revision_positive"),
        CheckConstraint(
            "submission_state IN ('not_submitted', 'submitted', "
            "'submission_unknown', 'reconciled')",
            name="ck_node_run_submission_state",
        ),
    )

    run_id: Mapped[str] = mapped_column(ForeignKey("workflow_runs.id"), nullable=False, index=True)
    node_key: Mapped[str] = mapped_column(String(128), nullable=False)
    status: Mapped[str] = mapped_column(String(32), nullable=False)
    logical_operation: Mapped[str] = mapped_column(String(255), nullable=False)
    scope_refs: Mapped[list[dict[str, object]]] = mapped_column(JSON_DOCUMENT, nullable=False)
    admission_refs: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)
    output_evidence: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)
    failure: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)
    submission_state: Mapped[str] = mapped_column(String(32), nullable=False)
    execution_route: Mapped[str] = mapped_column(String(32), nullable=False, default="legacy")
    workflow_type: Mapped[str] = mapped_column(String(128), nullable=False, default="phase_one_run")
    task_queue: Mapped[str] = mapped_column(String(128), nullable=False, default="agent-tasks")
    operation_snapshot: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)


class WorkflowRunInputSnapshotModel(IdentityRevisionMixin, Base):
    __tablename__ = "workflow_run_input_snapshots"
    __table_args__ = (
        ForeignKeyConstraint(
            ["workflow_version_id", "project_id"],
            ["published_workflow_versions.id", "published_workflow_versions.project_id"],
            name="fk_run_snapshot_source_project",
        ),
        UniqueConstraint("run_id", "id", name="uq_run_input_snapshot_run_id"),
        CheckConstraint("revision >= 1", name="ck_run_input_snapshot_revision"),
        CheckConstraint(hex64_check("workflow_content_hash"), name="ck_run_snapshot_content_hash"),
    )

    run_id: Mapped[str] = mapped_column(ForeignKey("workflow_runs.id"), nullable=False, index=True)
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    workflow_version_id: Mapped[str] = mapped_column(String(36), nullable=False)
    workflow_content_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    scope_refs: Mapped[list[dict[str, object]]] = mapped_column(JSON_DOCUMENT, nullable=False)
    owner_refs: Mapped[list[dict[str, object]]] = mapped_column(JSON_DOCUMENT, nullable=False)
    selection_snapshot: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)
    source_snapshot: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)
    node_inputs: Mapped[list[dict[str, object]]] = mapped_column(JSON_DOCUMENT, nullable=False)
    runnable: Mapped[bool] = mapped_column(nullable=False)
    diagnostic: Mapped[str | None] = mapped_column(String(255))


class WorkflowRunEventModel(IdentityRevisionMixin, Base):
    __tablename__ = "workflow_run_events"
    __table_args__ = (
        ForeignKeyConstraint(
            ["node_run_id", "run_id"],
            ["workflow_node_runs.id", "workflow_node_runs.run_id"],
            name="fk_run_event_node_run",
        ),
        UniqueConstraint("run_id", "sequence", name="uq_workflow_run_event_sequence"),
        CheckConstraint("sequence >= 1", name="ck_workflow_run_event_sequence_positive"),
        CheckConstraint("revision = 1", name="ck_workflow_run_event_immutable"),
    )

    run_id: Mapped[str] = mapped_column(ForeignKey("workflow_runs.id"), nullable=False, index=True)
    node_run_id: Mapped[str | None] = mapped_column(String(36))
    sequence: Mapped[int] = mapped_column(Integer, nullable=False)
    event_type: Mapped[str] = mapped_column(String(128), nullable=False)
    correlation_id: Mapped[str] = mapped_column(String(255), nullable=False)
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)
    retention_policy: Mapped[str] = mapped_column(String(64), nullable=False)
    retention_version: Mapped[str] = mapped_column(String(32), nullable=False)
    hold: Mapped[bool] = mapped_column(nullable=False)


class WorkflowIdempotencyKeyModel(IdentityRevisionMixin, Base):
    __tablename__ = "workflow_idempotency_keys"
    __table_args__ = (
        UniqueConstraint("key_kind", "idempotency_key", name="uq_workflow_idempotency_key"),
        CheckConstraint("revision = 1", name="ck_workflow_idempotency_immutable"),
        CheckConstraint(hex64_check("request_fingerprint"), name="ck_workflow_idempotency_hash"),
    )

    key_kind: Mapped[str] = mapped_column(String(32), nullable=False)
    idempotency_key: Mapped[str] = mapped_column(String(255), nullable=False)
    run_id: Mapped[str] = mapped_column(ForeignKey("workflow_runs.id"), nullable=False)
    request_fingerprint: Mapped[str] = mapped_column(String(64), nullable=False)


class WorkflowTemporalStartModel(IdentityRevisionMixin, Base):
    __tablename__ = "workflow_temporal_starts"
    __table_args__ = (
        ForeignKeyConstraint(
            ["node_run_id", "run_id"],
            ["workflow_node_runs.id", "workflow_node_runs.run_id"],
            name="fk_temporal_start_node_run",
        ),
        UniqueConstraint("workflow_id", name="uq_temporal_start_workflow_id"),
        UniqueConstraint("run_id", "logical_operation", name="uq_temporal_start_run_operation"),
        CheckConstraint(
            "status IN ('pending', 'started', 'submission_unknown', 'reconciled')",
            name="ck_temporal_start_status",
        ),
        CheckConstraint(hex64_check("request_fingerprint"), name="ck_temporal_start_hash"),
    )

    run_id: Mapped[str] = mapped_column(ForeignKey("workflow_runs.id"), nullable=False)
    node_run_id: Mapped[str] = mapped_column(String(36), nullable=False)
    logical_operation: Mapped[str] = mapped_column(String(255), nullable=False)
    workflow_id: Mapped[str] = mapped_column(String(512), nullable=False)
    request_fingerprint: Mapped[str] = mapped_column(String(64), nullable=False)
    status: Mapped[str] = mapped_column(String(32), nullable=False)


class WorkflowBudgetGateModel(IdentityRevisionMixin, Base):
    __tablename__ = "workflow_budget_gates"
    __table_args__ = (
        ForeignKeyConstraint(
            ["node_run_id", "run_id"],
            ["workflow_node_runs.id", "workflow_node_runs.run_id"],
            name="fk_budget_gate_node_run",
        ),
        UniqueConstraint("run_id", "logical_operation", name="uq_budget_gate_run_operation"),
        CheckConstraint("revision >= 1", name="ck_budget_gate_revision"),
        CheckConstraint("batch_size >= 1", name="ck_budget_gate_batch_size"),
        CheckConstraint("cost_status IN ('known', 'unknown')", name="ck_budget_gate_cost_status"),
        CheckConstraint(
            "status IN ('pending_confirmation', 'confirmed')", name="ck_budget_gate_status"
        ),
    )

    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False)
    run_id: Mapped[str] = mapped_column(ForeignKey("workflow_runs.id"), nullable=False)
    node_run_id: Mapped[str] = mapped_column(String(36), nullable=False)
    logical_operation: Mapped[str] = mapped_column(String(255), nullable=False)
    request_fingerprint: Mapped[str] = mapped_column(String(255), nullable=False)
    operation_kind: Mapped[str] = mapped_column(String(128), nullable=False)
    batch_size: Mapped[int] = mapped_column(Integer, nullable=False)
    cost_status: Mapped[str] = mapped_column(String(16), nullable=False)
    estimated_cost: Mapped[str | None] = mapped_column(String(64))
    currency: Mapped[str | None] = mapped_column(String(16))
    threshold_snapshot_id: Mapped[str | None] = mapped_column(String(255))
    threshold_revision: Mapped[int | None] = mapped_column(Integer)
    status: Mapped[str] = mapped_column(String(32), nullable=False)
    confirmation_id: Mapped[str | None] = mapped_column(String(255))
    user_uuid: Mapped[str | None] = mapped_column(String(36))
    retention_policy: Mapped[str] = mapped_column(String(64), nullable=False)
    retention_version: Mapped[str] = mapped_column(String(32), nullable=False)
    hold: Mapped[bool] = mapped_column(nullable=False)


class WorkflowOutboxEventModel(IdentityRevisionMixin, Base):
    __tablename__ = "workflow_outbox_events"
    __table_args__ = (
        UniqueConstraint("run_event_id", name="uq_workflow_outbox_run_event"),
        CheckConstraint("revision >= 1", name="ck_workflow_outbox_revision"),
        CheckConstraint("status IN ('pending', 'published')", name="ck_workflow_outbox_status"),
    )

    run_id: Mapped[str] = mapped_column(ForeignKey("workflow_runs.id"), nullable=False)
    run_event_id: Mapped[str] = mapped_column(ForeignKey("workflow_run_events.id"), nullable=False)
    event_type: Mapped[str] = mapped_column(String(128), nullable=False)
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)
    status: Mapped[str] = mapped_column(String(32), nullable=False)


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
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    approval: Mapped[str] = mapped_column(String(64), nullable=False, default="pending")
    feature_gate: Mapped[str] = mapped_column(String(32), nullable=False, default="MVP-A")
    adapter_installed: Mapped[bool] = mapped_column(nullable=False, default=False)
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), server_default=func.now())


class ProviderProfile(Base):
    __tablename__ = "provider_profiles"

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    provider_id: Mapped[str] = mapped_column(ForeignKey("providers.id"), nullable=False, index=True)
    name: Mapped[str] = mapped_column(String(255), nullable=False)
    enabled: Mapped[bool] = mapped_column(nullable=False, default=True)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    adapter_identity: Mapped[str] = mapped_column(
        String(128), nullable=False, default="local_workspace"
    )
    explicit_live_opt_in: Mapped[bool] = mapped_column(nullable=False, default=False)
    credential_status: Mapped[str] = mapped_column(
        String(32), nullable=False, default="unconfigured"
    )
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
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
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
    profile_id: Mapped[str | None] = mapped_column(ForeignKey("provider_profiles.id"))
    credential_id: Mapped[str | None] = mapped_column(String(255))
    algorithm: Mapped[str | None] = mapped_column(String(64))
    aad_version: Mapped[str | None] = mapped_column(String(32))
    key_version: Mapped[str] = mapped_column(String(64), nullable=False)
    masked_prefix: Mapped[str | None] = mapped_column(String(32))
    last4: Mapped[str | None] = mapped_column(String(4))
    ciphertext: Mapped[str] = mapped_column(Text, nullable=False)
    nonce: Mapped[str] = mapped_column(String(128), nullable=False)
    tag: Mapped[str] = mapped_column(String(128), nullable=False)


class CapabilitySnapshotModel(Base):
    __tablename__ = "capability_snapshots"

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    provider_id: Mapped[str] = mapped_column(ForeignKey("providers.id"), nullable=False)
    profile_id: Mapped[str] = mapped_column(ForeignKey("provider_profiles.id"), nullable=False)
    model_id: Mapped[str | None] = mapped_column(ForeignKey("models.id"))
    operation: Mapped[str] = mapped_column(String(128), nullable=False)
    runnable: Mapped[bool] = mapped_column(nullable=False)
    capabilities: Mapped[list[str]] = mapped_column(JSON_DOCUMENT, nullable=False, default=list)
    captured_at: Mapped[str] = mapped_column(String(64), nullable=False)
    retention_policy: Mapped[str] = mapped_column(String(64), nullable=False)
    retention_version: Mapped[str] = mapped_column(String(32), nullable=False)
    hold: Mapped[bool] = mapped_column(nullable=False, default=False)


class SkillRevisionModel(Base):
    __tablename__ = "skill_revisions"

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    name: Mapped[str] = mapped_column(String(255), nullable=False)
    version: Mapped[str] = mapped_column(String(64), nullable=False)
    provenance: Mapped[str] = mapped_column(String(64), nullable=False)
    approval: Mapped[str] = mapped_column(String(64), nullable=False)
    enabled: Mapped[bool] = mapped_column(nullable=False)
    source_identity: Mapped[str] = mapped_column(Text, nullable=False)
    digest: Mapped[str] = mapped_column(String(128), nullable=False)
    source_type: Mapped[str] = mapped_column(String(64), nullable=False)
    license_status: Mapped[str] = mapped_column(String(64), nullable=False)
    capabilities: Mapped[list[str]] = mapped_column(JSON_DOCUMENT, nullable=False, default=list)


class ProviderCallModel(Base):
    __tablename__ = "provider_calls"
    __table_args__ = (
        UniqueConstraint("run_id", "logical_operation", name="uq_provider_call_run_operation"),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False)
    run_id: Mapped[str] = mapped_column(String(36), nullable=False)
    node_run_id: Mapped[str | None] = mapped_column(String(36))
    logical_operation: Mapped[str] = mapped_column(String(255), nullable=False)
    operation: Mapped[str] = mapped_column(String(128), nullable=False)
    provider_id: Mapped[str] = mapped_column(ForeignKey("providers.id"), nullable=False)
    profile_id: Mapped[str] = mapped_column(ForeignKey("provider_profiles.id"), nullable=False)
    model_id: Mapped[str] = mapped_column(ForeignKey("models.id"), nullable=False)
    capability_snapshot_id: Mapped[str | None] = mapped_column(
        ForeignKey("capability_snapshots.id")
    )
    request_fingerprint: Mapped[str] = mapped_column(String(128), nullable=False)
    status: Mapped[str] = mapped_column(String(32), nullable=False)
    cost_status: Mapped[str] = mapped_column(String(16), nullable=False)
    cost_value: Mapped[str | None] = mapped_column(String(64))
    cost_currency: Mapped[str | None] = mapped_column(String(16))
    cost_source: Mapped[str | None] = mapped_column(String(255))
    provider_request_id: Mapped[str | None] = mapped_column(String(255))
    native_usage: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)
    failure_code: Mapped[str | None] = mapped_column(String(128))
    outbound_correlation: Mapped[str | None] = mapped_column(String(128))
    lookup_outcome: Mapped[str] = mapped_column(String(32), nullable=False, default="not_attempted")
    remote_lookup_protocol: Mapped[str | None] = mapped_column(String(128))
    remote_lookup_binding: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)
    admission_refs: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)
    retention_policy: Mapped[str] = mapped_column(String(64), nullable=False)
    retention_version: Mapped[str] = mapped_column(String(32), nullable=False)
    hold: Mapped[bool] = mapped_column(nullable=False, default=False)


class VideoOperationModel(Base):
    """Agnes async intent/state; poll observations remain evidence, not RunEvent history."""

    __tablename__ = "video_operations"
    __table_args__ = (
        UniqueConstraint("run_id", "logical_operation", name="uq_video_operation_run_logical"),
        CheckConstraint(
            "status IN ('pending','submitted','running','submission_unknown',"
            "'succeeded','failed','cancelled')",
            name="ck_video_operation_status",
        ),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    episode_id: Mapped[str] = mapped_column(String(36), nullable=False)
    target_id: Mapped[str] = mapped_column(String(36), nullable=False)
    asset_id: Mapped[str] = mapped_column(String(36), nullable=False)
    run_id: Mapped[str] = mapped_column(String(36), nullable=False, index=True)
    logical_operation: Mapped[str] = mapped_column(String(255), nullable=False)
    provider_id: Mapped[str] = mapped_column(ForeignKey("providers.id"), nullable=False)
    profile_id: Mapped[str] = mapped_column(ForeignKey("provider_profiles.id"), nullable=False)
    model_id: Mapped[str] = mapped_column(ForeignKey("models.id"), nullable=False)
    capability_snapshot_id: Mapped[str] = mapped_column(
        ForeignKey("capability_snapshots.id"), nullable=False
    )
    source_asset_version_id: Mapped[str] = mapped_column(String(36), nullable=False)
    source_asset_version_revision: Mapped[int] = mapped_column(Integer, nullable=False)
    source_asset_version_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    source_candidate_id: Mapped[str | None] = mapped_column(String(36))
    source_provenance: Mapped[str | None] = mapped_column(String(128))
    shot_spec_id: Mapped[str] = mapped_column(String(36), nullable=False)
    shot_spec_revision: Mapped[int] = mapped_column(Integer, nullable=False)
    shot_spec_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    duration_seconds: Mapped[float] = mapped_column(nullable=False)
    aspect_ratio: Mapped[str] = mapped_column(String(16), nullable=False)
    status: Mapped[str] = mapped_column(String(32), nullable=False, default="pending")
    provider_request_id: Mapped[str | None] = mapped_column(String(255))
    outbound_correlation: Mapped[str | None] = mapped_column(String(128))
    lookup_outcome: Mapped[str] = mapped_column(String(32), nullable=False, default="not_attempted")
    admission_refs: Mapped[dict[str, object] | None] = mapped_column(JSON_DOCUMENT)
    cancel_requested: Mapped[bool] = mapped_column(nullable=False, default=False)
    observation_fingerprints: Mapped[list[str]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=list
    )
    retention_policy: Mapped[str] = mapped_column(
        String(64), nullable=False, default="long-term-audit"
    )
    retention_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1")
    hold: Mapped[bool] = mapped_column(nullable=False, default=False)


class VideoTakeCandidateModel(Base):
    """Immutable verified result candidate; review revision is the only mutable fact."""

    __tablename__ = "video_take_candidates"
    __table_args__ = (
        UniqueConstraint("run_id", "logical_operation", name="uq_video_candidate_run_logical"),
        CheckConstraint(
            "status IN ('pending_review','accepted','rejected','stale')",
            name="ck_video_candidate_status",
        ),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    episode_id: Mapped[str] = mapped_column(String(36), nullable=False)
    target_id: Mapped[str] = mapped_column(String(36), nullable=False)
    run_id: Mapped[str] = mapped_column(String(36), nullable=False, index=True)
    logical_operation: Mapped[str] = mapped_column(String(255), nullable=False)
    source_asset_version_id: Mapped[str] = mapped_column(String(36), nullable=False)
    source_asset_version_revision: Mapped[int] = mapped_column(Integer, nullable=False)
    source_asset_version_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    source_candidate_id: Mapped[str | None] = mapped_column(String(36))
    source_provenance: Mapped[str] = mapped_column(
        String(128), nullable=False, default="agnes_video"
    )
    shot_spec_id: Mapped[str] = mapped_column(String(36), nullable=False)
    shot_spec_revision: Mapped[int] = mapped_column(Integer, nullable=False)
    shot_spec_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    duration_seconds: Mapped[float] = mapped_column(nullable=False)
    aspect_ratio: Mapped[str] = mapped_column(String(16), nullable=False)
    asset_version_id: Mapped[str] = mapped_column(String(36), nullable=False)
    asset_version_revision: Mapped[int] = mapped_column(Integer, nullable=False)
    asset_version_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    provider_request_id: Mapped[str | None] = mapped_column(String(255))
    status: Mapped[str] = mapped_column(String(32), nullable=False, default="pending_review")
    retention_policy: Mapped[str] = mapped_column(
        String(64), nullable=False, default="long-term-audit"
    )
    retention_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1")
    hold: Mapped[bool] = mapped_column(nullable=False, default=False)


class AssetEditSessionModel(Base):
    __tablename__ = "asset_edit_sessions"
    __table_args__ = (UniqueConstraint("id", "project_id", name="uq_asset_edit_session_project"),)

    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    episode_id: Mapped[str] = mapped_column(ForeignKey("episodes.id"), nullable=False, index=True)
    status: Mapped[str] = mapped_column(String(32), nullable=False, default="active")
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)


class AssetEditConversationModel(Base):
    __tablename__ = "asset_edit_conversations"
    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    episode_id: Mapped[str] = mapped_column(ForeignKey("episodes.id"), nullable=False, index=True)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")


class AssetEditMessageModel(Base):
    __tablename__ = "asset_edit_messages"
    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    session_id: Mapped[str] = mapped_column(
        ForeignKey("asset_edit_sessions.id"), nullable=False, index=True
    )
    sequence: Mapped[int] = mapped_column(Integer, nullable=False)
    role: Mapped[str] = mapped_column(String(16), nullable=False)
    content_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    status: Mapped[str] = mapped_column(String(16), nullable=False)
    correlation_id: Mapped[str] = mapped_column(String(255), nullable=False)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    __table_args__ = (
        UniqueConstraint("session_id", "sequence", name="uq_asset_edit_message_sequence"),
    )


class AssetEditTurnModel(Base):
    __tablename__ = "asset_edit_turns"
    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    session_id: Mapped[str] = mapped_column(
        ForeignKey("asset_edit_sessions.id"), nullable=False, index=True
    )
    sequence: Mapped[int] = mapped_column(Integer, nullable=False)
    user_message_id: Mapped[str] = mapped_column(
        ForeignKey("asset_edit_messages.id"), nullable=False
    )
    agent_message_id: Mapped[str | None] = mapped_column(String(36))
    status: Mapped[str] = mapped_column(String(16), nullable=False, default="pending")
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    __table_args__ = (
        UniqueConstraint("session_id", "sequence", name="uq_asset_edit_turn_sequence"),
    )


class AssetEditPlanModel(Base):
    __tablename__ = "asset_edit_plans"
    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    episode_id: Mapped[str] = mapped_column(ForeignKey("episodes.id"), nullable=False, index=True)
    status: Mapped[str] = mapped_column(String(32), nullable=False, default="pending_review")
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)


class AssetEditExecutionModel(Base):
    __tablename__ = "asset_edit_executions"
    __table_args__ = (
        UniqueConstraint(
            "run_id", "node_run_id", "logical_operation", name="uq_asset_edit_execution_key"
        ),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    plan_id: Mapped[str] = mapped_column(
        ForeignKey("asset_edit_plans.id"), nullable=False, index=True
    )
    run_id: Mapped[str] = mapped_column(String(36), nullable=False, index=True)
    node_run_id: Mapped[str] = mapped_column(String(36), nullable=False)
    logical_operation: Mapped[str] = mapped_column(String(255), nullable=False)
    status: Mapped[str] = mapped_column(String(32), nullable=False, default="queued")
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)


class AssetEditCandidateModel(Base):
    __tablename__ = "asset_edit_candidates"
    __table_args__ = (UniqueConstraint("id", "project_id", name="uq_asset_edit_candidate_project"),)

    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    plan_id: Mapped[str] = mapped_column(
        ForeignKey("asset_edit_plans.id"), nullable=False, index=True
    )
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    status: Mapped[str] = mapped_column(String(32), nullable=False, default="pending_review")
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)


class AcceptDecisionModel(Base):
    __tablename__ = "asset_edit_accept_decisions"
    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    candidate_id: Mapped[str] = mapped_column(
        ForeignKey("asset_edit_candidates.id"), nullable=False, index=True
    )
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    action: Mapped[str] = mapped_column(String(32), nullable=False)
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)


class EditImpactModel(Base):
    __tablename__ = "asset_edit_impacts"
    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    plan_id: Mapped[str] = mapped_column(
        ForeignKey("asset_edit_plans.id"), nullable=False, index=True
    )
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    status: Mapped[str] = mapped_column(String(32), nullable=False)
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)


class ProviderQuotaSnapshotModel(Base):
    __tablename__ = "provider_quota_snapshots"

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    provider_id: Mapped[str] = mapped_column(ForeignKey("providers.id"), nullable=False)
    profile_id: Mapped[str] = mapped_column(ForeignKey("provider_profiles.id"), nullable=False)
    operation: Mapped[str] = mapped_column(String(128), nullable=False)
    status: Mapped[str] = mapped_column(String(16), nullable=False)
    remaining: Mapped[int | None] = mapped_column(Integer)
    reset_at: Mapped[str | None] = mapped_column(String(64))
    source: Mapped[str] = mapped_column(String(255), nullable=False)
    captured_at: Mapped[str] = mapped_column(String(64), nullable=False)


class ProviderOperationPolicyModel(Base):
    __tablename__ = "provider_operation_policies"

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    profile_id: Mapped[str] = mapped_column(ForeignKey("provider_profiles.id"), nullable=False)
    operation: Mapped[str] = mapped_column(String(128), nullable=False)
    max_concurrency: Mapped[int] = mapped_column(Integer, nullable=False)
    rate_limit: Mapped[int] = mapped_column(Integer, nullable=False)
    rate_window_seconds: Mapped[int] = mapped_column(Integer, nullable=False)


class CostConfirmationModel(Base):
    __tablename__ = "cost_confirmations"

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False)
    run_id: Mapped[str] = mapped_column(String(36), nullable=False)
    logical_operation: Mapped[str] = mapped_column(String(255), nullable=False)
    request_fingerprint: Mapped[str] = mapped_column(String(128), nullable=False)
    user_uuid: Mapped[str] = mapped_column(String(36), nullable=False)
    threshold_snapshot_id: Mapped[str | None] = mapped_column(String(255))
    threshold_revision: Mapped[int | None] = mapped_column(Integer)
    estimated_cost: Mapped[str | None] = mapped_column(String(64))
    cost_status: Mapped[str] = mapped_column(String(16), nullable=False)
    operation_kind: Mapped[str] = mapped_column(String(128), nullable=False)
    batch_size: Mapped[int] = mapped_column(Integer, nullable=False)
    retention_policy: Mapped[str] = mapped_column(String(64), nullable=False)
    retention_version: Mapped[str] = mapped_column(String(32), nullable=False)
    hold: Mapped[bool] = mapped_column(nullable=False, default=False)


class ModelSyncCandidateModel(Base):
    __tablename__ = "model_sync_candidates"

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    profile_id: Mapped[str] = mapped_column(ForeignKey("provider_profiles.id"), nullable=False)
    remote_models: Mapped[list[str]] = mapped_column(JSON_DOCUMENT, nullable=False)
    added: Mapped[list[str]] = mapped_column(JSON_DOCUMENT, nullable=False)
    removed: Mapped[list[str]] = mapped_column(JSON_DOCUMENT, nullable=False)
    changed: Mapped[list[str]] = mapped_column(JSON_DOCUMENT, nullable=False)
    status: Mapped[str] = mapped_column(String(16), nullable=False)


class SkillAccessAuditModel(Base):
    __tablename__ = "skill_access_audits"

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    skill_revision_id: Mapped[str] = mapped_column(ForeignKey("skill_revisions.id"), nullable=False)
    run_id: Mapped[str] = mapped_column(String(36), nullable=False)
    node_run_id: Mapped[str] = mapped_column(String(36), nullable=False)
    access: Mapped[str] = mapped_column(String(32), nullable=False)
    allowed: Mapped[bool] = mapped_column(nullable=False)
    reason: Mapped[str] = mapped_column(String(255), nullable=False)


class PhaseOneDocument(Base):
    """阶段一 owner 文档账本。

    该表只保存可重放的结构化事实，不保存媒体正文、凭据明文或 Provider 原始响应。
    owner/collection 作为逻辑边界，revision 用于 command 级 CAS。
    """

    __tablename__ = "phase_one_documents"
    __table_args__ = (
        UniqueConstraint("owner", "collection", name="uq_phase_one_document_owner_collection"),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    owner: Mapped[str] = mapped_column(String(64), nullable=False)
    collection: Mapped[str] = mapped_column(String(128), nullable=False)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=0)
    document: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False, default=dict)
    retention_policy: Mapped[str] = mapped_column(String(64), nullable=False, default="phase-one")
    retention_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1")
    hold: Mapped[bool] = mapped_column(nullable=False, default=False)
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), server_default=func.now())
    updated_at: Mapped[datetime] = mapped_column(
        DateTime(timezone=True), server_default=func.now(), onupdate=func.now()
    )


class StorageProfileModel(Base):
    __tablename__ = "storage_profiles"
    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    revision: Mapped[int] = mapped_column(Integer, nullable=False, default=1)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    name: Mapped[str] = mapped_column(String(255), nullable=False, default="")
    adapter_key: Mapped[str] = mapped_column(String(32), nullable=False, default="tos")
    endpoint: Mapped[str] = mapped_column(String(1024), nullable=False)
    bucket: Mapped[str] = mapped_column(String(255), nullable=False)
    region: Mapped[str] = mapped_column(String(128), nullable=False)
    private_bucket: Mapped[bool] = mapped_column(nullable=False, default=True)
    enabled: Mapped[bool] = mapped_column(nullable=False, default=False)
    bucket_binding_id: Mapped[str] = mapped_column(String(255), nullable=False, default="")
    credential_status: Mapped[str] = mapped_column(
        String(32), nullable=False, default="unconfigured"
    )
    credential_ref: Mapped[str | None] = mapped_column(String(255))
    connect_timeout_ms: Mapped[int] = mapped_column(Integer, nullable=False, default=10000)
    read_timeout_ms: Mapped[int] = mapped_column(Integer, nullable=False, default=30000)
    write_timeout_ms: Mapped[int] = mapped_column(Integer, nullable=False, default=60000)
    presign_max_ttl_seconds: Mapped[int] = mapped_column(Integer, nullable=False, default=900)
    project_scope: Mapped[list[str]] = mapped_column(JSON_DOCUMENT, nullable=False, default=list)
    masked_credential_summary: Mapped[str | None] = mapped_column(String(255))


class StorageBucketBindingModel(Base):
    __tablename__ = "storage_bucket_bindings"
    __table_args__ = (
        UniqueConstraint("profile_id", "bucket", name="uq_storage_bucket_profile_bucket"),
        CheckConstraint("private_bucket = true", name="ck_storage_bucket_private"),
    )
    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    profile_id: Mapped[str] = mapped_column(ForeignKey("storage_profiles.id"), nullable=False)
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False)
    bucket: Mapped[str] = mapped_column(String(255), nullable=False)
    region: Mapped[str] = mapped_column(String(128), nullable=False)
    endpoint: Mapped[str] = mapped_column(String(1024), nullable=False)
    private_bucket: Mapped[bool] = mapped_column(nullable=False, default=True)


class StorageUploadOperationModel(Base):
    __tablename__ = "storage_upload_operations"
    __table_args__ = (
        CheckConstraint(
            "status IN ('active', 'completed', 'aborted', 'unknown', 'failed')",
            name="ck_storage_operation_status",
        ),
    )
    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False)
    profile_id: Mapped[str] = mapped_column(ForeignKey("storage_profiles.id"), nullable=False)
    operation_key: Mapped[str] = mapped_column(String(255), nullable=False, unique=True)
    session_id: Mapped[str | None] = mapped_column(String(255))
    object_key: Mapped[str] = mapped_column(String(1024), nullable=False)
    status: Mapped[str] = mapped_column(String(32), nullable=False, default="active")
    object_ref: Mapped[str | None] = mapped_column(String(1024))
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False, default=dict)


class StorageUploadSessionModel(Base):
    __tablename__ = "storage_upload_sessions"
    __table_args__ = (
        CheckConstraint(
            "status IN ('active', 'completed', 'aborted', 'unknown', 'failed')",
            name="ck_storage_session_status",
        ),
        CheckConstraint(
            "expected_size_bytes IS NULL OR expected_size_bytes >= 0",
            name="ck_storage_session_size",
        ),
    )
    id: Mapped[str] = mapped_column(String(64), primary_key=True)
    operation_id: Mapped[str] = mapped_column(
        ForeignKey("storage_upload_operations.id"), nullable=False, unique=True
    )
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False)
    profile_id: Mapped[str] = mapped_column(ForeignKey("storage_profiles.id"), nullable=False)
    operation_key: Mapped[str] = mapped_column(String(255), nullable=False, unique=True)
    object_key: Mapped[str] = mapped_column(String(1024), nullable=False)
    status: Mapped[str] = mapped_column(String(32), nullable=False, default="active")
    expected_size_bytes: Mapped[int | None] = mapped_column(BigInteger)
    expected_checksum: Mapped[str | None] = mapped_column(String(64))
    expected_mime_type: Mapped[str | None] = mapped_column(String(255))


class StorageUploadPartModel(Base):
    __tablename__ = "storage_upload_parts"
    __table_args__ = (
        CheckConstraint("part_number >= 1", name="ck_storage_part_number"),
        CheckConstraint("size_bytes >= 0", name="ck_storage_part_size"),
    )
    session_id: Mapped[str] = mapped_column(
        ForeignKey("storage_upload_sessions.id"), primary_key=True
    )
    part_number: Mapped[int] = mapped_column(primary_key=True)
    checksum: Mapped[str] = mapped_column(String(64), nullable=False)
    etag: Mapped[str] = mapped_column(String(1024), nullable=False)
    size_bytes: Mapped[int] = mapped_column(BigInteger, nullable=False)


class StoredObjectModel(Base):
    __tablename__ = "stored_objects"
    __table_args__ = (
        UniqueConstraint("profile_id", "object_key", name="uq_stored_object_profile_key"),
        CheckConstraint("size_bytes >= 0", name="ck_stored_object_size"),
    )
    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False)
    profile_id: Mapped[str] = mapped_column(ForeignKey("storage_profiles.id"), nullable=False)
    operation_key: Mapped[str] = mapped_column(String(255), nullable=False, unique=True)
    bucket: Mapped[str] = mapped_column(String(255), nullable=False)
    object_key: Mapped[str] = mapped_column(String(1024), nullable=False)
    size_bytes: Mapped[int] = mapped_column(BigInteger, nullable=False)
    checksum: Mapped[str] = mapped_column(String(64), nullable=False)
    mime_type: Mapped[str] = mapped_column(String(255), nullable=False)
    etag: Mapped[str | None] = mapped_column(String(1024))
    verified: Mapped[bool] = mapped_column(nullable=False, default=False)


class StorageReferenceProofModel(Base):
    __tablename__ = "storage_reference_proofs"
    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False)
    object_id: Mapped[str] = mapped_column(ForeignKey("stored_objects.id"), nullable=False)
    checked_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), nullable=False)
    owner_results: Mapped[dict[str, object]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=dict
    )
    no_references: Mapped[bool] = mapped_column(nullable=False, default=False)


class StorageRecoveryRecordModel(Base):
    __tablename__ = "storage_recovery_records"
    __table_args__ = (
        CheckConstraint(
            "status IN ('reconciliation_required', 'failed', 'aborted', 'resolved')",
            name="ck_storage_recovery_status",
        ),
    )
    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    operation_id: Mapped[str] = mapped_column(
        ForeignKey("storage_upload_operations.id"), nullable=False
    )
    status: Mapped[str] = mapped_column(String(32), nullable=False)
    diagnostic: Mapped[str] = mapped_column(String(255), nullable=False)
    correlation_id: Mapped[str] = mapped_column(String(255), nullable=False)
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False, default=dict)


class AssetBibleModel(IdentityRevisionMixin, Base):
    __tablename__ = "asset_bibles"
    __table_args__ = (
        UniqueConstraint("project_id", name="uq_asset_bibles_project"),
        CheckConstraint("revision >= 1", name="ck_asset_bibles_revision_positive"),
    )

    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    current_version_map: Mapped[dict[str, str]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=dict
    )


class AssetBibleEntryModel(IdentityRevisionMixin, Base):
    __tablename__ = "asset_bible_entries"
    __table_args__ = (
        UniqueConstraint("id", "project_id", name="uq_asset_bible_entries_id_project"),
        CheckConstraint("revision >= 1", name="ck_asset_bible_entries_revision_positive"),
        CheckConstraint(
            "entry_type IN ('character', 'look', 'location', 'scene_visual', 'prop', "
            "'visual_style')",
            name="ck_asset_bible_entries_type",
        ),
    )

    asset_bible_id: Mapped[str] = mapped_column(
        ForeignKey("asset_bibles.id"), nullable=False, index=True
    )
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    entry_type: Mapped[str] = mapped_column(String(32), nullable=False)
    disabled: Mapped[bool] = mapped_column(nullable=False, default=False)
    current_version_id: Mapped[str | None] = mapped_column(String(36))


class AssetBibleEntryVersionModel(IdentityRevisionMixin, Base):
    __tablename__ = "asset_bible_entry_versions"
    __table_args__ = (
        UniqueConstraint("entry_id", "version_number", name="uq_asset_bible_version_number"),
        CheckConstraint("revision = 1", name="ck_asset_bible_versions_immutable_revision"),
        CheckConstraint("version_number >= 1", name="ck_asset_bible_versions_number_positive"),
        CheckConstraint(hex64_check("content_hash"), name="ck_asset_bible_versions_hash"),
    )

    entry_id: Mapped[str] = mapped_column(
        ForeignKey("asset_bible_entries.id"), nullable=False, index=True
    )
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    entry_type: Mapped[str] = mapped_column(String(32), nullable=False)
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)
    version_number: Mapped[int] = mapped_column(Integer, nullable=False)
    actor_uuid: Mapped[str] = mapped_column(String(36), nullable=False)
    reference_asset_version_refs: Mapped[list[dict[str, object]]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=list
    )
    generation_spec_refs: Mapped[list[dict[str, object]]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=list
    )
    content_hash: Mapped[str] = mapped_column(String(64), nullable=False)


class AssetBibleRelationshipModel(Base):
    __tablename__ = "asset_bible_relationships"
    __table_args__ = (
        UniqueConstraint(
            "source_entry_id",
            "target_entry_id",
            "kind",
            name="uq_asset_bible_relationship_edge",
        ),
        CheckConstraint(
            "kind IN ('character_look', 'location_scene_visual', 'related')",
            name="ck_asset_bible_relationship_kind",
        ),
        CheckConstraint("source_entry_id <> target_entry_id", name="ck_asset_bible_no_self_edge"),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    source_entry_id: Mapped[str] = mapped_column(
        ForeignKey("asset_bible_entries.id"), nullable=False
    )
    target_entry_id: Mapped[str] = mapped_column(
        ForeignKey("asset_bible_entries.id"), nullable=False
    )
    kind: Mapped[str] = mapped_column(String(32), nullable=False)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), server_default=func.now())


class ContinuityAssignmentModel(IdentityRevisionMixin, Base):
    __tablename__ = "asset_bible_assignments"
    __table_args__ = (
        UniqueConstraint(
            "project_id", "level", "target_id", "entry_id", name="uq_continuity_assignment_scope"
        ),
        CheckConstraint(
            "level IN ('project', 'episode', 'scene', 'shot')",
            name="ck_continuity_assignment_level",
        ),
        CheckConstraint("revision >= 1", name="ck_continuity_assignment_revision_positive"),
        CheckConstraint("scope_revision >= 1", name="ck_continuity_scope_revision_positive"),
        CheckConstraint("version_revision >= 1", name="ck_continuity_version_revision_positive"),
        CheckConstraint(hex64_check("content_hash"), name="ck_continuity_assignment_hash"),
    )

    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    level: Mapped[str] = mapped_column(String(16), nullable=False)
    target_id: Mapped[str] = mapped_column(String(36), nullable=False, index=True)
    entry_id: Mapped[str] = mapped_column(
        ForeignKey("asset_bible_entries.id"), nullable=False, index=True
    )
    version_id: Mapped[str] = mapped_column(
        ForeignKey("asset_bible_entry_versions.id"), nullable=False
    )
    version_revision: Mapped[int] = mapped_column(Integer, nullable=False)
    content_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    scope_revision: Mapped[int] = mapped_column(Integer, nullable=False)


class ResolvedContinuitySnapshotModel(IdentityRevisionMixin, Base):
    __tablename__ = "resolved_continuity_snapshots"
    __table_args__ = (
        CheckConstraint(
            "target_type IN ('project', 'episode', 'scene', 'shot')",
            name="ck_resolved_continuity_target_type",
        ),
        CheckConstraint(
            "status IN ('accepted', 'incomplete')", name="ck_resolved_continuity_status"
        ),
        CheckConstraint(hex64_check("content_hash"), name="ck_resolved_continuity_hash"),
    )

    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    target_type: Mapped[str] = mapped_column(String(16), nullable=False)
    target_id: Mapped[str] = mapped_column(String(36), nullable=False, index=True)
    target_revision: Mapped[int] = mapped_column(Integer, nullable=False)
    refs: Mapped[list[dict[str, object]]] = mapped_column(JSON_DOCUMENT, nullable=False)
    revision_chain: Mapped[list[object]] = mapped_column(JSON_DOCUMENT, nullable=False)
    override_chain: Mapped[list[dict[str, object]]] = mapped_column(JSON_DOCUMENT, nullable=False)
    status: Mapped[str] = mapped_column(String(16), nullable=False)
    content_hash: Mapped[str] = mapped_column(String(64), nullable=False)


class ContinuityImpactAnalysisModel(IdentityRevisionMixin, Base):
    __tablename__ = "continuity_impact_analyses"
    __table_args__ = (
        CheckConstraint("status IN ('complete', 'incomplete')", name="ck_continuity_impact_status"),
        CheckConstraint(hex64_check("candidate_payload_hash"), name="ck_continuity_candidate_hash"),
        CheckConstraint(hex64_check("target_set_hash"), name="ck_continuity_target_set_hash"),
    )

    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    entry_id: Mapped[str] = mapped_column(
        ForeignKey("asset_bible_entries.id"), nullable=False, index=True
    )
    base_version_id: Mapped[str] = mapped_column(
        ForeignKey("asset_bible_entry_versions.id"), nullable=False
    )
    candidate_payload_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    target_set_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    target_refs: Mapped[list[dict[str, object]]] = mapped_column(JSON_DOCUMENT, nullable=False)
    candidate_payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)
    reference_asset_version_refs: Mapped[list[dict[str, object]]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=list
    )
    generation_spec_refs: Mapped[list[dict[str, object]]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=list
    )
    status: Mapped[str] = mapped_column(String(16), nullable=False)
    diagnostic: Mapped[str | None] = mapped_column(String(255))


class AssetBibleAcceptDecisionModel(Base):
    __tablename__ = "asset_bible_accept_decisions"
    __table_args__ = (
        UniqueConstraint("fingerprint", name="uq_asset_bible_accept_fingerprint"),
        CheckConstraint(hex64_check("target_set_hash"), name="ck_asset_bible_decision_set_hash"),
        CheckConstraint(hex64_check("fingerprint"), name="ck_asset_bible_decision_fingerprint"),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    entry_id: Mapped[str] = mapped_column(ForeignKey("asset_bible_entries.id"), nullable=False)
    analysis_id: Mapped[str] = mapped_column(
        ForeignKey("continuity_impact_analyses.id"), nullable=False
    )
    old_version_id: Mapped[str] = mapped_column(
        ForeignKey("asset_bible_entry_versions.id"), nullable=False
    )
    new_version_id: Mapped[str] = mapped_column(
        ForeignKey("asset_bible_entry_versions.id"), nullable=False
    )
    target_set_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    actor_uuid: Mapped[str] = mapped_column(String(36), nullable=False)
    correlation_id: Mapped[str] = mapped_column(String(255), nullable=False)
    fingerprint: Mapped[str] = mapped_column(String(64), nullable=False)
    task_ids: Mapped[list[str]] = mapped_column(JSON_DOCUMENT, nullable=False, default=list)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), server_default=func.now())


class ContinuityRevisionTaskModel(IdentityRevisionMixin, Base):
    __tablename__ = "continuity_revision_tasks"
    __table_args__ = (
        UniqueConstraint(
            "target_type",
            "target_id",
            "entry_id",
            "new_version_id",
            name="uq_continuity_revision_task_target",
        ),
        CheckConstraint(
            "target_type IN ('episode', 'scene', 'shot')",
            name="ck_continuity_task_target_type",
        ),
        CheckConstraint(
            "status IN ('pending', 'acknowledged', 'resolved', 'superseded')",
            name="ck_continuity_task_status",
        ),
        CheckConstraint(hex64_check("snapshot_hash"), name="ck_continuity_task_snapshot_hash"),
    )

    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    target_type: Mapped[str] = mapped_column(String(16), nullable=False)
    target_id: Mapped[str] = mapped_column(String(36), nullable=False, index=True)
    target_revision: Mapped[int] = mapped_column(Integer, nullable=False)
    entry_id: Mapped[str] = mapped_column(ForeignKey("asset_bible_entries.id"), nullable=False)
    old_version_id: Mapped[str] = mapped_column(
        ForeignKey("asset_bible_entry_versions.id"), nullable=False
    )
    new_version_id: Mapped[str] = mapped_column(
        ForeignKey("asset_bible_entry_versions.id"), nullable=False
    )
    snapshot_id: Mapped[str] = mapped_column(
        ForeignKey("resolved_continuity_snapshots.id"), nullable=False
    )
    snapshot_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    reason: Mapped[str] = mapped_column(String(255), nullable=False)
    correlation_id: Mapped[str] = mapped_column(String(255), nullable=False)
    status: Mapped[str] = mapped_column(String(16), nullable=False)


class AssetBibleHandoffAckModel(Base):
    __tablename__ = "asset_bible_handoff_acks"
    __table_args__ = (
        UniqueConstraint("handoff_id", name="uq_asset_bible_handoff_id"),
        CheckConstraint(hex64_check("payload_hash"), name="ck_asset_bible_handoff_payload_hash"),
        CheckConstraint(hex64_check("fingerprint"), name="ck_asset_bible_handoff_fingerprint"),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    handoff_id: Mapped[str] = mapped_column(String(255), nullable=False)
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False, index=True)
    payload_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    fingerprint: Mapped[str] = mapped_column(String(64), nullable=False)
    entry_version_refs: Mapped[list[object]] = mapped_column(JSON_DOCUMENT, nullable=False)
    correlation_id: Mapped[str] = mapped_column(String(255), nullable=False)
    schema_version: Mapped[str] = mapped_column(String(32), nullable=False, default="1.0.0")
    created_at: Mapped[datetime] = mapped_column(DateTime(timezone=True), server_default=func.now())


class TimelineCurrentCutModel(IdentityRevisionMixin, Base):
    __tablename__ = "timeline_current_cuts"
    __table_args__ = (
        UniqueConstraint("episode_id", name="uq_timeline_current_cuts_episode_id"),
        CheckConstraint("revision >= 1", name="ck_timeline_current_cut_revision"),
    )

    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False)
    episode_id: Mapped[str] = mapped_column(ForeignKey("episodes.id"), nullable=False)
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)


class TimelineClipModel(Base):
    __tablename__ = "timeline_clips"
    __table_args__ = (
        UniqueConstraint("cut_id", "position", name="uq_timeline_clip_cut_position"),
        CheckConstraint("position >= 0", name="ck_timeline_clip_position"),
        CheckConstraint("asset_version_revision >= 0", name="ck_timeline_clip_asset_revision"),
        CheckConstraint("source_in_frame >= 0", name="ck_timeline_clip_source_in"),
        CheckConstraint("duration_frames > 0", name="ck_timeline_clip_duration"),
        CheckConstraint("timeline_start_frame >= 0", name="ck_timeline_clip_start"),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    cut_id: Mapped[str] = mapped_column(ForeignKey("timeline_current_cuts.id"), nullable=False)
    position: Mapped[int] = mapped_column(Integer, nullable=False)
    asset_version_id: Mapped[str] = mapped_column(ForeignKey("asset_versions.id"), nullable=False)
    asset_version_revision: Mapped[int] = mapped_column(Integer, nullable=False)
    asset_version_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    derivative_fingerprint: Mapped[str] = mapped_column(String(64), nullable=False)
    source_in_frame: Mapped[int] = mapped_column(Integer, nullable=False)
    duration_frames: Mapped[int] = mapped_column(Integer, nullable=False)
    timeline_start_frame: Mapped[int] = mapped_column(Integer, nullable=False)
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)


class TimelineSoundCueModel(Base):
    __tablename__ = "timeline_sound_cues"
    __table_args__ = (
        UniqueConstraint("cut_id", "position", name="uq_timeline_cue_cut_position"),
        CheckConstraint(
            "track IN ('dialogue','music','ambience','effects')", name="ck_timeline_cue_track"
        ),
        CheckConstraint("start_frame >= 0", name="ck_timeline_cue_start"),
        CheckConstraint("duration_frames > 0", name="ck_timeline_cue_duration"),
        CheckConstraint("priority >= 0 AND priority <= 100", name="ck_timeline_cue_priority"),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    cut_id: Mapped[str] = mapped_column(ForeignKey("timeline_current_cuts.id"), nullable=False)
    position: Mapped[int] = mapped_column(Integer, nullable=False)
    track: Mapped[str] = mapped_column(String(16), nullable=False)
    asset_version_id: Mapped[str] = mapped_column(ForeignKey("asset_versions.id"), nullable=False)
    start_frame: Mapped[int] = mapped_column(Integer, nullable=False)
    duration_frames: Mapped[int] = mapped_column(Integer, nullable=False)
    priority: Mapped[int] = mapped_column(Integer, nullable=False)
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)


class TimelineCaptionModel(Base):
    __tablename__ = "timeline_captions"
    __table_args__ = (
        UniqueConstraint("cut_id", "position", name="uq_timeline_caption_cut_position"),
        CheckConstraint("start_frame >= 0", name="ck_timeline_caption_start"),
        CheckConstraint("end_frame > start_frame", name="ck_timeline_caption_range"),
        CheckConstraint("length(trim(text)) > 0", name="ck_timeline_caption_text"),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    cut_id: Mapped[str] = mapped_column(ForeignKey("timeline_current_cuts.id"), nullable=False)
    position: Mapped[int] = mapped_column(Integer, nullable=False)
    start_frame: Mapped[int] = mapped_column(Integer, nullable=False)
    end_frame: Mapped[int] = mapped_column(Integer, nullable=False)
    text: Mapped[str] = mapped_column(Text, nullable=False)


class TimelineVersionModel(IdentityRevisionMixin, Base):
    __tablename__ = "episode_timeline_versions"
    __table_args__ = (
        CheckConstraint("source_cut_revision >= 1", name="ck_timeline_version_source_revision"),
        CheckConstraint("revision = 1", name="ck_timeline_version_immutable_revision"),
        CheckConstraint("length(trim(name)) > 0", name="ck_timeline_version_name"),
    )

    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False)
    episode_id: Mapped[str] = mapped_column(ForeignKey("episodes.id"), nullable=False)
    source_cut_id: Mapped[str] = mapped_column(
        ForeignKey("timeline_current_cuts.id"), nullable=False
    )
    source_cut_revision: Mapped[int] = mapped_column(Integer, nullable=False)
    name: Mapped[str] = mapped_column(String(120), nullable=False)
    timeline_fingerprint: Mapped[str] = mapped_column(String(64), nullable=False)
    snapshot: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)


class MediaInspectionModel(Base):
    __tablename__ = "media_inspections"
    __table_args__ = (
        UniqueConstraint(
            "asset_version_id",
            "asset_version_revision",
            "source_hash",
            name="uq_media_inspection_source",
        ),
        CheckConstraint(
            "status IN ('pending','ready','failed','stale')",
            name="ck_media_inspection_status",
        ),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False)
    asset_version_id: Mapped[str] = mapped_column(ForeignKey("asset_versions.id"), nullable=False)
    asset_version_revision: Mapped[int] = mapped_column(Integer, nullable=False)
    source_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    status: Mapped[str] = mapped_column(String(16), nullable=False)
    revision: Mapped[int] = mapped_column(Integer, nullable=False)
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)


class MediaDerivativeModel(Base):
    __tablename__ = "media_derivatives"
    __table_args__ = (
        UniqueConstraint("inspection_id", "kind", name="uq_media_derivative_kind"),
        CheckConstraint(
            "kind IN ('proxy','thumbnail','keyframe_index','waveform')",
            name="ck_media_derivative_kind",
        ),
        CheckConstraint(
            "status IN ('pending','ready','failed','stale')",
            name="ck_media_derivative_status",
        ),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    inspection_id: Mapped[str] = mapped_column(ForeignKey("media_inspections.id"), nullable=False)
    kind: Mapped[str] = mapped_column(String(32), nullable=False)
    status: Mapped[str] = mapped_column(String(16), nullable=False)
    fingerprint: Mapped[str] = mapped_column(String(64), nullable=False)
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)


class TimelinePreviewArtifactModel(Base):
    __tablename__ = "timeline_preview_artifacts"
    __table_args__ = (
        CheckConstraint(
            "status IN ('pending','ready','failed','stale')", name="ck_timeline_preview_status"
        ),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    cut_id: Mapped[str] = mapped_column(ForeignKey("timeline_current_cuts.id"), nullable=False)
    cut_revision: Mapped[int] = mapped_column(Integer, nullable=False)
    timeline_fingerprint: Mapped[str] = mapped_column(String(64), nullable=False)
    render_plan_hash: Mapped[str] = mapped_column(String(64), nullable=False)
    status: Mapped[str] = mapped_column(String(16), nullable=False)
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)


class EpisodeExportBatchModel(IdentityRevisionMixin, Base):
    __tablename__ = "episode_export_batches"
    __table_args__ = (
        UniqueConstraint("project_id", "idempotency_key", name="uq_export_batch_idempotency"),
        CheckConstraint("export_profile = 'light'", name="ck_export_batch_profile"),
        CheckConstraint(
            "status IN ('queued','succeeded','partially_failed','failed')",
            name="ck_export_batch_status",
        ),
    )

    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False)
    export_profile: Mapped[str] = mapped_column(String(16), nullable=False)
    idempotency_key: Mapped[str] = mapped_column(String(255), nullable=False)
    status: Mapped[str] = mapped_column(String(32), nullable=False)
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)


class EpisodeExportMemberModel(Base):
    __tablename__ = "episode_export_members"
    __table_args__ = (
        UniqueConstraint("batch_id", "position", name="uq_export_member_position"),
        UniqueConstraint("batch_id", "episode_id", name="uq_export_member_episode"),
        UniqueConstraint("batch_id", "output_base_name", name="uq_export_member_name"),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True, default=new_id)
    batch_id: Mapped[str] = mapped_column(ForeignKey("episode_export_batches.id"), nullable=False)
    position: Mapped[int] = mapped_column(Integer, nullable=False)
    episode_id: Mapped[str] = mapped_column(ForeignKey("episodes.id"), nullable=False)
    timeline_version_id: Mapped[str] = mapped_column(
        ForeignKey("episode_timeline_versions.id"), nullable=False
    )
    timeline_version_revision: Mapped[int] = mapped_column(Integer, nullable=False)
    output_base_name: Mapped[str] = mapped_column(String(120), nullable=False)


class ExportJobModel(Base):
    __tablename__ = "episode_export_jobs"
    __table_args__ = (
        UniqueConstraint(
            "batch_id",
            "episode_id",
            "logical_operation",
            name="uq_export_job_logical_operation",
        ),
        CheckConstraint(
            "status IN ('queued','preflighting','rendering','packaging','succeeded',"
            "'failed','cancel_requested','cancelled')",
            name="ck_export_job_status",
        ),
        CheckConstraint(
            "packaging_phase IS NULL OR packaging_phase IN ('uploading','verifying','registering')",
            name="ck_export_job_packaging_phase",
        ),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    batch_id: Mapped[str] = mapped_column(ForeignKey("episode_export_batches.id"), nullable=False)
    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False)
    episode_id: Mapped[str] = mapped_column(ForeignKey("episodes.id"), nullable=False)
    timeline_version_id: Mapped[str] = mapped_column(
        ForeignKey("episode_timeline_versions.id"), nullable=False
    )
    revision: Mapped[int] = mapped_column(Integer, nullable=False)
    status: Mapped[str] = mapped_column(String(32), nullable=False)
    packaging_phase: Mapped[str | None] = mapped_column(String(16))
    logical_operation: Mapped[str] = mapped_column(String(255), nullable=False)
    render_plan_hash: Mapped[str | None] = mapped_column(String(64))
    renderer_diagnostic: Mapped[str | None] = mapped_column(Text)
    execution_snapshot: Mapped[dict[str, object]] = mapped_column(
        JSON_DOCUMENT, nullable=False, default=dict
    )
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)


class ExportDispatchOutboxModel(IdentityRevisionMixin, Base):
    __tablename__ = "export_dispatch_outbox"
    __table_args__ = (
        UniqueConstraint("job_id", name="uq_export_dispatch_job"),
        UniqueConstraint("workflow_id", name="uq_export_dispatch_workflow"),
        CheckConstraint("status IN ('pending','dispatched')", name="ck_export_dispatch_status"),
        CheckConstraint("attempts >= 0", name="ck_export_dispatch_attempts"),
    )

    project_id: Mapped[str] = mapped_column(ForeignKey("projects.id"), nullable=False)
    batch_id: Mapped[str] = mapped_column(ForeignKey("episode_export_batches.id"), nullable=False)
    job_id: Mapped[str] = mapped_column(ForeignKey("episode_export_jobs.id"), nullable=False)
    logical_operation: Mapped[str] = mapped_column(String(255), nullable=False)
    workflow_id: Mapped[str] = mapped_column(String(255), nullable=False)
    status: Mapped[str] = mapped_column(String(16), nullable=False)
    attempts: Mapped[int] = mapped_column(Integer, nullable=False, default=0)
    last_error: Mapped[str | None] = mapped_column(Text)
    dispatched_at: Mapped[datetime | None] = mapped_column(DateTime(timezone=True))
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)


class ExportArtifactModel(Base):
    __tablename__ = "export_artifacts"
    __table_args__ = (
        UniqueConstraint("export_job_id", "artifact_type", name="uq_export_artifact_type"),
        CheckConstraint(
            "artifact_type IN ('mp4','srt','light_manifest')",
            name="ck_export_artifact_type",
        ),
        CheckConstraint(
            "status IN ('pending','verified','failed','held')",
            name="ck_export_artifact_status",
        ),
        CheckConstraint("size_bytes IS NULL OR size_bytes >= 0", name="ck_export_artifact_size"),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    export_job_id: Mapped[str] = mapped_column(ForeignKey("episode_export_jobs.id"), nullable=False)
    artifact_type: Mapped[str] = mapped_column(String(32), nullable=False)
    status: Mapped[str] = mapped_column(String(16), nullable=False)
    size_bytes: Mapped[int | None] = mapped_column(BigInteger)
    checksum: Mapped[str | None] = mapped_column(String(64))
    mime_type: Mapped[str | None] = mapped_column(String(64))
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)


class ExportDiagnosticTargetModel(Base):
    __tablename__ = "export_diagnostic_targets"
    __table_args__ = (
        CheckConstraint(
            "target_type IN ('timeline','clip','caption','sound_cue','asset_version',"
            "'renderer','storage','artifact')",
            name="ck_export_diagnostic_target_type",
        ),
    )

    id: Mapped[str] = mapped_column(String(36), primary_key=True)
    export_job_id: Mapped[str] = mapped_column(ForeignKey("episode_export_jobs.id"), nullable=False)
    target_type: Mapped[str] = mapped_column(String(32), nullable=False)
    owner_id: Mapped[str | None] = mapped_column(String(255))
    owner_revision: Mapped[int | None] = mapped_column(Integer)
    field_path: Mapped[str | None] = mapped_column(String(255))
    route_token: Mapped[str] = mapped_column(String(128), nullable=False)
    code: Mapped[str] = mapped_column(String(128), nullable=False)
    payload: Mapped[dict[str, object]] = mapped_column(JSON_DOCUMENT, nullable=False)
