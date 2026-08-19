"""领域错误；错误码是 HTTP 和适配器共享的稳定契约。"""

from __future__ import annotations


class DomainError(Exception):
    code = "domain_error"

    def __init__(self, message: str | None = None) -> None:
        super().__init__(message or self.code)


class ValidationDomainError(DomainError):
    code = "validation"


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
