"""add relational AssetBible and continuity owner facts

Revision ID: 0014_asset_bible_owner
Revises: 0013_scenes_owner_repair
"""

from __future__ import annotations

import sqlalchemy as sa
from sqlalchemy.dialects import postgresql

from alembic import op

revision = "0014_asset_bible_owner"
down_revision = "0013_scenes_owner_repair"
branch_labels = None
depends_on = None

JSON_DOCUMENT = sa.JSON().with_variant(postgresql.JSONB(), "postgresql")
ENTRY_TYPES = "'character', 'look', 'location', 'scene_visual', 'prop', 'visual_style'"


def _hex64(column: str) -> str:
    stripped = f"lower({column})"
    for character in "0123456789abcdef":
        stripped = f"replace({stripped}, '{character}', '')"
    return f"length({column}) = 64 AND length({stripped}) = 0"


def _identity(*, revisioned: bool = True) -> list[sa.Column[object]]:
    columns: list[sa.Column[object]] = [sa.Column("id", sa.String(36), primary_key=True)]
    if revisioned:
        columns.append(sa.Column("revision", sa.Integer(), nullable=False, server_default="1"))
    columns.extend(
        [
            sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
            sa.Column(
                "created_at",
                sa.DateTime(timezone=True),
                nullable=False,
                server_default=sa.func.now(),
            ),
            sa.Column(
                "updated_at",
                sa.DateTime(timezone=True),
                nullable=False,
                server_default=sa.func.now(),
            ),
        ]
    )
    return columns


