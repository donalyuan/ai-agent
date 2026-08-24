from __future__ import annotations

from fastapi import Request

from video_agent_api.domain.errors import ProjectAccessForbiddenError


def project_scope(request: Request) -> str:
    value = getattr(request.state, "project_scope", None)
    if not isinstance(value, str) or not value:
        raise ProjectAccessForbiddenError("missing-project-scope")
    return value
