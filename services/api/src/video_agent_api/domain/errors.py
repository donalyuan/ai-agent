"""领域错误；错误码是 HTTP 和适配器共享的稳定契约。"""

from __future__ import annotations


class DomainError(Exception):
    code = "domain_error"

    def __init__(self, message: str | None = None) -> None:
        super().__init__(message or self.code)


class ValidationDomainError(DomainError):
    code = "validation"


class ProjectAccessForbiddenError(DomainError):
    code = "project_access_forbidden"

    def __init__(self, project_id: str) -> None:
        super().__init__(f"project access forbidden: {project_id}")
        self.project_id = project_id


class ProjectNotFoundError(DomainError):
    code = "project_not_found"

    def __init__(self, project_id: str) -> None:
        super().__init__(f"project not found: {project_id}")
        self.project_id = project_id


class EpisodeNotFoundError(DomainError):
    code = "episode_not_found"

    def __init__(self, episode_id: str) -> None:
        super().__init__(f"episode not found: {episode_id}")
        self.episode_id = episode_id


class EpisodeNumberConflictError(DomainError):
    code = "episode_number_conflict"

    def __init__(self, project_id: str, number: int) -> None:
        super().__init__(f"episode number already exists for project: {project_id}/{number}")
        self.project_id = project_id
        self.number = number


class RevisionConflictError(DomainError):
    code = "revision_conflict"

    def __init__(self, entity_id: str, expected_revision: int, current_revision: int) -> None:
        super().__init__(
            f"revision conflict for {entity_id}: expected {expected_revision}, "
            f"current {current_revision}"
        )
        self.entity_id = entity_id
        self.expected_revision = expected_revision
        self.current_revision = current_revision


class DatabaseUnavailableError(DomainError):
    code = "database_unavailable"


class WorkflowUnconfiguredError(DomainError):
    code = "workflow_unconfigured"


class WorkflowVersionUnavailableError(DomainError):
    code = "workflow_version_unavailable"


class WorkflowSourceConflictError(DomainError):
    code = "workflow_source_conflict"


class WorkflowRunNotFoundError(DomainError):
    code = "workflow_run_not_found"


class WorkflowRunConflictError(DomainError):
    code = "workflow_run_conflict"


class UnsupportedFeatureError(DomainError):
    code = "unsupported"


class AssetEditNotFoundError(DomainError):
    code = "asset_edit_not_found"


class AssetEditConflictError(DomainError):
    code = "base_version_conflict"


class AssetEditContinuityStaleError(DomainError):
    code = "continuity_stale"


class AssetEditReadOnlyError(DomainError):
    code = "forbidden_read_only"


class AssetEditUnconfiguredError(DomainError):
    code = "asset_edit_unconfigured"


class StorageProfileRevisionConflictError(DomainError):
    code = "storage_profile_revision_conflict"

    def __init__(self, profile_id: str, expected: int, current: int) -> None:
        super().__init__(f"storage profile revision conflict: {profile_id}")
        self.profile_id = profile_id
        self.expected_revision = expected
        self.current_revision = current


class StorageProfileNotFoundError(DomainError):
    code = "storage_profile_not_found"


class CredentialMasterKeyUnavailableError(DomainError):
    code = "credential_master_key_unavailable"


class AssetNotFoundError(DomainError):
    code = "asset_not_found"

    def __init__(self, asset_id: str) -> None:
        super().__init__(f"asset not found: {asset_id}")
        self.asset_id = asset_id


class AssetVersionNotFoundError(DomainError):
    code = "asset_version_not_found"

    def __init__(self, version_id: str) -> None:
        super().__init__(f"asset version not found: {version_id}")
        self.version_id = version_id


class AssetVersionConflictError(DomainError):
    code = "asset_version_conflict"

    def __init__(self, asset_id: str, version_number: int) -> None:
        super().__init__(f"asset version already exists: {asset_id}/{version_number}")
        self.asset_id = asset_id
        self.version_number = version_number


class ImmutableAssetVersionError(DomainError):
    code = "asset_version_immutable"


class RendererUnconfiguredError(DomainError):
    code = "renderer_unconfigured"


class RendererCapabilityUnsupportedError(DomainError):
    code = "renderer_capability_unsupported"