def upgrade() -> None:
    op.create_table(
        "asset_bibles",
        *_identity(),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("current_version_map", JSON_DOCUMENT, nullable=False),
        sa.UniqueConstraint("project_id", name="uq_asset_bibles_project"),
        sa.CheckConstraint("revision >= 1", name="ck_asset_bibles_revision_positive"),
    )
    op.create_index("ix_asset_bibles_project_id", "asset_bibles", ["project_id"])

    op.create_table(
        "asset_bible_entries",
        *_identity(),
        sa.Column(
            "asset_bible_id", sa.String(36), sa.ForeignKey("asset_bibles.id"), nullable=False
        ),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("entry_type", sa.String(32), nullable=False),
        sa.Column("disabled", sa.Boolean(), nullable=False, server_default=sa.false()),
        sa.Column("current_version_id", sa.String(36), nullable=True),
        sa.UniqueConstraint("id", "project_id", name="uq_asset_bible_entries_id_project"),
        sa.CheckConstraint("revision >= 1", name="ck_asset_bible_entries_revision_positive"),
        sa.CheckConstraint(f"entry_type IN ({ENTRY_TYPES})", name="ck_asset_bible_entries_type"),
    )
    op.create_index("ix_asset_bible_entries_project_id", "asset_bible_entries", ["project_id"])
    op.create_index(
        "ix_asset_bible_entries_asset_bible_id", "asset_bible_entries", ["asset_bible_id"]
    )

    op.create_table(
        "asset_bible_entry_versions",
        *_identity(),
        sa.Column("entry_id", sa.String(36), nullable=False),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("entry_type", sa.String(32), nullable=False),
        sa.Column("payload", JSON_DOCUMENT, nullable=False),
        sa.Column("version_number", sa.Integer(), nullable=False),
        sa.Column("actor_uuid", sa.String(36), nullable=False),
        sa.Column("reference_asset_version_refs", JSON_DOCUMENT, nullable=False),
        sa.Column("generation_spec_refs", JSON_DOCUMENT, nullable=False),
        sa.Column("content_hash", sa.String(64), nullable=False),
        sa.ForeignKeyConstraint(
            ["entry_id", "project_id"],
            ["asset_bible_entries.id", "asset_bible_entries.project_id"],
            name="fk_asset_bible_versions_entry_project",
        ),
        sa.UniqueConstraint("id", "project_id", name="uq_asset_bible_versions_id_project"),
        sa.UniqueConstraint("entry_id", "version_number", name="uq_asset_bible_version_number"),
        sa.CheckConstraint("revision = 1", name="ck_asset_bible_versions_immutable_revision"),
        sa.CheckConstraint("version_number >= 1", name="ck_asset_bible_versions_number_positive"),
        sa.CheckConstraint(f"entry_type IN ({ENTRY_TYPES})", name="ck_asset_bible_versions_type"),
        sa.CheckConstraint(_hex64("content_hash"), name="ck_asset_bible_versions_hash"),
    )
    op.create_index(
        "ix_asset_bible_entry_versions_entry_id", "asset_bible_entry_versions", ["entry_id"]
    )
    op.create_index(
        "ix_asset_bible_entry_versions_project_id", "asset_bible_entry_versions", ["project_id"]
    )

    op.create_table(
        "asset_bible_relationships",
        *_identity(revisioned=False),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("source_entry_id", sa.String(36), nullable=False),
        sa.Column("target_entry_id", sa.String(36), nullable=False),
        sa.Column("kind", sa.String(32), nullable=False),
        sa.ForeignKeyConstraint(
            ["source_entry_id", "project_id"],
            ["asset_bible_entries.id", "asset_bible_entries.project_id"],
            name="fk_asset_bible_relationship_source_project",
        ),
        sa.ForeignKeyConstraint(
            ["target_entry_id", "project_id"],
            ["asset_bible_entries.id", "asset_bible_entries.project_id"],
            name="fk_asset_bible_relationship_target_project",
        ),
        sa.UniqueConstraint(
            "source_entry_id", "target_entry_id", "kind", name="uq_asset_bible_relationship_edge"
        ),
        sa.CheckConstraint(
            "kind IN ('character_look', 'location_scene_visual', 'related')",
            name="ck_asset_bible_relationship_kind",
        ),
        sa.CheckConstraint(
            "source_entry_id <> target_entry_id", name="ck_asset_bible_no_self_edge"
        ),
    )

    op.create_table(
        "asset_bible_assignments",
        *_identity(),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("level", sa.String(16), nullable=False),
        sa.Column("target_id", sa.String(36), nullable=False),
        sa.Column("entry_id", sa.String(36), nullable=False),
        sa.Column("version_id", sa.String(36), nullable=False),
        sa.Column("version_revision", sa.Integer(), nullable=False),
        sa.Column("content_hash", sa.String(64), nullable=False),
        sa.Column("scope_revision", sa.Integer(), nullable=False),
        sa.ForeignKeyConstraint(
            ["entry_id", "project_id"],
            ["asset_bible_entries.id", "asset_bible_entries.project_id"],
            name="fk_continuity_assignment_entry_project",
        ),
        sa.ForeignKeyConstraint(
            ["version_id", "project_id"],
            ["asset_bible_entry_versions.id", "asset_bible_entry_versions.project_id"],
            name="fk_continuity_assignment_version_project",
        ),
        sa.UniqueConstraint(
            "project_id", "level", "target_id", "entry_id", name="uq_continuity_assignment_scope"
        ),
        sa.CheckConstraint(
            "level IN ('project', 'episode', 'scene', 'shot')",
            name="ck_continuity_assignment_level",
        ),
        sa.CheckConstraint("revision >= 1", name="ck_continuity_assignment_revision_positive"),
        sa.CheckConstraint("scope_revision >= 1", name="ck_continuity_scope_revision_positive"),
        sa.CheckConstraint("version_revision >= 1", name="ck_continuity_version_revision_positive"),
        sa.CheckConstraint(_hex64("content_hash"), name="ck_continuity_assignment_hash"),
    )
    op.create_index(
        "ix_asset_bible_assignments_project_target",
        "asset_bible_assignments",
        ["project_id", "target_id"],
    )

    op.create_table(
        "resolved_continuity_snapshots",
        *_identity(),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("target_type", sa.String(16), nullable=False),
        sa.Column("target_id", sa.String(36), nullable=False),
        sa.Column("target_revision", sa.Integer(), nullable=False),
        sa.Column("refs", JSON_DOCUMENT, nullable=False),
        sa.Column("revision_chain", JSON_DOCUMENT, nullable=False),
        sa.Column("override_chain", JSON_DOCUMENT, nullable=False),
        sa.Column("status", sa.String(16), nullable=False),
        sa.Column("content_hash", sa.String(64), nullable=False),
        sa.CheckConstraint(
            "target_type IN ('project', 'episode', 'scene', 'shot')",
            name="ck_resolved_continuity_target_type",
        ),
        sa.CheckConstraint(
            "status IN ('accepted', 'incomplete')", name="ck_resolved_continuity_status"
        ),
        sa.CheckConstraint(_hex64("content_hash"), name="ck_resolved_continuity_hash"),
    )
    op.create_index(
        "ix_resolved_continuity_project_target",
        "resolved_continuity_snapshots",
        ["project_id", "target_type", "target_id"],
    )

    op.create_table(
        "continuity_impact_analyses",
        *_identity(),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("entry_id", sa.String(36), nullable=False),
        sa.Column("base_version_id", sa.String(36), nullable=False),
        sa.Column("candidate_payload_hash", sa.String(64), nullable=False),
        sa.Column("target_set_hash", sa.String(64), nullable=False),
        sa.Column("target_refs", JSON_DOCUMENT, nullable=False),
        sa.Column("candidate_payload", JSON_DOCUMENT, nullable=False),
        sa.Column("reference_asset_version_refs", JSON_DOCUMENT, nullable=False),
        sa.Column("generation_spec_refs", JSON_DOCUMENT, nullable=False),
        sa.Column("status", sa.String(16), nullable=False),
        sa.Column("diagnostic", sa.String(255), nullable=True),
        sa.ForeignKeyConstraint(
            ["entry_id", "project_id"],
            ["asset_bible_entries.id", "asset_bible_entries.project_id"],
            name="fk_continuity_impact_entry_project",
        ),
        sa.ForeignKeyConstraint(
            ["base_version_id", "project_id"],
            ["asset_bible_entry_versions.id", "asset_bible_entry_versions.project_id"],
            name="fk_continuity_impact_base_version_project",
        ),
        sa.CheckConstraint(
            "status IN ('complete', 'incomplete')", name="ck_continuity_impact_status"
        ),
        sa.CheckConstraint(_hex64("candidate_payload_hash"), name="ck_continuity_candidate_hash"),
        sa.CheckConstraint(_hex64("target_set_hash"), name="ck_continuity_target_set_hash"),
    )

    op.create_table(
        "asset_bible_accept_decisions",
        *_identity(revisioned=False),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("entry_id", sa.String(36), nullable=False),
        sa.Column(
            "analysis_id",
            sa.String(36),
            sa.ForeignKey("continuity_impact_analyses.id"),
            nullable=False,
        ),
        sa.Column("old_version_id", sa.String(36), nullable=False),
        sa.Column("new_version_id", sa.String(36), nullable=False),
        sa.Column("target_set_hash", sa.String(64), nullable=False),
        sa.Column("actor_uuid", sa.String(36), nullable=False),
        sa.Column("correlation_id", sa.String(255), nullable=False),
        sa.Column("fingerprint", sa.String(64), nullable=False),
        sa.Column("task_ids", JSON_DOCUMENT, nullable=False),
        sa.ForeignKeyConstraint(
            ["entry_id", "project_id"],
            ["asset_bible_entries.id", "asset_bible_entries.project_id"],
            name="fk_asset_bible_decision_entry_project",
        ),
        sa.ForeignKeyConstraint(
            ["old_version_id", "project_id"],
            ["asset_bible_entry_versions.id", "asset_bible_entry_versions.project_id"],
            name="fk_asset_bible_decision_old_version_project",
        ),
        sa.ForeignKeyConstraint(
            ["new_version_id", "project_id"],
            ["asset_bible_entry_versions.id", "asset_bible_entry_versions.project_id"],
            name="fk_asset_bible_decision_new_version_project",
        ),
        sa.UniqueConstraint("fingerprint", name="uq_asset_bible_accept_fingerprint"),
        sa.CheckConstraint(_hex64("target_set_hash"), name="ck_asset_bible_decision_set_hash"),
        sa.CheckConstraint(_hex64("fingerprint"), name="ck_asset_bible_decision_fingerprint"),
    )

    op.create_table(
        "continuity_revision_tasks",
        *_identity(),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("target_type", sa.String(16), nullable=False),
        sa.Column("target_id", sa.String(36), nullable=False),
        sa.Column("target_revision", sa.Integer(), nullable=False),
        sa.Column("entry_id", sa.String(36), nullable=False),
        sa.Column("old_version_id", sa.String(36), nullable=False),
        sa.Column("new_version_id", sa.String(36), nullable=False),
        sa.Column(
            "snapshot_id",
            sa.String(36),
            sa.ForeignKey("resolved_continuity_snapshots.id"),
            nullable=False,
        ),
        sa.Column("snapshot_hash", sa.String(64), nullable=False),
        sa.Column("reason", sa.String(255), nullable=False),
        sa.Column("correlation_id", sa.String(255), nullable=False),
        sa.Column("status", sa.String(16), nullable=False),
        sa.ForeignKeyConstraint(
            ["entry_id", "project_id"],
            ["asset_bible_entries.id", "asset_bible_entries.project_id"],
            name="fk_continuity_task_entry_project",
        ),
        sa.ForeignKeyConstraint(
            ["old_version_id", "project_id"],
            ["asset_bible_entry_versions.id", "asset_bible_entry_versions.project_id"],
            name="fk_continuity_task_old_version_project",
        ),
        sa.ForeignKeyConstraint(
            ["new_version_id", "project_id"],
            ["asset_bible_entry_versions.id", "asset_bible_entry_versions.project_id"],
            name="fk_continuity_task_new_version_project",
        ),
        sa.UniqueConstraint(
            "target_type",
            "target_id",
            "entry_id",
            "new_version_id",
            name="uq_continuity_revision_task_target",
        ),
        sa.CheckConstraint(
            "target_type IN ('episode', 'scene', 'shot')",
            name="ck_continuity_task_target_type",
        ),
        sa.CheckConstraint(
            "status IN ('pending', 'acknowledged', 'resolved', 'superseded')",
            name="ck_continuity_task_status",
        ),
        sa.CheckConstraint(_hex64("snapshot_hash"), name="ck_continuity_task_snapshot_hash"),
    )
    op.create_index(
        "ix_continuity_revision_tasks_project_target",
        "continuity_revision_tasks",
        ["project_id", "target_type", "target_id"],
    )

    op.create_table(
        "asset_bible_handoff_acks",
        *_identity(revisioned=False),
        sa.Column("handoff_id", sa.String(255), nullable=False),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("payload_hash", sa.String(64), nullable=False),
        sa.Column("fingerprint", sa.String(64), nullable=False),
        sa.Column("entry_version_refs", JSON_DOCUMENT, nullable=False),
        sa.Column("correlation_id", sa.String(255), nullable=False),
        sa.UniqueConstraint("handoff_id", name="uq_asset_bible_handoff_id"),
        sa.CheckConstraint(_hex64("payload_hash"), name="ck_asset_bible_handoff_payload_hash"),
        sa.CheckConstraint(_hex64("fingerprint"), name="ck_asset_bible_handoff_fingerprint"),
    )


