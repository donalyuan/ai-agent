"""Persist timeline, media inspection, preview, and export owner facts."""

from __future__ import annotations

import sqlalchemy as sa

from alembic import op

revision = "0020_timeline_export_owner"
down_revision = "0019_storage_owner"
branch_labels = None
depends_on = None


def upgrade() -> None:
    json_type = sa.JSON()
    op.create_table(
        "timeline_current_cuts",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column(
            "episode_id", sa.String(36), sa.ForeignKey("episodes.id"), nullable=False, unique=True
        ),
        sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
        sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
        sa.Column("payload", json_type, nullable=False),
        sa.Column("created_at", sa.DateTime(timezone=True), server_default=sa.func.now()),
        sa.Column("updated_at", sa.DateTime(timezone=True), server_default=sa.func.now()),
        sa.CheckConstraint("revision >= 1", name="ck_timeline_current_cut_revision"),
    )
    op.create_table(
        "timeline_clips",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column(
            "cut_id", sa.String(36), sa.ForeignKey("timeline_current_cuts.id"), nullable=False
        ),
        sa.Column("position", sa.Integer(), nullable=False),
        sa.Column(
            "asset_version_id", sa.String(36), sa.ForeignKey("asset_versions.id"), nullable=False
        ),
        sa.Column("asset_version_revision", sa.Integer(), nullable=False),
        sa.Column("asset_version_hash", sa.String(64), nullable=False),
        sa.Column("derivative_fingerprint", sa.String(64), nullable=False),
        sa.Column("source_in_frame", sa.Integer(), nullable=False),
        sa.Column("duration_frames", sa.Integer(), nullable=False),
        sa.Column("timeline_start_frame", sa.Integer(), nullable=False),
        sa.Column("payload", json_type, nullable=False),
        sa.UniqueConstraint("cut_id", "position", name="uq_timeline_clip_cut_position"),
        sa.CheckConstraint("position >= 0", name="ck_timeline_clip_position"),
        sa.CheckConstraint("asset_version_revision >= 0", name="ck_timeline_clip_asset_revision"),
        sa.CheckConstraint("source_in_frame >= 0", name="ck_timeline_clip_source_in"),
        sa.CheckConstraint("duration_frames > 0", name="ck_timeline_clip_duration"),
        sa.CheckConstraint("timeline_start_frame >= 0", name="ck_timeline_clip_start"),
    )
    op.create_table(
        "timeline_sound_cues",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column(
            "cut_id", sa.String(36), sa.ForeignKey("timeline_current_cuts.id"), nullable=False
        ),
        sa.Column("position", sa.Integer(), nullable=False),
        sa.Column("track", sa.String(16), nullable=False),
        sa.Column(
            "asset_version_id", sa.String(36), sa.ForeignKey("asset_versions.id"), nullable=False
        ),
        sa.Column("start_frame", sa.Integer(), nullable=False),
        sa.Column("duration_frames", sa.Integer(), nullable=False),
        sa.Column("priority", sa.Integer(), nullable=False),
        sa.Column("payload", json_type, nullable=False),
        sa.UniqueConstraint("cut_id", "position", name="uq_timeline_cue_cut_position"),
        sa.CheckConstraint(
            "track IN ('dialogue','music','ambience','effects')", name="ck_timeline_cue_track"
        ),
        sa.CheckConstraint("start_frame >= 0", name="ck_timeline_cue_start"),
        sa.CheckConstraint("duration_frames > 0", name="ck_timeline_cue_duration"),
        sa.CheckConstraint("priority >= 0 AND priority <= 100", name="ck_timeline_cue_priority"),
    )
    op.create_table(
        "timeline_captions",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column(
            "cut_id", sa.String(36), sa.ForeignKey("timeline_current_cuts.id"), nullable=False
        ),
        sa.Column("position", sa.Integer(), nullable=False),
        sa.Column("start_frame", sa.Integer(), nullable=False),
        sa.Column("end_frame", sa.Integer(), nullable=False),
        sa.Column("text", sa.Text(), nullable=False),
        sa.UniqueConstraint("cut_id", "position", name="uq_timeline_caption_cut_position"),
        sa.CheckConstraint("start_frame >= 0", name="ck_timeline_caption_start"),
        sa.CheckConstraint("end_frame > start_frame", name="ck_timeline_caption_range"),
        sa.CheckConstraint("length(trim(text)) > 0", name="ck_timeline_caption_text"),
    )
    op.create_table(
        "episode_timeline_versions",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("episode_id", sa.String(36), sa.ForeignKey("episodes.id"), nullable=False),
        sa.Column(
            "source_cut_id",
            sa.String(36),
            sa.ForeignKey("timeline_current_cuts.id"),
            nullable=False,
        ),
        sa.Column("source_cut_revision", sa.Integer(), nullable=False),
        sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
        sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
        sa.Column("name", sa.String(120), nullable=False),
        sa.Column("timeline_fingerprint", sa.String(64), nullable=False),
        sa.Column("snapshot", json_type, nullable=False),
        sa.Column("created_at", sa.DateTime(timezone=True), server_default=sa.func.now()),
        sa.Column("updated_at", sa.DateTime(timezone=True), server_default=sa.func.now()),
        sa.CheckConstraint("source_cut_revision >= 1", name="ck_timeline_version_source_revision"),
        sa.CheckConstraint("revision = 1", name="ck_timeline_version_immutable_revision"),
        sa.CheckConstraint("length(trim(name)) > 0", name="ck_timeline_version_name"),
    )
    op.create_table(
        "media_inspections",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column(
            "asset_version_id", sa.String(36), sa.ForeignKey("asset_versions.id"), nullable=False
        ),
        sa.Column("asset_version_revision", sa.Integer(), nullable=False),
        sa.Column("source_hash", sa.String(64), nullable=False),
        sa.Column("status", sa.String(16), nullable=False),
        sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
        sa.Column("payload", json_type, nullable=False),
        sa.UniqueConstraint(
            "asset_version_id",
            "asset_version_revision",
            "source_hash",
            name="uq_media_inspection_source",
        ),
        sa.CheckConstraint(
            "status IN ('pending','ready','failed','stale')", name="ck_media_inspection_status"
        ),
    )
    op.create_table(
        "media_derivatives",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column(
            "inspection_id", sa.String(36), sa.ForeignKey("media_inspections.id"), nullable=False
        ),
        sa.Column("kind", sa.String(32), nullable=False),
        sa.Column("status", sa.String(16), nullable=False),
        sa.Column("fingerprint", sa.String(64), nullable=False),
        sa.Column("payload", json_type, nullable=False),
        sa.UniqueConstraint("inspection_id", "kind", name="uq_media_derivative_kind"),
        sa.CheckConstraint(
            "kind IN ('proxy','thumbnail','keyframe_index','waveform')",
            name="ck_media_derivative_kind",
        ),
        sa.CheckConstraint(
            "status IN ('pending','ready','failed','stale')", name="ck_media_derivative_status"
        ),
    )
    op.create_table(
        "timeline_preview_artifacts",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column(
            "cut_id", sa.String(36), sa.ForeignKey("timeline_current_cuts.id"), nullable=False
        ),
        sa.Column("cut_revision", sa.Integer(), nullable=False),
        sa.Column("timeline_fingerprint", sa.String(64), nullable=False),
        sa.Column("render_plan_hash", sa.String(64), nullable=False),
        sa.Column("status", sa.String(16), nullable=False),
        sa.Column("payload", json_type, nullable=False),
        sa.CheckConstraint(
            "status IN ('pending','ready','failed','stale')", name="ck_timeline_preview_status"
        ),
    )
    op.create_table(
        "episode_export_batches",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
        sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
        sa.Column("export_profile", sa.String(16), nullable=False),
        sa.Column("idempotency_key", sa.String(255), nullable=False),
        sa.Column("status", sa.String(32), nullable=False),
        sa.Column("payload", json_type, nullable=False),
        sa.Column("created_at", sa.DateTime(timezone=True), server_default=sa.func.now()),
        sa.Column("updated_at", sa.DateTime(timezone=True), server_default=sa.func.now()),
        sa.UniqueConstraint("project_id", "idempotency_key", name="uq_export_batch_idempotency"),
        sa.CheckConstraint("export_profile = 'light'", name="ck_export_batch_profile"),
        sa.CheckConstraint(
            "status IN ('queued','succeeded','partially_failed','failed')",
            name="ck_export_batch_status",
        ),
    )
    op.create_table(
        "episode_export_members",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column(
            "batch_id", sa.String(36), sa.ForeignKey("episode_export_batches.id"), nullable=False
        ),
        sa.Column("position", sa.Integer(), nullable=False),
        sa.Column("episode_id", sa.String(36), sa.ForeignKey("episodes.id"), nullable=False),
        sa.Column(
            "timeline_version_id",
            sa.String(36),
            sa.ForeignKey("episode_timeline_versions.id"),
            nullable=False,
        ),
        sa.Column("timeline_version_revision", sa.Integer(), nullable=False),
        sa.Column("output_base_name", sa.String(120), nullable=False),
        sa.UniqueConstraint("batch_id", "position", name="uq_export_member_position"),
        sa.UniqueConstraint("batch_id", "episode_id", name="uq_export_member_episode"),
        sa.UniqueConstraint("batch_id", "output_base_name", name="uq_export_member_name"),
    )
    op.create_table(
        "episode_export_jobs",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column(
            "batch_id", sa.String(36), sa.ForeignKey("episode_export_batches.id"), nullable=False
        ),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("episode_id", sa.String(36), sa.ForeignKey("episodes.id"), nullable=False),
        sa.Column(
            "timeline_version_id",
            sa.String(36),
            sa.ForeignKey("episode_timeline_versions.id"),
            nullable=False,
        ),
        sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
        sa.Column("status", sa.String(32), nullable=False),
        sa.Column("packaging_phase", sa.String(16)),
        sa.Column("logical_operation", sa.String(255), nullable=False),
        sa.Column("render_plan_hash", sa.String(64)),
        sa.Column("renderer_diagnostic", sa.Text()),
        sa.Column("payload", json_type, nullable=False),
        sa.UniqueConstraint(
            "batch_id", "episode_id", "logical_operation", name="uq_export_job_logical_operation"
        ),
        sa.CheckConstraint(
            "status IN ('queued','preflighting','rendering','packaging','succeeded',"
            "'failed','cancel_requested','cancelled')",
            name="ck_export_job_status",
        ),
        sa.CheckConstraint(
            "packaging_phase IS NULL OR packaging_phase IN ('uploading','verifying','registering')",
            name="ck_export_job_packaging_phase",
        ),
    )
    op.create_table(
        "export_artifacts",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column(
            "export_job_id",
            sa.String(36),
            sa.ForeignKey("episode_export_jobs.id"),
            nullable=False,
        ),
        sa.Column("artifact_type", sa.String(32), nullable=False),
        sa.Column("status", sa.String(16), nullable=False),
        sa.Column("size_bytes", sa.BigInteger()),
        sa.Column("checksum", sa.String(64)),
        sa.Column("mime_type", sa.String(64)),
        sa.Column("payload", json_type, nullable=False),
        sa.UniqueConstraint("export_job_id", "artifact_type", name="uq_export_artifact_type"),
        sa.CheckConstraint(
            "artifact_type IN ('mp4','srt','light_manifest')", name="ck_export_artifact_type"
        ),
        sa.CheckConstraint(
            "status IN ('pending','verified','failed','held')", name="ck_export_artifact_status"
        ),
        sa.CheckConstraint("size_bytes IS NULL OR size_bytes >= 0", name="ck_export_artifact_size"),
    )
    op.create_table(
        "export_diagnostic_targets",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column(
            "export_job_id",
            sa.String(36),
            sa.ForeignKey("episode_export_jobs.id"),
            nullable=False,
        ),
        sa.Column("target_type", sa.String(32), nullable=False),
        sa.Column("owner_id", sa.String(255)),
        sa.Column("owner_revision", sa.Integer()),
        sa.Column("field_path", sa.String(255)),
        sa.Column("route_token", sa.String(128), nullable=False),
        sa.Column("code", sa.String(128), nullable=False),
        sa.Column("payload", json_type, nullable=False),
        sa.CheckConstraint(
            "target_type IN ('timeline','clip','caption','sound_cue','asset_version',"
            "'renderer','storage','artifact')",
            name="ck_export_diagnostic_target_type",
        ),
    )


def downgrade() -> None:
    for name in (
        "export_diagnostic_targets",
        "export_artifacts",
        "episode_export_jobs",
        "episode_export_members",
        "episode_export_batches",
        "timeline_preview_artifacts",
        "media_derivatives",
        "media_inspections",
        "episode_timeline_versions",
        "timeline_captions",
        "timeline_sound_cues",
        "timeline_clips",
        "timeline_current_cuts",
    ):
        op.drop_table(name)
