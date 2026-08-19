"""版本冲突和不可变记录由领域层显式表达。"""

from __future__ import annotations

from dataclasses import dataclass

_TRANSITIONS = {
    "draft": {"generated", "archived"},
    "generated": {"pending_review", "superseded", "archived"},
    "pending_review": {"approved", "rejected", "superseded", "archived"},
    "approved": {"superseded", "archived"},
    "rejected": {"draft", "archived"},
    "superseded": {"archived"},
    "archived": set(),
}


class ImmutableVersionError(RuntimeError):
    """已发布或已创建的版本记录不允许就地覆盖。"""


@dataclass(slots=True)
class RevisionConflictError(RuntimeError):
    draft_id: str
    expected_revision: int
    current_revision: int

    def __str__(self) -> str:
        return (
            f"revision conflict for {self.draft_id}: expected {self.expected_revision}, "
            f"current {self.current_revision}"
        )


def require_valid_transition(current: str, target: str) -> None:
    if target not in _TRANSITIONS.get(current, set()):
        raise ValueError(f"invalid state transition: {current} -> {target}")


class WorkflowService:
    """阶段 0 只提供服务边界，持久化事务由后续 API 命令接入。"""

    def update_draft(self, draft_id: str, expected_revision: int, actual_revision: int) -> int:
        if expected_revision != actual_revision:
            raise RevisionConflictError(draft_id, expected_revision, actual_revision)
        return actual_revision + 1

    def update_published_version(self, version_id: str) -> None:
        raise ImmutableVersionError(f"published workflow version is immutable: {version_id}")


class AssetService:
    def update_asset_version(self, version_id: str) -> None:
        raise ImmutableVersionError(f"asset version is immutable: {version_id}")