def downgrade() -> None:
    op.drop_table("asset_bible_handoff_acks")
    op.drop_index(
        "ix_continuity_revision_tasks_project_target", table_name="continuity_revision_tasks"
    )
    op.drop_table("continuity_revision_tasks")
    op.drop_table("asset_bible_accept_decisions")
    op.drop_table("continuity_impact_analyses")
    op.drop_index(
        "ix_resolved_continuity_project_target", table_name="resolved_continuity_snapshots"
    )
    op.drop_table("resolved_continuity_snapshots")
    op.drop_index("ix_asset_bible_assignments_project_target", table_name="asset_bible_assignments")
    op.drop_table("asset_bible_assignments")
    op.drop_table("asset_bible_relationships")
    op.drop_index(
        "ix_asset_bible_entry_versions_project_id", table_name="asset_bible_entry_versions"
    )
    op.drop_index("ix_asset_bible_entry_versions_entry_id", table_name="asset_bible_entry_versions")
    op.drop_table("asset_bible_entry_versions")
    op.drop_index("ix_asset_bible_entries_asset_bible_id", table_name="asset_bible_entries")
    op.drop_index("ix_asset_bible_entries_project_id", table_name="asset_bible_entries")
    op.drop_table("asset_bible_entries")
    op.drop_index("ix_asset_bibles_project_id", table_name="asset_bibles")
    op.drop_table("asset_bibles")
