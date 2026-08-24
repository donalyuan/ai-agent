"""Agent AssetEdit session conversation facts; message writes are explicit commands."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal
from uuid import uuid4

from .errors import RevisionConflictError, ValidationDomainError


@dataclass(frozen=True, slots=True)
class ConversationMessage:
    session_id: str
    sequence: int
    role: Literal["user", "agent"]
    content_hash: str
    status: Literal["complete", "pending", "failed"] = "complete"
    correlation_id: str = ""
    id: str = field(default_factory=lambda: str(uuid4()))

    def __post_init__(self) -> None:
        if self.role not in {"user", "agent"} or self.sequence < 1 or len(self.content_hash) != 64:
            raise ValidationDomainError("conversation message is invalid")


@dataclass(slots=True)
class ConversationTurn:
    session_id: str
    sequence: int
    user_message_id: str
    status: Literal["pending", "complete", "failed", "cancelled"] = "pending"
    agent_message_id: str | None = None
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1

    def complete(self, expected_revision: int, agent_message_id: str) -> None:
        if expected_revision != self.revision:
            raise RevisionConflictError(self.id, expected_revision, self.revision)
        if self.status != "pending":
            raise ValidationDomainError("conversation turn is terminal")
        self.agent_message_id = agent_message_id
        self.status = "complete"
        self.revision += 1


@dataclass(slots=True)
class AgentConversation:
    project_id: str
    episode_id: str
    revision: int = 1
    id: str = field(default_factory=lambda: str(uuid4()))
    messages: list[ConversationMessage] = field(default_factory=list)
    turns: list[ConversationTurn] = field(default_factory=list)

    def append_user_message(self, content_hash: str, correlation_id: str) -> ConversationTurn:
        sequence = len(self.messages) + 1
        user = ConversationMessage(
            self.id, sequence, "user", content_hash, correlation_id=correlation_id
        )
        turn = ConversationTurn(self.id, len(self.turns) + 1, user.id)
        self.messages.append(user)
        self.turns.append(turn)
        self.revision += 1
        return turn

    def append_agent_reply(
        self,
        turn_id: str,
        content_hash: str,
        correlation_id: str,
        expected_turn_revision: int,
        status: Literal["complete", "failed"] = "complete",
    ) -> ConversationMessage:
        turn = next((item for item in self.turns if item.id == turn_id), None)
        if turn is None:
            raise ValidationDomainError("conversation turn not found")
        if status == "complete":
            message = ConversationMessage(
                self.id,
                len(self.messages) + 1,
                "agent",
                content_hash,
                correlation_id=correlation_id,
            )
            turn.complete(expected_turn_revision, message.id)
        else:
            if expected_turn_revision != turn.revision or turn.status != "pending":
                raise RevisionConflictError(turn.id, expected_turn_revision, turn.revision)
            message = ConversationMessage(
                self.id,
                len(self.messages) + 1,
                "agent",
                content_hash,
                status="failed",
                correlation_id=correlation_id,
            )
            turn.status = "failed"
            turn.revision += 1
        self.messages.append(message)
        self.revision += 1
        return message
