"""HTTP/Pydantic 边界只表达阶段 0 的并发和显式作用域契约。"""

from __future__ import annotations

from typing import Literal
from uuid import UUID

from pydantic import BaseModel, ConfigDict, Field, field_validator


def contract_alias(field_name: str) -> str:
    """与 JSON Schema 保持同一命名：schema_version 是唯一 snake_case 保留字段。"""
    if field_name == "schema_version":
        return field_name
    head, *tail = field_name.split("_")
    return head + "".join(part.capitalize() for part in tail)


class ContractBoundary(BaseModel):
    model_config = ConfigDict(alias_generator=contract_alias, populate_by_name=True, extra="forbid")


class RevisionUpdateCommand(ContractBoundary):
    expected_revision: int = Field(ge=1)


class WorkflowDraftBoundary(ContractBoundary):
    id: UUID
    schema_version: str = Field(pattern=r"^[0-9]+\.[0-9]+\.[0-9]+$")
    revision: int = Field(strict=True, ge=0)
    status: Literal[
        "draft", "generated", "pending_review", "approved", "rejected", "superseded", "archived"
    ]
    project_id: UUID
    scope_type: Literal["project", "episode", "scene", "shot"]
    scope_ids: list[UUID] = Field(min_length=1)
    definition: dict[str, object] = Field(min_length=1)

    @field_validator("scope_ids")
    @classmethod
    def require_unique_ids(cls, value: list[UUID]) -> list[UUID]:
        if len(value) != len(set(value)):
            raise ValueError("scope_ids must contain unique identifiers")
        return value
