"""projects/episodes HTTP adapter; no Session access crosses this boundary."""

from __future__ import annotations

from typing import Annotated, cast

from fastapi import APIRouter, Depends, Header, HTTPException, Request, status
from pydantic import BaseModel, ConfigDict, Field, field_validator

from video_agent_api.application.projects_episodes import (
    CreateEpisodeCommand,
    CreateProjectCommand,
    ProjectsEpisodesService,
    UpdateEpisodeCommand,
    UpdateProjectCommand,
)
from video_agent_api.domain.entities import Episode, Project
from video_agent_api.domain.errors import (
    DatabaseUnavailableError,
    DomainError,
    EpisodeNotFoundError,
    EpisodeNumberConflictError,
    ProjectNotFoundError,
    RevisionConflictError,
    ValidationDomainError,
)

router = APIRouter(tags=["projects"])


class _DTO(BaseModel):
    model_config = ConfigDict(
        alias_generator=lambda value: "".join(
            [value.split("_")[0], *[part.capitalize() for part in value.split("_")[1:]]]
        ),
        populate_by_name=True,
        extra="forbid",
    )


class ProjectCreateRequest(_DTO):
    name: str = Field(min_length=1)

    @field_validator("name")
    @classmethod
    def non_blank(cls, value: str) -> str:
        if not value.strip():
            raise ValueError("name must not be blank")
        return value


class ProjectPatchRequest(_DTO):
    name: str | None = Field(default=None, min_length=1)


class EpisodeCreateRequest(_DTO):
    number: int = Field(ge=1)
    title: str = Field(min_length=1)

    @field_validator("title")
    @classmethod
    def non_blank(cls, value: str) -> str:
        if not value.strip():
            raise ValueError("title must not be blank")
        return value


class EpisodePatchRequest(_DTO):
    number: int | None = Field(default=None, ge=1)
    title: str | None = Field(default=None, min_length=1)


class ProjectResponse(_DTO):
    id: str
    schema_version: str
    revision: int
    status: str
    name: str


class EpisodeResponse(_DTO):
    id: str
    schema_version: str
    revision: int
    status: str
    project_id: str
    number: int
    title: str


def _project_response(value: Project) -> ProjectResponse:
    return ProjectResponse.model_validate(
        value.__dict__
        if hasattr(value, "__dict__")
        else {
            "id": value.id,
            "schema_version": value.schema_version,
            "revision": value.revision,
            "status": value.status,
            "name": value.name,
        }
    )


def _episode_response(value: Episode) -> EpisodeResponse:
    return EpisodeResponse.model_validate(
        {
            "id": value.id,
            "schema_version": value.schema_version,
            "revision": value.revision,
            "status": value.status,
            "project_id": value.project_id,
            "number": value.number,
            "title": value.title,
        }
    )


def service_dependency(request: Request) -> ProjectsEpisodesService:
    service = getattr(request.app.state, "projects_episodes_service", None)
    if service is None:
        raise DatabaseUnavailableError("business database is not configured")
    return cast(ProjectsEpisodesService, service)


Service = Annotated[ProjectsEpisodesService, Depends(service_dependency)]


def _error(error: DomainError) -> HTTPException:
    code = error.code
    if isinstance(error, (ProjectNotFoundError, EpisodeNotFoundError)):
        http_status = status.HTTP_404_NOT_FOUND
    elif isinstance(error, (EpisodeNumberConflictError, RevisionConflictError)):
        http_status = status.HTTP_409_CONFLICT
    elif isinstance(error, ValidationDomainError):
        http_status = status.HTTP_422_UNPROCESSABLE_ENTITY
    elif isinstance(error, DatabaseUnavailableError):
        http_status = status.HTTP_503_SERVICE_UNAVAILABLE
    else:
        http_status = status.HTTP_422_UNPROCESSABLE_ENTITY
    return HTTPException(http_status, detail={"type": code, "message": str(error)})


def _if_match(value: str | None) -> int:
    if value is None or not value.isdecimal() or int(value) < 1:
        raise _error(RevisionConflictError("unknown", 0, 0))
    return int(value)


@router.post("/v1/projects", response_model=ProjectResponse, status_code=201)
async def create_project(payload: ProjectCreateRequest, service: Service) -> ProjectResponse:
    try:
        return _project_response(await service.create_project(CreateProjectCommand(payload.name)))
    except DomainError as error:
        raise _error(error) from error


@router.get("/v1/projects", response_model=list[ProjectResponse])
async def list_projects(service: Service) -> list[ProjectResponse]:
    try:
        return [_project_response(value) for value in await service.list_projects()]
    except DomainError as error:
        raise _error(error) from error


@router.get("/v1/projects/{project_id}", response_model=ProjectResponse)
async def get_project(project_id: str, service: Service) -> ProjectResponse:
    try:
        return _project_response(await service.get_project(project_id))
    except DomainError as error:
        raise _error(error) from error


@router.patch("/v1/projects/{project_id}", response_model=ProjectResponse)
async def update_project(
    project_id: str,
    payload: ProjectPatchRequest,
    service: Service,
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> ProjectResponse:
    try:
        revision = _if_match(if_match)
        if payload.name is None:
            raise ValidationDomainError("at least one project field is required")
        return _project_response(
            await service.update_project(UpdateProjectCommand(project_id, revision, payload.name))
        )
    except DomainError as error:
        raise _error(error) from error


@router.post("/v1/projects/{project_id}/episodes", response_model=EpisodeResponse, status_code=201)
async def create_episode(
    project_id: str, payload: EpisodeCreateRequest, service: Service
) -> EpisodeResponse:
    try:
        return _episode_response(
            await service.create_episode(
                CreateEpisodeCommand(project_id, payload.title, payload.number)
            )
        )
    except DomainError as error:
        raise _error(error) from error


@router.get("/v1/projects/{project_id}/episodes", response_model=list[EpisodeResponse])
async def list_episodes(project_id: str, service: Service) -> list[EpisodeResponse]:
    try:
        return [_episode_response(value) for value in await service.list_episodes(project_id)]
    except DomainError as error:
        raise _error(error) from error


@router.get("/v1/episodes/{episode_id}", response_model=EpisodeResponse)
async def get_episode(episode_id: str, service: Service) -> EpisodeResponse:
    try:
        return _episode_response(await service.get_episode(episode_id))
    except DomainError as error:
        raise _error(error) from error


@router.patch("/v1/episodes/{episode_id}", response_model=EpisodeResponse)
async def update_episode(
    episode_id: str,
    payload: EpisodePatchRequest,
    service: Service,
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> EpisodeResponse:
    try:
        revision = _if_match(if_match)
        if payload.title is None and payload.number is None:
            raise ValidationDomainError("at least one episode field is required")
        return _episode_response(
            await service.update_episode(
                UpdateEpisodeCommand(episode_id, revision, payload.title, payload.number)
            )
        )
    except DomainError as error:
        raise _error(error) from error
