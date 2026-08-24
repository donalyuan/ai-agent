"""Persist storage profile and upload operation owner metadata."""

from __future__ import annotations

import sqlalchemy as sa

from alembic import op

revision = "0019_storage_owner"
down_revision = "0018_asset_edit_owner"
branch_labels = None
depends_on = None


def upgrade() -> None:
    op.create_table(
        "storage_profiles",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
        sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
        sa.Column("name", sa.String(255), nullable=False, server_default=""),
        sa.Column("adapter_key", sa.String(32), nullable=False, server_default="tos"),
        sa.Column("endpoint", sa.String(1024), nullable=False),
        sa.Column("bucket", sa.String(255), nullable=False),
        sa.Column("region", sa.String(128), nullable=False),
        sa.Column("private_bucket", sa.Boolean(), nullable=False, server_default=sa.true()),
        sa.Column("enabled", sa.Boolean(), nullable=False, server_default=sa.false()),
        sa.Column("bucket_binding_id", sa.String(255), nullable=False, server_default=""),
        sa.Column(
            "credential_status", sa.String(32), nullable=False, server_default="unconfigured"
        ),
        sa.Column("credential_ref", sa.String(255)),
        sa.Column("connect_timeout_ms", sa.Integer(), nullable=False, server_default="10000"),
        sa.Column("read_timeout_ms", sa.Integer(), nullable=False, server_default="30000"),
        sa.Column("write_timeout_ms", sa.Integer(), nullable=False, server_default="60000"),
        sa.Column("presign_max_ttl_seconds", sa.Integer(), nullable=False, server_default="900"),
        sa.Column("project_scope", sa.JSON(), nullable=False, server_default="[]"),
        sa.Column("masked_credential_summary", sa.String(255)),
    )
    op.create_index("ix_storage_profiles_project_id", "storage_profiles", ["project_id"])
    op.create_table(
        "storage_bucket_bindings",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column(
            "profile_id", sa.String(36), sa.ForeignKey("storage_profiles.id"), nullable=False
        ),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("bucket", sa.String(255), nullable=False),
        sa.Column("region", sa.String(128), nullable=False),
        sa.Column("endpoint", sa.String(1024), nullable=False),
        sa.Column("private_bucket", sa.Boolean(), nullable=False, server_default=sa.true()),
        sa.UniqueConstraint("profile_id", "bucket", name="uq_storage_bucket_profile_bucket"),
        sa.CheckConstraint("private_bucket = true", name="ck_storage_bucket_private"),
    )
    op.create_table(
        "storage_upload_operations",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column(
            "profile_id", sa.String(36), sa.ForeignKey("storage_profiles.id"), nullable=False
        ),
        sa.Column("operation_key", sa.String(255), nullable=False, unique=True),
        sa.Column("session_id", sa.String(255)),
        sa.Column("object_key", sa.String(1024), nullable=False),
        sa.Column("status", sa.String(32), nullable=False, server_default="active"),
        sa.Column("object_ref", sa.String(1024)),
        sa.Column("payload", sa.JSON(), nullable=False, server_default="{}"),
        sa.CheckConstraint(
            "status IN ('active', 'completed', 'aborted', 'unknown', 'failed')",
            name="ck_storage_operation_status",
        ),
    )
    op.create_table(
        "storage_upload_sessions",
        sa.Column("id", sa.String(64), primary_key=True),
        sa.Column(
            "operation_id",
            sa.String(36),
            sa.ForeignKey("storage_upload_operations.id"),
            nullable=False,
            unique=True,
        ),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column(
            "profile_id", sa.String(36), sa.ForeignKey("storage_profiles.id"), nullable=False
        ),
        sa.Column("operation_key", sa.String(255), nullable=False, unique=True),
        sa.Column("object_key", sa.String(1024), nullable=False),
        sa.Column("status", sa.String(32), nullable=False, server_default="active"),
        sa.Column("expected_size_bytes", sa.BigInteger()),
        sa.Column("expected_checksum", sa.String(64)),
        sa.Column("expected_mime_type", sa.String(255)),
        sa.CheckConstraint(
            "status IN ('active', 'completed', 'aborted', 'unknown', 'failed')",
            name="ck_storage_session_status",
        ),
        sa.CheckConstraint(
            "expected_size_bytes IS NULL OR expected_size_bytes >= 0",
            name="ck_storage_session_size",
        ),
    )
    op.create_table(
        "storage_upload_parts",
        sa.Column(
            "session_id",
            sa.String(64),
            sa.ForeignKey("storage_upload_sessions.id"),
            primary_key=True,
        ),
        sa.Column("part_number", sa.Integer(), primary_key=True),
        sa.Column("checksum", sa.String(64), nullable=False),
        sa.Column("etag", sa.String(1024), nullable=False),
        sa.Column("size_bytes", sa.BigInteger(), nullable=False),
        sa.CheckConstraint("part_number >= 1", name="ck_storage_part_number"),
        sa.CheckConstraint("size_bytes >= 0", name="ck_storage_part_size"),
    )
    op.create_table(
        "stored_objects",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column(
            "profile_id", sa.String(36), sa.ForeignKey("storage_profiles.id"), nullable=False
        ),
        sa.Column("operation_key", sa.String(255), nullable=False, unique=True),
        sa.Column("bucket", sa.String(255), nullable=False),
        sa.Column("object_key", sa.String(1024), nullable=False),
        sa.Column("size_bytes", sa.BigInteger(), nullable=False),
        sa.Column("checksum", sa.String(64), nullable=False),
        sa.Column("mime_type", sa.String(255), nullable=False),
        sa.Column("etag", sa.String(1024)),
        sa.Column("verified", sa.Boolean(), nullable=False, server_default=sa.false()),
        sa.UniqueConstraint("profile_id", "object_key", name="uq_stored_object_profile_key"),
        sa.CheckConstraint("size_bytes >= 0", name="ck_stored_object_size"),
    )
    op.create_table(
        "storage_reference_proofs",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("object_id", sa.String(36), sa.ForeignKey("stored_objects.id"), nullable=False),
        sa.Column("checked_at", sa.DateTime(timezone=True), nullable=False),
        sa.Column("owner_results", sa.JSON(), nullable=False, server_default="{}"),
        sa.Column("no_references", sa.Boolean(), nullable=False, server_default=sa.false()),
    )
    op.create_table(
        "storage_recovery_records",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column(
            "operation_id",
            sa.String(36),
            sa.ForeignKey("storage_upload_operations.id"),
            nullable=False,
        ),
        sa.Column("status", sa.String(32), nullable=False),
        sa.Column("diagnostic", sa.String(255), nullable=False),
        sa.Column("correlation_id", sa.String(255), nullable=False),
        sa.Column("payload", sa.JSON(), nullable=False, server_default="{}"),
        sa.CheckConstraint(
            "status IN ('reconciliation_required', 'failed', 'aborted', 'resolved')",
            name="ck_storage_recovery_status",
        ),
    )


def downgrade() -> None:
    op.drop_table("storage_recovery_records")
    op.drop_table("storage_reference_proofs")
    op.drop_table("stored_objects")
    op.drop_table("storage_upload_parts")
    op.drop_table("storage_upload_sessions")
    op.drop_table("storage_upload_operations")
    op.drop_table("storage_bucket_bindings")
    op.drop_index("ix_storage_profiles_project_id", table_name="storage_profiles")
    op.drop_table("storage_profiles")
